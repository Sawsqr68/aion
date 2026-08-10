//! The §3.4 translation point — the single place Norn's native event/command shapes are named.
//!
//! Two directions, both pure and independently testable:
//!
//! - **OUT:** [`notification_to_event`] maps a Norn `event/*` JSON-RPC notification (its `method`
//!   and `params`, as emitted by Norn's `agent_event_to_value` with `agent_id`/`agent_role` added)
//!   into a neutral [`ActivityEvent`]. Anything the adapter cannot classify falls through to
//!   [`ActivityEventKind::Raw`], so nothing is ever silently dropped.
//! - **IN:** [`intervention_to_request`] maps a neutral [`InterventionKind`] into the Norn
//!   `intervene/*` request (method + params). A primitive Norn does not advertise
//!   ([`InterventionKind::PauseResume`] / [`UpdateBudget`] / [`RespondToApproval`]) is rejected
//!   with [`HarnessError::CapabilityNotSupported`] and never sent.
//!
//! Plus [`parse_capabilities`], which gates the `initialize` result on the driven-mode protocol
//! version (`norn-driven/1`) and reads it into the neutral [`InterventionCapabilities`] the
//! server/console gate on, and [`run_result_to_outcome`], which interprets the `run/execute`
//! versioned stop envelope into the canonical [`AgentOutcome`] record (or the honest error a
//! non-completed stop — or an output the agent-outcome contract cannot carry — is).
//!
//! [`UpdateBudget`]: InterventionKind::UpdateBudget
//! [`RespondToApproval`]: InterventionKind::RespondToApproval

use aion_core::{
    ActivityEvent, ActivityEventKind, AgentOutcome, InterventionCapabilities, InterventionKind,
    InterventionPrimitive, MessageRole, ProgressDetail, StopKind,
};
use aion_integrations::HarnessError;
use chrono::Utc;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::protocol;

/// Identity carried on every event a session produces, stamped from the run spec + handshake.
///
/// The `(workflow, run, activity, attempt)` key and the per-notification `agent_id`/`agent_role`
/// (read from the notification params) together fill an [`ActivityEvent`]'s identity fields; the
/// producer-side ordering fields are filled by the session (a best-effort local sequence).
#[derive(Clone, Debug)]
pub struct EventIdentity {
    /// The workflow this run belongs to.
    pub workflow_id: aion_core::WorkflowId,
    /// The concrete run of that workflow this attempt belongs to — stamped from the run spec,
    /// never synthesised here. It is the axis that keeps two generations of a continue-as-new
    /// chain from writing one fused transcript.
    pub run_id: aion_core::RunId,
    /// The activity within the workflow.
    pub activity_id: aion_core::ActivityId,
    /// The attempt number of the run.
    pub attempt: u32,
}

/// Reads Norn's `initialize` result into the neutral [`InterventionCapabilities`], gating on the
/// driven-mode protocol version first.
///
/// The result must advertise `protocol: "norn-driven/1"` (which replaced the old
/// `protocolVersion: "2.0"`); a missing or different value is a [`HarnessError::Protocol`] naming
/// the expected and received values — the honest "your norn binary is stale" signal, surfaced
/// before any run is issued.
///
/// Norn advertises a `capabilities.interventions` array of neutral primitive labels
/// (`inject_message`, `cancel`, …). Unknown labels are ignored (forward-compat), and a missing
/// array yields the empty (observability-only) set — never an error, because an empty capability
/// advertisement is first-class.
///
/// # Errors
///
/// Returns [`HarnessError::Protocol`] when the `protocol` field is missing or is not exactly
/// [`protocol::PROTOCOL_VERSION`]; a present-but-wrong value (any JSON type) is rendered
/// verbatim in the message.
pub fn parse_capabilities(
    initialize_result: &Value,
) -> Result<InterventionCapabilities, HarnessError> {
    match initialize_result.get(protocol::PROTOCOL_KEY) {
        Some(received) if received.as_str() == Some(protocol::PROTOCOL_VERSION) => {}
        // A present-but-wrong value — including a non-string — is reported as the JSON actually
        // received (a string renders quoted), never misdescribed as a missing field.
        Some(received) => {
            return Err(HarnessError::protocol(format!(
                "norn driven-mode protocol mismatch: expected {:?}, received {received} — \
                 the norn binary is stale or incompatible with this adapter",
                protocol::PROTOCOL_VERSION,
            )));
        }
        None => {
            return Err(HarnessError::protocol(format!(
                "norn driven-mode protocol mismatch: expected {:?}, received no `{}` field — \
                 the norn binary is stale or incompatible with this adapter",
                protocol::PROTOCOL_VERSION,
                protocol::PROTOCOL_KEY,
            )));
        }
    }
    let labels = initialize_result
        .get(protocol::CAPABILITIES_KEY)
        .and_then(|caps| caps.get(protocol::INTERVENTIONS_KEY))
        .and_then(Value::as_array);
    let Some(labels) = labels else {
        return Ok(InterventionCapabilities::none());
    };
    let primitives = labels
        .iter()
        .filter_map(Value::as_str)
        .filter_map(capability_label_to_primitive);
    Ok(InterventionCapabilities::from_primitives(primitives))
}

/// Interprets a `run/execute` success result — the versioned stop envelope — into the canonical
/// [`AgentOutcome`] record, or the honest error a non-completion (or an off-contract output) is.
///
/// - `stop.reason == "completed"` with a **String** `output` → `Ok(AgentOutcome)`: the string is
///   the record's `text`, and `stop_reason` is the canonical spelling of the completed run
///   (see [`completed_stop_kind`] — norn's real `stop.reason`, mapped, never invented).
/// - `stop.reason == "completed"` with a **non-String** `output` (an object, array, number, bool,
///   or null — a run configured with an output schema, or a null final output) →
///   [`HarnessError::Contract`] NAMING the canonical agent-outcome contract and what was found.
///   The old behaviour passed the value through verbatim and left a workflow-side decoder to
///   choke on it ("Expected String, found Dict" — run `cc1d82c2`); the ruling on #181 makes that
///   a typed contract refusal at THIS boundary, never a serde decode error downstream. It is the
///   `Contract` variant — not `Protocol` — because it is DETERMINISTIC: the worker maps it to a
///   terminal activity failure, so a permanent misconfiguration never burns a retry budget of
///   live agent runs.
/// - any other `stop.reason` (`schema_unreachable`, `max_iterations`, `timed_out`, `cancelled`,
///   `truncated`) → [`HarnessError::Harness`] whose message carries the whole `stop` object
///   verbatim, so the reason AND its per-variant detail fields (`elapsed_ms`, `attempts`,
///   `validation_errors`, `truncation`, …) survive into the error text the caller judges retry
///   on. When the envelope also carries a non-null `output` (the partial a `timed_out` /
///   `truncated` / `schema_unreachable` stop may hold), it rides in the same message, labelled
///   `partial output:` and bounded — a partial longer than the bound is cut with an explicit
///   `partial output truncated for this message` marker naming the shown and total byte counts, so
///   a caller's accept-the-partial policy stays reachable and never mistakes a cut for the whole.
/// - a `completed` envelope with the `output` KEY ABSENT → [`HarnessError::Protocol`] — the
///   contract says a completed run always carries `output`.
/// - a result that is not the envelope shape (no `envelope_version` / `stop` / `stop.reason`) →
///   [`HarnessError::Protocol`] naming what was missing — no silent passthrough of unknown
///   shapes.
///
/// # Errors
///
/// Returns [`HarnessError::Protocol`] for a non-envelope result or a completed envelope without
/// an `output` field, [`HarnessError::Contract`] for a completed `output` the agent-outcome
/// contract cannot carry, and [`HarnessError::Harness`] for every non-`completed` stop.
pub fn run_result_to_outcome(result: &Value) -> Result<AgentOutcome, HarnessError> {
    if result.get(protocol::ENVELOPE_VERSION_KEY).is_none() {
        return Err(HarnessError::protocol(format!(
            "run/execute result is not a norn stop envelope: missing `{}`",
            protocol::ENVELOPE_VERSION_KEY,
        )));
    }
    let stop = result.get(protocol::STOP_KEY).ok_or_else(|| {
        HarnessError::protocol(format!(
            "run/execute result is not a norn stop envelope: missing `{}`",
            protocol::STOP_KEY,
        ))
    })?;
    let reason = stop
        .get(protocol::STOP_REASON_KEY)
        .and_then(Value::as_str)
        .ok_or_else(|| {
            HarnessError::protocol(format!(
                "run/execute result is not a norn stop envelope: `{}` carries no string `{}`",
                protocol::STOP_KEY,
                protocol::STOP_REASON_KEY,
            ))
        })?;
    if reason != protocol::STOP_REASON_COMPLETED {
        // The whole stop object goes into the message so the reason and its per-variant detail
        // fields (timed_out{elapsed_ms,iterations}, schema_unreachable{attempts,
        // validation_errors}, truncated{truncation,iterations}) survive verbatim for the caller
        // to judge retry on. A partial `output` rides along, clearly labelled, so accepting the
        // partial stays a reachable caller policy.
        let mut message = format!("run stopped without completing: stop: {stop}");
        if let Some(partial) = result
            .get(protocol::OUTPUT_KEY)
            .filter(|output| !output.is_null())
        {
            message.push_str("; partial output: ");
            message.push_str(&bounded_for_message(&partial.to_string()));
        }
        return Err(HarnessError::harness(message));
    }
    // The contract says a completed envelope ALWAYS carries `output`; a frame without the key is
    // off-contract.
    let output = result
        .get(protocol::OUTPUT_KEY)
        .ok_or_else(|| HarnessError::protocol("completed envelope carried no output field"))?;
    // The agent seam's completed result is the canonical agent-outcome record, and its `text` is
    // a String. Anything else — the structured output of a run configured with an output schema,
    // or a null final output — cannot fill the record, and refusing it HERE, naming the contract,
    // is the ruling's cure for the workflow-side "Expected String, found Dict" decode error.
    let Value::String(text) = output else {
        return Err(HarnessError::contract(format!(
            "the run completed but its output cannot satisfy the canonical agent-outcome \
             contract (a record of `text: String` and `stop_reason: String`): the envelope's \
             `output` is {}, not the String that becomes `text` — a norn run serving an `agent` \
             action must end with terminal text (a structured output schema has no place in \
             the agent seam)",
            json_kind(output)
        )));
    };
    Ok(AgentOutcome::new(text.clone(), &completed_stop_kind()))
}

/// The neutral stop kind a `completed` envelope maps to: [`StopKind::EndTurn`].
///
/// Norn's envelope reason `completed` means the run finished normally with its final output
/// (DRIVEN-PROTOCOL.md, "Stop envelope": exit 0, `output` holds the final value) — exactly the
/// semantics of the neutral `EndTurn` ("the agent completed its turn normally and produced its
/// result"). The stream-event mapping ([`stop_kind`]) never sees `completed` — it maps norn's
/// per-turn `event/stop` reasons (`end_turn`/`tool_use`/`max_tokens`) — so this run-level
/// resolution lives beside it, against norn's protocol document, and is pinned by
/// `a_completed_envelope_maps_to_the_canonical_end_turn_stop_reason`.
fn completed_stop_kind() -> StopKind {
    StopKind::EndTurn
}

/// A human-readable name for a JSON value's kind, for contract-refusal messages.
fn json_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a JSON boolean",
        Value::Number(_) => "a JSON number",
        Value::String(_) => "a JSON string",
        Value::Array(_) => "a JSON array",
        Value::Object(_) => "a JSON object",
    }
}

/// The most of a non-completed run's partial `output` that rides in the harness-error message.
///
/// The bound is presentational truncation of an ERROR MESSAGE, not data loss — the run has
/// already failed; it only keeps an enormous partial from bloating the persisted failure text.
const PARTIAL_OUTPUT_MESSAGE_BYTES: usize = 4000;

/// Bounds `rendered` to [`PARTIAL_OUTPUT_MESSAGE_BYTES`], appending an explicit truncation
/// marker when it was cut. The cut lands on a char boundary so the message stays valid UTF-8.
fn bounded_for_message(rendered: &str) -> String {
    if rendered.len() <= PARTIAL_OUTPUT_MESSAGE_BYTES {
        return rendered.to_owned();
    }
    let mut end = PARTIAL_OUTPUT_MESSAGE_BYTES;
    while !rendered.is_char_boundary(end) {
        end -= 1;
    }
    format!(
        "{}… [partial output truncated for this message: {} of {} bytes shown]",
        &rendered[..end],
        end,
        rendered.len()
    )
}

/// Maps a neutral capability label Norn advertises onto its [`InterventionPrimitive`].
///
/// Returns `None` for a label this neutral set does not enumerate, so an unknown label is ignored
/// rather than fabricated into a primitive.
fn capability_label_to_primitive(label: &str) -> Option<InterventionPrimitive> {
    match label {
        protocol::CAPABILITY_INJECT_MESSAGE => Some(InterventionPrimitive::InjectMessage),
        protocol::CAPABILITY_CANCEL => Some(InterventionPrimitive::Cancel),
        protocol::CAPABILITY_PAUSE_RESUME => Some(InterventionPrimitive::PauseResume),
        protocol::CAPABILITY_UPDATE_BUDGET => Some(InterventionPrimitive::UpdateBudget),
        protocol::CAPABILITY_RESPOND_TO_APPROVAL => Some(InterventionPrimitive::RespondToApproval),
        _ => None,
    }
}

/// The Norn `intervene/*` request a neutral command maps onto: a method plus its params.
///
/// Only the two primitives Norn advertises produce a request; the other three are rejected before
/// this returns, so a request is never built for an unsupported primitive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NornRequest {
    /// The Norn `intervene/*` method name.
    pub method: &'static str,
    /// The request params.
    pub params: Value,
}

/// Maps a neutral [`InterventionKind`] into the Norn `intervene/*` request that carries it.
///
/// `InjectMessage` → `intervene/injectMessage` (with a `text` + neutral `priority`), `Cancel` →
/// `intervene/cancel` (with a `reason`). The three primitives Norn does not advertise
/// (`PauseResume`, `UpdateBudget`, `RespondToApproval`) are rejected with
/// [`HarnessError::CapabilityNotSupported`] and NEVER sent — the adapter does not fabricate a
/// Norn mechanism that does not exist.
///
/// # Errors
///
/// Returns [`HarnessError::CapabilityNotSupported`] for a primitive Norn does not support.
pub fn intervention_to_request(kind: &InterventionKind) -> Result<NornRequest, HarnessError> {
    match kind {
        InterventionKind::InjectMessage { text, priority } => {
            let priority_label = match priority {
                aion_core::InjectPriority::Interrupt => protocol::PRIORITY_INTERRUPT,
                aion_core::InjectPriority::Normal => protocol::PRIORITY_NORMAL,
            };
            Ok(NornRequest {
                method: protocol::METHOD_INTERVENE_INJECT,
                params: json!({
                    protocol::PARAM_TEXT: text,
                    protocol::PARAM_PRIORITY: priority_label,
                }),
            })
        }
        InterventionKind::Cancel { reason } => Ok(NornRequest {
            method: protocol::METHOD_INTERVENE_CANCEL,
            params: json!({ protocol::PARAM_REASON: reason }),
        }),
        InterventionKind::PauseResume { .. }
        | InterventionKind::UpdateBudget { .. }
        | InterventionKind::RespondToApproval { .. } => Err(
            HarnessError::capability_not_supported(primitive_label(kind.primitive())),
        ),
    }
}

/// A neutral, human-readable label for a primitive (used in the capability-gate rejection).
fn primitive_label(primitive: InterventionPrimitive) -> &'static str {
    match primitive {
        InterventionPrimitive::InjectMessage => protocol::CAPABILITY_INJECT_MESSAGE,
        InterventionPrimitive::Cancel => protocol::CAPABILITY_CANCEL,
        InterventionPrimitive::PauseResume => protocol::CAPABILITY_PAUSE_RESUME,
        InterventionPrimitive::UpdateBudget => protocol::CAPABILITY_UPDATE_BUDGET,
        InterventionPrimitive::RespondToApproval => protocol::CAPABILITY_RESPOND_TO_APPROVAL,
    }
}

/// Maps a Norn `event/*` notification (`method` + `params`) into a neutral [`ActivityEvent`].
///
/// The `params` are Norn's native per-event payload (from `agent_event_to_value`) with
/// `agent_id` / `agent_role` added. The `method` carries the coarse category; the params' `type`
/// field carries the fine-grained native label. Any shape the classifier does not recognise falls
/// through to [`ActivityEventKind::Raw`] carrying the method as its source, so nothing is dropped.
#[must_use]
pub fn notification_to_event(
    identity: &EventIdentity,
    worker_seq: u64,
    method: &str,
    params: &Value,
) -> ActivityEvent {
    let (agent_id, agent_role) = event_attribution(params);
    let (kind, ephemeral) = classify_event(method, params);
    ActivityEvent {
        workflow_id: identity.workflow_id.clone(),
        run_id: identity.run_id.clone(),
        activity_id: identity.activity_id.clone(),
        attempt: identity.attempt,
        agent_id,
        agent_role,
        emitted_at: Utc::now(),
        worker_seq,
        store_seq: None,
        ephemeral,
        kind,
    }
}

/// Reads the `agent_id` (a UUID string) and `agent_role` from a notification's params.
///
/// A missing/unparseable `agent_id` falls back to the nil UUID and a missing `agent_role` to an
/// empty label — attribution is best-effort and never fails the event.
fn event_attribution(params: &Value) -> (Uuid, String) {
    let agent_id = params
        .get(protocol::EVENT_AGENT_ID)
        .and_then(Value::as_str)
        .and_then(|raw| Uuid::parse_str(raw).ok())
        .unwrap_or(Uuid::nil());
    let agent_role = params
        .get(protocol::EVENT_AGENT_ROLE)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    (agent_id, agent_role)
}

/// Classifies a notification into an [`ActivityEventKind`] and whether it is ephemeral.
///
/// Dispatches on the coarse `event/*` method, then refines on the params' native `type` label.
/// Per-token streams (text/thinking deltas, `tool_call_delta`) are the ephemeral arms
/// (WS-forward only, never persisted).
fn classify_event(method: &str, params: &Value) -> (ActivityEventKind, bool) {
    let event_type = params.get(protocol::EVENT_TYPE).and_then(Value::as_str);
    match method {
        "event/message" => (message_kind(event_type, params), false),
        "event/toolCall" => (tool_call_kind(params), false),
        "event/toolResult" => (tool_result_kind(params), false),
        "event/progress" => progress_kind(event_type, params),
        "event/stop" => (stop_kind(params), false),
        _ if is_per_token_type(event_type) => {
            // A provider-native per-token frame surfacing through an
            // unclassified method (e.g. `event/raw` carrying
            // `response.function_call_arguments.delta`): same class as the
            // `event/progress` delta arms, so same treatment — an ephemeral
            // note with the payload dropped. Persisting these wrote the
            // finalized turn's content thousands of times over.
            let detail = ProgressDetail::Note {
                text: event_type.unwrap_or_default().to_owned(),
            };
            (ActivityEventKind::Progress { detail }, true)
        }
        _ => (raw_kind(method, params), false),
    }
}

/// Whether a native event `type` label names a per-token streaming frame.
///
/// Providers stream deltas under dotted `*.delta` labels (`response.output_text.delta`,
/// `response.function_call_arguments.delta`, ...). One frame per token: never
/// worth persisting, whatever method it arrived under.
fn is_per_token_type(event_type: Option<&str>) -> bool {
    event_type.is_some_and(|label| label.contains('.') && label.rsplit('.').next() == Some("delta"))
}

/// A required string field, or an empty string when absent — string fields are never fatal.
fn string_field(params: &Value, key: &str) -> String {
    params
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

/// Builds a [`ActivityEventKind::Message`] from an `event/message` notification.
///
/// Every Norn `event/message` is agent-produced (assistant text/thinking, or an inter-agent
/// message lifecycle), so the neutral role is always [`MessageRole::Assistant`]; an operator
/// injection is a distinct intervention, never an `event/message`. The message text is `text`
/// (assistant output) or the `content` of a message-lifecycle event when present.
fn message_kind(_event_type: Option<&str>, params: &Value) -> ActivityEventKind {
    let text = params
        .get("text")
        .or_else(|| params.get("content"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    ActivityEventKind::Message {
        role: MessageRole::Assistant,
        text,
    }
}

/// Builds a [`ActivityEventKind::ToolCall`] from an `event/toolCall` notification.
fn tool_call_kind(params: &Value) -> ActivityEventKind {
    ActivityEventKind::ToolCall {
        tool: string_field(params, "name"),
        call_id: string_field(params, "call_id"),
        input: params.get("arguments").cloned().unwrap_or(Value::Null),
    }
}

/// Builds a [`ActivityEventKind::ToolResult`] from an `event/toolResult` notification.
fn tool_result_kind(params: &Value) -> ActivityEventKind {
    ActivityEventKind::ToolResult {
        call_id: string_field(params, "tool_call_id"),
        output: params.get("output").cloned().unwrap_or(Value::Null),
        is_error: params
            .get("is_error")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    }
}

/// Builds a progress kind (and its ephemeral flag) from an `event/progress` notification.
///
/// Token/thinking deltas map to an ephemeral [`ActivityEventKind::Delta`] and tool-argument
/// deltas to an ephemeral [`ProgressDetail::Note`]; a usage estimate maps to a persisted
/// [`ProgressDetail::UsageEstimate`]; anything else to a persisted [`ProgressDetail::Note`].
fn progress_kind(event_type: Option<&str>, params: &Value) -> (ActivityEventKind, bool) {
    match event_type {
        Some("text_delta" | "thinking_delta") => {
            let delta = ActivityEventKind::Delta {
                message_id: string_field(params, "item_id"),
                text_fragment: string_field(params, "text"),
            };
            (delta, true)
        }
        // Tool-argument streaming is per-token, like text/thinking deltas: durable
        // persistence would write thousands of rows per large tool call for data
        // the finalized tool_call event already carries in full.
        Some("tool_call_delta") => {
            let detail = ProgressDetail::Note {
                text: "tool_call_delta".to_owned(),
            };
            (ActivityEventKind::Progress { detail }, true)
        }
        Some("usage_estimate") => {
            let detail = ProgressDetail::UsageEstimate {
                input_tokens: params.get("input_tokens").and_then(Value::as_u64),
                output_tokens: params.get("output_tokens").and_then(Value::as_u64),
            };
            (ActivityEventKind::Progress { detail }, false)
        }
        _ => {
            let detail = ProgressDetail::Note {
                text: event_type.unwrap_or("progress").to_owned(),
            };
            (ActivityEventKind::Progress { detail }, false)
        }
    }
}

/// Builds a [`ActivityEventKind::Stop`] from an `event/stop` notification's `stop_reason`.
fn stop_kind(params: &Value) -> ActivityEventKind {
    let reason = match params.get("stop_reason").and_then(Value::as_str) {
        Some("end_turn") => StopKind::EndTurn,
        Some("tool_use") => StopKind::ToolUse,
        Some("max_tokens") => StopKind::LimitReached,
        Some(other) => StopKind::Other {
            reason: other.to_owned(),
        },
        None => StopKind::Other {
            reason: "unknown".to_owned(),
        },
    };
    ActivityEventKind::Stop { reason }
}

/// Builds a [`ActivityEventKind::Raw`] passthrough for an unclassified notification.
fn raw_kind(method: &str, params: &Value) -> ActivityEventKind {
    ActivityEventKind::Raw {
        source: method.to_owned(),
        value: params.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aion_core::{ActivityId, InjectPriority, WorkflowId};
    use serde_json::json;

    /// Failures are reported as errors naming what was expected, so no assertion needs `unwrap`.
    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn identity() -> EventIdentity {
        EventIdentity {
            workflow_id: WorkflowId::new(Uuid::nil()),
            run_id: aion_core::RunId::new(Uuid::from_u128(0x11)),
            activity_id: ActivityId::from_sequence_position(7),
            attempt: 3,
        }
    }

    /// The `event/*` notification params Norn emits (`agent_event_to_value` shape, with
    /// `agent_id` / `agent_role` added by its emitter).
    ///
    /// # Errors
    ///
    /// Returns an error when the fixture is not a JSON object — every `event/*` params is one.
    fn params(value: Value, agent_role: &str) -> Result<Value, Box<dyn std::error::Error>> {
        let mut object = match value {
            Value::Object(object) => object,
            other => {
                return Err(format!(
                    "an event/* params fixture must be a JSON object, got {other}"
                )
                .into());
            }
        };
        object.insert(
            "agent_id".to_owned(),
            json!("11111111-1111-1111-1111-111111111111"),
        );
        object.insert("agent_role".to_owned(), json!(agent_role));
        Ok(Value::Object(object))
    }

    // --- OUT: event/* notification -> ActivityEvent, one per kind ---

    #[test]
    fn message_notification_maps_to_message_event() -> TestResult {
        let p = params(json!({ "type": "text", "text": "hello" }), "root")?;
        let event = notification_to_event(&identity(), 0, "event/message", &p);
        assert_eq!(event.attempt, 3);
        assert_eq!(event.agent_role, "root");
        let expected_agent = Uuid::parse_str("11111111-1111-1111-1111-111111111111")
            .map_err(|error| format!("the fixture agent id must parse: {error}"))?;
        assert_eq!(event.agent_id, expected_agent);
        assert!(!event.ephemeral);
        match event.kind {
            ActivityEventKind::Message { role, text } => {
                assert_eq!(role, MessageRole::Assistant);
                assert_eq!(text, "hello");
            }
            other => return Err(format!("expected a Message event, got {other:?}").into()),
        }
        Ok(())
    }

    #[test]
    fn tool_call_notification_maps_to_tool_call_event() -> TestResult {
        let p = params(
            json!({
                "type": "tool_call",
                "call_id": "c1",
                "name": "read_file",
                "arguments": { "path": "/tmp/x" },
                "kind": "function",
            }),
            "root",
        )?;
        let event = notification_to_event(&identity(), 1, "event/toolCall", &p);
        match event.kind {
            ActivityEventKind::ToolCall {
                tool,
                call_id,
                input,
            } => {
                assert_eq!(tool, "read_file");
                assert_eq!(call_id, "c1");
                assert_eq!(input, json!({ "path": "/tmp/x" }));
            }
            other => return Err(format!("expected a ToolCall event, got {other:?}").into()),
        }
        Ok(())
    }

    #[test]
    fn tool_result_notification_maps_to_tool_result_event() -> TestResult {
        let p = params(
            json!({
                "type": "tool_result",
                "tool_call_id": "c1",
                "tool_name": "read_file",
                "output": { "bytes": 12 },
                "duration_ms": 3,
            }),
            "root",
        )?;
        let event = notification_to_event(&identity(), 2, "event/toolResult", &p);
        match event.kind {
            ActivityEventKind::ToolResult {
                call_id,
                output,
                is_error,
            } => {
                assert_eq!(call_id, "c1");
                assert_eq!(output, json!({ "bytes": 12 }));
                assert!(!is_error);
            }
            other => return Err(format!("expected a ToolResult event, got {other:?}").into()),
        }
        Ok(())
    }

    #[test]
    fn text_delta_notification_maps_to_ephemeral_delta() -> TestResult {
        let p = params(
            json!({ "type": "text_delta", "item_id": "m1", "text": "wor" }),
            "root",
        )?;
        let event = notification_to_event(&identity(), 3, "event/progress", &p);
        assert!(event.ephemeral, "a token delta is ephemeral");
        assert_eq!(event.store_seq, None);
        match event.kind {
            ActivityEventKind::Delta {
                message_id,
                text_fragment,
            } => {
                assert_eq!(message_id, "m1");
                assert_eq!(text_fragment, "wor");
            }
            other => return Err(format!("expected a Delta event, got {other:?}").into()),
        }
        Ok(())
    }

    #[test]
    fn tool_call_delta_notification_is_ephemeral() -> TestResult {
        let p = params(
            json!({ "type": "tool_call_delta", "text": "{\"pa" }),
            "root",
        )?;
        let event = notification_to_event(&identity(), 3, "event/progress", &p);
        assert!(
            event.ephemeral,
            "tool-argument deltas are per-token and must never be persisted"
        );
        assert_eq!(event.store_seq, None);
        match event.kind {
            ActivityEventKind::Progress {
                detail: ProgressDetail::Note { text },
            } => assert_eq!(text, "tool_call_delta"),
            other => return Err(format!("expected a Progress::Note event, got {other:?}").into()),
        }
        Ok(())
    }

    #[test]
    fn raw_per_token_delta_notification_is_ephemeral_and_drops_its_payload() -> TestResult {
        // The 2026-07-23 remediation-packet flood: provider-native per-token
        // frames arriving under `event/raw` were persisted whole (payload and
        // all), writing gigabytes per hour. They are the same class as the
        // `event/progress` delta arms and get the same ephemeral-note fate.
        let p = params(
            json!({
                "type": "response.function_call_arguments.delta",
                "delta": "{\"pa",
                "obfuscation": "xK9dQ2"
            }),
            "root",
        )?;
        let event = notification_to_event(&identity(), 3, "event/raw", &p);
        assert!(
            event.ephemeral,
            "per-token provider frames must never be persisted, whatever method they arrive under"
        );
        assert_eq!(event.store_seq, None);
        match event.kind {
            ActivityEventKind::Progress {
                detail: ProgressDetail::Note { text },
            } => assert_eq!(text, "response.function_call_arguments.delta"),
            other => return Err(format!("expected a Progress::Note event, got {other:?}").into()),
        }
        Ok(())
    }

    #[test]
    fn raw_non_delta_notification_stays_durable() -> TestResult {
        let p = params(json!({ "type": "response.completed", "id": "r1" }), "root")?;
        let event = notification_to_event(&identity(), 3, "event/raw", &p);
        assert!(
            !event.ephemeral,
            "unclassified non-delta frames keep the durable Raw passthrough"
        );
        match event.kind {
            ActivityEventKind::Raw { source, .. } => assert_eq!(source, "event/raw"),
            other => return Err(format!("expected a Raw event, got {other:?}").into()),
        }
        Ok(())
    }

    #[test]
    fn usage_estimate_notification_maps_to_progress() -> TestResult {
        let p = params(
            json!({ "type": "usage_estimate", "input_tokens": 100 }),
            "root",
        )?;
        let event = notification_to_event(&identity(), 4, "event/progress", &p);
        assert!(!event.ephemeral);
        match event.kind {
            ActivityEventKind::Progress {
                detail: ProgressDetail::UsageEstimate { input_tokens, .. },
            } => assert_eq!(input_tokens, Some(100)),
            other => {
                return Err(
                    format!("expected a Progress::UsageEstimate event, got {other:?}").into(),
                );
            }
        }
        Ok(())
    }

    #[test]
    fn stop_notification_maps_stop_reasons() -> TestResult {
        for (label, expected) in [
            ("end_turn", StopKind::EndTurn),
            ("tool_use", StopKind::ToolUse),
            ("max_tokens", StopKind::LimitReached),
        ] {
            let p = params(json!({ "type": "done", "stop_reason": label }), "root")?;
            let event = notification_to_event(&identity(), 5, "event/stop", &p);
            match event.kind {
                ActivityEventKind::Stop { reason } => assert_eq!(reason, expected),
                other => {
                    return Err(
                        format!("expected a Stop event for `{label}`, got {other:?}").into(),
                    );
                }
            }
        }
        Ok(())
    }

    #[test]
    fn unknown_stop_reason_falls_through_to_other() -> TestResult {
        let p = params(json!({ "type": "done", "stop_reason": "weird" }), "root")?;
        let event = notification_to_event(&identity(), 6, "event/stop", &p);
        match event.kind {
            ActivityEventKind::Stop {
                reason: StopKind::Other { reason },
            } => assert_eq!(reason, "weird"),
            other => return Err(format!("expected a Stop::Other event, got {other:?}").into()),
        }
        Ok(())
    }

    #[test]
    fn unclassifiable_notification_falls_through_to_raw() -> TestResult {
        let p = params(json!({ "type": "compaction", "item_type": "x" }), "root")?;
        let event = notification_to_event(&identity(), 7, "event/raw", &p);
        match event.kind {
            ActivityEventKind::Raw { source, value } => {
                assert_eq!(source, "event/raw");
                assert_eq!(value["type"], json!("compaction"));
            }
            other => return Err(format!("expected a Raw event, got {other:?}").into()),
        }
        Ok(())
    }

    #[test]
    fn missing_agent_id_falls_back_to_nil_never_panics() {
        // A notification with no agent attribution still yields a well-formed event.
        let event = notification_to_event(
            &identity(),
            0,
            "event/message",
            &json!({ "type": "text", "text": "hi" }),
        );
        assert_eq!(event.agent_id, Uuid::nil());
        assert_eq!(event.agent_role, "");
    }

    // --- IN: neutral InterventionKind -> Norn intervene/* request ---

    #[test]
    fn inject_interrupt_maps_to_inject_message_request() -> TestResult {
        let req = intervention_to_request(&InterventionKind::InjectMessage {
            text: "use the other module".to_owned(),
            priority: InjectPriority::Interrupt,
        })
        .map_err(|error| format!("an advertised inject must build a request: {error}"))?;
        assert_eq!(req.method, "intervene/injectMessage");
        assert_eq!(req.params["text"], json!("use the other module"));
        assert_eq!(req.params["priority"], json!("interrupt"));
        Ok(())
    }

    #[test]
    fn inject_normal_maps_to_normal_priority() -> TestResult {
        let req = intervention_to_request(&InterventionKind::InjectMessage {
            text: "context".to_owned(),
            priority: InjectPriority::Normal,
        })
        .map_err(|error| format!("an advertised inject must build a request: {error}"))?;
        assert_eq!(req.params["priority"], json!("normal"));
        Ok(())
    }

    #[test]
    fn cancel_maps_to_cancel_request() -> TestResult {
        let req = intervention_to_request(&InterventionKind::Cancel {
            reason: "operator abort".to_owned(),
        })
        .map_err(|error| format!("an advertised cancel must build a request: {error}"))?;
        assert_eq!(req.method, "intervene/cancel");
        assert_eq!(req.params["reason"], json!("operator abort"));
        Ok(())
    }

    #[test]
    fn unsupported_primitives_are_capability_not_supported_and_never_build_a_request() -> TestResult
    {
        for kind in [
            InterventionKind::PauseResume { paused: true },
            InterventionKind::UpdateBudget {
                max_tokens: Some(1),
                max_turns: None,
            },
            InterventionKind::RespondToApproval {
                call_id: "c".to_owned(),
                decision: aion_core::ApprovalDecision::Approve,
                note: None,
            },
        ] {
            let Err(error) = intervention_to_request(&kind) else {
                return Err(
                    format!("{kind:?} must be refused, never turned into a request").into(),
                );
            };
            assert!(
                matches!(error, HarnessError::CapabilityNotSupported { .. }),
                "an unsupported primitive must be capability-gated, got {error:?}"
            );
        }
        Ok(())
    }

    // --- capabilities parse (behind the protocol-version gate) ---

    #[test]
    fn parse_capabilities_reads_norns_advertised_set() -> TestResult {
        // The exact shape Norn's `initialize` result emits.
        let init = json!({
            "protocol": "norn-driven/1",
            "capabilities": { "interventions": ["inject_message", "cancel"] }
        });
        let caps = parse_capabilities(&init)
            .map_err(|error| format!("a norn-driven/1 advertisement must parse: {error}"))?;
        assert!(caps.supports_primitive(InterventionPrimitive::InjectMessage));
        assert!(caps.supports_primitive(InterventionPrimitive::Cancel));
        assert!(!caps.supports_primitive(InterventionPrimitive::PauseResume));
        Ok(())
    }

    #[test]
    fn parse_capabilities_missing_array_is_empty_set() -> TestResult {
        let init = json!({ "protocol": "norn-driven/1", "capabilities": {} });
        let caps = parse_capabilities(&init).map_err(|error| {
            format!("an advertisement without an interventions array must parse: {error}")
        })?;
        assert!(
            caps.is_empty(),
            "no interventions advertised = observability-only"
        );
        Ok(())
    }

    #[test]
    fn parse_capabilities_ignores_unknown_labels() -> TestResult {
        let init = json!({
            "protocol": "norn-driven/1",
            "capabilities": { "interventions": ["inject_message", "teleport"] }
        });
        let caps = parse_capabilities(&init).map_err(|error| {
            format!("an advertisement with an unknown label must parse: {error}")
        })?;
        assert!(caps.supports_primitive(InterventionPrimitive::InjectMessage));
        // "teleport" is unknown and silently ignored, never fabricated into a primitive.
        assert_eq!(caps.supported.len(), 1);
        Ok(())
    }

    // --- the protocol-version gate ---

    #[test]
    fn missing_protocol_field_is_a_protocol_error_naming_expected_and_received() -> TestResult {
        // The pre-norn-driven/1 shape: `protocolVersion: "2.0"` and no `protocol` field. This is
        // the "your norn binary is stale" signal.
        let init = json!({
            "protocolVersion": "2.0",
            "capabilities": { "interventions": ["inject_message", "cancel"] }
        });
        let Err(error) = parse_capabilities(&init) else {
            return Err("an advertisement with no `protocol` field must be refused".into());
        };
        assert!(
            matches!(error, HarnessError::Protocol { .. }),
            "a stale advertisement is a protocol error, got {error:?}"
        );
        let message = error.to_string();
        assert!(
            message.contains("norn-driven/1"),
            "the error names the expected version: {message}"
        );
        assert!(
            message.contains("no `protocol` field"),
            "the error names what was received: {message}"
        );
        Ok(())
    }

    #[test]
    fn different_protocol_version_is_a_protocol_error_naming_expected_and_received() -> TestResult {
        let init = json!({
            "protocol": "norn-driven/2",
            "capabilities": { "interventions": ["inject_message"] }
        });
        let Err(error) = parse_capabilities(&init) else {
            return Err("a norn-driven/2 advertisement must be refused".into());
        };
        assert!(matches!(error, HarnessError::Protocol { .. }));
        let message = error.to_string();
        assert!(
            message.contains("norn-driven/1") && message.contains("norn-driven/2"),
            "the error names the expected AND received versions: {message}"
        );
        Ok(())
    }

    #[test]
    fn non_string_protocol_field_is_reported_as_the_value_received() -> TestResult {
        // A present-but-non-string `protocol` (e.g. `1`) must be reported as what actually
        // arrived, never misdescribed as a missing field.
        let Err(error) = parse_capabilities(&json!({ "protocol": 1 })) else {
            return Err("a non-string `protocol` field must be refused".into());
        };
        assert!(matches!(error, HarnessError::Protocol { .. }));
        let message = error.to_string();
        assert!(
            message.contains("received 1"),
            "the error renders the received JSON value: {message}"
        );
        assert!(
            !message.contains("no `protocol` field"),
            "a present field must not be reported as missing: {message}"
        );
        Ok(())
    }

    // --- the run/execute stop envelope ---

    #[test]
    fn completed_string_output_becomes_the_agent_outcome_record() -> TestResult {
        let envelope = json!({
            "envelope_version": 1,
            "stop": { "reason": "completed" },
            "output": "the final answer",
        });
        let outcome = run_result_to_outcome(&envelope)
            .map_err(|error| format!("a completed string output must yield the record: {error}"))?;
        assert_eq!(outcome.text, "the final answer");
        assert_eq!(outcome.stop_reason, "end_turn");
        Ok(())
    }

    /// THE DECISION, encoded as a test rather than a comment (ruling #181): norn's envelope
    /// reason `completed` maps to the canonical `end_turn`. The stream mapping ([`stop_kind`])
    /// carries no `completed` arm — it maps per-turn `event/stop` reasons — so the run-level
    /// reason is resolved against norn's own protocol document (DRIVEN-PROTOCOL.md, "Stop
    /// envelope": `completed` = the run finished normally, exit 0, `output` holds the final
    /// value), which is exactly the neutral `EndTurn` semantics. Never invented: it is the
    /// envelope's real `stop.reason`, mapped.
    #[test]
    fn a_completed_envelope_maps_to_the_canonical_end_turn_stop_reason() -> TestResult {
        let envelope = json!({
            "envelope_version": 1,
            "stop": { "reason": "completed" },
            "output": "done",
        });
        let outcome = run_result_to_outcome(&envelope)
            .map_err(|error| format!("a completed envelope must yield the record: {error}"))?;
        assert_eq!(
            outcome.stop_reason,
            StopKind::EndTurn.canonical_stop_reason(),
            "norn `completed` resolves to the canonical `end_turn`, through the one mapping"
        );
        Ok(())
    }

    /// The envelope-Dict path (ruling #181 acceptance 3 + 6, live repro `cc1d82c2`): a completed
    /// envelope whose `output` is not a String is a typed contract refusal NAMING the canonical
    /// agent-outcome contract and what was found — never a value passed through for a
    /// workflow-side decoder to fail on with "Expected String, found Dict".
    #[test]
    fn a_completed_non_string_output_is_a_contract_refusal_naming_what_was_found() -> TestResult {
        let cases = [
            (json!({ "answer": 7 }), "a JSON object"),
            (json!(["a", "b"]), "a JSON array"),
            (json!(7), "a JSON number"),
            (json!(true), "a JSON boolean"),
            (json!(null), "null"),
        ];
        for (output, found) in cases {
            let envelope = json!({
                "envelope_version": 1,
                "stop": { "reason": "completed" },
                "output": output,
            });
            let Err(error) = run_result_to_outcome(&envelope) else {
                return Err(format!(
                    "a completed `output` of {found} must be refused, never yield a record"
                )
                .into());
            };
            assert!(
                matches!(error, HarnessError::Contract { .. }),
                "the refusal is the typed (terminal) Contract variant, got {error:?}"
            );
            let message = error.to_string();
            for fragment in [
                "agent-outcome",
                "`text: String`",
                "`stop_reason: String`",
                found,
            ] {
                assert!(
                    message.contains(fragment),
                    "the refusal must name `{fragment}`: {message}"
                );
            }
        }
        Ok(())
    }

    #[test]
    fn non_completed_stop_is_a_harness_error_carrying_the_stop_object_and_partial() -> TestResult {
        let envelope = json!({
            "envelope_version": 1,
            "stop": { "reason": "timed_out", "elapsed_ms": 30000, "iterations": 4 },
            "output": "a half-written summary",
        });
        let Err(error) = run_result_to_outcome(&envelope) else {
            return Err("a timed_out envelope must be refused, never yield an output".into());
        };
        assert!(matches!(error, HarnessError::Harness { .. }));
        let message = error.to_string();
        for fragment in ["timed_out", "elapsed_ms", "30000", "iterations", "4"] {
            assert!(
                message.contains(fragment),
                "the stop detail survives verbatim ({fragment}): {message}"
            );
        }
        assert!(
            message.contains("partial output: \"a half-written summary\""),
            "the partial output rides the message, labelled: {message}"
        );
        Ok(())
    }

    #[test]
    fn non_completed_stop_with_null_output_carries_no_partial_label() -> TestResult {
        // `max_iterations`/`cancelled` envelopes carry `output: null` — nothing to preserve, so
        // the message must not fabricate a partial.
        let envelope = json!({
            "envelope_version": 1,
            "stop": { "reason": "max_iterations" },
            "output": null,
        });
        let Err(error) = run_result_to_outcome(&envelope) else {
            return Err("a max_iterations envelope must be refused, never yield an output".into());
        };
        let message = error.to_string();
        assert!(
            !message.contains("partial output"),
            "a null output is not a partial: {message}"
        );
        Ok(())
    }

    #[test]
    fn an_enormous_partial_output_is_bounded_with_an_explicit_marker() -> TestResult {
        // The bound is presentational truncation of the error MESSAGE, not data loss — the run
        // already failed. The marker makes the cut explicit.
        let huge = "x".repeat(10_000);
        let envelope = json!({
            "envelope_version": 1,
            "stop": { "reason": "truncated", "truncation": "max_tokens", "iterations": 2 },
            "output": huge,
        });
        let Err(error) = run_result_to_outcome(&envelope) else {
            return Err("a truncated envelope must be refused, never yield an output".into());
        };
        let message = error.to_string();
        assert!(
            message.contains("partial output: "),
            "the partial still rides, bounded: {message:.120}"
        );
        assert!(
            message.contains("truncated for this message"),
            "the truncation marker is explicit"
        );
        assert!(
            message.len() < 5_000,
            "the message is bounded, got {} bytes",
            message.len()
        );
        Ok(())
    }

    #[test]
    fn a_completed_envelope_without_the_output_key_is_a_protocol_error() -> TestResult {
        // The contract says completed ALWAYS carries `output`; an absent key is off-contract and
        // must never be silently coerced into a null payload.
        let envelope = json!({
            "envelope_version": 1,
            "stop": { "reason": "completed" },
        });
        let Err(error) = run_result_to_outcome(&envelope) else {
            return Err(
                "a completed envelope without `output` must be refused, never yield an output"
                    .into(),
            );
        };
        assert!(
            matches!(error, HarnessError::Protocol { .. }),
            "a completed envelope without output is a protocol error, got {error:?}"
        );
        assert!(
            error
                .to_string()
                .contains("completed envelope carried no output field"),
            "the error names the off-contract shape: {error}"
        );
        Ok(())
    }

    #[test]
    fn a_present_null_output_is_distinct_from_a_missing_key_and_refused_as_null() -> TestResult {
        // A present `"output": null` is norn's legal null output, distinct from a MISSING key (a
        // broken frame). Under the agent-outcome contract neither can complete — but the refusal
        // must name what actually arrived: `null`, never "carried no output field".
        //
        // THE CLASSIFICATION DECISION (#181 mi6), settled against norn's own protocol document
        // (DRIVEN-PROTOCOL.md, "Stop envelope" table): `null` output belongs ONLY to the
        // non-completed reasons (`max_iterations`, `cancelled`, `error`); the `completed` row
        // holds "the final output value" with no "if any" qualifier — unlike `timed_out` /
        // `schema_unreachable`, whose partials are explicitly optional. A completed envelope
        // carrying `null` is therefore configuration- or peer-determined (an output schema
        // admitting null, or an off-doc peer), never a transient property of a legitimate run —
        // so it is the DETERMINISTIC `Contract` refusal (terminal in the worker's mapping),
        // exactly like every other non-String completed output.
        let envelope = json!({
            "envelope_version": 1,
            "stop": { "reason": "completed" },
            "output": null,
        });
        let Err(error) = run_result_to_outcome(&envelope) else {
            return Err("a null output cannot fill `text: String`; it must be refused".into());
        };
        assert!(
            matches!(error, HarnessError::Contract { .. }),
            "a completed null output is the deterministic Contract refusal, got {error:?}"
        );
        let message = error.to_string();
        assert!(
            message.contains("is null"),
            "the refusal names the null that arrived: {message}"
        );
        assert!(
            !message.contains("carried no output field"),
            "a present null must not be reported as a missing key: {message}"
        );
        Ok(())
    }

    #[test]
    fn a_result_without_envelope_version_is_a_protocol_error_naming_it() -> TestResult {
        // The pre-envelope shape must never silently pass through.
        let Err(error) = run_result_to_outcome(&json!({ "result": "completed" })) else {
            return Err("a result without `envelope_version` must be refused".into());
        };
        assert!(matches!(error, HarnessError::Protocol { .. }));
        assert!(error.to_string().contains("envelope_version"));
        Ok(())
    }

    #[test]
    fn a_result_without_stop_is_a_protocol_error_naming_it() -> TestResult {
        let Err(error) = run_result_to_outcome(&json!({ "envelope_version": 1, "output": "x" }))
        else {
            return Err("an envelope without `stop` must be refused".into());
        };
        assert!(matches!(error, HarnessError::Protocol { .. }));
        assert!(error.to_string().contains("`stop`"));
        Ok(())
    }

    #[test]
    fn a_stop_without_a_string_reason_is_a_protocol_error() -> TestResult {
        let envelope = json!({ "envelope_version": 1, "stop": { "reason": 3 } });
        let Err(error) = run_result_to_outcome(&envelope) else {
            return Err("a `stop` without a string `reason` must be refused".into());
        };
        assert!(matches!(error, HarnessError::Protocol { .. }));
        assert!(error.to_string().contains("`reason`"));
        Ok(())
    }
}
