//! [`NornHarness`] — the [`AgentHarness`] implementation that drives a real Norn process.
//!
//! [`AgentHarness::start`] spawns `norn --protocol jsonrpc` with piped stdin/stdout (stderr is
//! inherited so Norn's human logs go straight to the parent's stderr, never into the structured
//! channel), performs the `initialize` handshake — gating on the advertised
//! `protocol: "norn-driven/1"` version and reading Norn's advertised
//! [`InterventionCapabilities`] — issues the `run/execute` request carrying the spec's input as
//! the prompt, and hands the outstanding `run/execute` id to a [`NornSession`] whose reader pump
//! captures its Response as the terminal result.
//!
//! The harness names the `norn` binary by path but takes **no** cargo dependency on Norn: the wire
//! contract it speaks lives in [`crate::protocol`] / [`crate::translate`], mapped from Norn's
//! documented `--protocol jsonrpc` behaviour.

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;

use aion_core::{InterventionCapabilities, InterventionPrimitive};
use aion_integrations::contract::AgentHarness;
use aion_integrations::jsonrpc::{IncomingMessage, JsonRpcConnection, JsonRpcId, JsonRpcRequest};
use aion_integrations::{AgentRunSpec, ChildEnvironment, HarnessError, HarnessWorkspace};
use async_trait::async_trait;
use serde_json::{Value, json};
use tokio::process::{ChildStdin, ChildStdout, Command};

use crate::protocol;
use crate::session::NornSession;
use crate::translate::{self, EventIdentity};

/// The default binary name used when no explicit path is configured: `norn` on `PATH`.
const DEFAULT_NORN_BINARY: &str = "norn";

/// The neutral intervention primitives this adapter can honour, published for a
/// worker's REGISTRATION-time advertisement.
///
/// # Why the adapter publishes this, and why it is not a guess
///
/// A worker must declare its [`InterventionCapabilities`] when it REGISTERS —
/// the server routes an operator command only for a primitive the worker
/// advertised — and registration happens before any `norn` process exists, so
/// the handshake cannot supply it. Without a published set every composition
/// root hardcodes its own idea of what Norn supports, and each copy is a place
/// the truth can rot.
///
/// This set is a fact about the ADAPTER, not a prediction about a binary:
/// [`translate::intervention_to_request`] can build a Norn request for exactly
/// [`InterventionPrimitive::InjectMessage`] and [`InterventionPrimitive::Cancel`]
/// (`intervene/injectMessage` and `intervene/cancel`), and answers every other
/// primitive with [`HarnessError::capability_not_supported`]. So this is the
/// ceiling on what any command routed here could possibly achieve.
///
/// # The registration set and the session set can disagree
///
/// [`NornSession::capabilities`] carries the set the SPAWNED binary advertised in
/// its `initialize` result, and the session gates each command on that. The two
/// sets are read from different places and can differ in both directions:
///
/// * A binary advertising FEWER primitives (an older build, or one with
///   interventions disabled) makes the session gate a command this registration
///   promised: the operator is offered a control and the attempt refuses it.
/// * A binary advertising MORE (say `pause_resume`) passes the session's gate
///   and is then refused by the translation, because this adapter has no request
///   to send for it.
///
/// Neither case is silent — both surface as an attempt-scoped
/// [`HarnessError::CapabilityNotSupported`] naming the primitive — and neither can
/// be resolved at registration time, because the binary that will answer is not
/// running yet. Advertising the adapter's ceiling is therefore the honest choice:
/// it never hides a control that would work, and a control that will not work
/// says so, by name, on the attempt that asked for it.
///
/// [`NornSession::capabilities`]: aion_integrations::AgentSession::capabilities
/// [`HarnessError::capability_not_supported`]: aion_integrations::HarnessError::capability_not_supported
/// [`HarnessError::CapabilityNotSupported`]: aion_integrations::HarnessError::CapabilityNotSupported
#[must_use]
pub fn registration_capabilities() -> InterventionCapabilities {
    InterventionCapabilities::from_primitives([
        InterventionPrimitive::InjectMessage,
        InterventionPrimitive::Cancel,
    ])
}

#[derive(Clone, Debug)]
enum NornArgument {
    Template(String),
    Literal(String),
}

/// The exact child invocation [`AgentHarness::start`] would exec for one run,
/// previewable without spawning anything.
///
/// This exists so a composition root can ASSERT the invocation it composed: a
/// flag that is only ever observed through a live agent run is untested in the
/// case that matters — which is exactly how a mandatory `--account` once went
/// missing from a worker's harness argv. [`NornHarness::invocation`] returns
/// this and the private spawn path is built FROM it, so the preview and the
/// exec'd reality cannot drift.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NornInvocation {
    /// The `norn` binary to exec.
    pub program: PathBuf,
    /// The directory the child is exec'd in. Carried in the preview because the spawn sets
    /// it: a working directory that appeared only at exec time would be the one part of the
    /// invocation a composition root could not assert, and it is the part that decides which
    /// tree the agent reads and edits.
    pub cwd: PathBuf,
    /// The full argument list, in exec order: the fixed `--protocol jsonrpc`
    /// followed by the configured extra arguments with their run-identity
    /// placeholders expanded.
    pub argv: Vec<String>,
    /// The child's WHOLE declared environment, when one was declared. Present means the spawn
    /// CLEARS the inherited environment and sets exactly these pairs; absent means the child
    /// inherits, which is what a hand-written worker that declares nothing still gets.
    pub declared_environment: Option<ChildEnvironment>,
    /// The `KEY=VALUE` pairs set on the child's environment, in order.
    pub env: Vec<(String, String)>,
    /// The variables removed from the child's environment. Removals are
    /// applied AFTER the sets, so removing a key wins over setting it.
    pub env_removed: Vec<String>,
}

/// A harness that runs each activity attempt as a `norn --protocol jsonrpc` child process.
///
/// Holds the harness-specific settings (the binary location and any fixed extra arguments) so the
/// [`AgentRunSpec`] stays harness-blind — the spec never carries Norn configuration.
#[derive(Clone, Debug)]
pub struct NornHarness {
    binary: PathBuf,
    workspace: HarnessWorkspace,
    extra_args: Vec<NornArgument>,
    declared_environment: Option<ChildEnvironment>,
    env: Vec<(String, String)>,
    env_removed: Vec<String>,
}

// NO `Default`, deliberately. A defaulted harness is one that never says where its agent
// works, and the only value such a default could carry is "wherever this worker process
// happened to be started" — which is the inheritance this type exists to end. The workspace
// is a constructor parameter rather than a builder setter for the same reason: an
// `Option` left `None` would either fall back to inheriting (the defect, intact) or refuse
// at spawn time (a harness that compiles and cannot run). Positional means the compiler asks
// every composition root the question, and none of them can decline it — and the two forms
// of `HarnessWorkspace` mean the honest answer "it depends on the job" is now sayable
// instead of forcing a single tree to be named.

impl NornHarness {
    /// A harness that invokes `norn` from `PATH` with no extra arguments, working in
    /// `workspace`.
    ///
    /// Whichever form `workspace` takes, the directory it yields must be absolute and must
    /// exist; both are validated at [`AgentHarness::start`], where the failure can be
    /// reported as a typed error rather than a panic.
    #[must_use]
    pub fn new(workspace: HarnessWorkspace) -> Self {
        Self {
            binary: PathBuf::from(DEFAULT_NORN_BINARY),
            workspace,
            extra_args: Vec::new(),
            declared_environment: None,
            env: Vec::new(),
            env_removed: Vec::new(),
        }
    }

    /// A harness that invokes the `norn` binary at an explicit path, working in
    /// `workspace`.
    ///
    /// Whichever form `workspace` takes, the directory it yields must be absolute and must
    /// exist; both are validated at [`AgentHarness::start`], where the failure can be
    /// reported as a typed error rather than a panic.
    #[must_use]
    pub fn with_binary(binary: impl Into<PathBuf>, workspace: HarnessWorkspace) -> Self {
        Self {
            binary: binary.into(),
            workspace,
            extra_args: Vec::new(),
            declared_environment: None,
            env: Vec::new(),
            env_removed: Vec::new(),
        }
    }

    /// Adds a fixed extra argument passed to every spawned `norn` process.
    ///
    /// Used to configure the harness (e.g. a provider or model flag) without leaking Norn config
    /// into the neutral [`AgentRunSpec`].
    ///
    /// The argument may carry run-identity placeholders, expanded from the [`AgentRunSpec`] at
    /// spawn time (see `expand_arg`): `{workflow_id}` becomes the spec's workflow id and
    /// `{activity_type}` becomes the dispatched activity-type name. Exactly those two are
    /// recognised; any other `{...}` text passes through literally.
    #[must_use]
    pub fn with_arg(mut self, arg: impl Into<String>) -> Self {
        self.extra_args.push(NornArgument::Template(arg.into()));
        self
    }

    /// Adds a fixed argument without expanding `{workflow_id}` or `{activity_type}` substrings.
    #[must_use]
    pub fn with_literal_arg(mut self, arg: impl Into<String>) -> Self {
        self.extra_args.push(NornArgument::Literal(arg.into()));
        self
    }

    /// Declares the `norn` child's WHOLE environment.
    ///
    /// Without this the child inherits the worker process's environment, which is invisible
    /// configuration nobody wrote down and nobody reviewed. With it the child's environment is
    /// CONSTRUCTED: the spawn clears everything first and sets exactly the resolved pairs, so a
    /// variable the declaration does not name is absent in the child whatever the parent carries.
    ///
    /// [`Self::with_env`] and [`Self::without_env`] still apply, on top of the declared set.
    #[must_use]
    pub fn with_declared_environment(mut self, environment: ChildEnvironment) -> Self {
        self.declared_environment = Some(environment);
        self
    }

    /// Sets an environment variable on every spawned `norn` process.
    ///
    /// Applied to the CHILD only (never the parent process), so a harness can supply Norn's
    /// configuration environment (e.g. a provider API-key variable) without mutating shared
    /// process state.
    #[must_use]
    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }

    /// Removes an environment variable from every spawned `norn` process.
    ///
    /// Applied to the CHILD only (never the parent process), so a harness can strip ambient
    /// configuration a spawned Norn must not inherit — e.g. removing `OPENAI_API_KEY` forces
    /// Norn onto the operator's `ChatGPT` OAuth login when a stray ambient key would otherwise
    /// take precedence and fail. Removals are applied AFTER the explicit [`Self::with_env`]
    /// sets, so removing a key wins over setting the same key.
    #[must_use]
    pub fn without_env(mut self, key: impl Into<String>) -> Self {
        self.env_removed.push(key.into());
        self
    }

    /// The exact child invocation [`AgentHarness::start`] would exec for `spec`: the fixed
    /// `norn --protocol jsonrpc` arguments followed by the configured extra arguments with
    /// their run-identity placeholders expanded (see [`expand_arg`]), plus the configured
    /// child-environment sets and removals.
    ///
    /// Public so a composition root can assert what it composed without spawning a process;
    /// [`Self::command`] is built from this value, so the preview IS the spawn.
    ///
    /// # Errors
    ///
    /// Returns [`HarnessError::Configuration`] when the harness takes its working directory
    /// from the job and THIS job does not state one. That is why the preview is fallible at
    /// all: a directory the run supplies is not knowable until there is a run, so the
    /// preview's promise — that it IS the spawn — can only be kept by resolving the same
    /// value the spawn will use, and by failing where the spawn would.
    pub fn invocation(&self, spec: &AgentRunSpec) -> Result<NornInvocation, HarnessError> {
        let mut argv = vec!["--protocol".to_owned(), "jsonrpc".to_owned()];
        for argument in &self.extra_args {
            argv.push(match argument {
                NornArgument::Template(template) => expand_arg(template, spec),
                NornArgument::Literal(literal) => literal.clone(),
            });
        }
        Ok(NornInvocation {
            program: self.binary.clone(),
            cwd: self.workspace.for_attempt(&spec.input)?,
            argv,
            declared_environment: self.declared_environment.clone(),
            env: self.env.clone(),
            env_removed: self.env_removed.clone(),
        })
    }

    /// Refuses a `cwd` this launch cannot stand in, before anything is spawned.
    ///
    /// Both refusals are raised as [`HarnessError::configuration`] and NOT as protocol or
    /// transport errors, because the classification is load-bearing: `Configuration` is
    /// deterministic, so the worker maps it to a TERMINAL activity failure. `cwd` comes from
    /// the document, and the next attempt reads the same document — classing either refusal
    /// retryable would spend the whole attempt budget re-confirming a value that cannot
    /// change, and would tell the operator a story about a flaky channel when the truth is
    /// one wrong line they can fix.
    ///
    /// The EXISTENCE arm is here because this adapter now sets `current_dir`. Before it did,
    /// a spawn failure could only be the binary, and [`Self::spawn`]'s message says so by
    /// name. With a working directory set, `Command::spawn` answers a missing directory with
    /// the same `No such file or directory` — a retryable transport refusal, naming the
    /// binary, for a fault that is neither. Checking first is what keeps that message honest.
    ///
    /// The directory is never CREATED here. A `cwd` names the tree the agent is meant to read
    /// and edit; conjuring an empty one would turn a mistyped path into an agent working
    /// against nothing, which is the failure this whole setting exists to make impossible.
    /// (`{workspace_root}` is different and is expanded, and its root created, one layer up —
    /// that root is the launcher's own directory to make, not the document's claim about a
    /// tree that should already be there.)
    ///
    /// Takes the RESOLVED directory rather than reading a field, because there is no longer
    /// one directory to read: a harness whose tree arrives with the job has a different
    /// answer per attempt, and a check that consulted configuration instead of the value
    /// about to be used would be checking something the spawn never sees.
    fn require_launchable_cwd(cwd: &std::path::Path) -> Result<(), HarnessError> {
        if !cwd.is_absolute() {
            return Err(HarnessError::configuration(format!(
                "the norn harness `cwd` must be an ABSOLUTE path, got `{}`. A relative path is \
                 resolved against whatever directory this worker process was started in, which \
                 is the inherited working directory this setting exists to replace — and it \
                 would be inherited while the document appears to have declared one.",
                cwd.display()
            )));
        }
        if !cwd.is_dir() {
            return Err(HarnessError::configuration(format!(
                "the norn harness `cwd` is `{}`, which is not a directory on this box, so no \
                 agent can be started in it. The declared working directory names the tree the \
                 agent reads and edits; it is not created here, because an empty directory \
                 conjured from a mistyped path would put the agent to work against nothing.",
                cwd.display()
            )));
        }
        Ok(())
    }

    /// Builds the spawn command for one run from [`Self::invocation`], with piped
    /// stdin/stdout and inherited stderr.
    fn command(&self, spec: &AgentRunSpec) -> Result<Command, HarnessError> {
        let invocation = self.invocation(spec)?;
        Self::require_launchable_cwd(&invocation.cwd)?;
        let mut command = Command::new(&invocation.program);
        command.args(&invocation.argv).current_dir(&invocation.cwd);
        if let Some(declared) = &invocation.declared_environment {
            command.env_clear();
            command.envs(declared.pairs().iter().map(|(key, value)| (key, value)));
        }
        command
            .envs(invocation.env.iter().map(|(key, value)| (key, value)))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true);
        // Removals AFTER the sets, so `without_env` wins over `with_env` for the same key.
        for key in &invocation.env_removed {
            command.env_remove(key);
        }
        Ok(command)
    }

    /// Spawns the `norn` child for one run, its arguments expanded from `spec`.
    fn spawn(&self, spec: &AgentRunSpec) -> Result<tokio::process::Child, HarnessError> {
        self.command(spec)?.spawn().map_err(|source| {
            HarnessError::transport(format!(
                "failed to spawn `{}`: {source}",
                self.binary.display()
            ))
        })
    }
}

/// Expands the run-identity placeholders in one configured extra argument.
///
/// Exactly two placeholders are recognised: `{workflow_id}` (the spec's workflow id, rendered as
/// its canonical string form) and `{activity_type}` (the activity-type name the engine
/// dispatched). Any other `{...}` text is **not** a placeholder and passes through literally, so
/// arguments containing unrelated braces (e.g. a JSON snippet) survive unchanged.
fn expand_arg(template: &str, spec: &AgentRunSpec) -> String {
    template
        .replace("{workflow_id}", &spec.workflow_id.to_string())
        .replace("{activity_type}", &spec.activity_type)
}

/// Takes the child's piped stdout + stdin, or a transport error if either was not piped.
fn take_child_io(
    child: &mut tokio::process::Child,
) -> Result<(ChildStdout, ChildStdin), HarnessError> {
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| HarnessError::transport("spawned norn child has no piped stdout"))?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| HarnessError::transport("spawned norn child has no piped stdin"))?;
    Ok((stdout, stdin))
}

/// Sends `initialize` and reads its Response, returning the negotiated capabilities.
///
/// The result is gated on `protocol: "norn-driven/1"` (inside
/// [`translate::parse_capabilities`]): a missing or different version is a
/// [`HarnessError::Protocol`] naming the expected and received values — the honest "your norn
/// binary is stale" signal, raised before any run is issued.
///
/// The handshake is synchronous request/response before the run starts, so the pre-run reads here
/// consume only the `initialize` Response — event notifications only begin after `run/execute`.
async fn handshake<R, W>(
    connection: &JsonRpcConnection<R, W>,
) -> Result<aion_core::InterventionCapabilities, HarnessError>
where
    R: tokio::io::AsyncRead + Unpin + Send,
    W: tokio::io::AsyncWrite + Unpin + Send,
{
    let init_id = connection.next_request_id();
    let request = JsonRpcRequest::new(init_id.clone(), protocol::METHOD_INITIALIZE, None);
    connection.send_request(&request).await?;
    let result = read_response_for(connection, &init_id).await?;
    translate::parse_capabilities(&result)
}

/// Reads inbound frames until the Response for `expected_id` arrives, returning its `result`.
///
/// Any notification arriving during the handshake (none is expected before `run/execute`, but the
/// read is defensive) is skipped; a non-matching Response or an error Response is a protocol
/// violation.
async fn read_response_for<R, W>(
    connection: &JsonRpcConnection<R, W>,
    expected_id: &JsonRpcId,
) -> Result<Value, HarnessError>
where
    R: tokio::io::AsyncRead + Unpin + Send,
    W: tokio::io::AsyncWrite + Unpin + Send,
{
    loop {
        let message = connection
            .recv()
            .await?
            .ok_or_else(|| HarnessError::transport("norn closed the channel during handshake"))?;
        match message {
            IncomingMessage::Response(response) if &response.id == expected_id => {
                if let Some(error) = response.error {
                    return Err(HarnessError::harness(format!(
                        "initialize failed (code {}): {}",
                        error.code, error.message
                    )));
                }
                return response.result.ok_or_else(|| {
                    HarnessError::protocol("initialize response carried no result")
                });
            }
            IncomingMessage::Response(other) => {
                return Err(HarnessError::protocol(format!(
                    "unexpected response id during handshake: {:?}",
                    other.id
                )));
            }
            IncomingMessage::Notification(_) => {}
            IncomingMessage::Request(request) => {
                return Err(HarnessError::protocol(format!(
                    "unexpected child request during handshake: {}",
                    request.method
                )));
            }
        }
    }
}

#[async_trait]
impl AgentHarness for NornHarness {
    type Session = NornSession<ChildStdout, ChildStdin>;

    async fn start(&self, spec: AgentRunSpec) -> Result<Self::Session, HarnessError> {
        // Read through the same workspace that decides the directory, so the field the
        // agent is asked with and the field it stands in are decided from one input by one
        // rule. An adapter that read the prompt on its own would be the second place that
        // rule lives, and the first place it drifts.
        let prompt = self.workspace.prompt_for_attempt(&spec.input, &[])?;
        // Named BEFORE the spawn: `norn` is resolved on `PATH` by default, and a declaration
        // that omits `PATH` would otherwise die as a bare `No such file or directory`.
        if let Some(declared) = &self.declared_environment {
            declared.require_for_program(&self.binary)?;
        }
        let mut child = self.spawn(&spec)?;
        let (stdout, stdin) = take_child_io(&mut child)?;
        let connection = Arc::new(JsonRpcConnection::new(stdout, stdin));

        // 1. initialize handshake → capabilities.
        let capabilities = handshake(connection.as_ref()).await?;

        // 2. run/execute request. Its id is the ONLY id the terminal result is captured under; the
        //    session's reader pump routes that Response to `wait_result`.
        let run_id = connection.next_request_id();
        let run_request = JsonRpcRequest::new(
            run_id.clone(),
            protocol::METHOD_RUN_EXECUTE,
            Some(json!({ protocol::PARAM_PROMPT: prompt })),
        );
        connection.send_request(&run_request).await?;

        let identity = EventIdentity {
            workflow_id: spec.workflow_id,
            run_id: spec.run_id,
            activity_id: spec.activity_id,
            attempt: spec.attempt,
        };
        Ok(NornSession::start(
            connection,
            capabilities,
            run_id,
            identity,
            Some(child),
        ))
    }
}

#[cfg(test)]
mod tests {
    //! Fast unit tests of spec-aware launch construction. Most inspect the built
    //! [`Command`] directly; the #246 collision fixture spawns only a local sleep
    //! script in a temporary directory.

    use std::error::Error;
    use std::os::unix::fs::PermissionsExt;

    use aion_core::{ActivityId, ContentType, Payload, RunId, WorkflowId};

    use aion_integrations::HarnessWorkspace;

    use super::{AgentRunSpec, HarnessError, NornHarness, expand_arg, registration_capabilities};

    /// The one tree these configuration tests root an agent in.
    const WORKSPACE: &str = "/srv/workspace";

    /// A workspace that answers with one directory for every attempt.
    fn fixed(directory: impl Into<std::path::PathBuf>) -> HarnessWorkspace {
        HarnessWorkspace::Fixed(directory.into())
    }

    type TestResult<T = ()> = Result<T, Box<dyn Error>>;

    fn spec() -> AgentRunSpec {
        spec_with_input(Payload::new(ContentType::Json, b"run".to_vec()))
    }

    fn spec_with_input(input: Payload) -> AgentRunSpec {
        AgentRunSpec::new(
            WorkflowId::new_v4(),
            RunId::new_v4(),
            ActivityId::from_sequence_position(1),
            1,
            "dev",
            input,
        )
    }

    fn sleeping_binary(root: &std::path::Path) -> TestResult<std::path::PathBuf> {
        let binary = root.join("held-agent.sh");
        std::fs::write(&binary, "#!/bin/sh\nsleep 30\n")?;
        let mut permissions = std::fs::metadata(&binary)?.permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&binary, permissions)?;
        Ok(binary)
    }

    /// The argv the built command would exec, decoded as UTF-8 strings.
    fn argv(harness: &NornHarness, spec: &AgentRunSpec) -> TestResult<Vec<String>> {
        let decoded = harness
            .command(spec)?
            .as_std()
            .get_args()
            .map(|arg| arg.to_str().map(str::to_owned).ok_or("argv is not UTF-8"))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(decoded)
    }

    /// The PUBLISHED registration set must be exactly the set this adapter can
    /// translate — no wider, no narrower. A wider set advertises a control the
    /// translation would refuse; a narrower one hides a control that works.
    ///
    /// Driven over EVERY primitive the neutral enum has, so adding a variant to
    /// [`translate::intervention_to_request`] without publishing it (or the
    /// reverse) fails here rather than at an operator's desk.
    #[test]
    fn the_published_set_is_exactly_what_the_adapter_can_translate() {
        use aion_core::{ApprovalDecision, InjectPriority, InterventionKind};

        use crate::translate::intervention_to_request;

        let every_primitive = [
            InterventionKind::InjectMessage {
                text: "steer".to_owned(),
                priority: InjectPriority::Interrupt,
            },
            InterventionKind::Cancel {
                reason: "stop".to_owned(),
            },
            InterventionKind::PauseResume { paused: true },
            InterventionKind::UpdateBudget {
                max_tokens: Some(1),
                max_turns: None,
            },
            InterventionKind::RespondToApproval {
                call_id: "call-1".to_owned(),
                decision: ApprovalDecision::Approve,
                note: None,
            },
        ];
        let published = registration_capabilities();
        for kind in &every_primitive {
            let primitive = kind.primitive();
            assert_eq!(
                published.supports_primitive(primitive),
                intervention_to_request(kind).is_ok(),
                "the published registration set disagrees with the translation for {primitive:?}"
            );
        }
    }

    #[test]
    fn expands_both_placeholders_amid_literal_text() {
        let spec = spec();
        let expanded = expand_arg("run={workflow_id}:{activity_type}!", &spec);
        assert_eq!(expanded, format!("run={}:dev!", spec.workflow_id));
    }

    #[test]
    fn argument_without_placeholders_passes_through_unchanged() {
        let spec = spec();
        assert_eq!(expand_arg("--verbose", &spec), "--verbose");
    }

    #[test]
    fn unrecognised_braced_text_is_literal() {
        // Only `{workflow_id}` / `{activity_type}` are placeholders; every other `{...}` (an
        // unknown name, a JSON snippet) is plain text and must survive verbatim.
        let spec = spec();
        assert_eq!(
            expand_arg("{attempt} {\"json\":true} {workflow-id}", &spec),
            "{attempt} {\"json\":true} {workflow-id}"
        );
    }

    #[test]
    fn removed_env_vars_are_expressed_as_child_removals() -> TestResult {
        let spec = spec();
        let tree = tempfile::tempdir()?;
        let harness = NornHarness::with_binary("/bin/norn", fixed(tree.path()))
            .with_env("OPENAI_API_KEY", "stray-ambient-key")
            .without_env("OPENAI_API_KEY");
        let command = harness.command(&spec)?;

        // On std's Command, an env REMOVAL is expressed as a `(key, None)` entry — the child
        // will not inherit the variable even when the parent environment carries it, and the
        // removal wins over an explicit set of the same key.
        let removal_expressed = command
            .as_std()
            .get_envs()
            .any(|(key, value)| key == "OPENAI_API_KEY" && value.is_none());
        assert!(
            removal_expressed,
            "without_env must express a child-side env removal on the spawned command"
        );
        Ok(())
    }

    /// The adapter reads the prompt through its workspace — the same value `start` sends as
    /// the `run/execute` prompt one line after the environment is checked. What the reader
    /// itself does with each shape of input is proven in
    /// `aion_integrations::HarnessWorkspace`; what this pins is that this adapter asks it
    /// rather than decoding the payload on its own, which is how the two came to disagree and
    /// the agent came to be sent `{"prompt":"…"}` as its instructions.
    #[test]
    fn the_prompt_is_read_through_the_same_workspace_that_places_the_agent() -> TestResult {
        let harness =
            NornHarness::with_binary("/bin/norn", HarnessWorkspace::PerRun("repo".to_owned()));
        let spec = spec_with_input(Payload::from_json(&serde_json::json!({
            "repo": WORKSPACE,
            "prompt": "review the diff",
        }))?);
        assert_eq!(
            harness.workspace.for_attempt(&spec.input)?,
            std::path::PathBuf::from(WORKSPACE)
        );
        assert_eq!(
            harness.workspace.prompt_for_attempt(&spec.input, &[])?,
            "review the diff"
        );
        Ok(())
    }

    #[test]
    fn the_invocation_preview_is_the_spawned_reality() -> TestResult {
        // `command` is BUILT from `invocation`, so asserting both against one
        // configured harness proves the public preview is what a spawn execs.
        let spec = spec();
        let tree = tempfile::tempdir()?;
        let harness = NornHarness::with_binary("/bin/norn", fixed(tree.path()))
            .with_arg("--session-id")
            .with_arg("{workflow_id}-{activity_type}")
            .with_env("NORN_LOG", "debug")
            .without_env("OPENAI_API_KEY");
        let invocation = harness.invocation(&spec)?;
        assert_eq!(
            invocation,
            super::NornInvocation {
                program: std::path::PathBuf::from("/bin/norn"),
                cwd: tree.path().to_path_buf(),
                argv: vec![
                    "--protocol".to_owned(),
                    "jsonrpc".to_owned(),
                    "--session-id".to_owned(),
                    format!("{}-dev", spec.workflow_id),
                ],
                // None: this harness declares no environment, so the child inherits —
                // the pre-declaration behaviour a hand-written worker still gets.
                declared_environment: None,
                env: vec![("NORN_LOG".to_owned(), "debug".to_owned())],
                env_removed: vec!["OPENAI_API_KEY".to_owned()],
            }
        );
        assert_eq!(
            argv(&harness, &spec)?,
            invocation.argv,
            "the command's argv must be exactly the previewed argv"
        );
        Ok(())
    }

    /// A DECLARED environment travels on the previewable invocation, so what a
    /// composition root asserts is what the spawn performs — the same reason the argv
    /// preview exists.
    #[test]
    fn a_declared_environment_rides_the_previewable_invocation() -> Result<(), String> {
        let spec = spec();
        let mut context = std::collections::BTreeMap::new();
        context.insert("PATH".to_owned(), "/usr/bin".to_owned());
        context.insert("CLAUDECODE".to_owned(), "1".to_owned());
        let declaration = aion_integrations::EnvironmentDeclaration::new(["PATH"])
            .map_err(|error| error.to_string())?;

        let harness = NornHarness::with_binary("/bin/norn", fixed(WORKSPACE))
            .with_declared_environment(declaration.resolve(&context));
        let invocation = harness
            .invocation(&spec)
            .map_err(|error| error.to_string())?;
        let Some(declared) = invocation.declared_environment else {
            return Err("the declaration must reach the invocation preview".to_owned());
        };
        assert_eq!(
            declared.pairs(),
            [("PATH".to_owned(), "/usr/bin".to_owned())]
        );
        assert!(
            !declared.carries("CLAUDECODE"),
            "a variable the declaration does not name is absent whatever the parent carries"
        );
        Ok(())
    }

    #[test]
    fn template_args_expand_and_literal_args_preserve_reserved_substrings() -> TestResult {
        let spec = spec();
        let tree = tempfile::tempdir()?;
        let harness = NornHarness::with_binary("/bin/norn", fixed(tree.path()))
            .with_arg("--label")
            .with_arg("{activity_type}/{workflow_id}")
            .with_literal_arg("{workflow_id}/{activity_type}")
            .with_arg("--model")
            .with_arg("mock-model");

        assert_eq!(
            argv(&harness, &spec)?,
            vec![
                "--protocol".to_owned(),
                "jsonrpc".to_owned(),
                "--label".to_owned(),
                format!("dev/{}", spec.workflow_id),
                "{workflow_id}/{activity_type}".to_owned(),
                "--model".to_owned(),
                "mock-model".to_owned(),
            ]
        );
        Ok(())
    }

    #[test]
    fn the_child_runs_in_the_declared_cwd() -> TestResult {
        let spec = spec();
        let tree = tempfile::tempdir()?;
        let harness = NornHarness::with_binary("/bin/norn", fixed(tree.path()));
        let command = harness.command(&spec)?;
        let directory = command
            .as_std()
            .get_current_dir()
            .ok_or("the spawned child's working directory must be the declared cwd")?;
        assert_eq!(directory, tree.path());
        Ok(())
    }

    /// #246 red: two dispatches for the same declared worktree currently both
    /// spawn. The second must instead return a typed configuration refusal that
    /// names the live sibling pid holding the tree.
    #[tokio::test]
    async fn second_spawn_in_one_declared_tree_is_refused_typed() -> TestResult {
        let tree = tempfile::tempdir()?;
        let binary = sleeping_binary(tree.path())?;
        let harness = NornHarness::with_binary(binary, fixed(tree.path()));
        let spec = spec();
        let first = harness.spawn(&spec)?;

        let refusal = harness
            .spawn(&spec)
            .err()
            .ok_or("today both sibling agents spawn into the same declared cwd")?;

        assert!(
            matches!(refusal, HarnessError::Configuration { .. }),
            "spawn collision must be typed: {refusal:?}"
        );
        assert!(
            refusal.to_string().contains("live sibling pid"),
            "{refusal}"
        );
        drop(first);
        Ok(())
    }

    /// The declared directory must reach the PREVIEW too, not only the spawn.
    ///
    /// `NornInvocation` is the value a composition root asserts against without spawning a
    /// process, and `command` is built from it. A `cwd` that appeared only inside `command`
    /// would be the one part of the launch nobody could check in advance — and it is the part
    /// that decides which tree the agent edits.
    #[test]
    fn the_preview_carries_the_declared_cwd() -> TestResult {
        let harness = NornHarness::with_binary("/bin/norn", fixed(WORKSPACE));
        assert_eq!(
            harness.invocation(&spec())?.cwd,
            std::path::Path::new(WORKSPACE)
        );
        Ok(())
    }

    /// The per-run form on the adapter that spawns: two jobs served by ONE harness stand in
    /// two different trees, and each stands in the tree its own input named. The second job is
    /// the control — a resolver that ignored the input would answer the same directory twice.
    #[test]
    fn each_job_runs_in_the_tree_that_job_named() -> TestResult {
        let harness =
            NornHarness::with_binary("/bin/norn", HarnessWorkspace::PerRun("repo".to_owned()));
        let mut seen = Vec::new();
        for tree in ["/srv/lane-3", "/srv/lane-9"] {
            let spec = spec_with_input(Payload::from_json(&serde_json::json!({ "repo": tree }))?);
            seen.push(harness.invocation(&spec)?.cwd);
        }
        assert_eq!(
            seen,
            vec![
                std::path::PathBuf::from("/srv/lane-3"),
                std::path::PathBuf::from("/srv/lane-9")
            ]
        );
        Ok(())
    }

    /// A job that does not name its tree is refused TERMINALLY rather than started in
    /// whichever directory this worker process happens to occupy.
    #[test]
    fn a_job_that_names_no_tree_is_refused_before_any_spawn() -> TestResult {
        let harness =
            NornHarness::with_binary("/bin/norn", HarnessWorkspace::PerRun("repo".to_owned()));
        let spec = spec_with_input(Payload::from_json(
            &serde_json::json!({ "branch": "main" }),
        )?);
        let error = harness
            .invocation(&spec)
            .err()
            .ok_or("a job with no directory cannot launch an agent")?;
        assert!(
            matches!(error, HarnessError::Configuration { .. }),
            "a job that did not say where to work is a configuration refusal: {error:?}"
        );
        assert!(
            error.is_deterministic(),
            "the same input meets the same wall on every attempt: {error:?}"
        );
        Ok(())
    }

    /// A directory that arrives WITH the job travels the same absolute-path net a written one
    /// does. Without this, the refusals below could hold for documents while a job-supplied
    /// relative path walked straight into a spawn.
    #[test]
    fn a_relative_tree_from_the_job_is_refused_the_same_way() -> TestResult {
        let harness =
            NornHarness::with_binary("/bin/norn", HarnessWorkspace::PerRun("repo".to_owned()));
        let spec = spec_with_input(Payload::from_json(
            &serde_json::json!({ "repo": "lane-3" }),
        )?);
        let error = harness
            .command(&spec)
            .err()
            .ok_or("a relative directory is relative wherever it came from")?;
        assert!(
            error.is_deterministic(),
            "a job-supplied relative path is identically relative on every attempt: {error:?}"
        );
        let message = error.to_string();
        assert!(
            message.contains("cwd") && message.contains("ABSOLUTE"),
            "the refusal names the setting and the requirement: {message}"
        );
        Ok(())
    }

    #[test]
    fn a_relative_cwd_is_refused_as_a_terminal_configuration_error() -> TestResult {
        let error = NornHarness::require_launchable_cwd(std::path::Path::new("workspace"))
            .err()
            .ok_or("a relative cwd must be refused")?;

        assert!(
            matches!(error, HarnessError::Configuration { .. }),
            "got {error:?}"
        );
        // The CLASSIFICATION is the load-bearing half, asserted separately from the variant so
        // that re-classifying the refusal fails here even if the variant name survives: `cwd`
        // comes from the document, the next attempt reads the same document, so a retryable
        // refusal would spend the whole attempt budget re-confirming one value that cannot
        // change — and would describe a fixable line to the operator as a flaky channel.
        assert!(
            error.is_deterministic(),
            "a refusal sourced from the document must be TERMINAL, not retryable: {error:?}"
        );

        let message = error.to_string();
        assert!(
            message.contains("cwd") && message.contains("ABSOLUTE"),
            "the refusal names the setting and the requirement: {message}"
        );
        Ok(())
    }

    /// A `cwd` that is absolute but is not there is refused HERE, terminally, rather than at
    /// the spawn.
    ///
    /// This adapter sets `current_dir`, so `Command::spawn` now answers a missing working
    /// directory with the same `No such file or directory` it answers a missing binary with —
    /// and [`NornHarness::spawn`] maps that to a retryable transport refusal naming the
    /// BINARY. Left to the spawn, one wrong line in a document would be re-attempted to the
    /// end of the budget while pointing the operator at the wrong setting.
    ///
    /// Both assertions are the pin: the classification, because that is what decides whether
    /// the worker retries, and the message naming the offending directory, because that is
    /// what the operator acts on.
    #[test]
    fn a_cwd_that_is_not_there_is_refused_before_the_spawn_can_blame_the_binary() -> TestResult {
        let scratch = tempfile::tempdir()?;
        let absent = scratch.path().join("no-such-tree");
        let error = NornHarness::require_launchable_cwd(&absent)
            .err()
            .ok_or("a cwd that does not exist must be refused")?;

        assert!(
            error.is_deterministic(),
            "a directory named by the document is identically absent on every attempt, so the \
             refusal is terminal: {error:?}"
        );
        let message = error.to_string();
        assert!(
            message.contains(&absent.display().to_string()) && message.contains("cwd"),
            "the refusal names the setting and the directory the operator must fix, rather \
             than the binary the spawn would have blamed: {message}"
        );
        Ok(())
    }

    /// The presence twin for both refusals above: with a directory that IS there, the same
    /// gate accepts. Without this, a gate that refused every `cwd` in existence would satisfy
    /// the two refusal tests exactly as well.
    #[test]
    fn an_absolute_cwd_that_exists_is_accepted() -> TestResult {
        let scratch = tempfile::tempdir()?;
        NornHarness::require_launchable_cwd(scratch.path())
            .map_err(|error| format!("an absolute, existing cwd must be accepted: {error:?}"))?;
        Ok(())
    }
}
