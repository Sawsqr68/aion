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
    ActivityEvent, ActivityEventKind, InterventionCapabilities, InterventionKind,
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

mod outcome;
pub use outcome::run_result_to_outcome;

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
mod tests;
