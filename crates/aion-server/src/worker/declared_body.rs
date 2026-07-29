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

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use aion::{ActivityDispatch, ActivityDispatcher};
use aion_package::ActionBodyContract;
use aion_worker::shell::ShellAction;

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
        /// How many distinct bodies were found.
        count: usize,
    },
    /// The catalog could not be read. The reader reports why; the dispatch
    /// is delegated so a readable worker path can still serve it.
    Unreadable(String),
}

/// A reader over the deployed contracts' declared action bodies.
pub trait DeclaredBodies: Send + Sync {
    /// Look up the declared body for `action` on `task_queue`.
    fn body_for(&self, task_queue: &str, action: &str) -> DeclaredBodyLookup;
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
    pub fn body_for(&self, task_queue: &str, action: &str) -> DeclaredBodyLookup {
        self.inner.get().map_or(DeclaredBodyLookup::None, |source| {
            source.body_for(task_queue, action)
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

impl DeclaredBodies for EngineDeclaredBodies {
    fn body_for(&self, task_queue: &str, action: &str) -> DeclaredBodyLookup {
        let contracts = match self.engine.worker_contracts_for_queue(task_queue) {
            Ok(contracts) => contracts,
            Err(error) => return DeclaredBodyLookup::Unreadable(error.to_string()),
        };
        // Distinct package versions legitimately coexist (content-hash
        // namespacing), and several may declare this same action. Identical
        // bodies collapse to one; DIFFERENT bodies refuse rather than guess
        // which deploy the running workflow meant.
        let mut bodies: Vec<ActionBodyContract> = Vec::new();
        for deployed in contracts {
            for declared in &deployed.contract.actions {
                if declared.name == action
                    && let Some(body) = &declared.body
                    && !bodies.contains(body)
                {
                    bodies.push(body.clone());
                }
            }
        }
        match bodies.len() {
            0 => DeclaredBodyLookup::None,
            1 => match bodies.pop() {
                Some(body) => DeclaredBodyLookup::Declared(body),
                // Unreachable by the length check; classified rather than
                // panicked so a refactor cannot turn this into an abort.
                None => DeclaredBodyLookup::None,
            },
            count => DeclaredBodyLookup::Ambiguous { count },
        }
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
}

impl DeclaredCommandDispatcher {
    /// Wrap `inner`, consulting `bodies` before every dispatch.
    #[must_use]
    pub fn new(
        inner: Arc<dyn ActivityDispatcher>,
        bodies: DeclaredBodySource,
        tokio: tokio::runtime::Handle,
    ) -> Self {
        Self {
            inner,
            bodies,
            tokio,
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
        let action = ShellAction::new(command).map_err(|error| {
            // The AWL checker refuses these at compile time, so reaching this
            // arm means a defective contract got deployed — name the defect
            // rather than hiding it behind a generic dispatch failure.
            format!("terminal:declared command failed to parse at dispatch: {error}")
        })?;
        let (context, cancellation) =
            aion_worker::ActivityContext::new(request.activity_id.clone(), request.attempt);

        tracing::info!(
            operation = "declared_command_dispatch",
            workflow_id = %request.workflow_id,
            activity_id = %request.activity_id,
            activity_name = %request.name,
            task_queue = %request.task_queue,
            attempt = request.attempt,
            "executing declared action body at the server"
        );

        let outcome = self.tokio.block_on(action.run(&arguments, &context));
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
        match self.bodies.body_for(&request.task_queue, &request.name) {
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
            DeclaredBodyLookup::Ambiguous { count } => Err(format!(
                "terminal:action `{}` on task queue `{}` declares {count} different bodies \
                 across retained package versions; refusing to guess which deploy this run \
                 meant — redeploy so one body remains",
                request.name, request.task_queue
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

    use super::{
        DeclaredBodies, DeclaredBodyLookup, DeclaredBodySource, DeclaredCommandDispatcher,
        decode_arguments,
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
        fn body_for(&self, _task_queue: &str, _action: &str) -> DeclaredBodyLookup {
            self.lookup.clone()
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
        let reached = Arc::new(Mutex::new(Vec::new()));
        let inner = RecordingInner {
            reached: Arc::clone(&reached),
            reply,
        };
        let bodies = DeclaredBodySource::default();
        bodies.install(Arc::new(FixedBodies { lookup }));
        let decorated = DeclaredCommandDispatcher::new(
            Arc::new(inner),
            bodies,
            tokio::runtime::Handle::current(),
        );
        (decorated, reached)
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

    #[tokio::test(flavor = "multi_thread")]
    async fn ambiguous_bodies_refuse_terminally_by_name() -> TestResult {
        let (decorated, reached) = dispatcher(
            DeclaredBodyLookup::Ambiguous { count: 2 },
            Ok(String::new()),
        );
        let handle = tokio::task::spawn_blocking(move || decorated.dispatch(request("torn", "{}")));
        let Err(error) = handle.await? else {
            return Err("ambiguous bodies must refuse".into());
        };
        assert!(error.starts_with("terminal:"), "{error}");
        assert!(error.contains("torn"), "{error}");
        assert!(reached_names(&reached).is_empty());
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
}
