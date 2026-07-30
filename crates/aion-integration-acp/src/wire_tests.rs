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

use std::sync::Arc;
use std::time::Duration;

use aion_core::{
    ActivityEvent, ActivityEventKind, ActivityId, InterventionCommand, InterventionKind,
    InterventionPrimitive, MessageRole, ProgressDetail, StopKind, WorkflowId,
};
use aion_integrations::contract::AgentSession;
use aion_integrations::jsonrpc::{
    IncomingMessage, JsonRpcConnection, JsonRpcId, JsonRpcNotification, JsonRpcRequest,
    JsonRpcResponse,
};
use aion_integrations::{ContentType, HarnessError};
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
        "/srv/workspace",
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
        .connect(connection.as_ref(), "do the thing".to_owned())
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

#[tokio::test]
async fn the_handshake_negotiates_version_one_and_opens_a_session() -> TestResult {
    // `wire_up` asserts the wire form of all three opening frames: `protocolVersion` as the integer
    // 1, `session/new` with an absolute cwd and a PRESENT `mcpServers`, and a `text` prompt block.
    let wired = wire_up(PermissionPolicy::Deny).await?;
    assert!(
        wired
            .session
            .capabilities()
            .supports_primitive(InterventionPrimitive::Cancel),
        "a negotiated ACP v1 session advertises cancel"
    );
    Ok(())
}

#[tokio::test]
async fn the_initialize_request_sends_the_integer_version_and_all_false_capabilities() -> TestResult
{
    // Read the client's actual `initialize` frame off the wire and assert its shape. ACP versions
    // are a uint16 — a semver string here would be silently unparseable to every real agent.
    let (client_io, agent_io) = duplex(DUPLEX_CAPACITY);
    let (client_read, client_write) = split(client_io);
    let (agent_read, agent_write) = split(agent_io);
    let connection = Arc::new(JsonRpcConnection::new(client_read, client_write));
    let agent = JsonRpcConnection::new(agent_read, agent_write);

    let client = tokio::spawn(async move {
        harness(PermissionPolicy::Deny)
            .connect(connection.as_ref(), "prompt".to_owned())
            .await
    });
    let request = expect_request(&agent, protocol::METHOD_INITIALIZE)
        .await
        .map_err(|error| fail("initialize must be the first frame", &error))?;
    let params = request
        .params
        .ok_or("initialize must carry params".to_owned())?;
    assert_eq!(
        params["protocolVersion"],
        json!(1),
        "the protocol version is the integer 1, never a string: {params}"
    );
    assert_eq!(
        params["clientCapabilities"]["fs"]["readTextFile"],
        json!(false)
    );
    assert_eq!(
        params["clientCapabilities"]["fs"]["writeTextFile"],
        json!(false)
    );
    assert_eq!(params["clientCapabilities"]["terminal"], json!(false));
    assert_eq!(
        params["clientInfo"]["name"],
        json!("aion-integration-acp"),
        "the client identifies itself honestly: {params}"
    );

    // The frame under test has been read. Close the channel and require the client to report the
    // broken handshake rather than hanging or reporting success against a dead peer.
    drop(agent);
    let outcome = client
        .await
        .map_err(|error| fail("the client task must not panic", &error))?;
    assert!(
        outcome.is_err(),
        "a handshake against a closed channel cannot succeed"
    );
    Ok(())
}

#[tokio::test]
async fn a_version_the_adapter_cannot_speak_is_refused_before_a_session_exists() -> TestResult {
    // The spec's rule: the agent may answer with its own latest version, and a client that cannot
    // speak it MUST disconnect. Refusing here — before `session/new` — is that disconnect.
    let (client_io, agent_io) = duplex(DUPLEX_CAPACITY);
    let (client_read, client_write) = split(client_io);
    let (agent_read, agent_write) = split(agent_io);
    let connection = Arc::new(JsonRpcConnection::new(client_read, client_write));
    let agent = Arc::new(JsonRpcConnection::new(agent_read, agent_write));

    let script = {
        let agent = Arc::clone(&agent);
        // 2 is a version this adapter does not implement.
        tokio::spawn(async move { agent_opens(agent.as_ref(), 2).await })
    };
    let error = harness(PermissionPolicy::Deny)
        .connect(connection.as_ref(), "prompt".to_owned())
        .await
        .err()
        .ok_or("a version mismatch must refuse the connection".to_owned())?;
    assert!(
        matches!(error, HarnessError::Protocol { .. }),
        "a version mismatch is a protocol error, got {error:?}"
    );
    let message = error.to_string();
    assert!(
        message.contains("version 1") && message.contains("version 2"),
        "the refusal names both versions: {message}"
    );
    script
        .await
        .map_err(|error| fail("the agent script task must not panic", &error))?
        .map_err(|error| fail("the agent script must complete", &error))?;
    Ok(())
}

// --- every session/update variant ---

#[tokio::test]
async fn an_agent_message_chunk_streams_a_delta_and_closes_a_durable_message() -> TestResult {
    let events = events_for(&json!({
        "sessionUpdate": "agent_message_chunk",
        "content": { "type": "text", "text": "the answer" },
        "messageId": "m1"
    }))
    .await?;
    let delta = events
        .iter()
        .find(|event| matches!(event.kind, ActivityEventKind::Delta { .. }))
        .ok_or("the chunk must stream a delta".to_owned())?;
    assert!(delta.ephemeral, "a per-token delta is never persisted");
    let message = events
        .iter()
        .find(|event| matches!(event.kind, ActivityEventKind::Message { .. }))
        .ok_or("the turn's end must close the message durably".to_owned())?;
    assert!(!message.ephemeral, "the completed message is durable");
    match &message.kind {
        ActivityEventKind::Message { role, text } => {
            assert_eq!(*role, MessageRole::Assistant);
            assert_eq!(text, "the answer");
        }
        other => return Err(format!("expected Message, got {other:?}")),
    }
    Ok(())
}

#[tokio::test]
async fn a_user_message_chunk_becomes_one_durable_user_message() -> TestResult {
    let events = events_for(&json!({
        "sessionUpdate": "user_message_chunk",
        "content": { "type": "text", "text": "do the thing" }
    }))
    .await?;
    let user_messages: Vec<&ActivityEvent> = events
        .iter()
        .filter(|event| {
            matches!(
                &event.kind,
                ActivityEventKind::Message {
                    role: MessageRole::User,
                    ..
                }
            )
        })
        .collect();
    let [only] = user_messages.as_slice() else {
        return Err(format!(
            "exactly one durable user message per turn, got {user_messages:?}"
        ));
    };
    assert!(!only.ephemeral, "a user turn is durable");
    match &only.kind {
        ActivityEventKind::Message { text, .. } => assert_eq!(text, "do the thing"),
        other => return Err(format!("expected Message, got {other:?}")),
    }
    Ok(())
}

#[tokio::test]
async fn a_multi_chunk_user_turn_is_still_one_durable_message() -> TestResult {
    // The defence against an echoing agent: N chunks must not become N durable events.
    let Wired {
        mut session,
        agent,
        prompt_id,
    } = wire_up(PermissionPolicy::Deny).await?;
    for fragment in ["do ", "the ", "thing"] {
        agent
            .send_notification(&session_update(&json!({
                "sessionUpdate": "user_message_chunk",
                "content": { "type": "text", "text": fragment },
                "messageId": "u1"
            })))
            .await
            .map_err(|error| fail("the chunk must be writable", &error))?;
    }
    agent
        .send_response(&prompt_response(prompt_id, "end_turn"))
        .await
        .map_err(|error| fail("the terminal response must be writable", &error))?;
    let events = session.events();
    let collected = within("draining the transcript", events.collect::<Vec<_>>()).await?;

    let texts: Vec<&str> = collected
        .iter()
        .filter_map(|event| match &event.kind {
            ActivityEventKind::Message {
                role: MessageRole::User,
                text,
            } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        texts,
        vec!["do the thing"],
        "three chunks, one durable message: {collected:?}"
    );
    drop(session);
    drop(agent);
    Ok(())
}

#[tokio::test]
async fn an_agent_thought_chunk_streams_live_and_survives_durably() -> TestResult {
    let events = events_for(&json!({
        "sessionUpdate": "agent_thought_chunk",
        "content": { "type": "text", "text": "considering" }
    }))
    .await?;
    let note = events
        .iter()
        .find(|event| {
            matches!(
                &event.kind,
                ActivityEventKind::Progress {
                    detail: ProgressDetail::Note { .. }
                }
            )
        })
        .ok_or("reasoning must reach the live transcript".to_owned())?;
    assert!(note.ephemeral, "the live reasoning stream is ephemeral");
    match &note.kind {
        ActivityEventKind::Progress {
            detail: ProgressDetail::Note { text },
        } => assert_eq!(text, "considering"),
        other => return Err(format!("expected Progress::Note, got {other:?}")),
    }

    // The ruling: reasoning that vanishes at end-of-run defeats durable observability.
    let durable = events
        .iter()
        .find(|event| {
            matches!(
                &event.kind,
                ActivityEventKind::Message {
                    role: MessageRole::Assistant,
                    ..
                }
            )
        })
        .ok_or_else(|| format!("reasoning must also reach the DURABLE record: {events:?}"))?;
    assert!(!durable.ephemeral, "the accumulated reasoning is durable");
    match &durable.kind {
        ActivityEventKind::Message { text, .. } => assert_eq!(text, "considering"),
        other => return Err(format!("expected Message, got {other:?}")),
    }
    Ok(())
}

#[tokio::test]
async fn reasoning_never_becomes_the_activitys_result() -> TestResult {
    // The one distinction the neutral vocabulary cannot express in a role, and the one that matters:
    // the payload's text is the answer, never the thinking that preceded it.
    let Wired {
        session,
        agent,
        prompt_id,
    } = wire_up(PermissionPolicy::Deny).await?;
    agent
        .send_notification(&session_update(&json!({
            "sessionUpdate": "agent_thought_chunk",
            "content": { "type": "text", "text": "maybe I should refuse" }
        })))
        .await
        .map_err(|error| fail("the thought chunk must be writable", &error))?;
    agent
        .send_notification(&session_update(&json!({
            "sessionUpdate": "agent_message_chunk",
            "content": { "type": "text", "text": "42" },
            "messageId": "m1"
        })))
        .await
        .map_err(|error| fail("the message chunk must be writable", &error))?;
    agent
        .send_response(&prompt_response(prompt_id, "end_turn"))
        .await
        .map_err(|error| fail("the terminal response must be writable", &error))?;

    let payload = within("taking the terminal result", session.wait_result())
        .await?
        .map_err(|error| fail("end_turn is a completed run", &error))?;
    let decoded = payload
        .to_json()
        .map_err(|error| fail("the payload is JSON", &error))?;
    assert_eq!(decoded, json!({ "stopReason": "end_turn", "text": "42" }));
    drop(agent);
    Ok(())
}

#[tokio::test]
async fn a_tool_call_and_its_completion_become_a_call_and_a_result() -> TestResult {
    let created = events_for(&json!({
        "sessionUpdate": "tool_call",
        "toolCallId": "t1",
        "title": "Read src/main.rs",
        "kind": "read",
        "status": "pending",
        "rawInput": { "path": "src/main.rs" }
    }))
    .await?;
    let call = created
        .iter()
        .find_map(|event| match &event.kind {
            ActivityEventKind::ToolCall {
                tool,
                call_id,
                input,
            } => Some((tool.clone(), call_id.clone(), input.clone())),
            _ => None,
        })
        .ok_or("a tool call must be classified".to_owned())?;
    assert_eq!(call.0, "Read src/main.rs");
    assert_eq!(call.1, "t1");
    assert_eq!(call.2, json!({ "path": "src/main.rs" }));
    assert!(
        has_raw(&created, "session/update/tool_call"),
        "the verbatim frame preserves `kind`, which the neutral ToolCall cannot carry: {created:?}"
    );

    let updated = events_for(&json!({
        "sessionUpdate": "tool_call_update",
        "toolCallId": "t1",
        "status": "completed",
        "rawOutput": { "lines": 12 }
    }))
    .await?;
    let result = updated
        .iter()
        .find_map(|event| match &event.kind {
            ActivityEventKind::ToolResult {
                call_id,
                output,
                is_error,
            } => Some((call_id.clone(), output.clone(), *is_error)),
            _ => None,
        })
        .ok_or("a completed update must be classified as a result".to_owned())?;
    assert_eq!(result.0, "t1");
    assert_eq!(result.1, json!({ "lines": 12 }));
    assert!(!result.2);
    assert!(has_raw(&updated, "session/update/tool_call_update"));
    Ok(())
}

#[tokio::test]
async fn a_failed_tool_call_update_is_an_error_result() -> TestResult {
    let events = events_for(&json!({
        "sessionUpdate": "tool_call_update",
        "toolCallId": "t2",
        "status": "failed"
    }))
    .await?;
    let is_error = events.iter().any(|event| {
        matches!(
            &event.kind,
            ActivityEventKind::ToolResult { is_error: true, .. }
        )
    });
    assert!(is_error, "a failed status is an error result: {events:?}");
    Ok(())
}

#[tokio::test]
async fn the_six_remaining_update_variants_each_reach_the_transcript() -> TestResult {
    // Table-driven, but every row genuinely goes over the wire as the ACP frame an agent sends.
    // Losing an update silently is not acceptable: each must produce a transcript entry.
    let cases: Vec<(Value, &str)> = vec![
        (
            json!({
                "sessionUpdate": "plan",
                "entries": [
                    { "content": "read the code", "priority": "high", "status": "pending" },
                    { "content": "write the fix", "priority": "medium", "status": "in_progress" }
                ]
            }),
            "session/update/plan",
        ),
        (
            json!({
                "sessionUpdate": "available_commands_update",
                "availableCommands": [{ "name": "create_plan", "description": "draft a plan" }]
            }),
            "session/update/available_commands_update",
        ),
        (
            json!({ "sessionUpdate": "current_mode_update", "currentModeId": "architect" }),
            "session/update/current_mode_update",
        ),
        (
            json!({ "sessionUpdate": "config_option_update", "configOptions": [] }),
            "session/update/config_option_update",
        ),
        (
            json!({ "sessionUpdate": "session_info_update", "title": "investigating" }),
            "session/update/session_info_update",
        ),
        (
            json!({ "sessionUpdate": "usage_update", "used": 4_096, "size": 200_000 }),
            "session/update/usage_update",
        ),
    ];
    for (update, source) in cases {
        let events = events_for(&update).await?;
        assert!(
            has_raw(&events, source),
            "`{update}` must reach the transcript under `{source}`, got {events:?}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn a_plan_reaches_the_transcript_with_every_entry() -> TestResult {
    // A plan is a COMPLETE replacement, so every entry has to survive or an operator's plan view
    // goes stale without anything reporting it.
    let events = events_for(&json!({
        "sessionUpdate": "plan",
        "entries": [
            { "content": "one", "priority": "high", "status": "completed" },
            { "content": "two", "priority": "low", "status": "pending" }
        ]
    }))
    .await?;
    let entries = events
        .iter()
        .find_map(|event| match &event.kind {
            ActivityEventKind::Raw { source, value } if source == "session/update/plan" => {
                value["entries"].as_array().cloned()
            }
            _ => None,
        })
        .ok_or("the plan entries must survive".to_owned())?;
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0]["content"], json!("one"));
    assert_eq!(entries[1]["status"], json!("pending"));
    Ok(())
}

#[tokio::test]
async fn an_unrecognised_update_variant_still_reaches_the_transcript() -> TestResult {
    // Forward compatibility: a variant from a future protocol release must surface, not vanish and
    // not fail the run.
    let events = events_for(&json!({
        "sessionUpdate": "teleport_update",
        "destination": "mars"
    }))
    .await?;
    assert!(
        has_raw(&events, "session/update/undecodable"),
        "an unknown variant must be recorded verbatim: {events:?}"
    );
    Ok(())
}

// --- the five stop reasons ---

#[tokio::test]
async fn end_turn_is_the_completed_run_and_carries_the_agents_text() -> TestResult {
    let Wired {
        mut session,
        agent,
        prompt_id,
    } = wire_up(PermissionPolicy::Deny).await?;
    agent
        .send_notification(&session_update(&json!({
            "sessionUpdate": "agent_message_chunk",
            "content": { "type": "text", "text": "42" },
            "messageId": "m1"
        })))
        .await
        .map_err(|error| fail("the chunk must be writable", &error))?;
    agent
        .send_response(&prompt_response(prompt_id, "end_turn"))
        .await
        .map_err(|error| fail("the terminal response must be writable", &error))?;

    // Drain the transcript to END-OF-STREAM, exactly as the platform's driver does. Breaking out at
    // the Stop event would hide the defect this shape exists to catch: the agent is still alive, so
    // the stream can only end because the adapter closed it.
    let events = session.events();
    let collected = within("draining the transcript", events.collect::<Vec<_>>()).await?;
    let saw_stop = collected.iter().any(|event| {
        matches!(
            event.kind,
            ActivityEventKind::Stop {
                reason: StopKind::EndTurn
            }
        )
    });
    assert!(saw_stop, "a completed turn emits a terminal Stop event");

    let payload = within("taking the terminal result", session.wait_result())
        .await?
        .map_err(|error| fail("end_turn is a completed run", &error))?;
    assert_eq!(payload.content_type(), &ContentType::Json);
    let decoded = payload
        .to_json()
        .map_err(|error| fail("the payload is JSON", &error))?;
    assert_eq!(decoded, json!({ "stopReason": "end_turn", "text": "42" }));
    Ok(())
}

#[tokio::test]
async fn refusal_and_the_two_limits_are_failures_naming_themselves() -> TestResult {
    for reason in ["refusal", "max_tokens", "max_turn_requests"] {
        let Wired {
            session,
            agent,
            prompt_id,
        } = wire_up(PermissionPolicy::Deny).await?;
        agent
            .send_response(&prompt_response(prompt_id, reason))
            .await
            .map_err(|error| fail("the terminal response must be writable", &error))?;
        let error =
            session.wait_result().await.err().ok_or_else(|| {
                format!("`{reason}` must never be reported as a completed result")
            })?;
        assert!(
            matches!(error, HarnessError::Harness { .. }),
            "`{reason}` is a harness failure, got {error:?}"
        );
        let message = error.to_string();
        assert!(
            message.contains(reason),
            "the failure names `{reason}`: {message}"
        );
        assert!(
            !message.contains(protocol::CANCELLED_DETAIL_PREFIX),
            "`{reason}` is not a cancellation: {message}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn a_cancelled_stop_reason_is_distinguishable_from_a_failure() -> TestResult {
    let Wired {
        session,
        agent,
        prompt_id,
    } = wire_up(PermissionPolicy::Deny).await?;
    agent
        .send_response(&prompt_response(prompt_id, "cancelled"))
        .await
        .map_err(|error| fail("the terminal response must be writable", &error))?;
    let error = session
        .wait_result()
        .await
        .err()
        .ok_or("a cancelled turn is not a completed result".to_owned())?;
    assert!(
        error
            .to_string()
            .contains(protocol::CANCELLED_DETAIL_PREFIX),
        "a cancellation carries the documented prefix: {error}"
    );
    Ok(())
}

// --- cancellation ---

#[tokio::test]
async fn cancelling_sends_session_cancel_and_ends_the_turn_as_cancelled() -> TestResult {
    let Wired {
        session,
        agent,
        prompt_id,
    } = wire_up(PermissionPolicy::AllowOnce).await?;

    session
        .intervene(cancel_command())
        .await
        .map_err(|error| fail("cancel is an advertised primitive", &error))?;

    // The agent observes the notification — a NOTIFICATION, never a request: it carries no id, so it
    // can never be mistaken for something awaiting a reply.
    let message = agent
        .recv()
        .await
        .map_err(|error| fail("the cancel frame must be readable", &error))?
        .ok_or("the cancel frame must arrive".to_owned())?;
    match message {
        IncomingMessage::Notification(note) => {
            assert_eq!(note.method, protocol::METHOD_SESSION_CANCEL);
            let params = note
                .params
                .ok_or("session/cancel carries params".to_owned())?;
            assert_eq!(params["sessionId"], json!(SESSION));
        }
        other => {
            return Err(format!(
                "session/cancel must be a notification, got {other:?}"
            ));
        }
    }

    // The spec: the agent MUST then answer the still-open prompt with `cancelled`, never an error.
    agent
        .send_response(&prompt_response(prompt_id, "cancelled"))
        .await
        .map_err(|error| fail("the terminal response must be writable", &error))?;
    let error = session
        .wait_result()
        .await
        .err()
        .ok_or("a cancelled turn is not a completed result".to_owned())?;
    assert!(
        error
            .to_string()
            .contains(protocol::CANCELLED_DETAIL_PREFIX),
        "the terminal outcome is a cancellation: {error}"
    );
    Ok(())
}

#[tokio::test]
async fn an_unadvertised_primitive_is_gated_before_anything_is_written() -> TestResult {
    let Wired {
        session,
        agent,
        prompt_id,
    } = wire_up(PermissionPolicy::Deny).await?;
    let mut command = cancel_command();
    command.kind = InterventionKind::InjectMessage {
        text: "steer".to_owned(),
        priority: aion_core::InjectPriority::Interrupt,
    };
    let error = session
        .intervene(command)
        .await
        .err()
        .ok_or("ACP cannot inject into a running turn".to_owned())?;
    assert!(
        matches!(error, HarnessError::CapabilityNotSupported { .. }),
        "an unadvertised primitive is capability-gated, got {error:?}"
    );
    assert_eq!(
        error.to_string(),
        "capability not supported: inject_message",
        "the rejection names the primitive in the neutral vocabulary"
    );

    // Nothing was written: ending the turn is the only frame the agent ever sees, and the run still
    // completes normally.
    close_turn(session, &agent, prompt_id, "end_turn").await
}

#[tokio::test]
async fn a_permission_request_arriving_after_a_cancel_is_answered_cancelled() -> TestResult {
    // The other half of the client's cancellation obligation: a client that cancels MUST answer
    // every pending permission request with the `cancelled` outcome, whatever its policy would
    // otherwise have chosen.
    let Wired {
        session,
        agent,
        prompt_id,
    } = wire_up(PermissionPolicy::AllowOnce).await?;
    session
        .intervene(cancel_command())
        .await
        .map_err(|error| fail("cancel is an advertised primitive", &error))?;
    // Consume the cancel notification so the next agent-side read is the permission answer.
    let cancel = agent
        .recv()
        .await
        .map_err(|error| fail("the cancel frame must be readable", &error))?
        .ok_or("the cancel frame must arrive".to_owned())?;
    assert!(
        matches!(&cancel, IncomingMessage::Notification(note)
            if note.method == protocol::METHOD_SESSION_CANCEL),
        "the first frame after intervene is session/cancel, got {cancel:?}"
    );

    let outcome = ask_permission(&agent, 5_001).await?;
    assert_eq!(
        outcome["outcome"],
        json!("cancelled"),
        "an allow-once policy is overridden by the cancellation: {outcome}"
    );

    close_turn(session, &agent, prompt_id, "cancelled").await
}

// --- permission requests ---

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

/// Sends one `session/request_permission` from the fake agent and returns the `outcome` object the
/// adapter answered with.
async fn ask_permission(agent: &Wire, request_id: u64) -> TestResult<Value> {
    let request = permission_request(JsonRpcId::number(request_id));
    agent
        .send_request(&request)
        .await
        .map_err(|error| fail("the permission request must be writable", &error))?;
    let message = agent
        .recv()
        .await
        .map_err(|error| fail("the answer must be readable", &error))?
        .ok_or("a permission request must ALWAYS be answered".to_owned())?;
    match message {
        IncomingMessage::Response(response) => {
            if response.id != JsonRpcId::number(request_id) {
                return Err(format!(
                    "the answer must be id-matched, got {:?}",
                    response.id
                ));
            }
            let result = response.result.ok_or_else(|| {
                format!("the answer must carry a result, got {:?}", response.error)
            })?;
            Ok(result["outcome"].clone())
        }
        other => Err(format!("expected a response, got {other:?}")),
    }
}

#[tokio::test]
async fn the_deny_policy_selects_a_reject_option_and_records_the_decision() -> TestResult {
    let Wired {
        mut session,
        agent,
        prompt_id,
    } = wire_up(PermissionPolicy::Deny).await?;
    let mut events = session.events();

    let outcome = ask_permission(&agent, 4_001).await?;
    assert_eq!(outcome["outcome"], json!("selected"));
    assert_eq!(
        outcome["optionId"],
        json!("deny-1"),
        "the deny policy picks the reject option: {outcome}"
    );

    // Both the ask and the decision must be on the transcript: a policy answer an operator cannot
    // audit after the fact is exactly what this event pair exists to prevent.
    let asked = next_raw(&mut events, protocol::METHOD_SESSION_REQUEST_PERMISSION).await?;
    assert_eq!(asked["toolCall"]["toolCallId"], json!("t1"));
    let decided = next_raw(
        &mut events,
        &format!("{}/decision", protocol::METHOD_SESSION_REQUEST_PERMISSION),
    )
    .await?;
    assert_eq!(decided["policy"], json!("deny"));
    assert_eq!(decided["outcome"]["optionId"], json!("deny-1"));
    assert_eq!(decided["toolCallId"], json!("t1"));
    drop(events);

    close_turn(session, &agent, prompt_id, "end_turn").await
}

#[tokio::test]
async fn the_allow_once_policy_selects_allow_once_and_records_the_decision() -> TestResult {
    let Wired {
        mut session,
        agent,
        prompt_id,
    } = wire_up(PermissionPolicy::AllowOnce).await?;
    let mut events = session.events();

    let outcome = ask_permission(&agent, 4_002).await?;
    assert_eq!(
        outcome["optionId"],
        json!("allow-1"),
        "the allow-once policy picks allow_once, never allow_always: {outcome}"
    );

    let asked = next_raw(&mut events, protocol::METHOD_SESSION_REQUEST_PERMISSION).await?;
    assert_eq!(asked["toolCall"]["toolCallId"], json!("t1"));
    let decided = next_raw(
        &mut events,
        &format!("{}/decision", protocol::METHOD_SESSION_REQUEST_PERMISSION),
    )
    .await?;
    assert_eq!(decided["policy"], json!("allow_once"));
    assert_eq!(decided["outcome"]["optionId"], json!("allow-1"));
    drop(events);

    close_turn(session, &agent, prompt_id, "end_turn").await
}

/// Reads the event stream until a `Raw` event with exactly `source` arrives, returning its value.
async fn next_raw(
    events: &mut futures::stream::BoxStream<'static, ActivityEvent>,
    source: &str,
) -> TestResult<Value> {
    while let Some(event) = events.next().await {
        if let ActivityEventKind::Raw { source: got, value } = event.kind
            && got == source
        {
            return Ok(value);
        }
    }
    Err(format!("no `{source}` event reached the transcript"))
}

// --- unadvertised peer requests ---

#[tokio::test]
async fn an_inbound_fs_read_gets_an_error_response_rather_than_silence() -> TestResult {
    // The whole point: we advertise `fs.readTextFile: false`, but a non-conforming agent may ask
    // anyway, and an unanswered request blocks it forever. It must get an ANSWER.
    let Wired {
        mut session,
        agent,
        prompt_id,
    } = wire_up(PermissionPolicy::Deny).await?;
    let mut events = session.events();

    let request = JsonRpcRequest::new(
        JsonRpcId::number(7_001),
        protocol::METHOD_FS_READ_TEXT_FILE,
        Some(json!({ "sessionId": SESSION, "path": "/etc/hosts" })),
    );
    agent
        .send_request(&request)
        .await
        .map_err(|error| fail("the fs request must be writable", &error))?;
    let message = agent
        .recv()
        .await
        .map_err(|error| fail("the refusal must be readable", &error))?
        .ok_or("an unadvertised request must still be answered".to_owned())?;
    match message {
        IncomingMessage::Response(response) => {
            assert_eq!(response.id, JsonRpcId::number(7_001));
            let error = response
                .error
                .ok_or("the refusal is an error response".to_owned())?;
            assert_eq!(
                error.code,
                protocol::method_not_found_code(),
                "an unadvertised capability is `method not found`"
            );
            assert!(
                error.message.contains(protocol::METHOD_FS_READ_TEXT_FILE),
                "the refusal names the method it refused: {}",
                error.message
            );
        }
        other => return Err(format!("expected an error response, got {other:?}")),
    }

    // And the refusal is on the transcript, because it changes what the agent could do.
    let recorded = next_raw(
        &mut events,
        &format!("refused/{}", protocol::METHOD_FS_READ_TEXT_FILE),
    )
    .await?;
    assert_eq!(recorded["params"]["path"], json!("/etc/hosts"));
    drop(events);

    // The run is still alive: the turn completes normally after the refusal.
    agent
        .send_response(&prompt_response(prompt_id, "end_turn"))
        .await
        .map_err(|error| fail("the terminal response must be writable", &error))?;
    let payload = session
        .wait_result()
        .await
        .map_err(|error| fail("a refused peer request must not kill the run", &error))?;
    assert_eq!(payload.content_type(), &ContentType::Json);
    Ok(())
}

#[tokio::test]
async fn an_inbound_terminal_create_is_refused_the_same_way() -> TestResult {
    let Wired {
        session,
        agent,
        prompt_id,
    } = wire_up(PermissionPolicy::Deny).await?;
    let request = JsonRpcRequest::new(
        JsonRpcId::number(7_002),
        "terminal/create",
        Some(json!({ "sessionId": SESSION, "command": "ls" })),
    );
    agent
        .send_request(&request)
        .await
        .map_err(|error| fail("the terminal request must be writable", &error))?;
    let message = agent
        .recv()
        .await
        .map_err(|error| fail("the refusal must be readable", &error))?
        .ok_or("an unadvertised request must still be answered".to_owned())?;
    let refused = matches!(
        message,
        IncomingMessage::Response(ref response)
            if response.error.as_ref().map(|error| error.code)
                == Some(protocol::method_not_found_code())
    );
    assert!(refused, "terminal/create must be refused, got {message:?}");

    // The run survives the refusal and completes normally.
    close_turn(session, &agent, prompt_id, "end_turn").await
}

#[tokio::test]
async fn the_agent_closing_the_channel_is_a_transport_error_not_a_hang() -> TestResult {
    // An agent that dies without answering must not leave `wait_result` waiting forever.
    let Wired {
        session,
        agent,
        prompt_id: _,
    } = wire_up(PermissionPolicy::Deny).await?;
    drop(agent);
    let error = within("taking the terminal result", session.wait_result())
        .await?
        .err()
        .ok_or("a closed channel yields no result".to_owned())?;
    assert!(
        matches!(error, HarnessError::Transport { .. }),
        "a dead agent is a transport failure, got {error:?}"
    );
    // The message must name what ACTUALLY happened. `StreamEnded` is the agent exiting without
    // answering, and it must not be reported with the same words as a pump that died holding an
    // answer, nor as a reader that hit an I/O fault.
    let message = error.to_string();
    assert!(
        message.contains("closed its stdout") && message.contains("without answering"),
        "the failure names the end-of-stream it observed: {message}"
    );
    Ok(())
}

// --- the driver contract, and the frames that must not break it ---

#[tokio::test]
async fn the_driver_contract_completes_against_an_agent_that_stays_alive() -> TestResult {
    // The exact shape of `aion-worker`'s `spawn_agent`: pump `events.next()` until the stream ends,
    // and only THEN take the result. The agent is a persistent stdio server — it answers
    // `session/prompt` and keeps running, its write half held open for the whole test — so the
    // stream can end only because the adapter closed it at the answer. Before that fix, every
    // successful turn hung here forever with a good result unread.
    let Wired {
        mut session,
        agent,
        prompt_id,
    } = wire_up(PermissionPolicy::Deny).await?;
    agent
        .send_notification(&session_update(&json!({
            "sessionUpdate": "agent_message_chunk",
            "content": { "type": "text", "text": "ablative" },
            "messageId": "m1"
        })))
        .await
        .map_err(|error| fail("the chunk must be writable", &error))?;
    agent
        .send_response(&prompt_response(prompt_id, "end_turn"))
        .await
        .map_err(|error| fail("the terminal response must be writable", &error))?;

    let mut events = AgentSession::events(&mut session);
    let mut collected = Vec::new();
    within("the driver's event loop", async {
        while let Some(event) = events.next().await {
            collected.push(event);
        }
    })
    .await?;
    drop(events);

    // The agent is STILL ALIVE, and provably so: it can still write to a channel the adapter has
    // not closed. The transcript ended because the ADAPTER ended it, which is the whole point.
    agent
        .send_notification(&JsonRpcNotification::new("agent/still_here", None))
        .await
        .map_err(|error| {
            fail(
                "the agent must still be writable after the transcript ended",
                &error,
            )
        })?;

    let last = collected
        .last()
        .ok_or("the turn must produce events".to_owned())?;
    assert!(
        matches!(
            last.kind,
            ActivityEventKind::Stop {
                reason: StopKind::EndTurn
            }
        ),
        "the LAST event is the terminal boundary, got {:?}",
        last.kind
    );

    let payload = within("taking the terminal result", session.wait_result())
        .await?
        .map_err(|error| fail("the driver contract must yield a result", &error))?;
    let decoded = payload
        .to_json()
        .map_err(|error| fail("the payload is JSON", &error))?;
    assert_eq!(
        decoded,
        json!({ "stopReason": "end_turn", "text": "ablative" })
    );
    drop(agent);
    Ok(())
}

#[tokio::test]
async fn the_terminal_response_is_recorded_verbatim_on_the_transcript() -> TestResult {
    // Every other frame gets a verbatim twin; the one that decides the result is no exception.
    // `PromptResponse._meta` lives nowhere else once the payload is formed.
    let Wired {
        mut session,
        agent,
        prompt_id,
    } = wire_up(PermissionPolicy::Deny).await?;
    agent
        .send_response(&JsonRpcResponse::success(
            prompt_id,
            json!({ "stopReason": "end_turn", "_meta": { "costUsd": 0.02 } }),
        ))
        .await
        .map_err(|error| fail("the terminal response must be writable", &error))?;
    let events = session.events();
    let collected = within("draining the transcript", events.collect::<Vec<_>>()).await?;

    let recorded = collected
        .iter()
        .find_map(|event| match &event.kind {
            ActivityEventKind::Raw { source, value }
                if source == protocol::METHOD_SESSION_PROMPT =>
            {
                Some(value.clone())
            }
            _ => None,
        })
        .ok_or_else(|| format!("the terminal frame must be recorded: {collected:?}"))?;
    assert_eq!(
        recorded["result"]["_meta"]["costUsd"],
        json!(0.02),
        "the agent's `_meta` survives: {recorded}"
    );
    drop(session);
    drop(agent);
    Ok(())
}

#[tokio::test]
async fn a_failed_terminal_frame_still_emits_a_terminal_boundary() -> TestResult {
    // A transcript that just stops mid-stream is not an honest record of a failed turn, and
    // `StopKind::Error` exists for exactly this. Nothing is invented by using it.
    let Wired {
        mut session,
        agent,
        prompt_id,
    } = wire_up(PermissionPolicy::Deny).await?;
    agent
        .send_response(&JsonRpcResponse::failure(
            prompt_id,
            aion_integrations::jsonrpc::JsonRpcError::new(-32603, "the model exploded"),
        ))
        .await
        .map_err(|error| fail("the terminal response must be writable", &error))?;
    let events = session.events();
    let collected = within("draining the transcript", events.collect::<Vec<_>>()).await?;

    let stop = collected
        .iter()
        .find_map(|event| match &event.kind {
            ActivityEventKind::Stop {
                reason: StopKind::Error { message },
            } => Some(message.clone()),
            _ => None,
        })
        .ok_or_else(|| format!("an error Response must still stop the run: {collected:?}"))?;
    assert!(
        stop.contains("-32603") && stop.contains("the model exploded"),
        "the boundary names the failure: {stop}"
    );
    // And the offending frame is recorded, not merely summarised into the stop message.
    assert!(
        has_raw(&collected, protocol::METHOD_SESSION_PROMPT),
        "the error frame must be on the transcript verbatim: {collected:?}"
    );
    drop(session);
    drop(agent);
    Ok(())
}

#[tokio::test]
async fn an_undecodable_terminal_result_still_emits_a_terminal_boundary() -> TestResult {
    let Wired {
        mut session,
        agent,
        prompt_id,
    } = wire_up(PermissionPolicy::Deny).await?;
    agent
        .send_response(&JsonRpcResponse::success(
            prompt_id,
            json!({ "stopReason": "teleported" }),
        ))
        .await
        .map_err(|error| fail("the terminal response must be writable", &error))?;
    let events = session.events();
    let collected = within("draining the transcript", events.collect::<Vec<_>>()).await?;

    let stop = collected
        .iter()
        .find_map(|event| match &event.kind {
            ActivityEventKind::Stop {
                reason: StopKind::Error { message },
            } => Some(message.clone()),
            _ => None,
        })
        .ok_or_else(|| format!("an undecodable result must still stop the run: {collected:?}"))?;
    assert!(
        stop.contains("PromptResponse"),
        "the boundary names why the frame could not be read: {stop}"
    );
    assert!(
        has_raw(&collected, protocol::METHOD_SESSION_PROMPT),
        "the undecodable frame must be recorded verbatim: {collected:?}"
    );
    drop(session);
    drop(agent);
    Ok(())
}

#[tokio::test]
async fn answering_a_flood_of_peer_requests_does_not_stall_the_read_loop() -> TestResult {
    // The deadlock: the transport holds the writer mutex across the write, so answering inline
    // stops the read loop as soon as the child's stdin fills — and the child cannot drain its stdin
    // while its stdout is not being read. A small duplex reproduces it exactly: this agent NEVER
    // reads its inbound pipe, so every answer after the first kilobyte blocks forever.
    let Wired {
        session,
        agent,
        prompt_id,
    } = wire_up_with_capacity(PermissionPolicy::Deny, 1_024).await?;

    let script = {
        let agent = Arc::clone(&agent);
        let prompt_id = prompt_id.clone();
        tokio::spawn(async move {
            for id in 0..40_u64 {
                agent
                    .send_request(&permission_request(JsonRpcId::number(9_000 + id)))
                    .await?;
            }
            agent
                .send_response(&prompt_response(prompt_id, "end_turn"))
                .await
        })
    };

    // With the write awaited inline this never resolves; the turn must still reach its result.
    let payload = within(
        "the terminal result behind a flood of unread answers",
        session.wait_result(),
    )
    .await?
    .map_err(|error| fail("the flood must not fail the turn", &error))?;
    assert_eq!(payload.content_type(), &ContentType::Json);

    script
        .await
        .map_err(|error| fail("the agent script task must not panic", &error))?
        .map_err(|error| fail("the agent script must complete", &error))?;
    drop(agent);
    Ok(())
}

#[tokio::test]
async fn a_notification_read_during_the_handshake_reaches_the_transcript() -> TestResult {
    // The handshake reads frames before the pump exists. Dropping one there broke the crate's
    // "nothing is dropped" rule, so they are carried forward and replayed as the transcript's first
    // events instead.
    let (client_io, agent_io) = duplex(DUPLEX_CAPACITY);
    let (client_read, client_write) = split(client_io);
    let (agent_read, agent_write) = split(agent_io);
    let connection = Arc::new(JsonRpcConnection::new(client_read, client_write));
    let agent = Arc::new(JsonRpcConnection::new(agent_read, agent_write));

    let script = {
        let agent = Arc::clone(&agent);
        tokio::spawn(async move {
            let initialize = expect_request(agent.as_ref(), protocol::METHOD_INITIALIZE).await?;
            // Sent BEFORE the initialize answer, so it is read while the handshake is blocked.
            agent
                .send_notification(&JsonRpcNotification::new(
                    "agent/booting",
                    Some(json!({ "pid": 4_242 })),
                ))
                .await?;
            agent
                .send_response(&JsonRpcResponse::success(
                    initialize.id,
                    json!({
                        "protocolVersion": protocol::PROTOCOL_VERSION.as_u16(),
                        "agentCapabilities": {},
                        "authMethods": [],
                        "agentInfo": { "name": "fake-acp-agent", "version": "0.0.0" }
                    }),
                ))
                .await?;
            let new_session = expect_request(agent.as_ref(), protocol::METHOD_SESSION_NEW).await?;
            agent
                .send_response(&JsonRpcResponse::success(
                    new_session.id,
                    json!({ "sessionId": SESSION }),
                ))
                .await?;
            expect_request(agent.as_ref(), protocol::METHOD_SESSION_PROMPT)
                .await
                .map(|_prompt| ())
        })
    };
    let opened = harness(PermissionPolicy::Deny)
        .connect(connection.as_ref(), "do the thing".to_owned())
        .await
        .map_err(|error| fail("the handshake must succeed", &error))?;
    script
        .await
        .map_err(|error| fail("the agent script task must not panic", &error))?
        .map_err(|error| fail("the agent script must complete", &error))?;
    assert_eq!(
        opened.deferred.len(),
        1,
        "the handshake-window notification must be carried forward, not dropped"
    );

    let prompt_id = opened.prompt_id.clone();
    let mut session = AcpSession::start(SessionStart {
        connection,
        session_id: opened.session_id,
        prompt_id: opened.prompt_id,
        policy: PermissionPolicy::Deny,
        identity: identity(),
        deferred: opened.deferred,
        child: None,
        exit_grace: EXIT_GRACE,
    });
    agent
        .send_response(&prompt_response(prompt_id, "end_turn"))
        .await
        .map_err(|error| fail("the terminal response must be writable", &error))?;
    let events = session.events();
    let collected = within("draining the transcript", events.collect::<Vec<_>>()).await?;
    let first = collected
        .first()
        .ok_or("the deferred notification must lead the transcript".to_owned())?;
    match &first.kind {
        ActivityEventKind::Raw { source, value } => {
            assert_eq!(source, "agent/booting");
            assert_eq!(value["pid"], json!(4_242));
        }
        other => return Err(format!("expected the deferred Raw first, got {other:?}")),
    }
    drop(session);
    drop(agent);
    Ok(())
}

// --- a line on stdout that is not a JSON-RPC frame at all ---
//
// No typed send can produce one, so these tests hold the fake agent's write half RAW and write every
// frame as a literal line — which is what the module claims to do anyway.

/// The fake agent's reader when the test writes its own frames.
///
/// The writer is a [`Sink`] because the real writing happens on the raw half; the connection exists
/// only to classify the client's outbound frames.
type RawWire = JsonRpcConnection<ReadHalf<DuplexStream>, Sink>;

/// A live pairing where the agent's stdout is written by hand.
struct RawWired {
    session: Session,
    reader: RawWire,
    writer: WriteHalf<DuplexStream>,
    prompt_id: JsonRpcId,
}

/// Writes one line verbatim on the agent's stdout, newline-terminated.
async fn write_line(writer: &mut WriteHalf<DuplexStream>, line: &str) -> TestResult {
    writer
        .write_all(line.as_bytes())
        .await
        .map_err(|error| fail("the raw line must be writable", &error))?;
    writer
        .write_all(b"\n")
        .await
        .map_err(|error| fail("the frame terminator must be writable", &error))?;
    writer
        .flush()
        .await
        .map_err(|error| fail("the raw line must flush", &error))
}

/// The literal JSON of a JSON-RPC id, for embedding in a hand-written frame.
fn id_json(id: &JsonRpcId) -> TestResult<String> {
    serde_json::to_string(id).map_err(|error| fail("a request id must encode", &error))
}

/// Opens a turn against an agent whose stdout the test writes itself.
async fn wire_up_raw(policy: PermissionPolicy) -> TestResult<RawWired> {
    let (client_io, agent_io) = duplex(DUPLEX_CAPACITY);
    let (client_read, client_write) = split(client_io);
    let (agent_read, agent_write) = split(agent_io);
    let connection = Arc::new(JsonRpcConnection::new(client_read, client_write));
    let reader: RawWire = JsonRpcConnection::new(agent_read, tokio::io::sink());

    let script = tokio::spawn(async move {
        let mut writer = agent_write;
        let initialize = expect_request(&reader, protocol::METHOD_INITIALIZE)
            .await
            .map_err(|error| fail("initialize must arrive", &error))?;
        write_line(
            &mut writer,
            &format!(
                r#"{{"jsonrpc":"2.0","id":{},"result":{{"protocolVersion":1,"agentCapabilities":{{}},"authMethods":[],"agentInfo":{{"name":"raw-agent","version":"0.0.0"}}}}}}"#,
                id_json(&initialize.id)?
            ),
        )
        .await?;
        let new_session = expect_request(&reader, protocol::METHOD_SESSION_NEW)
            .await
            .map_err(|error| fail("session/new must arrive", &error))?;
        write_line(
            &mut writer,
            &format!(
                r#"{{"jsonrpc":"2.0","id":{},"result":{{"sessionId":"{SESSION}"}}}}"#,
                id_json(&new_session.id)?
            ),
        )
        .await?;
        expect_request(&reader, protocol::METHOD_SESSION_PROMPT)
            .await
            .map_err(|error| fail("session/prompt must arrive", &error))?;
        Ok::<_, String>((reader, writer))
    });

    let opened = harness(policy)
        .connect(connection.as_ref(), "do the thing".to_owned())
        .await
        .map_err(|error| fail("the handshake must succeed", &error))?;
    let (reader, writer) = script
        .await
        .map_err(|error| fail("the agent script task must not panic", &error))??;

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
    Ok(RawWired {
        session,
        reader,
        writer,
        prompt_id,
    })
}

#[tokio::test]
async fn a_line_that_is_not_a_jsonrpc_frame_does_not_destroy_the_turn() -> TestResult {
    // The exact shape a `node`/`npx`-launched agent emits: a debugger banner on stdout between a
    // chunk and the terminal Response. It used to end the pump, throw away an answer that had
    // ALREADY been received, emit no Stop, and report "the agent closed the channel" — a cause that
    // had not happened.
    let RawWired {
        mut session,
        reader,
        mut writer,
        prompt_id,
    } = wire_up_raw(PermissionPolicy::Deny).await?;

    write_line(
        &mut writer,
        &format!(
            r#"{{"jsonrpc":"2.0","method":"session/update","params":{{"sessionId":"{SESSION}","update":{{"sessionUpdate":"agent_message_chunk","content":{{"type":"text","text":"42"}},"messageId":"m1"}}}}}}"#
        ),
    )
    .await?;
    // Not JSON at all.
    write_line(&mut writer, "Debugger listening on ws://127.0.0.1:9229").await?;
    // Valid JSON, but not any of the three JSON-RPC shapes.
    write_line(&mut writer, r#"{"hello":"world"}"#).await?;
    write_line(
        &mut writer,
        &format!(
            r#"{{"jsonrpc":"2.0","id":{},"result":{{"stopReason":"end_turn"}}}}"#,
            id_json(&prompt_id)?
        ),
    )
    .await?;

    let events = session.events();
    let collected = within("draining the transcript", events.collect::<Vec<_>>()).await?;
    assert!(
        has_raw(&collected, "acp/unreadable_frame"),
        "the rejected line must be recorded, not swallowed: {collected:?}"
    );
    let unreadable = collected
        .iter()
        .filter(|event| {
            matches!(&event.kind, ActivityEventKind::Raw { source, .. }
                if source == "acp/unreadable_frame")
        })
        .count();
    assert_eq!(unreadable, 2, "both bad lines are recorded: {collected:?}");
    assert!(
        collected.iter().any(|event| matches!(
            event.kind,
            ActivityEventKind::Stop {
                reason: StopKind::EndTurn
            }
        )),
        "the turn still reaches its terminal boundary: {collected:?}"
    );

    let payload = within("taking the terminal result", session.wait_result())
        .await?
        .map_err(|error| fail("a bad line must not fail a completed turn", &error))?;
    let decoded = payload
        .to_json()
        .map_err(|error| fail("the payload is JSON", &error))?;
    assert_eq!(
        decoded,
        json!({ "stopReason": "end_turn", "text": "42" }),
        "the answer received before the bad line survives it"
    );
    drop(reader);
    drop(writer);
    Ok(())
}
