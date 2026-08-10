use std::path::Path;

use aion_awl_package::{PrepareAwlError, compile_and_assemble_awl};
use aion_core::DEFAULT_TASK_QUEUE;
use aion_package::{ExtractionLimits, Package, PackageError};
use aion_proto::WireError;
use serde::{Deserialize, Serialize};

use super::handlers::{CheckRequest, check_source};
use super::revisions::{self, DeploymentRecord, RevisionError};
use crate::authoring::AuthoringApiError;
use crate::worker::admission_audit::RememberedRefusal;
use crate::{CallerIdentity, ServerError, ServerState};

#[derive(Debug, Deserialize)]
pub struct EmitRequest {
    pub source: String,
    pub path: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct EmitResponse {
    pub emitted: String,
    pub bytes: usize,
    pub synthesized_workflows: Vec<EmittedWorkflowEntry>,
}

#[derive(Debug, Serialize)]
pub struct EmittedWorkflowEntry {
    pub workflow_type: String,
    pub entry_module: String,
    pub entry_function: String,
    pub input_schema: serde_json::Value,
    pub output_schema: serde_json::Value,
    /// Authored workflow timeout in whole seconds, or `None` when the document
    /// declared none for this synthesized child.
    pub timeout_seconds: Option<u64>,
    pub internal: bool,
}

#[derive(Debug, Deserialize)]
pub struct DeployAuthoringRequest {
    pub path: String,
    pub content_hash: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct GuidedStepResult {
    pub step: &'static str,
    pub detail: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct DeployAuthoringResponse {
    pub deployment: DeploymentRecord,
    pub steps: Vec<GuidedStepResult>,
}

#[derive(Debug, Deserialize)]
pub struct BindRunRequest {
    pub workflow_id: String,
    pub run_id: String,
}

#[derive(Debug, Deserialize)]
pub struct WorkerAvailabilityRequest {
    pub namespace: String,
    pub task_queue: String,
}

#[derive(Debug, Serialize)]
pub struct WorkerAvailabilityResponse {
    /// Whether every reachable action on THIS queue can be served right now:
    /// server-run actions always (the server executes a declared body itself),
    /// and every worker-owed action has at least one admitted worker
    /// advertising it on a node its declaration can dispatch to.
    ///
    /// This is NOT "is a worker connected": a queue whose reachable actions
    /// all carry bodies is fully served with zero workers — answering "no
    /// worker" for such a queue refused starts that succeed, the #154 defect,
    /// hit live by the operator at the tutorial's own ship-and-run ending.
    /// Nor is a connection alone enough: a worker admitted on the queue but
    /// not advertising a demanded action, or pinned to the wrong node for it,
    /// does not serve it.
    ///
    /// The answer is scoped to the ONE queue asked about. A document that
    /// declares several worker blocks has several queues, and each must be
    /// asked about separately.
    ///
    /// The answer reads DECLARATION-level node pins only. A call-site `node`
    /// override (a step calling an unpinned action with `node …`) is not in
    /// the deployed contract this reads — admission shares the same limit —
    /// so such a dispatch can still park on a queue answered available.
    pub available: bool,
    pub task_queue: String,
    pub connected_workers: usize,
    /// Reachable actions a connected worker must serve, sorted. Empty means
    /// no worker is owed — including the vacuous case of a queue with nothing
    /// deployed, which admission likewise holds nobody to.
    pub worker_actions: Vec<String>,
    /// Reachable actions the server executes itself (declared bodies), sorted.
    pub server_run_actions: Vec<String>,
    pub scaffold_hint: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RunStatusResponse {
    pub deployment: DeploymentRecord,
    pub deployed_source: String,
    pub drifted: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum RunLoopError {
    #[error(
        "document revision does not match the saved document: requested {requested}, saved {saved}"
    )]
    RevisionMismatch { requested: String, saved: String },
    #[error("AWL check refused deployment: {0}")]
    CheckRefused(String),
    #[error("AWL emission refused deployment: {0}")]
    EmitRefused(String),
    #[error(transparent)]
    Direct(#[from] PrepareAwlError),
    #[error("direct AWL package could not be loaded: {0}")]
    Package(#[from] PackageError),
    #[error(transparent)]
    Revision(#[from] RevisionError),
    /// A workspace-document failure surfaced by the deploy path — kept typed
    /// so a missing document reaches every transport as `DocumentNotFound`,
    /// never as a stringified revision fault.
    #[error(transparent)]
    Document(#[from] super::documents::DocumentError),
    #[error("authoring deploy was refused")]
    Authoring(AuthoringApiError),
    #[error("worker registry inspection failed: {0}")]
    WorkerRegistry(String),
    #[error("engine unavailable for queue contract inspection")]
    EngineUnavailable {
        #[source]
        source: ServerError,
    },
    #[error("queue contract inspection failed")]
    QueueContracts {
        #[source]
        source: aion::EngineError,
    },
}

pub fn emit(
    state: &ServerState,
    caller: &CallerIdentity,
    request: &EmitRequest,
) -> Result<EmitResponse, RunLoopError> {
    state
        .deploy_guard()
        .authorize(caller)
        .map_err(|error| RunLoopError::Authoring(AuthoringApiError::Wire(error.to_wire_error())))?;
    let checked = check_source(&CheckRequest {
        source: request.source.clone(),
        path: request.path.clone(),
    });
    if !checked.deploys_green {
        let reason = checked.diagnostics.first().map_or_else(
            || "document does not deploy green".to_owned(),
            |item| item.message.clone(),
        );
        return Err(RunLoopError::CheckRefused(reason));
    }
    let document = aion_awl::parse(&request.source)
        .map_err(|error| RunLoopError::CheckRefused(error.message))?;
    // `/awl/emit` accepts unsaved source, so request.path is a display label,
    // never ambient filesystem authority. Imported schemas intentionally refuse
    // here; saved-document deploy stages them through the workspace capability.
    let artifact = aion_awl::emit_artifact(&document)
        .map_err(|error| RunLoopError::EmitRefused(error.message))?;
    let synthesized_workflows = artifact
        .synthesized_workflows
        .into_iter()
        .map(|entry| EmittedWorkflowEntry {
            workflow_type: entry.workflow_type,
            entry_module: entry.entry_module,
            entry_function: entry.entry_function,
            input_schema: entry.input_schema,
            output_schema: entry.output_schema,
            timeout_seconds: entry.timeout.map(|timeout| timeout.as_secs()),
            internal: entry.internal,
        })
        .collect();
    Ok(EmitResponse {
        bytes: artifact.source.len(),
        emitted: artifact.source,
        synthesized_workflows,
    })
}

pub async fn deploy(
    state: &ServerState,
    caller: &CallerIdentity,
    root: &Path,
    transport: &'static str,
    request: DeployAuthoringRequest,
) -> Result<DeployAuthoringResponse, RunLoopError> {
    crate::authoring::handlers::admit_mutation(state, caller, transport, "awl.deploy")?;
    // The document error stays TYPED: a deploy of a path that does not exist
    // must reach the caller as the document refusal it is (`DocumentNotFound`
    // with document guidance), not as a stringified revision-record fault.
    let saved = super::documents::read(root, &request.path).await?;
    if saved.content_hash != request.content_hash {
        return Err(RunLoopError::RevisionMismatch {
            requested: request.content_hash,
            saved: saved.content_hash,
        });
    }
    let revision = revisions::store(root, &saved.source).await?;
    let workspace_root = root.to_owned();
    let document_path = request.path.clone();
    let revision_source = revision.source.clone();
    let prepared = tokio::task::spawn_blocking(move || {
        // Typed, not stringified: a schema import that fails staging (a
        // symlinked import, a confinement refusal, an I/O fault) surfaces as
        // its own `DocumentError` class on every transport, exactly as the
        // document read above does.
        let (_staging, schema_root) = super::handlers::stage_schema_imports(
            &workspace_root,
            &document_path,
            &revision_source,
        )
        .map_err(RunLoopError::Document)?;
        // The staged root mirrors the document's own directory, so the
        // archived document name is the document's basename and every
        // archived schema path stays relative to that root.
        let document_filename = Path::new(&document_path)
            .file_name()
            .and_then(std::ffi::OsStr::to_str)
            .ok_or_else(|| {
                RevisionError::InvalidRecord(format!(
                    "document path `{document_path}` has no filename"
                ))
            })?;
        compile_and_assemble_awl(&revision_source, &schema_root, document_filename)
            .map_err(RunLoopError::from)
    })
    .await
    .map_err(|error| {
        RunLoopError::Revision(RevisionError::InvalidRecord(format!(
            "AWL compile task failed: {error}"
        )))
    })??;
    let task_queue = match &prepared.compiled.first_worker {
        Some(worker) => worker.clone(),
        None => DEFAULT_TASK_QUEUE.to_owned(),
    };
    let workflow_name = prepared.compiled.workflow_name.clone();
    let beam_bytes = prepared.compiled.beam_bytes.len();
    let package = Package::load_from_bytes(prepared.archive, ExtractionLimits::unbounded())?;
    crate::authoring::handlers::validate_document_identity(&package, &workflow_name)?;
    let loaded = crate::authoring::handlers::load_admitted_package(
        state,
        caller,
        transport,
        "awl.deploy",
        package,
    )
    .await?;
    let deployment = DeploymentRecord {
        deployment_id: uuid::Uuid::new_v4().to_string(),
        document_path: request.path,
        content_hash: revision.content_hash,
        package_id: loaded.content_hash.clone(),
        workflow_type: loaded.workflow_type.clone(),
        task_queue,
        workflow_id: None,
        run_id: None,
    };
    revisions::record_deployment(root, &deployment).await?;
    Ok(DeployAuthoringResponse {
        steps: vec![
            GuidedStepResult {
                step: "check",
                detail: format!("direct compiler accepted workflow {workflow_name}"),
            },
            GuidedStepResult {
                step: "compile",
                detail: format!("{beam_bytes} bytes of direct BEAM compiled"),
            },
            GuidedStepResult {
                step: "package",
                detail: format!("package {} built", loaded.content_hash),
            },
            GuidedStepResult {
                step: "deploy",
                detail: format!("deployment {} loaded", deployment.deployment_id),
            },
        ],
        deployment,
    })
}

/// Answer whether a queue's reachable actions can be served, before a start.
///
/// The question is service, not connection. Who serves an action is decided
/// by its deployed declaration: a body-carrying action is executed by the
/// server itself ([`crate::worker::DeclaredCommandDispatcher`]), a bodyless
/// one by a connected worker. The worker-owed set comes from the same
/// reachable contracts admission reads
/// ([`aion::Engine::worker_contracts_for_admission`], split by
/// [`aion_package::ActionContract::worker_owed`]), so this answer and the
/// admission gate cannot disagree about what a queue demands. Supply is then
/// measured per dispatch address: each worker-owed `(action, node)` pair is
/// censused against the live fleet
/// ([`crate::worker::WorkerRegistry::pool_census`]), which counts admitted
/// workers ADVERTISING that action on a compatible node — a worker connected
/// to the queue but not advertising the action, or pinned to the wrong node,
/// serves nothing and vouches for nothing.
///
/// # Errors
///
/// Returns [`RunLoopError::WorkerRegistry`] when the connected-worker registry
/// cannot be read, [`RunLoopError::EngineUnavailable`] when the state carries
/// no engine handle, and [`RunLoopError::QueueContracts`] when the deployed
/// contract catalog cannot be read. An unreadable catalog errors rather than
/// guessing in either direction: admitting would re-open #154's false refusal
/// inverted, refusing would fabricate the very verdict this function exists
/// to correct.
pub fn worker_availability(
    state: &ServerState,
    request: WorkerAvailabilityRequest,
) -> Result<WorkerAvailabilityResponse, RunLoopError> {
    let registry = state.worker_registry();
    let workers = registry
        .all_workers()
        .map_err(|error| RunLoopError::WorkerRegistry(error.to_string()))?;
    let task_queue = if request.task_queue.is_empty() {
        DEFAULT_TASK_QUEUE.to_owned()
    } else {
        request.task_queue
    };
    let connected_workers = workers
        .iter()
        .filter(|worker| {
            worker.task_queue() == task_queue && worker.namespaces().contains(&request.namespace)
        })
        .count();
    let admission = state
        .engine()
        .map_err(|source| RunLoopError::EngineUnavailable { source })?
        .worker_contracts_for_admission(&task_queue)
        .map_err(|source| RunLoopError::QueueContracts { source })?;
    let demand = service_demand(&admission);
    let mut unserved: Option<UnservedAddress> = None;
    for (action, node) in &demand.worker_addresses {
        let census = registry
            .pool_census(&request.namespace, &task_queue, action, node.as_deref())
            .map_err(|error| RunLoopError::WorkerRegistry(error.to_string()))?;
        if !census.is_served() {
            unserved = Some(UnservedAddress {
                action: action.clone(),
                node: node.clone(),
                census,
            });
            break;
        }
    }
    Ok(WorkerAvailabilityResponse {
        available: unserved.is_none(),
        task_queue: task_queue.clone(),
        connected_workers,
        scaffold_hint: unserved.as_ref().map(|address| {
            unserved_hint(
                &task_queue,
                address,
                &registry.admission_audit().refusals_on_queue(&task_queue),
            )
        }),
        worker_actions: demand.worker,
        server_run_actions: demand.server_run,
    })
}

/// The first dispatch address the census found unserved, with the census that
/// said so — carried to the hint so the diagnosis can tell WHICH failure this
/// is instead of guessing from the connection count.
struct UnservedAddress {
    /// The unserved action's name.
    action: String,
    /// The node its declaring version pins it to, `None` when unpinned.
    node: Option<String>,
    /// The fleet census at that address, taken at decision time.
    census: crate::worker::PoolCensus,
}

/// Which of a queue's reachable actions are owed a worker, and which the
/// server runs itself.
///
/// An action lands under `worker` when ANY reachable version declares it
/// without a body: that version's dispatch delegates to a worker, so the name
/// is demanded even if another version carries a body for it. The two name
/// sets may therefore overlap, and each is deduplicated and sorted on its
/// own. `worker_addresses` keeps the node axis the name sets flatten away:
/// supply is checked per dispatch address, because an action pinned to node
/// `a` is not served by a worker on node `b`.
struct QueueServiceDemand {
    /// Every distinct dispatch address a worker is owed on: the action name
    /// paired with the node its declaring version pins it to (`None` when
    /// unpinned). One name can appear under several nodes when reachable
    /// versions pin it differently — each such address must be served.
    worker_addresses: Vec<(String, Option<String>)>,
    /// Action names a connected worker must serve, deduplicated and sorted.
    worker: Vec<String>,
    /// Action names the server executes from their declared bodies,
    /// deduplicated and sorted.
    server_run: Vec<String>,
}

fn service_demand(admission: &aion::QueueAdmission) -> QueueServiceDemand {
    let mut worker_addresses = std::collections::BTreeSet::new();
    let mut worker = std::collections::BTreeSet::new();
    let mut server_run = std::collections::BTreeSet::new();
    for required in &admission.required {
        for action in &required.contract.contract.actions {
            if action.worker_owed() {
                worker_addresses.insert((action.name.clone(), action.node.clone()));
                worker.insert(action.name.clone());
            } else {
                server_run.insert(action.name.clone());
            }
        }
    }
    QueueServiceDemand {
        worker_addresses: worker_addresses.into_iter().collect(),
        worker: worker.into_iter().collect(),
        server_run: server_run.into_iter().collect(),
    }
}

/// What to tell an operator whose queue has an unserved worker-owed action.
///
/// Four distinguishable worlds, each with a different remedy, and the hint
/// must name the right one — telling an operator whose worker IS running to
/// "scaffold and run this worker" describes the wrong problem entirely, and
/// during the #146 self-run it was the only signal there was.
///
/// * A refusal is on record: a worker is dialling and being turned away. The
///   refusal record decides this first — a refused worker never enters the
///   registry, so every census below would read it as absent.
/// * The census saw an empty pool: nothing is connected, and starting a
///   worker really is the remedy.
/// * Workers are in the pool but none advertises the action: the connected
///   worker was built for a different action surface, and starting another
///   copy of it cannot help.
/// * Workers advertise the action but none on the declared node: the action
///   is pinned, and the operator needs a worker THERE, not another one here.
///
/// The census travels from the availability decision itself, so the diagnosis
/// and the verdict cannot be about two different fleets.
fn unserved_hint(
    task_queue: &str,
    address: &UnservedAddress,
    refusals: &[RememberedRefusal],
) -> String {
    let Some(refusal) = refusals.first() else {
        let action = &address.action;
        let census = &address.census;
        if census.workers_in_pool == 0 {
            return format!(
                "No connected worker serves task queue `{task_queue}`. Scaffold and \
                 run this worker from Workers & Actions, then retry start."
            );
        }
        if census.workers_serving_activity == 0 {
            return format!(
                "{count} worker connection(s) ARE on task queue `{task_queue}`, but none \
                 advertises action `{action}`, so nothing serves it. Starting another copy \
                 of the same worker will not help — rebuild or rewire the worker to serve \
                 `{action}`, then retry start.",
                count = census.workers_in_pool,
            );
        }
        // Advertised but not compatible: the only remaining axis is the node
        // pin. `node` is always `Some` here — an unpinned address counts every
        // advertising worker as compatible — but the message must not depend
        // on that reasoning staying true, so the unpinned face stays honest.
        return match &address.node {
            Some(node) => format!(
                "Action `{action}` on task queue `{task_queue}` is pinned to node \
                 `{node}`, and none of the {count} worker(s) advertising it is on that \
                 node. Start a worker on node `{node}`, then retry start.",
                count = census.workers_serving_activity,
            ),
            None => format!(
                "Action `{action}` on task queue `{task_queue}` is advertised by \
                 {count} worker(s), but none is compatible with its dispatch. Check the \
                 worker's node against the document's declaration, then retry start.",
                count = census.workers_serving_activity,
            ),
        };
    };
    let others = match refusals.len() - 1 {
        0 => String::new(),
        1 => " One other connection was refused on this queue too.".to_owned(),
        more => format!(" {more} other connections were refused on this queue too."),
    };
    let node = match &refusal.node {
        Some(node) => format!(" on node `{node}`"),
        None => String::new(),
    };
    format!(
        "A worker IS dialling task queue `{task_queue}`{node} and the server is \
         REFUSING it, so nothing serves the queue. Starting another worker will \
         not help — this one has to be fixed. Worker build `{}` was refused \
         because: {}{others}",
        refusal.identity, refusal.reason
    )
}

pub async fn status(root: &Path, deployment_id: &str) -> Result<RunStatusResponse, RunLoopError> {
    let deployment = revisions::deployment(root, deployment_id).await?;
    let revision = revisions::fetch(root, &deployment.content_hash).await?;
    let drifted = revisions::current_drifted(root, &deployment).await?;
    Ok(RunStatusResponse {
        deployment,
        deployed_source: revision.source,
        drifted,
    })
}

pub fn wire_error(error: &RunLoopError) -> WireError {
    match error {
        RunLoopError::RevisionMismatch { .. }
        | RunLoopError::CheckRefused(_)
        | RunLoopError::EmitRefused(_)
        | RunLoopError::Direct(_)
        | RunLoopError::Package(_)
        | RunLoopError::Revision(_) => {
            WireError::invalid_input(error.to_string()).with_error_type(error_type(error))
        }
        // The document class mapping is the document error's own: one truth
        // for which code a missing document answers with, on every surface.
        RunLoopError::Document(error) => error.to_wire_error(),
        RunLoopError::Authoring(_) | RunLoopError::WorkerRegistry(_) => {
            WireError::backend(error.to_string()).with_error_type(error_type(error))
        }
        RunLoopError::EngineUnavailable { .. } | RunLoopError::QueueContracts { .. } => {
            // These variants carry their cause as a typed `#[source]`, and
            // `Display` deliberately does not restate it — so the wire face
            // walks the chain to keep the cause an operator acts on.
            WireError::backend(error_chain(error)).with_error_type(error_type(error))
        }
    }
}

/// Renders an error and every cause beneath it as one `: `-joined line.
fn error_chain(error: &dyn std::error::Error) -> String {
    let mut message = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        message.push_str(": ");
        message.push_str(&cause.to_string());
        source = cause.source();
    }
    message
}

impl From<AuthoringApiError> for RunLoopError {
    fn from(error: AuthoringApiError) -> Self {
        Self::Authoring(error)
    }
}

fn error_type(error: &RunLoopError) -> &'static str {
    match error {
        RunLoopError::RevisionMismatch { .. } => "RevisionMismatch",
        RunLoopError::CheckRefused(_) => "CheckRefused",
        RunLoopError::EmitRefused(_) => "EmitRefused",
        RunLoopError::Direct(_) => "DirectCompile",
        RunLoopError::Package(_) => "Package",
        RunLoopError::Revision(_) => "RevisionStore",
        RunLoopError::Document(error) => error.error_type(),
        RunLoopError::Authoring(_) => "AuthoringDeploy",
        RunLoopError::WorkerRegistry(_) => "WorkerRegistry",
        RunLoopError::EngineUnavailable { .. } => "EngineUnavailable",
        RunLoopError::QueueContracts { .. } => "QueueContracts",
    }
}

#[cfg(test)]
#[path = "run_loop_tests.rs"]
mod run_loop_tests;
