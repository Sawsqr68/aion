//! The embedded update-check document: its bytes, its compiled identity, and
//! the verified names the server half keys on.
//!
//! # One document, one place
//!
//! `crates/aion-server/update-check-embed/update-check.awl` is the ONLY copy
//! of the update-check document in this repository, embedded exactly as the
//! assistant's is (`include_str!` from inside the crate, so `cargo package`
//! carries it and an installed binary holds the same bytes this repository
//! does). The boot install, the dispatch observer, and this module's tests
//! all read this one artifact.
//!
//! # The names are declared once and VERIFIED, never restated
//!
//! The dispatch observer ([`super::observer`]) recognises the check by its
//! task queue and action name, and trusts a completed dispatch only when the
//! run's resolved body is the exact command below. Those three names are
//! declared here as constants and [`EmbeddedUpdateCheck::load`] proves each
//! one against the compiled document before returning — a document edit that
//! renames the queue, the action, or the command fails loudly at load (at
//! boot, and in this module's own tests) instead of leaving the observer
//! keyed on names the document no longer carries.

use std::path::Path;
use std::sync::OnceLock;

use aion_awl::TypeBody;
use aion_package::{
    ActionBodyContract, ContentHash, ExtractionLimits, Package, PackageError, WorkerContract,
};

/// The embedded update-check document, compiled into the binary.
pub const EMBEDDED_UPDATE_CHECK_DOCUMENT: &str =
    include_str!("../../update-check-embed/update-check.awl");

/// The document's own filename, recorded in the assembled archive's `awl/`
/// provenance tree.
pub const EMBEDDED_UPDATE_CHECK_FILENAME: &str = "update-check.awl";

/// The workflow type an operator starts. Also the compiled entry module, which
/// [`EmbeddedUpdateCheck::load`] verifies.
///
/// # The console DUPLICATES this name, and nothing pins the two together
///
/// `apps/aion-ops-console/src/features/update/lib/contract.ts` carries the
/// same literal for its "check now" start, pinned on its side by
/// `contract.test.ts`. No build step connects that literal to this constant —
/// TypeScript cannot see Rust. **Renaming this workflow type therefore
/// requires renaming it there in the same change**, or every console's check
/// button starts a workflow type that no longer exists.
pub const UPDATE_CHECK_WORKFLOW_TYPE: &str = "update_check";

/// The task queue the check's one action is declared on. No worker serves it;
/// the declared body executes at the server.
pub const UPDATE_CHECK_QUEUE: &str = "update_check";

/// The one action: fetch the sparse-index line file for `aion-cli`.
pub const FETCH_ACTION: &str = "fetch_crate_index";

/// The declared command, verbatim as the document authors it. The observer
/// records a check's result ONLY when the dispatching run's resolved body is
/// exactly this command — see [`super::observer`] for why. The crate the URL
/// ends in is the crate the index parser demands of every line
/// (`super::index::INDEX_CRATE_NAME`); a document test pins the two together.
pub const FETCH_COMMAND: &str = "curl -fsS https://index.crates.io/ai/on/aion-cli";

/// A refusal to produce the embedded update-check package, naming the stage
/// that refused.
#[derive(Debug, thiserror::Error)]
pub enum EmbeddedUpdateCheckError {
    /// The embedded document does not parse.
    #[error("the embedded update-check document does not parse: {message}")]
    Parse {
        /// The parser's diagnostic, verbatim.
        message: String,
    },

    /// The embedded document carries a `schema(…)` import, which the binary —
    /// embedding one file and no directory — has nothing to resolve against.
    #[error(
        "the embedded update-check document imports schema `{path}`, but the binary embeds the \
         document alone and has no directory to resolve imports against; declare the type \
         inline in the document"
    )]
    SchemaImport {
        /// The import path, verbatim from the document.
        path: String,
    },

    /// The embedded document does not compile.
    #[error("the embedded update-check document does not compile: {message}")]
    Compile {
        /// The compiler's diagnostic, verbatim.
        message: String,
    },

    /// The compiled document could not be assembled into an archive.
    #[error("the embedded update-check document could not be packaged: {message}")]
    Assemble {
        /// The assembler's diagnostic, verbatim.
        message: String,
    },

    /// The assembled archive did not load back as a validated package.
    #[error("the embedded update-check package did not validate: {source}")]
    Package {
        /// The package validation failure.
        #[from]
        source: PackageError,
    },

    /// The compiled entry module is not [`UPDATE_CHECK_WORKFLOW_TYPE`].
    #[error(
        "the embedded update-check document compiles to workflow type `{found}`, but the server \
         half in crates/aion-server/src/update_check/document.rs names \
         `{UPDATE_CHECK_WORKFLOW_TYPE}`, so document and constants have diverged"
    )]
    WrongWorkflowType {
        /// The entry module the package actually carries.
        found: String,
    },

    /// The compiled package declares no contract at all.
    #[error("the embedded update-check package carries no readable contract: {message}")]
    MissingContract {
        /// Why the contract could not be read.
        message: String,
    },

    /// The contract does not declare [`FETCH_ACTION`] on
    /// [`UPDATE_CHECK_QUEUE`].
    #[error(
        "the embedded update-check contract declares no `{FETCH_ACTION}` action on queue \
         `{UPDATE_CHECK_QUEUE}`; the server half names both, so document and constants have \
         diverged"
    )]
    MissingAction,

    /// The action's declared body is absent or is not [`FETCH_COMMAND`].
    #[error(
        "the embedded update-check action `{FETCH_ACTION}` declares body {found:?}, but the \
         server half records results only from `{FETCH_COMMAND}`, so document and constants \
         have diverged"
    )]
    WrongBody {
        /// The body the document actually declares, or `None` when it
        /// declares no body at all.
        found: Option<String>,
    },
}

/// The embedded update check: the document's bytes and the package compiled
/// from them.
///
/// Construction is the verification: holding one of these is proof that the
/// embedded document compiled, packaged, and carries the exact queue, action,
/// and command the server half keys on.
#[derive(Debug, Clone)]
pub struct EmbeddedUpdateCheck {
    source: String,
    package: Package,
    workflow_type: String,
}

impl EmbeddedUpdateCheck {
    /// Compiles, packages, and verifies the embedded document.
    ///
    /// # Errors
    ///
    /// Returns [`EmbeddedUpdateCheckError`] naming the stage that refused.
    pub fn load() -> Result<Self, EmbeddedUpdateCheckError> {
        Self::from_source(EMBEDDED_UPDATE_CHECK_DOCUMENT)
    }

    /// The whole preparation, over an arbitrary document.
    ///
    /// [`Self::load`] is this applied to the embedded bytes; it is separate so
    /// the verification can be exercised against documents that deliberately
    /// omit a surface — a check nothing ever fails is a check nobody has
    /// measured. Takes any `&str` (the source is retained owned) so test
    /// documents need no `'static` lifetime and no leak.
    ///
    /// # Errors
    ///
    /// As [`Self::load`].
    pub fn from_source(source: &str) -> Result<Self, EmbeddedUpdateCheckError> {
        let document =
            aion_awl::parse(source).map_err(|error| EmbeddedUpdateCheckError::Parse {
                message: error.message,
            })?;
        for declaration in &document.types {
            if let TypeBody::SchemaImport { path, .. } = &declaration.body {
                return Err(EmbeddedUpdateCheckError::SchemaImport { path: path.clone() });
            }
        }

        // Never read (the load refuses schema imports above); a path that does
        // not exist so a future import fails loudly here rather than resolving
        // against whatever directory the server runs in. Same doctrine as the
        // assistant embed.
        let root = Path::new("<embedded-update-check-has-no-schema-directory>");
        let prepared = aion_awl_package::compile_and_assemble_awl(
            source,
            root,
            EMBEDDED_UPDATE_CHECK_FILENAME,
        )
        .map_err(|error| match error {
            aion_awl_package::PrepareAwlError::Compile(compile) => {
                EmbeddedUpdateCheckError::Compile {
                    message: compile.to_string(),
                }
            }
            other => EmbeddedUpdateCheckError::Assemble {
                message: other.to_string(),
            },
        })?;

        // Trusted, compile-time content assembled by this process moments ago —
        // not network input — so extraction carries no inflate ceiling.
        let package = Package::load_from_bytes(&prepared.archive, ExtractionLimits::unbounded())?;
        let workflow_type = package.manifest().entry_module.clone();
        if workflow_type != UPDATE_CHECK_WORKFLOW_TYPE {
            return Err(EmbeddedUpdateCheckError::WrongWorkflowType {
                found: workflow_type,
            });
        }

        let contract =
            package
                .contract()
                .map_err(|error| EmbeddedUpdateCheckError::MissingContract {
                    message: error.to_string(),
                })?;
        let declared_body = contract
            .workers
            .iter()
            .filter(|worker: &&WorkerContract| worker.task_queue == UPDATE_CHECK_QUEUE)
            .flat_map(|worker| worker.actions.iter())
            .find(|action| action.name == FETCH_ACTION)
            .ok_or(EmbeddedUpdateCheckError::MissingAction)?
            .body
            .clone();
        match declared_body {
            Some(ActionBodyContract::Run { ref command }) if command == FETCH_COMMAND => {}
            Some(ActionBodyContract::Run { command }) => {
                return Err(EmbeddedUpdateCheckError::WrongBody {
                    found: Some(command),
                });
            }
            None => return Err(EmbeddedUpdateCheckError::WrongBody { found: None }),
        }

        Ok(Self {
            source: source.to_owned(),
            package,
            workflow_type,
        })
    }

    /// The validated package the engine loads.
    #[must_use]
    pub const fn package(&self) -> &Package {
        &self.package
    }

    /// The workflow type an operator starts — [`UPDATE_CHECK_WORKFLOW_TYPE`],
    /// by construction.
    #[must_use]
    pub fn workflow_type(&self) -> &str {
        &self.workflow_type
    }

    /// The package's content hash — this document's version identity.
    #[must_use]
    pub const fn content_hash(&self) -> &ContentHash {
        self.package.content_hash()
    }

    /// The document source, verbatim.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }
}

/// The process-wide embedded update check, compiled once on first use.
///
/// # Errors
///
/// Returns the [`EmbeddedUpdateCheckError`] from the single load attempt.
pub fn embedded_update_check()
-> Result<&'static EmbeddedUpdateCheck, &'static EmbeddedUpdateCheckError> {
    static EMBEDDED: OnceLock<Result<EmbeddedUpdateCheck, EmbeddedUpdateCheckError>> =
        OnceLock::new();
    EMBEDDED.get_or_init(EmbeddedUpdateCheck::load).as_ref()
}

#[cfg(test)]
#[path = "document_tests.rs"]
mod document_tests;
