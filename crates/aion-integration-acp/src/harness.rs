//! [`AcpHarness`] — the [`AgentHarness`] implementation that drives any ACP agent subprocess.
//!
//! [`AgentHarness::start`] spawns the configured command with piped stdin/stdout (stderr is
//! **inherited**, so the agent's human logs go straight to the parent's stderr and never into the
//! structured channel), performs the versioned `initialize` handshake gating on ACP protocol version
//! `1`, creates a session with `session/new`, issues the one long-running `session/prompt` request
//! carrying the spec's input as a `text` content block, and hands that outstanding request's id to an
//! [`AcpSession`] whose reader pump captures its Response as the terminal result.
//!
//! # The launch command is configuration, never inference
//!
//! There is no standard ACP launch flag, and there is no way to derive one:
//!
//! | agent | invocation |
//! |---|---|
//! | Claude Code | `npx @agentclientprotocol/claude-agent-acp` (no args) |
//! | Codex | `npx @agentclientprotocol/codex-acp` (no args) |
//! | Gemini CLI | `npx @google/gemini-cli --acp` |
//! | `OpenCode` | `opencode acp` |
//! | others | `acp serve`, `serve acp`, `--acp=true`, `agent stdio`, … |
//!
//! So the command and its arguments are taken verbatim from the harness configuration and never
//! guessed at. A wrong guess would spawn the agent in its interactive mode, which speaks no JSON-RPC
//! at all and hangs the handshake until the activity times out.
//!
//! # What this adapter deliberately does not do
//!
//! - **It does not authenticate.** ACP's `authenticate` request drives an interactive flow (a browser
//!   login, a terminal prompt); an unattended activity cannot complete one. An agent that answers
//!   `-32000` gets a harness failure naming the authentication methods it advertised, so the operator
//!   can supply credentials through the harness environment instead of the adapter inventing them.
//! - **It offers no filesystem or terminal callbacks.** See [`crate::protocol`] for the ruling.
//!
//! # Where each session is rooted
//!
//! [`AcpHarness`] holds a [`HarnessWorkspace`], not a path, and resolves it once per attempt. In the
//! fixed form every session this harness starts is rooted at one directory, and two agents running
//! concurrently through it share that root with no isolation whatsoever: they can edit the same file
//! and the second writer wins. That is the honest reading of a worker that serves exactly one tree.
//!
//! In the per-run form each attempt takes its root from the job that dispatched it, so isolation is
//! whatever the DISPATCHER arranged — one worktree per lane, one checkout per issue — and this
//! adapter neither invents it nor conceals its absence. What it will not do is guess: a job that
//! does not state a directory is refused terminally rather than started somewhere plausible, because
//! a subdirectory named after the attempt would silently change where every operator's agent runs.
//!
//! [`AgentHarness`]: aion_integrations::AgentHarness
//! [`AgentHarness::start`]: aion_integrations::AgentHarness::start
//!
//! [`AcpHarness`]: crate::harness::AcpHarness
//! [`AcpSession`]: crate::session::AcpSession

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use agent_client_protocol_schema::v1::{
    AuthMethod, ContentBlock, InitializeRequest, InitializeResponse, NewSessionRequest,
    NewSessionResponse, PromptRequest, SessionId, TextContent,
};
use aion_integrations::contract::AgentHarness;
use aion_integrations::jsonrpc::{
    IncomingMessage, JsonRpcConnection, JsonRpcId, JsonRpcNotification, JsonRpcRequest,
};
use aion_integrations::{AgentRunSpec, ChildEnvironment, HarnessError, HarnessWorkspace};
use async_trait::async_trait;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::process::{ChildStdin, ChildStdout, Command};

use crate::permission::PermissionPolicy;
use crate::protocol;
use crate::session::{AcpSession, SessionStart, refuse_peer_request};
use crate::translate::{self, EventIdentity};

/// The neutral intervention primitives this adapter can honour, published for a worker's
/// REGISTRATION-time advertisement: exactly [`InterventionPrimitive::Cancel`].
///
/// # Why the adapter publishes this, and why it is not a guess
///
/// A worker must declare its [`InterventionCapabilities`] when it REGISTERS — the server routes an
/// operator command only for a primitive the worker advertised — and registration happens before any
/// agent process exists, so the per-session handshake cannot supply it. Without a published set every
/// composition root hardcodes its own idea of what an ACP agent supports, and each copy is a place
/// the truth can rot.
///
/// This set is a fact about the ADAPTER's translation ceiling, not a prediction about a binary:
/// [`translate::intervention_to_notification`] can build an ACP frame for exactly `Cancel`
/// (`session/cancel`) and answers every other primitive with
/// [`HarnessError::capability_not_supported`]. So it is the ceiling on what any command routed here
/// could possibly achieve, and the bidirectional test beside it fails if either side moves alone.
///
/// # Why it DELEGATES to the negotiated set instead of restating it
///
/// [`translate::negotiated_capabilities`] is the set a live [`AcpSession`] gates each command on, and
/// for ACP the two sets are the same value by construction — the intervention surface is fixed by the
/// PROTOCOL VERSION, which the handshake has already gated, rather than by anything an individual
/// agent advertises. That is the substantive difference from `aion-integration-norn`, where the
/// spawned binary reports its own set and the two can genuinely differ in both directions. Returning
/// the same function's value rather than repeating the literal is what keeps the equality true: a
/// future protocol version that made the negotiated set version-dependent cannot leave the
/// registration advertisement silently behind.
///
/// # The registration set can still be wrong about a particular agent, and that is honest
///
/// `session/cancel` is baseline in ACP v1: it is not capability-gated, and the spec REQUIRES the
/// agent to answer the still-open `session/prompt` with `stopReason: "cancelled"`. An agent that
/// ignores it is non-conforming, and registration has no way to know — the agent is not running yet,
/// and even a handshake would not tell us, because there is no capability flag for cancellation to
/// advertise. So the advertisement is the protocol's promise, and a non-conforming agent surfaces as
/// a turn that does not stop, on the attempt that asked, rather than as a control the console hid.
/// The converse — an agent that supports something richer than ACP can express — is refused by
/// [`translate::intervention_to_notification`] with a [`HarnessError::CapabilityNotSupported`] naming
/// the primitive, because this adapter has no frame to send for it.
///
/// Neither case is silent, and neither can be resolved at registration time. Advertising the
/// adapter's ceiling is therefore the honest choice: it never hides a control that would work, and a
/// control that will not work says so, by name, on the attempt that asked for it.
///
/// [`AcpSession`]: crate::session::AcpSession
/// [`HarnessError::capability_not_supported`]: aion_integrations::HarnessError::capability_not_supported
/// [`HarnessError::CapabilityNotSupported`]: aion_integrations::HarnessError::CapabilityNotSupported
/// [`InterventionCapabilities`]: aion_core::InterventionCapabilities
/// [`InterventionPrimitive::Cancel`]: aion_core::InterventionPrimitive::Cancel
#[must_use]
pub fn registration_capabilities() -> aion_core::InterventionCapabilities {
    translate::negotiated_capabilities()
}

/// A harness that runs each activity attempt as one ACP agent subprocess.
///
/// Holds the harness-specific settings — the launch command, its arguments, the session working
/// directory, the child environment, the permission policy, and the shutdown grace — so the
/// [`AgentRunSpec`] stays harness-blind: the spec never carries ACP configuration.
#[derive(Clone, Debug)]
pub struct AcpHarness {
    command: PathBuf,
    args: Vec<OsString>,
    declared_environment: Option<ChildEnvironment>,
    env: Vec<(String, String)>,
    env_removed: Vec<String>,
    workspace: HarnessWorkspace,
    additional_directories: Vec<PathBuf>,
    permission_policy: PermissionPolicy,
    exit_grace: Duration,
}

/// What the transport-independent opening of a turn produces.
///
/// Grouped rather than returned as a tuple so the deferred notifications cannot be silently dropped
/// at a call site that only wanted the two ids.
pub(crate) struct OpenedTurn {
    /// The session `session/new` minted.
    pub(crate) session_id: SessionId,
    /// The id of the outstanding `session/prompt` request.
    pub(crate) prompt_id: JsonRpcId,
    /// Notifications read during the handshake, before the transcript pump existed.
    pub(crate) deferred: Vec<JsonRpcNotification>,
}

impl AcpHarness {
    /// A harness that launches `command` with `args`, sessions rooted at `cwd`, answering permission
    /// requests from `permission_policy`, and allowing the agent `exit_grace` to shut down.
    ///
    /// Every argument is required, and none of them has a default, because none of them can be
    /// inferred without guessing:
    ///
    /// - `command` / `args` — there is no standard ACP launch flag (see the module docs). They are
    ///   passed to the child exactly as given.
    /// - `workspace` — `session/new` **requires** an absolute working directory, and it is the root
    ///   the agent reads and writes under. Defaulting it to the worker's own current directory would
    ///   silently scope an agent to whatever directory the worker happened to be started in. Its two
    ///   forms decide whether every session shares one root or each job states its own; see the
    ///   module docs.
    /// - `permission_policy` — `session/request_permission` blocks the agent until answered, and an
    ///   unattended activity has no operator to ask, so the answer must be decided up front. See
    ///   [`crate::permission`] for why there is no default.
    /// - `exit_grace` — how long the agent may take to exit on its own once the turn is over and its
    ///   stdin has closed, before it is killed. How long a given agent needs to flush its logs and
    ///   release what it holds is a property of that agent, so the value is the operator's; a
    ///   "sensible default" here would be a guess about somebody else's process. [`Duration::ZERO`]
    ///   is a legitimate setting and means "close stdin, then kill at once". There is no
    ///   wait-forever setting: a worker must not be hung by an agent that ignores EOF.
    ///
    /// Whatever directory the workspace yields is validated as absolute at [`AgentHarness::start`],
    /// where the failure can be reported as a [`HarnessError`] rather than swallowed by an
    /// infallible constructor.
    #[must_use]
    pub fn new(
        command: impl Into<PathBuf>,
        args: impl IntoIterator<Item = impl Into<OsString>>,
        workspace: HarnessWorkspace,
        permission_policy: PermissionPolicy,
        exit_grace: Duration,
    ) -> Self {
        Self {
            command: command.into(),
            args: args.into_iter().map(Into::into).collect(),
            declared_environment: None,
            env: Vec::new(),
            env_removed: Vec::new(),
            workspace,
            additional_directories: Vec::new(),
            permission_policy,
            exit_grace,
        }
    }

    /// Declares the agent child's WHOLE environment.
    ///
    /// Without this the child inherits the worker process's environment, which is how an agent's
    /// own shell variable once reached a nested `claude-code-acp` and made it refuse every
    /// `session/new`. With it the child's environment is CONSTRUCTED: the spawn clears everything
    /// first and sets exactly the resolved pairs, so a variable the declaration does not name is
    /// absent in the child whatever the parent carries.
    ///
    /// [`Self::with_env`] and [`Self::without_env`] still apply, on top of the declared set rather
    /// than on top of an inherited one — a caller that both declares an environment and sets a
    /// value gets the value it set.
    #[must_use]
    pub fn with_declared_environment(mut self, environment: ChildEnvironment) -> Self {
        self.declared_environment = Some(environment);
        self
    }

    /// Sets an environment variable on every spawned agent process.
    ///
    /// Applied to the CHILD only (never the parent), so a harness can supply an agent's credentials
    /// or model configuration without mutating shared process state.
    #[must_use]
    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }

    /// Removes an environment variable from every spawned agent process.
    ///
    /// Applied to the CHILD only, so a harness can strip ambient configuration an agent must not
    /// inherit — an ambient API key that would otherwise take precedence over the operator's chosen
    /// credentials, say. Removals are applied AFTER the [`Self::with_env`] sets, so removing a key
    /// wins over setting the same key.
    #[must_use]
    pub fn without_env(mut self, key: impl Into<String>) -> Self {
        self.env_removed.push(key.into());
        self
    }

    /// Adds a workspace root beyond `cwd` for every session this harness creates.
    ///
    /// ACP's `additionalDirectories` expands the session's filesystem scope without moving `cwd`,
    /// which stays the base for relative paths. Each path must be absolute; a relative one is
    /// refused at [`AgentHarness::start`] rather than resolved against an unstated base.
    #[must_use]
    pub fn with_additional_directory(mut self, directory: impl Into<PathBuf>) -> Self {
        self.additional_directories.push(directory.into());
        self
    }

    /// Builds the launch command: the configured argv, the configured child-environment sets and
    /// removals, piped stdin/stdout, and inherited stderr.
    ///
    /// The child's working directory is set to the session `cwd` as well as being sent in
    /// `session/new`: an agent that resolves a relative path itself must land in the same place the
    /// protocol says the session is rooted.
    fn command(&self, cwd: &Path) -> Command {
        let mut command = Command::new(&self.command);
        command.args(&self.args).current_dir(cwd);
        // A DECLARED environment is total: everything the parent carries is cleared and the
        // declared pairs are the child's whole environment. Without a declaration the child
        // inherits, which is the pre-declaration behaviour a hand-written worker still gets.
        if let Some(declared) = &self.declared_environment {
            command.env_clear();
            command.envs(declared.pairs().iter().map(|(key, value)| (key, value)));
        }
        command
            .envs(self.env.iter().map(|(key, value)| (key, value)))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            // Inherited, never piped: an agent's human-readable log lines must not be mistaken for
            // protocol frames, and a full unread stderr pipe would deadlock the agent.
            .stderr(Stdio::inherit())
            .kill_on_drop(true);
        // Removals AFTER the sets, so `without_env` wins over `with_env` for the same key.
        for key in &self.env_removed {
            command.env_remove(key);
        }
        command
    }

    /// Spawns the agent for one run.
    fn spawn(&self, cwd: &Path) -> Result<tokio::process::Child, HarnessError> {
        self.command(cwd).spawn().map_err(|source| {
            HarnessError::transport(format!(
                "failed to spawn the ACP agent `{}`: {source} — the command and its arguments are \
                 harness configuration (ACP has no standard launch flag) and must name a real \
                 executable",
                self.command.display()
            ))
        })
    }

    /// Refuses any configured path that is not absolute.
    ///
    /// ACP requires `cwd` and every `additionalDirectories` entry to be absolute. Resolving a
    /// relative path here would silently root the agent at the worker's current directory, which is
    /// not a place any operator chose.
    fn validate_paths(&self, cwd: &Path) -> Result<(), HarnessError> {
        require_absolute("cwd", cwd)?;
        for directory in &self.additional_directories {
            require_absolute("additionalDirectories entry", directory)?;
        }
        Ok(())
    }
}

/// Refuses a relative path, naming which setting it came from.
///
/// The refusal is a [`HarnessError::Configuration`], NOT a protocol fault, and the distinction
/// is load-bearing rather than cosmetic. `cwd` and `additionalDirectories` come from the worker
/// DOCUMENT: the next attempt reads the same document and reaches the same relative path, so a
/// retryable classification would spend the whole attempt budget re-confirming a value that
/// cannot change. `HarnessError::is_deterministic` says so in the type's own words —
/// `Configuration` is "deterministic by definition ... A refusal that presented as a transient
/// transport failure would tell the operator the wrong story AND spend the whole attempt budget
/// confirming it" — while `Protocol` stays retryable because a malformed frame can be a one-off
/// peer flake. Nothing about a relative path in a document is a flake.
fn require_absolute(label: &str, path: &Path) -> Result<(), HarnessError> {
    if path.is_absolute() {
        return Ok(());
    }
    Err(HarnessError::configuration(format!(
        "the ACP {label} must be an absolute path, got `{}` — the protocol requires absolute \
         session paths and this adapter will not resolve one against an unstated base",
        path.display()
    )))
}

/// Takes the child's piped stdout + stdin, or a transport error if either was not piped.
fn take_child_io(
    child: &mut tokio::process::Child,
) -> Result<(ChildStdout, ChildStdin), HarnessError> {
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| HarnessError::transport("the spawned ACP agent has no piped stdout"))?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| HarnessError::transport("the spawned ACP agent has no piped stdin"))?;
    Ok((stdout, stdin))
}

/// Sends one request and reads its typed result.
///
/// The generic call/reply used for the pre-prompt handshake: allocate an id, write the request, read
/// until the id-matched Response, and decode its `result`. Only `session/prompt` is left outstanding
/// past this point, which is what makes the session's terminal-result capture unambiguous.
///
/// `deferred` collects any notification read along the way, for the session's pump to replay.
///
/// # Errors
///
/// [`HarnessError::Protocol`] when the params cannot be encoded, the reply cannot be decoded, or a
/// Response arrives for an id this client never sent; [`HarnessError::Harness`] when the agent
/// answers with a JSON-RPC error; [`HarnessError::Transport`] on I/O failure or a channel that closed
/// mid-handshake.
async fn call<Req, Res, R, W>(
    connection: &JsonRpcConnection<R, W>,
    method: &str,
    params: &Req,
    deferred: &mut Vec<JsonRpcNotification>,
) -> Result<Res, HarnessError>
where
    Req: Serialize,
    Res: DeserializeOwned,
    R: tokio::io::AsyncRead + Unpin + Send,
    W: tokio::io::AsyncWrite + Unpin + Send,
{
    let id = connection.next_request_id();
    let encoded = serde_json::to_value(params).map_err(|source| {
        HarnessError::protocol(format!("`{method}` params are not encodable: {source}"))
    })?;
    let request = JsonRpcRequest::new(id.clone(), method, Some(encoded));
    connection.send_request(&request).await?;
    let result = read_result_for(connection, method, &id, deferred).await?;
    serde_json::from_value(result.clone()).map_err(|source| {
        HarnessError::protocol(format!(
            "the `{method}` result is not the shape ACP specifies ({source}): {result}"
        ))
    })
}

/// Reads inbound frames until the Response for `expected_id` arrives, returning its `result`.
///
/// A conforming agent sends nothing else during the handshake, but the read is defensive on every
/// arm because the alternative to handling a frame is dropping it:
///
/// - a **notification** is logged at `warn` (nothing has happened yet that an agent should be
///   reporting) and **pushed onto `deferred`**, for the session's pump to replay onto the transcript
///   as its first events. It is not recorded here because there is no transcript yet — and it is not
///   dropped either, which is what the handshake used to do and what the crate's "nothing is
///   dropped" rule forbids. This closes the window: the only frames read before the pump exists are
///   read by this function, and every one of them is now either answered or carried forward,
/// - a peer **request** is refused with `-32601` and the read continues, because leaving it
///   unanswered would block the agent before the run started,
/// - a Response for another id is a peer bug and fails the handshake rather than being taken as this
///   one's answer.
async fn read_result_for<R, W>(
    connection: &JsonRpcConnection<R, W>,
    method: &str,
    expected_id: &JsonRpcId,
    deferred: &mut Vec<JsonRpcNotification>,
) -> Result<Value, HarnessError>
where
    R: tokio::io::AsyncRead + Unpin + Send,
    W: tokio::io::AsyncWrite + Unpin + Send,
{
    loop {
        let message = connection.recv().await?.ok_or_else(|| {
            HarnessError::transport(format!(
                "the ACP agent closed the channel while `{method}` was outstanding — it exited \
                 before answering (its own diagnostics are on this process's stderr)"
            ))
        })?;
        match message {
            IncomingMessage::Response(response) if &response.id == expected_id => {
                if let Some(error) = response.error {
                    return Err(agent_error(method, &error));
                }
                return response.result.ok_or_else(|| {
                    HarnessError::protocol(format!(
                        "the `{method}` response carried neither result nor error"
                    ))
                });
            }
            IncomingMessage::Response(_) => {
                return Err(HarnessError::protocol(format!(
                    "the ACP agent answered an id this client never requested while `{method}` was \
                     outstanding"
                )));
            }
            IncomingMessage::Notification(note) => {
                tracing::warn!(
                    method = %note.method,
                    during = %method,
                    "acp adapter: unexpected notification during the handshake; deferred to the \
                     transcript"
                );
                deferred.push(note);
            }
            IncomingMessage::Request(request) => {
                refuse_peer_request(connection, request).await?;
            }
        }
    }
}

/// Turns a handshake JSON-RPC error into the harness error it is.
///
/// `-32000` is ACP's "authentication required": it is the agent asking the client to call
/// `authenticate`, which an unattended activity cannot do, so the message says so plainly instead of
/// reporting an opaque code.
fn agent_error(method: &str, error: &aion_integrations::jsonrpc::JsonRpcError) -> HarnessError {
    if error.code == protocol::auth_required_code() {
        return HarnessError::harness(format!(
            "the ACP agent requires authentication before `{method}` (code {}): {} — an unattended \
             activity cannot complete an interactive `authenticate` flow; supply the agent's \
             credentials through the harness environment",
            error.code, error.message
        ));
    }
    HarnessError::harness(format!(
        "`{method}` failed (code {}): {}",
        error.code, error.message
    ))
}

/// Performs the `initialize` handshake and gates on the negotiated protocol version.
///
/// # Errors
///
/// [`HarnessError::Protocol`] when the agent answers with a version this adapter cannot speak, plus
/// every error [`call`] can return.
async fn initialize<R, W>(
    connection: &JsonRpcConnection<R, W>,
    deferred: &mut Vec<JsonRpcNotification>,
) -> Result<InitializeResponse, HarnessError>
where
    R: tokio::io::AsyncRead + Unpin + Send,
    W: tokio::io::AsyncWrite + Unpin + Send,
{
    let request = InitializeRequest::new(protocol::PROTOCOL_VERSION)
        .client_capabilities(protocol::client_capabilities())
        .client_info(protocol::client_info());
    let response: InitializeResponse =
        call(connection, protocol::METHOD_INITIALIZE, &request, deferred).await?;
    translate::gate_protocol_version(response.protocol_version)?;
    tracing::debug!(
        version = response.protocol_version.as_u16(),
        agent = ?response.agent_info,
        capabilities = ?response.agent_capabilities,
        auth_methods = ?auth_method_ids(&response.auth_methods),
        "acp adapter: initialize negotiated"
    );
    Ok(response)
}

/// The ids of the authentication methods an agent advertised, for diagnostics.
///
/// Recorded so an operator whose run fails with `-32000` can see which methods were on offer without
/// re-running the agent by hand.
fn auth_method_ids(methods: &[AuthMethod]) -> Vec<String> {
    methods
        .iter()
        .map(|method| method.id().to_string())
        .collect()
}

impl AcpHarness {
    /// Runs the whole transport-independent opening of a turn over an already-connected channel:
    /// the `initialize` handshake, `session/new`, then the outstanding `session/prompt`.
    ///
    /// Split out from [`AgentHarness::start`] so the protocol conversation is exercised over any
    /// [`tokio::io::AsyncRead`] / [`tokio::io::AsyncWrite`] pair, not only a spawned child's stdio.
    ///
    /// # Errors
    ///
    /// Every error [`initialize`] and [`Self::open_turn`] can return.
    pub(crate) async fn connect<R, W>(
        &self,
        connection: &JsonRpcConnection<R, W>,
        cwd: PathBuf,
        prompt: String,
    ) -> Result<OpenedTurn, HarnessError>
    where
        R: tokio::io::AsyncRead + Unpin + Send,
        W: tokio::io::AsyncWrite + Unpin + Send,
    {
        let mut deferred = Vec::new();
        initialize(connection, &mut deferred).await?;
        self.open_turn(connection, cwd, prompt, deferred).await
    }

    /// Creates the session and issues the prompt, returning the session id, the outstanding
    /// `session/prompt` request id, and the notifications read before the pump could exist.
    async fn open_turn<R, W>(
        &self,
        connection: &JsonRpcConnection<R, W>,
        cwd: PathBuf,
        prompt: String,
        mut deferred: Vec<JsonRpcNotification>,
    ) -> Result<OpenedTurn, HarnessError>
    where
        R: tokio::io::AsyncRead + Unpin + Send,
        W: tokio::io::AsyncWrite + Unpin + Send,
    {
        // `mcpServers` is REQUIRED by `session/new`, so it must appear on the wire. This call sets
        // the value; it is the SCHEMA that guarantees the key's presence — the field carries no
        // `skip_serializing_if`, so an empty list serialises as `"mcpServers": []` whether or not
        // this call is made. It is written explicitly anyway, so the empty list is a stated decision
        // rather than a reliance on the schema's default, and `wire_tests` asserts the key is there.
        let new_session = NewSessionRequest::new(cwd)
            .mcp_servers(Vec::new())
            .additional_directories(self.additional_directories.clone());
        let session: NewSessionResponse = call(
            connection,
            protocol::METHOD_SESSION_NEW,
            &new_session,
            &mut deferred,
        )
        .await?;
        tracing::debug!(
            session = %session.session_id,
            modes = ?session.modes,
            "acp adapter: session created"
        );

        // `text` is one of the two content-block types every ACP agent MUST support (the other being
        // `resource_link`); the rest are gated on `promptCapabilities`, so a text block needs no
        // capability check.
        let prompt_request = PromptRequest::new(
            session.session_id.clone(),
            vec![ContentBlock::Text(TextContent::new(prompt))],
        );
        let params = serde_json::to_value(&prompt_request).map_err(|source| {
            HarnessError::protocol(format!("session/prompt params are not encodable: {source}"))
        })?;
        // This id is the ONLY id the terminal result is captured under: the request stays open for
        // the whole turn and its Response carries the stop reason.
        let prompt_id = connection.next_request_id();
        let frame = JsonRpcRequest::new(
            prompt_id.clone(),
            protocol::METHOD_SESSION_PROMPT,
            Some(params),
        );
        connection.send_request(&frame).await?;
        Ok(OpenedTurn {
            session_id: session.session_id,
            prompt_id,
            deferred,
        })
    }
}

#[async_trait]
impl AgentHarness for AcpHarness {
    type Session = AcpSession<ChildStdout, ChildStdin>;

    async fn start(&self, spec: AgentRunSpec) -> Result<Self::Session, HarnessError> {
        // Resolved ONCE and threaded through, because a per-run workspace has a different answer
        // per attempt: a second resolution could disagree with the first, and the two places it
        // reaches — the child's `current_dir` and `session/new`'s `cwd` — are exactly the pair that
        // must not disagree about where the agent is.
        let cwd = self.workspace.for_attempt(&spec.input)?;
        self.validate_paths(&cwd)?;
        // Named BEFORE the spawn: a declaration that omits the variable this program needs to be
        // found would otherwise surface as the kernel's `No such file or directory` against a
        // command the operator can see is spelled correctly.
        if let Some(declared) = &self.declared_environment {
            declared.require_for_program(&self.command)?;
        }
        // Read through the same workspace that decided `cwd` above, so the field the agent is
        // asked with and the field it stands in are decided from one input by one rule. An
        // adapter that read the prompt on its own would be the second place that rule lives,
        // and the first place it drifts.
        let prompt = self.workspace.prompt_for_attempt(&spec.input)?;
        let mut child = self.spawn(&cwd)?;
        let (stdout, stdin) = take_child_io(&mut child)?;
        let connection = Arc::new(JsonRpcConnection::new(stdout, stdin));

        // The handshake, the session, and the prompt whose id the session captures the result under.
        let opened = self.connect(connection.as_ref(), cwd, prompt).await?;

        let identity = EventIdentity {
            workflow_id: spec.workflow_id,
            run_id: spec.run_id,
            activity_id: spec.activity_id,
            attempt: spec.attempt,
        };
        Ok(AcpSession::start(SessionStart {
            connection,
            session_id: opened.session_id,
            prompt_id: opened.prompt_id,
            policy: self.permission_policy,
            identity,
            deferred: opened.deferred,
            child: Some(child),
            exit_grace: self.exit_grace,
        }))
    }
}

#[cfg(test)]
mod tests {
    //! Fast unit tests of the configuration, the path validation, and the prompt decoding — no
    //! process is spawned; the built [`Command`]'s argv is inspected directly (that argv is exactly
    //! what a spawn would exec).

    use std::error::Error;
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    use aion_core::{ActivityId, Payload, RunId, WorkflowId};
    use aion_integrations::{HarnessError, HarnessWorkspace};

    use super::{AcpHarness, AgentRunSpec, registration_capabilities};
    use crate::permission::PermissionPolicy;

    type TestResult<T = ()> = Result<T, Box<dyn Error>>;

    /// The PUBLISHED registration set must be exactly the set this adapter can translate — no wider,
    /// no narrower. A wider set advertises a control the translation would refuse; a narrower one
    /// hides a control that works.
    ///
    /// Driven over EVERY primitive the neutral enum has, so adding a variant to
    /// [`crate::translate::intervention_to_notification`] without publishing it (or the reverse)
    /// fails here rather than at an operator's desk. The session sends exactly what that function
    /// returns, so "translatable" here is the same thing as "deliverable on a live turn".
    #[test]
    fn the_published_set_is_exactly_what_the_adapter_can_translate() {
        use agent_client_protocol_schema::v1::SessionId;
        use aion_core::{ApprovalDecision, InjectPriority, InterventionKind};

        use crate::translate::intervention_to_notification;

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
        let session = SessionId::new("sess-1");
        let published = registration_capabilities();
        for kind in &every_primitive {
            let primitive = kind.primitive();
            assert_eq!(
                published.supports_primitive(primitive),
                intervention_to_notification(&session, kind).is_ok(),
                "the published registration set disagrees with the translation for {primitive:?}"
            );
        }
    }

    /// The registration-time set and the per-session set are the SAME value for ACP, because the
    /// intervention surface is fixed by the protocol version rather than by an individual agent's
    /// advertisement. Pinned so that a change to either one has to be a deliberate change to both.
    #[test]
    fn the_registration_set_is_the_negotiated_set() {
        assert_eq!(
            registration_capabilities(),
            crate::translate::negotiated_capabilities()
        );
    }

    /// The shutdown grace these configuration tests are built with. Nothing here spawns a process,
    /// so the value only has to be a real one an operator could choose.
    const GRACE: Duration = Duration::from_secs(2);

    fn spec_with_input(input: Payload) -> AgentRunSpec {
        AgentRunSpec::new(
            WorkflowId::new_v4(),
            RunId::new_v4(),
            ActivityId::from_sequence_position(1),
            1,
            "agent",
            input,
        )
    }

    /// The one tree these configuration tests root a session in.
    const WORKSPACE: &str = "/srv/workspace";

    /// A workspace that answers with one directory for every attempt.
    fn fixed(directory: &str) -> HarnessWorkspace {
        HarnessWorkspace::Fixed(PathBuf::from(directory))
    }

    /// A harness configured the way a Gemini CLI launch needs (`--acp` as an argument).
    fn harness() -> AcpHarness {
        AcpHarness::new(
            "/usr/local/bin/gemini",
            ["--acp"],
            fixed(WORKSPACE),
            PermissionPolicy::Deny,
            GRACE,
        )
    }

    /// The argv the built command would exec, decoded as UTF-8 strings.
    fn argv(harness: &AcpHarness) -> TestResult<Vec<String>> {
        let decoded = harness
            .command(Path::new(WORKSPACE))
            .as_std()
            .get_args()
            .map(|arg| arg.to_str().map(str::to_owned).ok_or("argv is not UTF-8"))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(decoded)
    }

    #[test]
    fn the_configured_command_and_args_are_passed_through_verbatim() -> TestResult {
        // ACP has no standard launch flag, so the argv must be exactly what the operator configured
        // — nothing added, nothing reordered.
        assert_eq!(argv(&harness())?, vec!["--acp".to_owned()]);

        let no_args = AcpHarness::new(
            "opencode",
            Vec::<String>::new(),
            fixed(WORKSPACE),
            PermissionPolicy::AllowOnce,
            GRACE,
        );
        assert!(argv(&no_args)?.is_empty(), "no args configured, none sent");

        let two_args = AcpHarness::new(
            "npx",
            ["@agentclientprotocol/claude-agent-acp", "--verbose"],
            fixed(WORKSPACE),
            PermissionPolicy::AllowOnce,
            GRACE,
        );
        assert_eq!(
            argv(&two_args)?,
            vec![
                "@agentclientprotocol/claude-agent-acp".to_owned(),
                "--verbose".to_owned()
            ]
        );
        Ok(())
    }

    #[test]
    fn the_child_runs_in_the_session_cwd() -> TestResult {
        let command = harness().command(Path::new(WORKSPACE));
        let directory = command
            .as_std()
            .get_current_dir()
            .ok_or("the child's working directory is set to the session cwd")?;
        assert_eq!(directory, Path::new(WORKSPACE));
        Ok(())
    }

    /// The whole point of the per-run form: two jobs served by ONE harness stand in two
    /// different trees, and the tree each stands in is the one its own input named. The
    /// control is the second job — a resolver that had ignored the input entirely would
    /// answer the same directory twice and fail here.
    #[test]
    fn each_job_roots_its_session_in_the_tree_that_job_named() -> TestResult {
        let harness = AcpHarness::new(
            "/usr/local/bin/gemini",
            ["--acp"],
            HarnessWorkspace::PerRun("repo".to_owned()),
            PermissionPolicy::Deny,
            GRACE,
        );
        let mut seen = Vec::new();
        for tree in ["/srv/lane-3", "/srv/lane-9"] {
            let spec = spec_with_input(Payload::from_json(&serde_json::json!({ "repo": tree }))?);
            let cwd = harness.workspace.for_attempt(&spec.input)?;
            let command = harness.command(&cwd);
            let directory = command
                .as_std()
                .get_current_dir()
                .ok_or("the child's working directory is set from the job")?
                .to_path_buf();
            seen.push(directory);
        }
        assert_eq!(
            seen,
            vec![PathBuf::from("/srv/lane-3"), PathBuf::from("/srv/lane-9")]
        );
        Ok(())
    }

    /// A job that does not name its tree is refused TERMINALLY rather than started in
    /// whichever directory this worker process happens to occupy — the hazard the setting
    /// exists to close, checked here at the adapter that would have done the spawning.
    #[test]
    fn a_job_that_names_no_tree_is_refused_before_any_spawn() -> TestResult {
        let harness = AcpHarness::new(
            "/usr/local/bin/gemini",
            ["--acp"],
            HarnessWorkspace::PerRun("repo".to_owned()),
            PermissionPolicy::Deny,
            GRACE,
        );
        let spec = spec_with_input(Payload::from_json(
            &serde_json::json!({ "branch": "main" }),
        )?);
        let error = harness
            .workspace
            .for_attempt(&spec.input)
            .err()
            .ok_or("a job with no directory cannot launch an agent")?;
        assert!(
            error.is_deterministic(),
            "the same input meets the same wall on every attempt: {error:?}"
        );
        Ok(())
    }

    #[test]
    fn removed_env_vars_are_expressed_as_child_removals() {
        let harness = harness()
            .with_env("ANTHROPIC_API_KEY", "stray-ambient-key")
            .without_env("ANTHROPIC_API_KEY");
        let command = harness.command(Path::new(WORKSPACE));

        // On std's Command an env REMOVAL is a `(key, None)` entry: the child will not inherit the
        // variable even when the parent carries it, and the removal wins over a set of the same key.
        let removal_expressed = command
            .as_std()
            .get_envs()
            .any(|(key, value)| key == "ANTHROPIC_API_KEY" && value.is_none());
        assert!(
            removal_expressed,
            "without_env must express a child-side env removal on the spawned command"
        );
    }

    #[test]
    fn a_relative_cwd_is_refused_as_a_terminal_configuration_error() -> TestResult {
        let harness = AcpHarness::new(
            "agent",
            ["acp"],
            fixed("workspace"),
            PermissionPolicy::Deny,
            GRACE,
        );
        let error = harness
            .validate_paths(Path::new("workspace"))
            .err()
            .ok_or("a relative cwd must be refused")?;
        // BOTH halves are the pin. The variant alone would pass on a future edit that kept the
        // name and moved the retry classification; `is_deterministic` is the property that
        // actually decides whether the worker retries, and a relative path in a document is
        // identically relative on every attempt.
        assert!(
            matches!(error, HarnessError::Configuration { .. }),
            "a document-sourced path refusal is a configuration refusal, not a protocol fault: \
             {error:?}"
        );
        assert!(
            error.is_deterministic(),
            "retrying a relative cwd re-reads the same document and hits the same wall, so the \
             refusal must be terminal rather than spend the whole attempt budget: {error:?}"
        );
        let message = error.to_string();
        assert!(
            message.contains("cwd") && message.contains("absolute"),
            "the refusal names the setting and the requirement: {message}"
        );
        Ok(())
    }

    #[test]
    fn a_relative_additional_directory_is_refused() -> TestResult {
        let harness = AcpHarness::new(
            "agent",
            ["acp"],
            fixed("/srv"),
            PermissionPolicy::Deny,
            GRACE,
        )
        .with_additional_directory("shared");
        let error = harness
            .validate_paths(Path::new("/srv"))
            .err()
            .ok_or("a relative additional directory must be refused")?;
        assert!(
            error.to_string().contains("additionalDirectories"),
            "the refusal names the setting: {error}"
        );
        assert!(
            error.is_deterministic(),
            "the second document-sourced path travels the same classification as `cwd`; without \
             this arm the pin above could hold while this one silently stayed retryable: {error:?}"
        );
        Ok(())
    }

    #[test]
    fn absolute_paths_pass_validation() -> Result<(), HarnessError> {
        harness()
            .with_additional_directory("/srv/shared")
            .validate_paths(Path::new(WORKSPACE))
    }

    /// A per-run directory travels the SAME absolute-path net as a written one. Without this
    /// the refusal above could hold for documents while a job-supplied relative path walked
    /// straight into a spawn.
    #[test]
    fn a_relative_directory_from_the_job_is_refused_the_same_way() -> TestResult {
        let harness = AcpHarness::new(
            "agent",
            ["acp"],
            HarnessWorkspace::PerRun("repo".to_owned()),
            PermissionPolicy::Deny,
            GRACE,
        );
        let spec = spec_with_input(Payload::from_json(
            &serde_json::json!({ "repo": "lane-3" }),
        )?);
        let cwd = harness.workspace.for_attempt(&spec.input)?;
        let error = harness
            .validate_paths(&cwd)
            .err()
            .ok_or("a relative directory is relative wherever it came from")?;
        assert!(
            error.is_deterministic(),
            "a job-supplied relative path is identically relative on every attempt: {error:?}"
        );
        Ok(())
    }

    /// The adapter reads the prompt through its workspace — the same value `start` hands to
    /// `session/prompt` one line after it resolves the directory. What the reader itself does
    /// with each shape of input is proven in `aion_integrations::HarnessWorkspace`; what this
    /// pins is that this adapter asks it rather than decoding the payload on its own, which is
    /// how the two came to disagree and the agent came to be sent `{"prompt":"…"}` as its
    /// instructions.
    #[test]
    fn the_prompt_is_read_through_the_same_workspace_that_places_the_agent() -> TestResult {
        let harness = AcpHarness::new(
            "agent",
            ["acp"],
            HarnessWorkspace::PerRun("repo".to_owned()),
            PermissionPolicy::Deny,
            GRACE,
        );
        let spec = spec_with_input(Payload::from_json(&serde_json::json!({
            "repo": WORKSPACE,
            "prompt": "review the diff",
        }))?);
        assert_eq!(
            harness.workspace.for_attempt(&spec.input)?,
            PathBuf::from(WORKSPACE)
        );
        assert_eq!(
            harness.workspace.prompt_for_attempt(&spec.input)?,
            "review the diff"
        );
        Ok(())
    }
}
