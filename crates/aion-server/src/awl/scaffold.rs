//! Ops-console worker scaffolding backed by the package codegen home.
//!
//! The endpoint compiles the submitted document through the same seam as
//! deployment, selects one compiled queue contract, and delegates every emitted
//! worker artifact to `aion-package`. The only server-owned addition is the AWL
//! document itself, without which neither generated runtime can derive its
//! advertised schemas.

use std::collections::BTreeMap;
use std::path::Path;

use aion_package::{
    AionDependency, AwlScaffoldError, AwlWorkerScaffold, DocumentRoot, ScaffoldedFile,
    WorkerContract, emit_shell_manifest, scaffold_awl_worker,
};
use serde::{Deserialize, Serialize};

/// A console request to scaffold one document worker for one supported runtime.
#[derive(Debug, Deserialize)]
pub struct ScaffoldRequest {
    /// The complete AWL document source.
    pub source: String,
    /// The task queue naming the document's worker block.
    pub worker: String,
    /// The requested scaffold runtime: `rust` or `shell`.
    pub runtime: String,
}

/// The stable console response: either a complete in-memory file set or one
/// typed refusal.
#[derive(Debug, Serialize)]
pub struct ScaffoldResponse {
    /// Whether generation succeeded.
    pub ok: bool,
    /// Generated files keyed by their path relative to the download root.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<BTreeMap<String, String>>,
    /// Why the request was refused, absent on success.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<ScaffoldRefusal>,
}

/// A typed scaffold refusal consumed by the frozen ops-console bundle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum ScaffoldRefusal {
    /// The submitted source did not compile as an AWL document.
    InvalidDocument {
        /// The compiler diagnostic rendered for the author.
        reason: String,
    },
    /// The compiled document declares no worker with the requested queue.
    UnknownWorker {
        /// A refusal naming the requested queue and every declared queue.
        reason: String,
    },
    /// The requested runtime has no scaffold generator.
    UnsupportedRuntime {
        /// A refusal naming the runtime and the supported alternatives.
        reason: String,
    },
    /// Every declared action is server-executed, leaving no worker to write.
    NoServableAction {
        /// The shared demand planner's explanation.
        reason: String,
    },
    /// The queue requires node topology the shell runtime cannot express.
    NodePinnedQueue {
        /// The shell generator's topology explanation and Rust alternative.
        reason: String,
    },
    /// The shared worker codegen home refused to emit a worker.
    Scaffold {
        /// The codegen refusal rendered for the author.
        reason: String,
    },
}

impl ScaffoldResponse {
    /// Builds a successful response containing every generated file.
    fn generated(files: BTreeMap<String, String>) -> Self {
        Self {
            ok: true,
            files: Some(files),
            refusal: None,
        }
    }

    /// Builds a failed response containing one typed refusal and no partial
    /// file set.
    fn refused(refusal: ScaffoldRefusal) -> Self {
        Self {
            ok: false,
            files: None,
            refusal: Some(refusal),
        }
    }
}

/// Compiles a submitted document and scaffolds one of its worker queues.
///
/// `schema_root` is the authoring workspace root, matching the deployment
/// compiler seam: imported schemas in the submitted source resolve against it.
/// Every successful response includes the document beside the generated worker
/// artifacts so the download can build or run without hidden server state.
///
/// # Errors
///
/// Returns a [`super::documents::DocumentError`] when the authoring workspace
/// cannot be opened or staged. Document parse, import, and compile diagnostics
/// remain successful endpoint responses carrying an `invalid_document` refusal.
pub fn scaffold(
    request: &ScaffoldRequest,
    schema_root: &Path,
) -> Result<ScaffoldResponse, super::documents::DocumentError> {
    let document = match aion_awl::parse(&request.source) {
        Ok(document) => document,
        Err(error) => {
            return Ok(ScaffoldResponse::refused(
                ScaffoldRefusal::InvalidDocument {
                    reason: error.to_string(),
                },
            ));
        }
    };
    let requested_path = format!("{}.awl", document.name);
    let workspace_absent = matches!(
        std::fs::metadata(schema_root),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound
    );
    let (_staging, staged_root) = if workspace_absent {
        // The workspace is minted only when its first document is saved, and
        // `documents::list` treats NotFound as an expected empty workspace.
        // Scaffolding is read-shaped, so use an empty temporary analysis root
        // rather than creating the configured workspace directory. Every other
        // metadata outcome goes through ConfinedDir so it retains its truthful
        // server-side error classification.
        let staging = tempfile::Builder::new().prefix("aion-schema-").tempdir()?;
        let staged_root = staging.path().to_owned();
        (staging, staged_root)
    } else {
        match super::handlers::stage_schema_imports(schema_root, &requested_path, &request.source) {
            Ok(staged) => staged,
            Err(
                error @ (super::documents::DocumentError::InvalidPath(_)
                | super::documents::DocumentError::InvalidName(_)),
            ) => {
                return Ok(ScaffoldResponse::refused(
                    ScaffoldRefusal::InvalidDocument {
                        reason: error.to_string(),
                    },
                ));
            }
            Err(error) => return Err(error),
        }
    };
    let compiled = match aion_awl::compile(&request.source, &staged_root) {
        Ok(compiled) => compiled,
        Err(error) => {
            return Ok(ScaffoldResponse::refused(
                ScaffoldRefusal::InvalidDocument {
                    reason: error.to_string(),
                },
            ));
        }
    };
    let Some(contract) = compiled
        .contract
        .workers
        .iter()
        .find(|worker| worker.task_queue == request.worker)
    else {
        return Ok(ScaffoldResponse::refused(ScaffoldRefusal::UnknownWorker {
            reason: unknown_worker_reason(&request.worker, &compiled.contract.workers),
        }));
    };

    let document_name = format!("{}.awl", compiled.workflow_name);
    let generated = match request.runtime.as_str() {
        "rust" => {
            let crate_name = format!("{}-worker", contract.task_queue.replace('_', "-"));
            let document_include = format!("../{document_name}");
            let document_directory = DocumentRoot::InCrateTree(".".to_owned());
            let dependencies = AionDependency::Version(env!("CARGO_PKG_VERSION").to_owned());
            scaffold_awl_worker(&AwlWorkerScaffold {
                contract,
                crate_name: &crate_name,
                document_include: &document_include,
                document_directory: &document_directory,
                document_name: &document_name,
                dependencies: &dependencies,
            })
            .map(|scaffold| scaffold.files)
        }
        "shell" => emit_shell_manifest(contract, &document_name),
        runtime => {
            return Ok(ScaffoldResponse::refused(
                ScaffoldRefusal::UnsupportedRuntime {
                    reason: format!(
                        "runtime `{runtime}` is not supported; choose `rust` or `shell`"
                    ),
                },
            ));
        }
    };

    Ok(match generated {
        Ok(files) => {
            ScaffoldResponse::generated(files_with_document(files, &document_name, &request.source))
        }
        Err(error) => ScaffoldResponse::refused(scaffold_refusal(&error)),
    })
}

/// Mirrors the CLI's worker-selection refusal and names the queues the document
/// actually declares.
fn unknown_worker_reason(requested: &str, workers: &[WorkerContract]) -> String {
    format!(
        "the document declares no `worker {requested}` block; it declares: [{}]",
        workers
            .iter()
            .map(|worker| format!("`{}`", worker.task_queue))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

/// Adds the submitted AWL document to the generator-owned artifact set.
fn files_with_document(
    files: Vec<ScaffoldedFile>,
    document_name: &str,
    source: &str,
) -> BTreeMap<String, String> {
    let mut mapped = files
        .into_iter()
        .map(|file| (file.relative, file.contents))
        .collect::<BTreeMap<_, _>>();
    mapped.insert(document_name.to_owned(), source.to_owned());
    mapped
}

/// Converts a package-codegen refusal to the endpoint's stable wire refusal.
fn scaffold_refusal(error: &AwlScaffoldError) -> ScaffoldRefusal {
    let reason = error.to_string();
    match error {
        AwlScaffoldError::NoServableAction { .. } => ScaffoldRefusal::NoServableAction { reason },
        AwlScaffoldError::NodePinnedQueue { .. } => ScaffoldRefusal::NodePinnedQueue { reason },
        _ => ScaffoldRefusal::Scaffold { reason },
    }
}
