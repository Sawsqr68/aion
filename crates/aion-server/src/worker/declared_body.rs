//! Server-side execution of declared action bodies.
//!
//! An action whose deployed contract carries an [`ActionBodyContract`] is
//! executed BY THE SERVER, with no connected worker: the dispatch is
//! intercepted at the [`ActivityDispatcher`] seam before task-queue routing,
//! the declared command runs through the worker SDK's own executor
//! ([`aion_worker::shell::ShellAction`] — argv-element substitution, no
//! shell, process-group containment), and the result flows back through the
//! engine's normal completion path. The engine still schedules, records, and
//! replays the activity exactly as if a worker had served it.
//!
//! Actions with no declared body are delegated to the wrapped production
//! dispatcher unchanged, so remote workers keep working exactly as before.
//!
//! # The command is readable while it runs
//!
//! The executing activity carries a live transcript seam
//! ([`ActivityContext::with_transcript`](aion_worker::ActivityContext::with_transcript)),
//! so every line the command writes to stdout or stderr is published onto the
//! server's transcript sequencer AS IT ARRIVES — the same stream, envelope, and
//! cursor reads an agent step's transcript uses (see
//! [`super::declared_body_transcript`]). The activity's recorded result is
//! untouched by this: it still carries the command's complete output.

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use aion::{ActivityDispatch, ActivityDispatcher};
use aion_package::{ActionBodyContract, ContentHash};
use aion_worker::shell::ShellAction;

use super::declared_body_ambiguity::{DeclaringVersion, ambiguous_body_refusal};
use super::declared_body_selection::select_declared_body;
use super::declared_body_transcript::publish_declared_transcript;
use super::workspace_root::{WORKSPACE_ROOT_PLACEHOLDER, WorkspaceRoot};
use crate::activity_publisher::ActivityEventPublisher;

/// What a declared-body lookup found for one `(task_queue, action)` address.
#[derive(Clone, Debug)]
pub enum DeclaredBodyLookup {
    /// No retained contract declares a body for this action — it is a
    /// requirement on an out-of-band worker and must be delegated.
    None,
    /// Exactly one distinct body is declared across every retained package
    /// version. Safe to execute.
    Declared(ActionBodyContract),
    /// Retained package versions declare DIFFERENT bodies for this action.
    /// Executing one of them would guess which deploy the running workflow
    /// meant, so the dispatch is refused by name instead.
    Ambiguous {
        /// Every retained version that declares a body for this action, in
        /// catalog order. Carried rather than counted because the refusal has
        /// to name the versions the operator must retire — a bare count leaves
        /// them holding a terminal error with no way to act on it.
        declaring: Vec<DeclaringVersion>,
    },
    /// The catalog could not be read. The reader reports why; the dispatch
    /// is delegated so a readable worker path can still serve it.
    Unreadable(String),
}

/// Which run a declared-body lookup is being made for.
///
/// A body is a property of the run's own package version, not of the queue, so
/// the lookup cannot answer correctly without knowing whose dispatch it is —
/// see [`super::declared_body_selection`].
#[derive(Clone, Copy, Debug)]
pub struct DispatchingRun<'a> {
    /// The workflow the activity belongs to.
    pub workflow_id: &'a aion_core::WorkflowId,
    /// The concrete run within that workflow.
    pub run_id: &'a aion_core::RunId,
}

/// A reader over the deployed contracts' declared action bodies.
pub trait DeclaredBodies: Send + Sync {
    /// Look up the declared body for `action` on `task_queue`, as the run
    /// issuing the dispatch sees it.
    fn body_for(
        &self,
        task_queue: &str,
        action: &str,
        run: DispatchingRun<'_>,
    ) -> DeclaredBodyLookup;
}

/// Shared, install-once handle the dispatcher holds from construction and the
/// boot path fills in once the engine exists.
///
/// Mirrors [`super::QueueDeclarationSource`]: the dispatcher is built before
/// the engine, so the seam it consults is handed over afterwards through a
/// clone of this handle rather than by rebuilding the dispatcher.
#[derive(Clone, Default)]
pub struct DeclaredBodySource {
    inner: Arc<OnceLock<Arc<dyn DeclaredBodies>>>,
}

impl std::fmt::Debug for DeclaredBodySource {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DeclaredBodySource")
            .field("installed", &self.inner.get().is_some())
            .finish()
    }
}

impl DeclaredBodySource {
    /// Install the reader. A second install is ignored and logged: the source
    /// is process-wide and must not silently change identity.
    pub fn install(&self, source: Arc<dyn DeclaredBodies>) {
        if self.inner.set(source).is_err() {
            tracing::warn!("declared body source already installed; ignoring duplicate set");
        }
    }

    /// Look up the declared body, or [`DeclaredBodyLookup::None`] when no
    /// reader is installed yet — before the engine exists nothing has been
    /// deployed, so there is no body a dispatch could be missing.
    #[must_use]
    pub fn body_for(
        &self,
        task_queue: &str,
        action: &str,
        run: DispatchingRun<'_>,
    ) -> DeclaredBodyLookup {
        self.inner.get().map_or(DeclaredBodyLookup::None, |source| {
            source.body_for(task_queue, action, run)
        })
    }
}

/// Reads declared bodies out of the engine's live workflow catalog.
pub struct EngineDeclaredBodies {
    engine: Arc<aion::Engine>,
}

impl EngineDeclaredBodies {
    /// Build a reader over `engine`'s catalog.
    #[must_use]
    pub const fn new(engine: Arc<aion::Engine>) -> Self {
        Self { engine }
    }
}

impl std::fmt::Debug for EngineDeclaredBodies {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("EngineDeclaredBodies")
    }
}

impl EngineDeclaredBodies {
    /// The package version `run` is pinned to, or `None` when the registry
    /// cannot name it.
    ///
    /// Two ways to reach `None`, and both are reported rather than swallowed:
    /// the run has no handle (it left the registry), or the registry could not
    /// be read at all. Neither is a reason to guess a body — the caller falls
    /// back to the queue-wide reading, which refuses on disagreement.
    fn version_of(&self, run: DispatchingRun<'_>) -> Option<ContentHash> {
        match self.engine.registry().get(run.workflow_id, run.run_id) {
            Ok(Some(handle)) => Some(handle.loaded_version().clone()),
            Ok(None) => {
                tracing::warn!(
                    operation = "declared_command_dispatch",
                    workflow_id = %run.workflow_id,
                    run_id = %run.run_id,
                    "no registry handle for the dispatching run; resolving its body \
                     from the whole queue instead of from its own package version"
                );
                None
            }
            Err(error) => {
                tracing::error!(
                    operation = "declared_command_dispatch",
                    workflow_id = %run.workflow_id,
                    run_id = %run.run_id,
                    %error,
                    "registry unreadable while resolving the dispatching run's version; \
                     resolving its body from the whole queue instead"
                );
                None
            }
        }
    }
}

impl DeclaredBodies for EngineDeclaredBodies {
    fn body_for(
        &self,
        task_queue: &str,
        action: &str,
        run: DispatchingRun<'_>,
    ) -> DeclaredBodyLookup {
        let contracts = match self.engine.worker_contracts_for_queue(task_queue) {
            Ok(contracts) => contracts,
            Err(error) => return DeclaredBodyLookup::Unreadable(error.to_string()),
        };
        // The RAW retained set is the right input here, unlike worker admission
        // (see `Engine::worker_contracts_for_queue`): a run pinned to a version
        // nothing else can reach still has to execute that version's body. What
        // narrows the answer is the run's own identity, not reachability.
        select_declared_body(&contracts, action, self.version_of(run).as_ref())
    }
}

/// The dispatcher decorator that executes declared bodies at the server.
///
/// Wraps the production dispatcher. Consults the declared-body source before
/// every dispatch; delegates untouched whenever the action carries no body.
pub struct DeclaredCommandDispatcher {
    inner: Arc<dyn ActivityDispatcher>,
    bodies: DeclaredBodySource,
    tokio: tokio::runtime::Handle,
    workspace_root: WorkspaceRoot,
    transcript: ActivityEventPublisher,
}

impl DeclaredCommandDispatcher {
    /// Wrap `inner`, consulting `bodies` before every dispatch, expanding
    /// `{workspace_root}` in declared commands with the server-resolved
    /// `workspace_root`, and streaming each executed command's output onto
    /// `transcript` — the deployment's one transcript sequencer, shared with
    /// every agent step.
    #[must_use]
    pub fn new(
        inner: Arc<dyn ActivityDispatcher>,
        bodies: DeclaredBodySource,
        tokio: tokio::runtime::Handle,
        workspace_root: WorkspaceRoot,
        transcript: ActivityEventPublisher,
    ) -> Self {
        Self {
            inner,
            bodies,
            tokio,
            workspace_root,
            transcript,
        }
    }

    /// Execute one declared command attempt and encode the outcome onto the
    /// FFI string contract (`retryable:`/`terminal:` on the error side).
    fn run_declared_command(
        &self,
        request: &ActivityDispatch,
        command: &str,
    ) -> Result<String, String> {
        let arguments = decode_arguments(&request.input)?;
        // Ratification condition (#139): a body that USES the placeholder is
        // refused terminally, by name, when the root cannot resolve to an
        // absolute directory that exists — no fallback to cwd, temp, or
        // anything else. A body without the placeholder never reaches the
        // resolution at all (`expand` returns `Ok(None)` untouched).
        let expanded = self.workspace_root.expand(command).map_err(|error| {
            format!(
                "terminal:declared body for action `{name}` uses the {placeholder} \
                 placeholder and cannot dispatch: {error}",
                name = request.name,
                placeholder = WORKSPACE_ROOT_PLACEHOLDER,
            )
        })?;
        if let Some(expansion) = &expanded {
            tracing::info!(
                operation = "declared_command_dispatch",
                workflow_id = %request.workflow_id,
                activity_id = %request.activity_id,
                activity_name = %request.name,
                task_queue = %request.task_queue,
                attempt = request.attempt,
                workspace_root = %expansion.workspace_root,
                "expanded the workspace-root placeholder in the declared command"
            );
        }
        let command = expanded
            .as_ref()
            .map_or(command, |expansion| expansion.command.as_str());
        let action = ShellAction::new(command).map_err(|error| {
            // The AWL checker refuses these at compile time, so reaching this
            // arm means a defective contract got deployed — name the defect
            // rather than hiding it behind a generic dispatch failure.
            format!("terminal:declared command failed to parse at dispatch: {error}")
        })?;
        // The live transcript seam for this attempt. The context owns the
        // sending end, so dropping it after the run closes the stream and ends
        // the pump — which is then awaited, so no observed line is abandoned
        // unpublished when the command finishes.
        let (events, drain) = tokio::sync::mpsc::unbounded_channel();
        let (context, cancellation) = aion_worker::ActivityContext::with_transcript(
            request.workflow_id.clone(),
            request.run_id.clone(),
            request.activity_id.clone(),
            request.attempt,
            events,
        );

        tracing::info!(
            operation = "declared_command_dispatch",
            workflow_id = %request.workflow_id,
            activity_id = %request.activity_id,
            activity_name = %request.name,
            task_queue = %request.task_queue,
            attempt = request.attempt,
            "executing declared action body at the server"
        );

        let transcript = self.transcript.clone();
        let outcome = self.tokio.block_on(async move {
            let pump = tokio::spawn(publish_declared_transcript(transcript, drain));
            let outcome = action.run(&arguments, &context).await;
            // Closing the seam is what ends the pump; the context holds it.
            drop(context);
            if let Err(error) = pump.await {
                tracing::warn!(
                    %error,
                    operation = "declared_command_dispatch",
                    "declared command transcript: the publishing task ended abnormally; some \
                     output lines may not have been retained"
                );
            }
            outcome
        });
        // Held, not wired: nothing cancels a single declared-command attempt
        // today. Dropped only after the run so a future wiring cannot race a
        // handle that died early.
        drop(cancellation);

        match outcome {
            Ok(result) => serde_json::to_string(&result).map_err(|error| {
                format!("terminal:declared command result failed to encode: {error}")
            }),
            Err(failure) => {
                let prefix = match failure.classification() {
                    aion_worker::Classification::Retryable => "retryable",
                    aion_worker::Classification::Terminal => "terminal",
                };
                Err(format!("{prefix}:{}", failure.message()))
            }
        }
    }
}

impl std::fmt::Debug for DeclaredCommandDispatcher {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DeclaredCommandDispatcher")
            .field("bodies", &self.bodies)
            .finish_non_exhaustive()
    }
}

impl ActivityDispatcher for DeclaredCommandDispatcher {
    fn dispatch(&self, request: ActivityDispatch) -> Result<String, String> {
        let run = DispatchingRun {
            workflow_id: &request.workflow_id,
            run_id: &request.run_id,
        };
        match self
            .bodies
            .body_for(&request.task_queue, &request.name, run)
        {
            DeclaredBodyLookup::None => self.inner.dispatch(request),
            DeclaredBodyLookup::Unreadable(reason) => {
                // Delegated, not refused: a catalog read failure must not
                // strand a queue that live workers could still serve. Loud so
                // an operator sees a bodied action falling through.
                tracing::error!(
                    operation = "declared_command_dispatch",
                    workflow_id = %request.workflow_id,
                    activity_name = %request.name,
                    task_queue = %request.task_queue,
                    %reason,
                    "declared-body catalog read failed; delegating to the worker path"
                );
                self.inner.dispatch(request)
            }
            DeclaredBodyLookup::Ambiguous { declaring } => Err(ambiguous_body_refusal(
                &request.name,
                &request.task_queue,
                &declaring,
            )),
            DeclaredBodyLookup::Declared(ActionBodyContract::Run { command }) => {
                self.run_declared_command(&request, &command)
            }
        }
    }
}

/// Decode the dispatch's JSON input into the declared action's arguments.
///
/// A declared action's parameters are named in its `.awl` declaration, so the
/// input must be a JSON object; anything else cannot bind to `$name`
/// references and is refused by shape. Retrying cannot change the input, so
/// the refusal is terminal.
fn decode_arguments(input: &str) -> Result<BTreeMap<String, serde_json::Value>, String> {
    let value: serde_json::Value = serde_json::from_str(input)
        .map_err(|error| format!("terminal:declared command input is not valid JSON: {error}"))?;
    match value {
        serde_json::Value::Object(members) => Ok(members.into_iter().collect()),
        other => Err(format!(
            "terminal:declared command input must be a JSON object binding the action's \
             parameters by name; got {}",
            json_kind(&other)
        )),
    }
}

/// A JSON value's kind, named for a refusal message.
const fn json_kind(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "a boolean",
        serde_json::Value::Number(_) => "a number",
        serde_json::Value::String(_) => "a string",
        serde_json::Value::Array(_) => "an array",
        serde_json::Value::Object(_) => "an object",
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};

    use aion::{ActivityDispatch, ActivityDispatcher};
    use aion_core::{ActivityId, RunId, WorkflowId};
    use aion_package::ActionBodyContract;

    use aion_core::ActivityEventKind;
    use aion_store::ActivityStreamKey;

    use super::super::workspace_root::{WorkspaceRoot, WorkspaceRootError};
    use super::{
        ActivityEventPublisher, DeclaredBodies, DeclaredBodyLookup, DeclaredBodySource,
        DeclaredCommandDispatcher, DeclaringVersion, DispatchingRun, decode_arguments,
    };

    /// What a test returns. Every fallible step is carried rather than
    /// unwrapped, because the workspace denies panicking accessors in test
    /// code as firmly as in library code.
    type TestResult = Result<(), Box<dyn std::error::Error>>;

    /// Inner dispatcher that records whether it was reached.
    struct RecordingInner {
        reached: Arc<Mutex<Vec<String>>>,
        reply: Result<String, String>,
    }

    impl ActivityDispatcher for RecordingInner {
        fn dispatch(&self, request: ActivityDispatch) -> Result<String, String> {
            match self.reached.lock() {
                Ok(mut names) => names.push(request.name),
                Err(poisoned) => poisoned.into_inner().push(request.name),
            }
            self.reply.clone()
        }
    }

    struct FixedBodies {
        lookup: DeclaredBodyLookup,
    }

    impl DeclaredBodies for FixedBodies {
        fn body_for(
            &self,
            _task_queue: &str,
            _action: &str,
            _run: DispatchingRun<'_>,
        ) -> DeclaredBodyLookup {
            self.lookup.clone()
        }
    }

    /// A reader that records whose dispatch it was asked about.
    ///
    /// The selection rule is unit-tested on its own inputs, which proves the
    /// rule and nothing about the plumbing. This double closes that gap: it
    /// captures the [`DispatchingRun`] the dispatcher hands over, so the
    /// identity can be compared against the request it came from.
    struct RecordingBodies {
        seen: Arc<Mutex<Vec<(WorkflowId, RunId)>>>,
    }

    impl DeclaredBodies for RecordingBodies {
        fn body_for(
            &self,
            _task_queue: &str,
            _action: &str,
            run: DispatchingRun<'_>,
        ) -> DeclaredBodyLookup {
            let observed = (run.workflow_id.clone(), run.run_id.clone());
            match self.seen.lock() {
                Ok(mut seen) => seen.push(observed),
                Err(poisoned) => poisoned.into_inner().push(observed),
            }
            DeclaredBodyLookup::None
        }
    }

    fn request(name: &str, input: &str) -> ActivityDispatch {
        ActivityDispatch {
            namespace: "default".to_owned(),
            task_queue: "shell".to_owned(),
            node: None,
            workflow_id: WorkflowId::new_v4(),
            run_id: RunId::new_v4(),
            activity_id: ActivityId::from_sequence_position(1),
            name: name.to_owned(),
            input: input.to_owned(),
            config: "{}".to_owned(),
            attempt: 1,
            labels: BTreeMap::new(),
            advisory: false,
        }
    }

    fn dispatcher(
        lookup: DeclaredBodyLookup,
        reply: Result<String, String>,
    ) -> (DeclaredCommandDispatcher, Arc<Mutex<Vec<String>>>) {
        // These tests exercise bodies without the placeholder, so the root's
        // value is never read; it is an explicit existing directory rather
        // than a default so nothing here depends on resolution.
        let (decorated, reached, _transcript) = dispatcher_with_root(
            lookup,
            reply,
            WorkspaceRoot::from_resolution(Ok(std::env::temp_dir())),
        );
        (decorated, reached)
    }

    /// The live-tail buffer these tests give their transcript sequencer. A
    /// `const` match rather than an unwrap: the workspace denies panicking
    /// accessors in test code as firmly as in library code.
    const TRANSCRIPT_CAPACITY: std::num::NonZeroUsize = match std::num::NonZeroUsize::new(64) {
        Some(capacity) => capacity,
        None => std::num::NonZeroUsize::MIN,
    };

    fn dispatcher_with_root(
        lookup: DeclaredBodyLookup,
        reply: Result<String, String>,
        workspace_root: WorkspaceRoot,
    ) -> (
        DeclaredCommandDispatcher,
        Arc<Mutex<Vec<String>>>,
        ActivityEventPublisher,
    ) {
        let reached = Arc::new(Mutex::new(Vec::new()));
        let inner = RecordingInner {
            reached: Arc::clone(&reached),
            reply,
        };
        let bodies = DeclaredBodySource::default();
        bodies.install(Arc::new(FixedBodies { lookup }));
        let store: Arc<dyn aion_store::ObservabilityStore> =
            Arc::new(aion_store::InMemoryObservabilityStore::default());
        let transcript = ActivityEventPublisher::new(store, TRANSCRIPT_CAPACITY);
        let decorated = DeclaredCommandDispatcher::new(
            Arc::new(inner),
            bodies,
            tokio::runtime::Handle::current(),
            workspace_root,
            transcript.clone(),
        );
        (decorated, reached, transcript)
    }

    fn reached_names(reached: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
        match reached.lock() {
            Ok(names) => names.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_bodiless_action_is_delegated_untouched() -> TestResult {
        let (decorated, reached) =
            dispatcher(DeclaredBodyLookup::None, Ok("\"worker-served\"".to_owned()));
        let handle =
            tokio::task::spawn_blocking(move || decorated.dispatch(request("plain", "{}")));
        let result = handle.await?;
        assert_eq!(result, Ok("\"worker-served\"".to_owned()));
        assert_eq!(reached_names(&reached), vec!["plain".to_owned()]);
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_declared_body_executes_without_touching_the_worker_path() -> TestResult {
        let (decorated, reached) = dispatcher(
            DeclaredBodyLookup::Declared(ActionBodyContract::Run {
                command: "echo $greeting".to_owned(),
            }),
            Err("terminal:the worker path must never be reached".to_owned()),
        );
        let handle = tokio::task::spawn_blocking(move || {
            decorated.dispatch(request(
                "greet",
                "{\"greeting\":\"hello from the contract\"}",
            ))
        });
        let result = handle.await?;
        let encoded = result.map_err(|error| format!("declared command failed: {error}"))?;
        let outcome: serde_json::Value = serde_json::from_str(&encoded)?;
        assert_eq!(outcome["stdout"], "hello from the contract");
        assert_eq!(outcome["exit_code"], 0);
        assert!(
            reached_names(&reached).is_empty(),
            "the worker path must not be consulted for a bodied action"
        );
        Ok(())
    }

    /// THE MID-STEP ANSWER: a server-run declared body's output reaches the
    /// deployment's transcript sequencer as one event per line, on both streams,
    /// keyed to the dispatch's own `(workflow, activity, attempt)` — the same
    /// durable stream an agent step's transcript is read from, so every reader
    /// that already serves transcripts serves this without change.
    ///
    /// The completion contract is asserted on the same run: the recorded result
    /// still carries the command's whole stdout.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_declared_body_publishes_its_output_onto_the_transcript() -> TestResult {
        let (decorated, reached, transcript) = dispatcher_with_root(
            DeclaredBodyLookup::Declared(ActionBodyContract::Run {
                command: "sh -c 'echo one; echo two; echo warned >&2'".to_owned(),
            }),
            Err("terminal:the worker path must never be reached".to_owned()),
            WorkspaceRoot::from_resolution(Ok(std::env::temp_dir())),
        );
        let dispatch = request("noisy", "{}");
        let key = ActivityStreamKey::new(
            dispatch.workflow_id.clone(),
            dispatch.run_id.clone(),
            dispatch.activity_id.clone(),
            dispatch.attempt,
        );

        let handle = tokio::task::spawn_blocking(move || decorated.dispatch(dispatch));
        let encoded = handle
            .await?
            .map_err(|error| format!("declared command failed: {error}"))?;

        // The replay-authoritative result is untouched by the streaming.
        let outcome: serde_json::Value = serde_json::from_str(&encoded)?;
        assert_eq!(outcome["stdout"], "one\ntwo");
        assert_eq!(outcome["stderr"], "warned");
        assert!(reached_names(&reached).is_empty());

        // ...and the same output is on the durable transcript, line by line.
        let retained = transcript.replay_from(&key, 0).await?;
        let lines = retained
            .iter()
            .map(|record| match &record.event.kind {
                ActivityEventKind::Message { text, .. } => {
                    (record.event.agent_role.clone(), text.clone())
                }
                other => (record.event.agent_role.clone(), format!("{other:?}")),
            })
            .collect::<Vec<_>>();
        assert!(
            lines.contains(&("command stdout".to_owned(), "one".to_owned()))
                && lines.contains(&("command stdout".to_owned(), "two".to_owned())),
            "each stdout line must be its own transcript event: {lines:?}"
        );
        assert!(
            lines.contains(&("command stderr".to_owned(), "warned".to_owned())),
            "stderr must be on the transcript, labelled by its stream: {lines:?}"
        );
        // Sequencing is the publisher's: the durable order is gap-free from 0.
        let sequences = retained
            .iter()
            .map(|record| record.store_seq)
            .collect::<Vec<_>>();
        assert_eq!(
            sequences,
            (0..u64::try_from(retained.len())?).collect::<Vec<_>>(),
            "the sequencer assigns a gap-free durable order"
        );
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_failing_declared_command_reports_retryable_with_its_stderr() -> TestResult {
        let (decorated, _reached) = dispatcher(
            DeclaredBodyLookup::Declared(ActionBodyContract::Run {
                command: "sh -c 'echo boom >&2; exit 7'".to_owned(),
            }),
            Ok("unused".to_owned()),
        );
        let handle =
            tokio::task::spawn_blocking(move || decorated.dispatch(request("fails", "{}")));
        let Err(error) = handle.await? else {
            return Err("a non-zero exit must fail the dispatch".into());
        };
        assert!(
            error.starts_with("retryable:"),
            "a non-zero exit is retryable by default: {error}"
        );
        assert!(
            error.contains("boom"),
            "stderr must ride the failure: {error}"
        );
        Ok(())
    }

    /// The hash the refusal prints must be one the deploy API will accept, or
    /// the remedy is a command that cannot run — the exact failure the old
    /// "redeploy so one body remains" wording had.
    ///
    /// The oracle is the deploy API's own parser, not a length or a shape:
    /// `EngineDeclaredBodies` renders the version with `ContentHash::to_string`,
    /// so this takes a real hash through that rendering, pulls the token back
    /// out of the printed command, and parses it the way
    /// `decode_version_target` does.
    #[test]
    fn the_printed_hash_parses_back_as_a_content_hash() -> TestResult {
        let version = aion_package::ContentHash::from_bytes([0x5a; 32]);
        let routed = aion_package::ContentHash::from_bytes([0xa5; 32]);
        let refusal = super::ambiguous_body_refusal(
            "find_repositories",
            "local",
            &[
                DeclaringVersion {
                    content_hash: version.to_string(),
                    workflow_types: vec!["sweeper".to_owned()],
                    route_active: false,
                    body: 0,
                },
                DeclaringVersion {
                    content_hash: routed.to_string(),
                    workflow_types: vec!["sweeper".to_owned()],
                    route_active: true,
                    body: 1,
                },
            ],
        );
        let Some(command) = refusal.split("`aion unload sweeper ").nth(1) else {
            return Err(format!("no unload command in the refusal: {refusal}").into());
        };
        let Some(printed) = command.split('`').next() else {
            return Err(format!("the unload command is unterminated: {refusal}").into());
        };
        let parsed: aion_package::ContentHash = printed.parse()?;
        assert_eq!(
            parsed, version,
            "the printed hash must round-trip to the version it names"
        );
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn ambiguous_bodies_refuse_terminally_by_name() -> TestResult {
        let superseded = "1111111111111111111111111111111111111111111111111111111111111111";
        let routed = "2222222222222222222222222222222222222222222222222222222222222222";
        let (decorated, reached) = dispatcher(
            DeclaredBodyLookup::Ambiguous {
                declaring: vec![
                    DeclaringVersion {
                        content_hash: superseded.to_owned(),
                        workflow_types: vec!["sweeper".to_owned()],
                        route_active: false,
                        body: 0,
                    },
                    DeclaringVersion {
                        content_hash: routed.to_owned(),
                        workflow_types: vec!["sweeper".to_owned()],
                        route_active: true,
                        body: 1,
                    },
                ],
            },
            Ok(String::new()),
        );
        let handle = tokio::task::spawn_blocking(move || decorated.dispatch(request("torn", "{}")));
        let Err(error) = handle.await? else {
            return Err("ambiguous bodies must refuse".into());
        };
        assert!(error.starts_with("terminal:"), "{error}");
        assert!(error.contains("torn"), "{error}");
        // The refusal must reach the dispatcher carrying an act-on-able remedy,
        // not just a count: the operator reads this string and nothing else.
        assert!(
            error.contains(&format!("`aion unload sweeper {superseded}`")),
            "the dispatch refusal must name the version to retire: {error}"
        );
        assert!(reached_names(&reached).is_empty());
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_placeholder_bearing_body_executes_with_the_expanded_root() -> TestResult {
        let scratch = tempfile::tempdir()?;
        let root = scratch.path().join("clones");
        let root_text = root.to_string_lossy().into_owned();
        let (decorated, reached, _transcript) = dispatcher_with_root(
            DeclaredBodyLookup::Declared(ActionBodyContract::Run {
                command: "echo {workspace_root}".to_owned(),
            }),
            Err("terminal:the worker path must never be reached".to_owned()),
            WorkspaceRoot::from_resolution(Ok(root.clone())),
        );
        let handle =
            tokio::task::spawn_blocking(move || decorated.dispatch(request("provision", "{}")));
        let result = handle.await?;
        let encoded = result.map_err(|error| format!("declared command failed: {error}"))?;
        let outcome: serde_json::Value = serde_json::from_str(&encoded)?;
        assert_eq!(
            outcome["stdout"], root_text,
            "the command must observe the server-resolved root as its argv word"
        );
        assert_eq!(outcome["exit_code"], 0);
        assert!(
            root.is_dir(),
            "dispatching a placeholder-bearing body must create the missing root"
        );
        assert!(reached_names(&reached).is_empty());
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_placeholder_bearing_body_refuses_terminally_when_the_root_is_unresolved()
    -> TestResult {
        let (decorated, reached, _transcript) = dispatcher_with_root(
            DeclaredBodyLookup::Declared(ActionBodyContract::Run {
                command: "echo {workspace_root}".to_owned(),
            }),
            Ok("unused".to_owned()),
            WorkspaceRoot::from_resolution(Err(WorkspaceRootError::Unresolvable {
                reason: "cannot resolve Aion home: set AION_HOME or HOME".to_owned(),
            })),
        );
        let handle =
            tokio::task::spawn_blocking(move || decorated.dispatch(request("provision", "{}")));
        let Err(error) = handle.await? else {
            return Err("an unresolved root must refuse a placeholder-bearing body".into());
        };
        assert!(error.starts_with("terminal:"), "{error}");
        assert!(
            error.contains("provision"),
            "the refusal must name the action: {error}"
        );
        assert!(
            error.contains("cannot resolve Aion home"),
            "the refusal must carry the resolution failure's reason: {error}"
        );
        assert!(
            reached_names(&reached).is_empty(),
            "a refused body must not fall through to the worker path"
        );
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_shape_changing_root_refuses_terminally_naming_the_action() -> TestResult {
        // `$` in the root would open a parameter reference after splicing.
        let (decorated, reached, _transcript) = dispatcher_with_root(
            DeclaredBodyLookup::Declared(ActionBodyContract::Run {
                command: "echo {workspace_root}".to_owned(),
            }),
            Ok("unused".to_owned()),
            WorkspaceRoot::from_resolution(Ok(std::path::PathBuf::from("/absolute/with$dollar"))),
        );
        let handle =
            tokio::task::spawn_blocking(move || decorated.dispatch(request("provision", "{}")));
        let Err(error) = handle.await? else {
            return Err("a shape-changing root must refuse a placeholder-bearing body".into());
        };
        assert!(error.starts_with("terminal:"), "{error}");
        assert!(
            error.contains("provision"),
            "the refusal must name the action: {error}"
        );
        assert!(
            error.contains("would change the parsed shape"),
            "the refusal must carry the shape-changing diagnosis: {error}"
        );
        assert!(
            reached_names(&reached).is_empty(),
            "a refused body must not fall through to the worker path"
        );
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn an_uncreatable_root_refuses_terminally_naming_the_action() -> TestResult {
        // A root beneath a regular file cannot be created by any retry.
        let scratch = tempfile::tempdir()?;
        let file = scratch.path().join("occupied");
        std::fs::write(&file, b"not a directory")?;
        let (decorated, reached, _transcript) = dispatcher_with_root(
            DeclaredBodyLookup::Declared(ActionBodyContract::Run {
                command: "echo {workspace_root}".to_owned(),
            }),
            Ok("unused".to_owned()),
            WorkspaceRoot::from_resolution(Ok(file.join("clones"))),
        );
        let handle =
            tokio::task::spawn_blocking(move || decorated.dispatch(request("provision", "{}")));
        let Err(error) = handle.await? else {
            return Err("an uncreatable root must refuse a placeholder-bearing body".into());
        };
        assert!(error.starts_with("terminal:"), "{error}");
        assert!(
            error.contains("provision"),
            "the refusal must name the action: {error}"
        );
        assert!(
            error.contains("could not be created"),
            "the refusal must carry the creation-failure diagnosis: {error}"
        );
        assert!(
            reached_names(&reached).is_empty(),
            "a refused body must not fall through to the worker path"
        );
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_body_without_the_placeholder_is_untouched_by_resolution_failure() -> TestResult {
        let (decorated, _reached, _transcript) = dispatcher_with_root(
            DeclaredBodyLookup::Declared(ActionBodyContract::Run {
                command: "echo $greeting".to_owned(),
            }),
            Ok("unused".to_owned()),
            WorkspaceRoot::from_resolution(Err(WorkspaceRootError::Unresolvable {
                reason: "cannot resolve Aion home: set AION_HOME or HOME".to_owned(),
            })),
        );
        let handle = tokio::task::spawn_blocking(move || {
            decorated.dispatch(request("greet", "{\"greeting\":\"still served\"}"))
        });
        let result = handle.await?;
        let encoded = result.map_err(|error| format!("declared command failed: {error}"))?;
        let outcome: serde_json::Value = serde_json::from_str(&encoded)?;
        assert_eq!(outcome["stdout"], "still served");
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn an_unreadable_catalog_delegates_to_the_worker_path() -> TestResult {
        let (decorated, reached) = dispatcher(
            DeclaredBodyLookup::Unreadable("catalog offline".to_owned()),
            Ok("\"served anyway\"".to_owned()),
        );
        let handle =
            tokio::task::spawn_blocking(move || decorated.dispatch(request("resilient", "{}")));
        let result = handle.await?;
        assert_eq!(result, Ok("\"served anyway\"".to_owned()));
        assert_eq!(reached_names(&reached), vec!["resilient".to_owned()]);
        Ok(())
    }

    #[test]
    fn non_object_input_is_refused_terminally_by_shape() {
        for (input, kind) in [
            ("[1,2]", "an array"),
            ("\"text\"", "a string"),
            ("3", "a number"),
            ("null", "null"),
            ("true", "a boolean"),
        ] {
            let Err(error) = decode_arguments(input) else {
                unreachable_refusal(input);
                return;
            };
            assert!(error.starts_with("terminal:"), "{error}");
            assert!(error.contains(kind), "{error} must name {kind}");
        }
    }

    /// Fails the calling test without a panicking accessor.
    fn unreachable_refusal(input: &str) {
        assert!(
            input.is_empty(),
            "input `{input}` must have been refused by shape"
        );
    }

    /// The selection rule cannot be right if it is asked about the wrong run.
    ///
    /// `select_declared_body` is unit-tested on inputs the test itself
    /// constructs, which proves the rule and nothing about the plumbing. This
    /// asserts the other half: the identity the dispatcher hands the reader is
    /// the identity of the dispatch it is serving, not a placeholder and not
    /// another run's.
    #[tokio::test(flavor = "multi_thread")]
    async fn the_reader_is_asked_about_the_run_that_is_dispatching() -> TestResult {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let bodies = DeclaredBodySource::default();
        bodies.install(Arc::new(RecordingBodies {
            seen: Arc::clone(&seen),
        }));
        let reached = Arc::new(Mutex::new(Vec::new()));
        let decorated = DeclaredCommandDispatcher::new(
            Arc::new(RecordingInner {
                reached: Arc::clone(&reached),
                reply: Ok("\"worker-served\"".to_owned()),
            }),
            bodies,
            tokio::runtime::Handle::current(),
            WorkspaceRoot::from_resolution(Ok(std::env::temp_dir())),
            ActivityEventPublisher::new(
                Arc::new(aion_store::InMemoryObservabilityStore::default()),
                TRANSCRIPT_CAPACITY,
            ),
        );

        let dispatch = request("plain", "{}");
        let expected = (dispatch.workflow_id.clone(), dispatch.run_id.clone());
        let handle = tokio::task::spawn_blocking(move || decorated.dispatch(dispatch));
        handle
            .await?
            .map_err(|error| format!("dispatch failed: {error}"))?;

        let observed = match seen.lock() {
            Ok(observed) => observed.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        };
        assert_eq!(
            observed,
            vec![expected],
            "the body reader must be asked about the dispatching run itself"
        );
        Ok(())
    }
}
