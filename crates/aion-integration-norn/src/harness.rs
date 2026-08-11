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
use aion_integrations::{AgentRunSpec, ChildEnvironment, HarnessError};
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
    extra_args: Vec<NornArgument>,
    declared_environment: Option<ChildEnvironment>,
    env: Vec<(String, String)>,
    env_removed: Vec<String>,
}

impl Default for NornHarness {
    fn default() -> Self {
        Self::new()
    }
}

impl NornHarness {
    /// A harness that invokes `norn` from `PATH` with no extra arguments.
    #[must_use]
    pub fn new() -> Self {
        Self {
            binary: PathBuf::from(DEFAULT_NORN_BINARY),
            extra_args: Vec::new(),
            declared_environment: None,
            env: Vec::new(),
            env_removed: Vec::new(),
        }
    }

    /// A harness that invokes the `norn` binary at an explicit path.
    #[must_use]
    pub fn with_binary(binary: impl Into<PathBuf>) -> Self {
        Self {
            binary: binary.into(),
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
    #[must_use]
    pub fn invocation(&self, spec: &AgentRunSpec) -> NornInvocation {
        let mut argv = vec!["--protocol".to_owned(), "jsonrpc".to_owned()];
        for argument in &self.extra_args {
            argv.push(match argument {
                NornArgument::Template(template) => expand_arg(template, spec),
                NornArgument::Literal(literal) => literal.clone(),
            });
        }
        NornInvocation {
            program: self.binary.clone(),
            argv,
            declared_environment: self.declared_environment.clone(),
            env: self.env.clone(),
            env_removed: self.env_removed.clone(),
        }
    }

    /// Builds the spawn command for one run from [`Self::invocation`], with piped
    /// stdin/stdout and inherited stderr.
    fn command(&self, spec: &AgentRunSpec) -> Command {
        let invocation = self.invocation(spec);
        let mut command = Command::new(&invocation.program);
        command.args(&invocation.argv);
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
        command
    }

    /// Spawns the `norn` child for one run, its arguments expanded from `spec`.
    fn spawn(&self, spec: &AgentRunSpec) -> Result<tokio::process::Child, HarnessError> {
        self.command(spec).spawn().map_err(|source| {
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

/// Reads the prompt string an `AgentRunSpec` carries from its input payload.
///
/// Deliberate decoding semantics — the activity input is a serialized [`Payload`], not raw prompt
/// text, so a `String`-typed activity input arrives as its JSON encoding (surrounding quotes plus
/// `\n`/`\"` escapes):
///
/// - JSON content type, bytes parse as a JSON **string** → unwrap to the inner string, so the
///   agent receives the exact text the workflow passed (multi-line prompts survive verbatim).
/// - JSON content type, bytes parse as any **other** JSON value (object/array/number/…) → the raw
///   JSON text passes through unchanged; a structured payload is a legitimate prompt for some
///   harnesses, and inventing a projection here would lose information.
/// - JSON content type, bytes are **not** valid JSON → the raw UTF-8 text passes through
///   unchanged ([`Payload`] is a dumb carrier that does not validate on construction).
/// - Non-JSON content types → the bytes decoded as UTF-8 text, today's behaviour.
///
/// A non-UTF-8 input is a protocol mismatch surfaced honestly in every case.
fn prompt_from_spec(spec: &AgentRunSpec) -> Result<String, HarnessError> {
    let text = std::str::from_utf8(spec.input.bytes())
        .map(str::to_owned)
        .map_err(|source| {
            HarnessError::protocol(format!("run input is not valid UTF-8: {source}"))
        })?;
    if !matches!(spec.input.content_type(), aion_core::ContentType::Json) {
        return Ok(text);
    }
    match serde_json::from_str::<Value>(&text) {
        Ok(Value::String(inner)) => Ok(inner),
        _ => Ok(text),
    }
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
        let prompt = prompt_from_spec(&spec)?;
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
    //! Fast unit tests of the spec-aware argument construction and prompt decoding — no process
    //! is spawned; the built [`Command`]'s argv is inspected directly (that argv is exactly what
    //! a spawn would exec) and [`prompt_from_spec`] is exercised on payloads directly.

    use std::error::Error;

    use aion_core::{ActivityId, ContentType, Payload, RunId, WorkflowId};

    use super::{
        AgentRunSpec, NornHarness, expand_arg, prompt_from_spec, registration_capabilities,
    };

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

    /// The argv the built command would exec, decoded as UTF-8 strings.
    fn argv(harness: &NornHarness, spec: &AgentRunSpec) -> TestResult<Vec<String>> {
        let decoded = harness
            .command(spec)
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
    fn removed_env_vars_are_expressed_as_child_removals() {
        let spec = spec();
        let harness = NornHarness::with_binary("/bin/norn")
            .with_env("OPENAI_API_KEY", "stray-ambient-key")
            .without_env("OPENAI_API_KEY");
        let command = harness.command(&spec);

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
    }

    #[test]
    fn json_string_input_unwraps_to_the_exact_inner_text() -> TestResult {
        // A String-typed activity input arrives as its JSON encoding: surrounding quotes plus
        // \n / \" escapes. The prompt must be the exact multi-line text, not the encoding.
        let encoded = serde_json::to_vec(&serde_json::json!("line one\nsay \"hi\"\nline three"))?;
        let spec = spec_with_input(Payload::new(ContentType::Json, encoded));
        assert_eq!(prompt_from_spec(&spec)?, "line one\nsay \"hi\"\nline three");
        Ok(())
    }

    #[test]
    fn json_object_input_passes_through_as_its_json_text() -> TestResult {
        // A structured payload is a legitimate prompt for some harnesses: a non-string JSON
        // value must reach the agent as its raw JSON text, not be rejected or reshaped.
        let raw = r#"{"task":"build","steps":[1,2]}"#;
        let spec = spec_with_input(Payload::new(ContentType::Json, raw.as_bytes().to_vec()));
        assert_eq!(prompt_from_spec(&spec)?, raw);
        Ok(())
    }

    #[test]
    fn json_tagged_plain_text_input_passes_through_unchanged() -> TestResult {
        // Payload is a dumb carrier that does not validate on construction, so JSON-tagged
        // bytes that are NOT valid JSON keep today's UTF-8 pass-through behaviour. (This is
        // also the behaviour every non-JSON content type keeps; ContentType has only `Json`
        // today, so the non-JSON arm cannot be constructed in a test yet.)
        let spec = spec_with_input(Payload::new(
            ContentType::Json,
            b"plain prompt, not json".to_vec(),
        ));
        assert_eq!(prompt_from_spec(&spec)?, "plain prompt, not json");
        Ok(())
    }

    #[test]
    fn invalid_utf8_input_still_errors() -> TestResult {
        let spec = spec_with_input(Payload::new(ContentType::Json, vec![0xff, 0xfe, 0xfd]));
        let error = prompt_from_spec(&spec)
            .err()
            .ok_or("non-UTF-8 input must error")?;
        assert!(
            error.to_string().contains("not valid UTF-8"),
            "error must name the UTF-8 mismatch, got: {error}"
        );
        Ok(())
    }

    #[test]
    fn the_invocation_preview_is_the_spawned_reality() -> TestResult {
        // `command` is BUILT from `invocation`, so asserting both against one
        // configured harness proves the public preview is what a spawn execs.
        let spec = spec();
        let harness = NornHarness::with_binary("/bin/norn")
            .with_arg("--session-id")
            .with_arg("{workflow_id}-{activity_type}")
            .with_env("NORN_LOG", "debug")
            .without_env("OPENAI_API_KEY");
        let invocation = harness.invocation(&spec);
        assert_eq!(
            invocation,
            super::NornInvocation {
                program: std::path::PathBuf::from("/bin/norn"),
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

        let harness = NornHarness::with_binary("/bin/norn")
            .with_declared_environment(declaration.resolve(&context));
        let invocation = harness.invocation(&spec);
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
        let harness = NornHarness::with_binary("/bin/norn")
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
}
