//! Protocol tests driving a **real** [`JsonRpcConnection`] against a scripted fake ACP agent over
//! [`tokio::io::duplex`] — no process is spawned and no framing is stubbed.
//!
//! Every frame in this module is written as **literal ACP JSON**, not built from the schema's Rust
//! types. That is the point: constructing a `SessionUpdate` and asserting the adapter reads it back
//! would only prove that one crate's `Serialize` agrees with its own `Deserialize`. Writing the wire
//! form by hand tests the adapter against the protocol as an agent actually speaks it — the
//! `sessionUpdate` discriminator with its payload fields flattened beside it, `protocolVersion` as
//! the integer `1`, the `outcome` discriminator on a permission response.
//!
//! The fake agent runs on the other end of the same duplex the adapter uses, so the whole path is
//! exercised: newline framing, the `Option<id>` request/notification/response classification, the
//! pump's demultiplexing, the terminal-result capture, and every answer this client owes its peer.
//!
//! # The fake agent does NOT exit
//!
//! Every test here drains the transcript to `None` and only then takes the result, which is exactly
//! what the platform's agent driver does (`aion-worker`'s `spawn_agent`: "End of the event stream ==
//! end of run. Take the result next."). Crucially, none of them drops the agent end first: a real ACP
//! agent is a persistent stdio server that answers `session/prompt` and keeps running, so a test that
//! forces EOF — or that breaks out of the loop at the `Stop` event — proves nothing about whether a
//! real turn can ever finish. See [`the_driver_contract_completes_against_an_agent_that_stays_alive`].
//!
//! [`JsonRpcConnection`]: aion_integrations::jsonrpc::JsonRpcConnection
//! [`the_driver_contract_completes_against_an_agent_that_stays_alive`]: crate::wire_tests::the_driver_contract_completes_against_an_agent_that_stays_alive

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use aion_core::{
    ActivityEvent, ActivityEventKind, ActivityId, InterventionCommand, InterventionKind,
    InterventionPrimitive, MessageRole, ProgressDetail, RunId, StopKind, WorkflowId,
};
use aion_integrations::contract::AgentSession;
use aion_integrations::jsonrpc::{
    IncomingMessage, JsonRpcConnection, JsonRpcId, JsonRpcNotification, JsonRpcRequest,
    JsonRpcResponse,
};
use aion_integrations::{ContentType, HarnessError, HarnessWorkspace};
use chrono::Utc;
use futures::StreamExt;
use serde_json::{Value, json};
use tokio::io::{
    AsyncRead, AsyncWrite, AsyncWriteExt, DuplexStream, ReadHalf, Sink, WriteHalf, duplex, split,
};
use uuid::Uuid;

use crate::harness::AcpHarness;
use crate::permission::PermissionPolicy;
use crate::protocol;
use crate::session::{AcpSession, SessionStart};
use crate::translate::EventIdentity;

/// The duplex capacity. Large enough that no test's script can fill the pipe and deadlock on a
/// write while the other end is not reading — except where a test deliberately chooses a small one.
const DUPLEX_CAPACITY: usize = 1 << 16;

/// The bound on any test that must complete rather than hang.
///
/// Present so a regression in the end-of-run signalling or in the off-loop peer answering fails the
/// suite instead of wedging it. Nothing here waits on a real process, so a passing test never gets
/// anywhere near it.
const TEST_TIMEOUT: Duration = Duration::from_secs(10);

/// The shutdown grace the session under test is built with.
///
/// These sessions carry no child process, so the value is never used for a wait; it is stated
/// explicitly because the harness has no default for it.
const EXIT_GRACE: Duration = Duration::from_secs(2);

/// The session id the fake agent mints, and the one every frame in these tests is scoped to.
const SESSION: &str = "sess-under-test";

/// The tree every session in these tests is rooted in. Nothing is spawned, so it only has to
/// be an absolute path a real document could name.
const WORKSPACE: &str = "/srv/workspace";

/// The connection type on both ends of the in-memory duplex.
type Wire = JsonRpcConnection<ReadHalf<DuplexStream>, WriteHalf<DuplexStream>>;
/// The session under test, driven over that duplex.
type Session = AcpSession<ReadHalf<DuplexStream>, WriteHalf<DuplexStream>>;
/// Test failures are reported as strings so no assertion needs `unwrap` or `expect`.
type TestResult<T = ()> = Result<T, String>;

/// Renders any error as the string a test failure carries.
fn fail(context: &str, error: &impl std::fmt::Display) -> String {
    format!("{context}: {error}")
}

fn identity() -> EventIdentity {
    EventIdentity {
        workflow_id: WorkflowId::new(Uuid::nil()),
        run_id: aion_core::RunId::new(Uuid::from_u128(0x11)),
        activity_id: ActivityId::from_sequence_position(1),
        attempt: 1,
    }
}

/// A harness whose command is never spawned by these tests — they drive [`AcpHarness::connect`]
/// over the duplex instead, so only the protocol conversation is under test.
fn harness(policy: PermissionPolicy) -> AcpHarness {
    AcpHarness::new(
        "/nonexistent/acp-agent",
        ["acp"],
        HarnessWorkspace::Fixed(PathBuf::from(WORKSPACE)),
        policy,
        EXIT_GRACE,
    )
}

/// Runs `future` under [`TEST_TIMEOUT`], turning a hang into a named failure.
async fn within<T>(what: &str, future: impl Future<Output = T>) -> TestResult<T> {
    tokio::time::timeout(TEST_TIMEOUT, future)
        .await
        .map_err(|_elapsed| format!("{what} did not finish within {TEST_TIMEOUT:?} — it hung"))
}

/// A live pairing of the adapter and the fake agent.
struct Wired {
    /// The session under test.
    session: Session,
    /// The fake agent's end of the duplex.
    agent: Arc<Wire>,
    /// The id of the outstanding `session/prompt` request — the only id a terminal result can
    /// arrive under.
    prompt_id: JsonRpcId,
}

/// The fake agent's side of the opening conversation.
///
/// Answers `initialize` with `protocol_version`, answers `session/new` with [`SESSION`], and
/// CONSUMES the `session/prompt` request (without answering it, because the turn is what the test is
/// about) so a later `recv` on the agent end sees only frames the test itself provoked.
async fn agent_opens(agent: &Wire, protocol_version: u16) -> Result<(), HarnessError> {
    let initialize = expect_request(agent, protocol::METHOD_INITIALIZE).await?;
    agent
        .send_response(&JsonRpcResponse::success(
            initialize.id,
            json!({
                "protocolVersion": protocol_version,
                "agentCapabilities": { "loadSession": false },
                "authMethods": [],
                "agentInfo": { "name": "fake-acp-agent", "version": "0.0.0" }
            }),
        ))
        .await?;
    // A refused handshake stops here: the client disconnects rather than creating a session.
    if u32::from(protocol_version) != u32::from(protocol::PROTOCOL_VERSION.as_u16()) {
        return Ok(());
    }
    let new_session = expect_request(agent, protocol::METHOD_SESSION_NEW).await?;
    verify_new_session_params(new_session.params.as_ref())?;
    agent
        .send_response(&JsonRpcResponse::success(
            new_session.id,
            json!({ "sessionId": SESSION }),
        ))
        .await?;
    let prompt = expect_request(agent, protocol::METHOD_SESSION_PROMPT).await?;
    verify_prompt_params(prompt.params.as_ref())?;
    Ok(())
}

/// Asserts the wire form of `session/new`: an absolute `cwd`, and `mcpServers` PRESENT.
///
/// `mcpServers` is a required member: an agent is entitled to reject a `session/new` that omits it,
/// so an empty list must appear on the wire rather than being skipped.
fn verify_new_session_params(params: Option<&Value>) -> Result<(), HarnessError> {
    let params = params.ok_or_else(|| HarnessError::protocol("session/new carried no params"))?;
    if params["cwd"] != json!("/srv/workspace") {
        return Err(HarnessError::protocol(format!(
            "session/new must send the configured absolute cwd, got {}",
            params["cwd"]
        )));
    }
    if params.get("mcpServers") != Some(&json!([])) {
        return Err(HarnessError::protocol(format!(
            "session/new must always send `mcpServers`, got {:?}",
            params.get("mcpServers")
        )));
    }
    Ok(())
}

/// Asserts the wire form of `session/prompt`: the session id and one `text` content block.
fn verify_prompt_params(params: Option<&Value>) -> Result<(), HarnessError> {
    let params =
        params.ok_or_else(|| HarnessError::protocol("session/prompt carried no params"))?;
    if params["sessionId"] != json!(SESSION) {
        return Err(HarnessError::protocol(format!(
            "the prompt must be scoped to the minted session, got {}",
            params["sessionId"]
        )));
    }
    if params["prompt"][0]["type"] != json!("text") {
        return Err(HarnessError::protocol(format!(
            "the prompt must be a `text` content block, got {}",
            params["prompt"][0]
        )));
    }
    Ok(())
}

/// Reads the next frame on `wire` and requires it to be a request for `method`.
///
/// Deliberately strict: the opening conversation is fully ordered, so an unexpected frame is a real
/// divergence and must fail the test rather than be skipped over.
async fn expect_request<R, W>(
    wire: &JsonRpcConnection<R, W>,
    method: &str,
) -> Result<JsonRpcRequest, HarnessError>
where
    R: AsyncRead + Unpin + Send,
    W: AsyncWrite + Unpin + Send,
{
    let message = wire.recv().await?.ok_or_else(|| {
        HarnessError::transport(format!("the channel closed while awaiting `{method}`"))
    })?;
    match message {
        IncomingMessage::Request(request) if request.method == method => Ok(request),
        IncomingMessage::Request(other) => Err(HarnessError::protocol(format!(
            "expected `{method}`, got the request `{}`",
            other.method
        ))),
        IncomingMessage::Notification(note) => Err(HarnessError::protocol(format!(
            "expected `{method}`, got the notification `{}`",
            note.method
        ))),
        IncomingMessage::Response(_) => Err(HarnessError::protocol(format!(
            "expected `{method}`, got a response"
        ))),
    }
}

/// Opens a turn: runs the adapter's handshake against the fake agent and starts the session.
async fn wire_up(policy: PermissionPolicy) -> TestResult<Wired> {
    wire_up_with_capacity(policy, DUPLEX_CAPACITY).await
}

/// [`wire_up`] over a duplex of the given capacity, so a test can deliberately make the agent's
/// inbound pipe small enough to fill.
async fn wire_up_with_capacity(policy: PermissionPolicy, capacity: usize) -> TestResult<Wired> {
    let (client_io, agent_io) = duplex(capacity);
    let (client_read, client_write) = split(client_io);
    let (agent_read, agent_write) = split(agent_io);
    let connection = Arc::new(JsonRpcConnection::new(client_read, client_write));
    let agent = Arc::new(JsonRpcConnection::new(agent_read, agent_write));

    let script = {
        let agent = Arc::clone(&agent);
        tokio::spawn(async move {
            agent_opens(agent.as_ref(), protocol::PROTOCOL_VERSION.as_u16()).await
        })
    };
    let opened = harness(policy)
        .connect(
            connection.as_ref(),
            PathBuf::from(WORKSPACE),
            "do the thing".to_owned(),
            None,
        )
        .await
        .map_err(|error| fail("the handshake must succeed", &error))?;
    script
        .await
        .map_err(|error| fail("the agent script task must not panic", &error))?
        .map_err(|error| fail("the agent script must complete", &error))?;

    let prompt_id = opened.prompt_id.clone();
    let session = AcpSession::start(SessionStart {
        connection,
        session_id: opened.session_id,
        prompt_id: opened.prompt_id,
        policy,
        identity: identity(),
        deferred: opened.deferred,
        child: None,
        exit_grace: EXIT_GRACE,
    });
    Ok(Wired {
        session,
        agent,
        prompt_id,
    })
}

/// A `session/update` notification wrapping one raw update payload.
fn session_update(update: &Value) -> JsonRpcNotification {
    JsonRpcNotification::new(
        protocol::METHOD_SESSION_UPDATE,
        Some(json!({ "sessionId": SESSION, "update": update })),
    )
}

/// A `session/prompt` success response carrying `stop_reason`.
fn prompt_response(prompt_id: JsonRpcId, stop_reason: &str) -> JsonRpcResponse {
    JsonRpcResponse::success(prompt_id, json!({ "stopReason": stop_reason }))
}

/// Runs one raw `session/update` through a live session, ends the turn, and returns every event.
///
/// The agent is **kept alive** for the whole collection — its write half is never dropped, so the
/// client's reader never reaches end-of-stream. The events stream therefore terminates only because
/// the adapter closed it at the terminal Response, which is the property the platform's driver
/// depends on. The collection is complete, not a sample: it drains to `None`.
async fn events_for(update: &Value) -> TestResult<Vec<ActivityEvent>> {
    let Wired {
        mut session,
        agent,
        prompt_id,
    } = wire_up(PermissionPolicy::Deny).await?;
    agent
        .send_notification(&session_update(update))
        .await
        .map_err(|error| fail("the update must be writable", &error))?;
    agent
        .send_response(&prompt_response(prompt_id, "end_turn"))
        .await
        .map_err(|error| fail("the terminal response must be writable", &error))?;
    let events = session.events();
    let collected = within(
        "draining the transcript to end-of-stream",
        events.collect::<Vec<_>>(),
    )
    .await?;
    drop(session);
    drop(agent);
    Ok(collected)
}

/// Whether any collected event is a `Raw` with exactly `source`.
fn has_raw(events: &[ActivityEvent], source: &str) -> bool {
    events.iter().any(
        |event| matches!(&event.kind, ActivityEventKind::Raw { source: got, .. } if got == source),
    )
}

/// Ends the turn with `stop_reason` and requires the terminal outcome to match: `end_turn` must yield
/// a payload, anything else must yield a failure.
///
/// Used by the tests whose subject is something OTHER than the terminal result, so that closing the
/// run still asserts rather than discarding what came back.
async fn close_turn(
    session: Session,
    agent: &Wire,
    prompt_id: JsonRpcId,
    stop_reason: &str,
) -> TestResult {
    agent
        .send_response(&prompt_response(prompt_id, stop_reason))
        .await
        .map_err(|error| fail("the terminal response must be writable", &error))?;
    let outcome = session.wait_result().await;
    match (stop_reason, outcome) {
        ("end_turn", Ok(payload)) => {
            assert_eq!(payload.content_type(), &ContentType::Json);
            Ok(())
        }
        ("end_turn", Err(error)) => Err(fail("end_turn is a completed run", &error)),
        (_, Err(_)) => Ok(()),
        (reason, Ok(payload)) => Err(format!(
            "`{reason}` must not be reported as a completed result, got {payload:?}"
        )),
    }
}

/// A neutral cancel command for the run under test.
fn cancel_command() -> InterventionCommand {
    InterventionCommand {
        workflow_id: WorkflowId::new(Uuid::nil()),
        run_id: RunId::new_v4(),
        activity_id: ActivityId::from_sequence_position(1),
        attempt: 1,
        issued_by: Some("operator".to_owned()),
        issued_at: Utc::now(),
        kind: InterventionKind::Cancel {
            reason: "the operator stopped it".to_owned(),
        },
    }
}

// --- the handshake ---

/// One `session/request_permission` request as an agent sends it.
fn permission_request(id: JsonRpcId) -> JsonRpcRequest {
    JsonRpcRequest::new(
        id,
        protocol::METHOD_SESSION_REQUEST_PERMISSION,
        Some(json!({
            "sessionId": SESSION,
            "toolCall": { "toolCallId": "t1", "title": "rm -rf /", "kind": "execute" },
            "options": [
                { "optionId": "allow-1", "name": "Allow once", "kind": "allow_once" },
                { "optionId": "allow-2", "name": "Always allow", "kind": "allow_always" },
                { "optionId": "deny-1", "name": "Reject", "kind": "reject_once" }
            ]
        })),
    )
}

mod handshake_and_streams;
mod outcomes_and_permissions;
mod raw_and_resume;
mod terminal_and_pump;
