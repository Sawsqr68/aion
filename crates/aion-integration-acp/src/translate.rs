//! The translation point — the single place ACP's native shapes meet the neutral `aion-core` types.
//!
//! Three directions, all pure and independently tested:
//!
//! - **OUT:** [`update_to_kinds`] maps one `session/update` notification into the neutral
//!   [`ActivityEventKind`]s it produces, and [`permission_request_kind`] /
//!   [`permission_decision_kind`] / [`refused_request_kind`] map the agent's inbound requests onto
//!   the transcript.
//! - **TERMINAL:** [`stop_kind`] maps ACP's five [`StopReason`]s onto the neutral [`StopKind`],
//!   [`is_success`] states which single one of them is a completed run, and
//!   [`prompt_response_kind`] records the terminal Response frame verbatim.
//! - **HANDSHAKE:** [`gate_protocol_version`] enforces the negotiated protocol version and
//!   [`negotiated_capabilities`] states which neutral intervention primitives an ACP session can
//!   actually deliver.
//! - **IN:** [`intervention_to_notification`] maps a neutral [`InterventionKind`] onto the ACP frame
//!   that carries it. Only [`InterventionKind::Cancel`] has one; every other primitive is refused
//!   with [`HarnessError::CapabilityNotSupported`] and never reaches the wire.
//!
//! # Nothing is dropped
//!
//! Every `session/update` notification produces at least one [`ActivityEvent`]. Where a variant has
//! a faithful neutral mapping it gets one; where the neutral kinds cannot represent it, the frame
//! reaches the transcript verbatim as [`ActivityEventKind::Raw`]. Tool-call frames get **both**: the
//! classified event the console renders, and the verbatim `Raw` beside it, because the neutral
//! `ToolCall`/`ToolResult` shapes carry no `kind`, `locations`, or `title` and an audit must be able
//! to reconstruct exactly what the agent asked for. Tool frames are not per-token, so the duplicate
//! costs a bounded amount of storage; a per-token frame is never duplicated (see
//! [`crate::messages`]).
//!
//! # `#[non_exhaustive]` is load-bearing here — and it is NOT forward compatibility
//!
//! Every ACP schema type is `#[non_exhaustive]`. Each `match` in this module therefore has a
//! wildcard arm that degrades to `Raw` (or, for [`ToolKind`], to `Other`, which the protocol names
//! as the required behaviour for an unknown value). Those arms exist so that **bumping the pinned
//! schema crate** is a clean upgrade rather than a wall of non-exhaustive-match errors.
//!
//! They are not what happens when a *live agent* sends a value the pinned schema does not know.
//! Be precise about that, because the two are often confused:
//!
//! - a new **`session/update` variant** never reaches [`update_to_kinds`] at all: [`SessionUpdate`]
//!   is `#[serde(tag = "sessionUpdate")]` with no `#[serde(other)]`, so an unknown discriminator
//!   fails deserialisation in [`crate::pump`] and is recorded verbatim on the
//!   `session/update/undecodable` path. Nothing is lost, but the wildcard arm here is not the
//!   mechanism.
//! - a new **[`StopReason`]** makes the run **FAIL**: it fails [`PromptResponse`] deserialisation,
//!   which [`crate::outcome`] reports as a protocol error. The terminal result is the activity's
//!   replay-authoritative output, so guessing at an unknown stop reason is not an option — a
//!   protocol version that adds one is a version this adapter does not speak.
//!
//! [`PromptResponse`]: agent_client_protocol_schema::v1::PromptResponse
//! [`SessionUpdate`]: agent_client_protocol_schema::v1::SessionUpdate
//!
//! [`ActivityEvent`]: aion_core::ActivityEvent
//! [`ActivityEventKind`]: aion_core::ActivityEventKind
//! [`HarnessError::CapabilityNotSupported`]: aion_integrations::HarnessError::CapabilityNotSupported
//! [`InterventionKind`]: aion_core::InterventionKind
//! [`InterventionKind::Cancel`]: aion_core::InterventionKind::Cancel
//! [`ActivityEventKind::Raw`]: aion_core::ActivityEventKind::Raw
//! [`StopKind`]: aion_core::StopKind
//! [`StopReason`]: agent_client_protocol_schema::v1::StopReason
//! [`ToolKind`]: agent_client_protocol_schema::v1::ToolKind
//! [`gate_protocol_version`]: crate::translate::gate_protocol_version
//! [`intervention_to_notification`]: crate::translate::intervention_to_notification
//! [`is_success`]: crate::translate::is_success
//! [`negotiated_capabilities`]: crate::translate::negotiated_capabilities
//! [`permission_decision_kind`]: crate::translate::permission_decision_kind
//! [`permission_request_kind`]: crate::translate::permission_request_kind
//! [`prompt_response_kind`]: crate::translate::prompt_response_kind
//! [`refused_request_kind`]: crate::translate::refused_request_kind
//! [`stop_kind`]: crate::translate::stop_kind
//! [`update_to_kinds`]: crate::translate::update_to_kinds

use agent_client_protocol_schema::ProtocolVersion;
use agent_client_protocol_schema::v1::{
    CancelNotification, PermissionOption, RequestPermissionOutcome, RequestPermissionRequest,
    SessionId, SessionUpdate, StopReason, ToolCall, ToolCallStatus, ToolCallUpdate,
};
use aion_core::{
    ActivityEvent, ActivityEventKind, ActivityId, InterventionCapabilities, InterventionKind,
    InterventionPrimitive, RunId, StopKind, WorkflowId,
};
use aion_integrations::HarnessError;
use aion_integrations::jsonrpc::JsonRpcResponse;
use chrono::Utc;
use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::messages::MessageAccumulator;
use crate::permission::PermissionPolicy;
use crate::protocol;

/// One translated event: its neutral kind and whether it is ephemeral (live-only, never persisted).
pub type EventKind = (ActivityEventKind, bool);

/// The `Raw` source-label prefix for anything derived from a `session/update` notification.
pub const UPDATE_SOURCE_PREFIX: &str = "session/update";

/// Identity carried on every event a session produces, stamped from the run spec.
///
/// ACP carries no per-event producer identity (see [`protocol::AGENT_ROLE`]), so the
/// `(workflow, run, activity, attempt)` key is the whole of an event's identity; the ordering
/// fields are filled by the session's pump as a best-effort local sequence.
#[derive(Clone, Debug)]
pub struct EventIdentity {
    /// The workflow this run belongs to.
    pub workflow_id: WorkflowId,
    /// The concrete run of that workflow this attempt belongs to — stamped from the run spec,
    /// never synthesised here. It is the axis that keeps two generations of a continue-as-new
    /// chain from writing one fused transcript.
    pub run_id: RunId,
    /// The activity within the workflow.
    pub activity_id: ActivityId,
    /// The attempt number of the run.
    pub attempt: u32,
}

impl EventIdentity {
    /// Stamps one translated kind into a full [`ActivityEvent`].
    #[must_use]
    pub fn stamp(&self, worker_seq: u64, kind: EventKind) -> ActivityEvent {
        let (kind, ephemeral) = kind;
        ActivityEvent {
            workflow_id: self.workflow_id.clone(),
            run_id: self.run_id.clone(),
            activity_id: self.activity_id.clone(),
            attempt: self.attempt,
            // ACP has no sub-agent attribution: one session is one agent, and no frame carries a
            // producer id. The nil UUID is the honest "this protocol does not report one".
            agent_id: Uuid::nil(),
            agent_role: protocol::AGENT_ROLE.to_owned(),
            emitted_at: Utc::now(),
            worker_seq,
            store_seq: None,
            ephemeral,
            kind,
        }
    }
}

/// Gates the agent's echoed protocol version against the one this adapter speaks.
///
/// ACP negotiation is: the client sends its newest supported version, and the agent echoes it when
/// it supports it or answers with its own latest otherwise. A client that cannot speak the version
/// it got back MUST disconnect — this adapter speaks exactly [`protocol::PROTOCOL_VERSION`], so any
/// other value is refused here, before a session is created or a prompt is issued.
///
/// # Errors
///
/// Returns [`HarnessError::Protocol`] naming the expected and received versions when they differ.
pub fn gate_protocol_version(received: ProtocolVersion) -> Result<(), HarnessError> {
    if received == protocol::PROTOCOL_VERSION {
        return Ok(());
    }
    Err(HarnessError::protocol(format!(
        "ACP protocol version mismatch: this adapter speaks version {}, the agent answered with \
         version {} — the agent is incompatible with this client and the connection is refused",
        protocol::PROTOCOL_VERSION.as_u16(),
        received.as_u16(),
    )))
}

/// The neutral intervention primitives an ACP v1 session can actually deliver: exactly
/// [`InterventionPrimitive::Cancel`].
///
/// The advertisement is honest by construction — a primitive appears here only if this adapter can
/// map it onto a real ACP mechanism:
///
/// - **`Cancel`** → the `session/cancel` notification. Baseline in ACP v1: it is not
///   capability-gated, and the agent MUST answer the still-open `session/prompt` with
///   `stopReason: "cancelled"`. Deliverable, so advertised.
/// - **`InjectMessage`** → no mechanism. `session/prompt` is one request per turn and stays open for
///   its duration; ACP has no out-of-band way to add a user turn to a turn already running. Issuing
///   a second concurrent `session/prompt` for the same session is not defined behaviour, and
///   pretending otherwise would hand an operator a control that silently does nothing.
/// - **`PauseResume`** → no mechanism. ACP has no suspend/resume between steps.
/// - **`UpdateBudget`** → no mechanism. Token and turn limits are the agent's own configuration; ACP
///   exposes no mid-turn budget control.
/// - **`RespondToApproval`** → not routable. `session/request_permission` blocks the agent, and an
///   unattended activity must answer it immediately from its [`PermissionPolicy`]; by the time an
///   operator command could arrive the request has already been answered. Advertising it would
///   promise an approval channel that cannot be honoured.
///
/// The function takes no `agentCapabilities` argument because nothing in ACP's advertisement widens
/// or narrows this set: the whole intervention surface is fixed by the protocol version, which the
/// handshake has already gated through [`gate_protocol_version`]. The agent's advertised
/// capabilities are not discarded — [`crate::harness`] traces them at `debug` — they simply have no
/// bearing on which neutral primitives are deliverable.
#[must_use]
pub fn negotiated_capabilities() -> InterventionCapabilities {
    InterventionCapabilities::from_primitives([InterventionPrimitive::Cancel])
}

/// The ACP frame a neutral intervention maps onto: a method name plus its params.
///
/// Only the one primitive ACP can deliver produces a frame, so a frame is never built for a
/// primitive the protocol has no mechanism for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcpNotification {
    /// The ACP method name, from [`crate::protocol`] — never a literal spelled at a call site.
    pub method: &'static str,
    /// The encoded params for that method.
    pub params: Value,
}

/// Maps a neutral [`InterventionKind`] onto the ACP notification that carries it for `session`.
///
/// [`InterventionKind::Cancel`] → the `session/cancel` notification. The other four primitives have
/// no ACP mechanism at all (see [`negotiated_capabilities`] for the per-primitive reasoning) and are
/// refused with [`HarnessError::CapabilityNotSupported`] naming the primitive, so nothing is
/// fabricated onto the wire.
///
/// This is the SINGLE authority on what an intervention can put on an ACP connection:
/// [`crate::session`] gates on the negotiated set and then sends whatever this returns, so there is
/// no second list of deliverable primitives anywhere that could drift from this one. That is what
/// makes the published registration set checkable against the adapter's real ceiling
/// ([`crate::harness::registration_capabilities`]).
///
/// The operator's cancellation `reason` is deliberately NOT carried: ACP's `session/cancel` params
/// are `{ sessionId }` and nothing else, so the reason is traced by the session instead of being
/// attached to a protocol field that does not exist.
///
/// # Errors
///
/// Returns [`HarnessError::CapabilityNotSupported`] for a primitive ACP cannot deliver, and
/// [`HarnessError::Protocol`] when the params cannot be encoded (a `serde_json` failure on a
/// schema-owned type, which would mean the pinned schema crate changed under the adapter).
pub fn intervention_to_notification(
    session: &SessionId,
    kind: &InterventionKind,
) -> Result<AcpNotification, HarnessError> {
    match kind {
        InterventionKind::Cancel { .. } => {
            let params = serde_json::to_value(CancelNotification::new(session.clone())).map_err(
                |source| {
                    HarnessError::protocol(format!(
                        "session/cancel params are not encodable: {source}"
                    ))
                },
            )?;
            Ok(AcpNotification {
                method: protocol::METHOD_SESSION_CANCEL,
                params,
            })
        }
        InterventionKind::InjectMessage { .. }
        | InterventionKind::PauseResume { .. }
        | InterventionKind::UpdateBudget { .. }
        | InterventionKind::RespondToApproval { .. } => Err(
            HarnessError::capability_not_supported(primitive_label(kind.primitive())),
        ),
    }
}

/// The neutral `snake_case` label naming a primitive in a refusal.
///
/// Written out rather than derived from `Debug`, so an operator reading a rejection sees the same
/// spelling the neutral capability vocabulary uses.
pub(crate) fn primitive_label(primitive: InterventionPrimitive) -> &'static str {
    match primitive {
        InterventionPrimitive::InjectMessage => "inject_message",
        InterventionPrimitive::Cancel => "cancel",
        InterventionPrimitive::PauseResume => "pause_resume",
        InterventionPrimitive::UpdateBudget => "update_budget",
        InterventionPrimitive::RespondToApproval => "respond_to_approval",
    }
}

/// Whether an ACP stop reason means the turn COMPLETED and its output is a usable activity result.
///
/// Only [`StopReason::EndTurn`] does. `refusal`, `max_tokens`, and `max_turn_requests` are all
/// non-completions — the agent stopped short of finishing the work — and `cancelled` is a
/// cancellation. Reporting any of them as a completed result would hand a workflow a truncated or
/// refused answer as though it were the real one.
#[must_use]
pub fn is_success(reason: StopReason) -> bool {
    matches!(reason, StopReason::EndTurn)
}

/// A stable label for an ACP stop reason, used in transcript events and error messages.
///
/// The labels are the protocol's own `snake_case` wire spellings, so an operator reading an error
/// sees the same token the spec and the agent's own logs use. An unknown reason from a future
/// protocol release renders as `other`, never as a guess.
#[must_use]
pub fn stop_reason_label(reason: StopReason) -> &'static str {
    match reason {
        StopReason::EndTurn => "end_turn",
        StopReason::MaxTokens => "max_tokens",
        StopReason::MaxTurnRequests => "max_turn_requests",
        StopReason::Refusal => "refusal",
        StopReason::Cancelled => "cancelled",
        _ => "other",
    }
}

/// Maps an ACP stop reason onto the neutral [`StopKind`] for the terminal transcript event.
///
/// `max_tokens` and `max_turn_requests` are both resource ceilings and both map to
/// [`StopKind::LimitReached`]; the precise ceiling survives in the terminal error message, which
/// names the reason label. `refusal` has no neutral counterpart and keeps its raw label through
/// [`StopKind::Other`].
#[must_use]
pub fn stop_kind(reason: StopReason) -> StopKind {
    match reason {
        StopReason::EndTurn => StopKind::EndTurn,
        StopReason::MaxTokens | StopReason::MaxTurnRequests => StopKind::LimitReached,
        StopReason::Cancelled => StopKind::Cancelled,
        other => StopKind::Other {
            reason: stop_reason_label(other).to_owned(),
        },
    }
}

/// Maps one `session/update` notification's payload into the neutral events it produces.
///
/// `accumulator` carries the streamed-message state across calls (see [`crate::messages`]); the
/// chunk arms are the only ones that touch it. The returned vector is in emission order and is
/// never empty: no update is ever silently dropped.
pub fn update_to_kinds(
    update: &SessionUpdate,
    accumulator: &mut MessageAccumulator,
) -> Vec<EventKind> {
    match update {
        // All three chunk streams go through the accumulator. Every one of them is a
        // `ContentChunk`, and ACP promises nothing about how many chunks any of them arrives in, so
        // none of them may become one durable event per chunk. See [`crate::messages`] for the
        // per-stream live/durable split and for why reasoning never enters the run's result.
        SessionUpdate::AgentMessageChunk(chunk) => accumulator.push_agent_chunk(chunk),
        SessionUpdate::AgentThoughtChunk(chunk) => accumulator.push_thought_chunk(chunk),
        SessionUpdate::UserMessageChunk(chunk) => accumulator.push_user_chunk(chunk),
        SessionUpdate::ToolCall(call) => tool_call_kinds(call),
        SessionUpdate::ToolCallUpdate(update) => tool_call_update_kinds(update),
        // A plan is always a COMPLETE replacement of the previous one, and the neutral kinds model
        // no plan, so the whole entry list reaches the transcript verbatim. A partial mapping (a
        // Note naming the entry count, say) would make an operator's plan view silently wrong.
        SessionUpdate::Plan(plan) => vec![update_raw("plan", plan)],
        SessionUpdate::AvailableCommandsUpdate(commands) => {
            vec![update_raw("available_commands_update", commands)]
        }
        SessionUpdate::CurrentModeUpdate(mode) => vec![update_raw("current_mode_update", mode)],
        SessionUpdate::ConfigOptionUpdate(config) => {
            vec![update_raw("config_option_update", config)]
        }
        SessionUpdate::SessionInfoUpdate(info) => vec![update_raw("session_info_update", info)],
        // ACP's usage update reports CONTEXT-WINDOW occupancy (`used` of `size`) plus a cumulative
        // cost — not the input/output token counts the neutral UsageEstimate models. Mapping `used`
        // onto `input_tokens` would put a different quantity behind a familiar name, and `size` and
        // `cost` would be dropped outright, so the frame passes through verbatim instead.
        SessionUpdate::UsageUpdate(usage) => vec![update_raw("usage_update", usage)],
        // A variant added by a future schema release. It reaches the transcript whole rather than
        // failing the run or vanishing.
        other => vec![update_raw("unrecognised", other)],
    }
}

/// The events a `tool_call` creation frame produces.
///
/// The classified [`ActivityEventKind::ToolCall`] plus the verbatim frame. `tool` is the ACP
/// `title`, because ACP carries no tool NAME — only a human-readable title and a coarse `kind` — and
/// the title is the closest true thing to put in a field the console labels as the tool. A frame
/// that already carries a terminal status (an agent may report a whole completed call in one frame)
/// also yields the matching [`ActivityEventKind::ToolResult`].
fn tool_call_kinds(call: &ToolCall) -> Vec<EventKind> {
    let mut events = vec![(
        ActivityEventKind::ToolCall {
            tool: call.title.clone(),
            call_id: call.tool_call_id.to_string(),
            input: call.raw_input.clone().unwrap_or(Value::Null),
        },
        false,
    )];
    if let Some(is_error) = terminal_status(call.status) {
        events.push(tool_result_kind(
            &call.tool_call_id.to_string(),
            call.raw_output.as_ref(),
            &call.content,
            is_error,
        ));
    }
    events.push(update_raw("tool_call", call));
    events
}

/// The events a `tool_call_update` frame produces.
///
/// A terminal status (`completed` / `failed`) is the neutral [`ActivityEventKind::ToolResult`]. A
/// non-terminal update (a status change to `in_progress`, streamed content, a retitle) has no
/// faithful neutral kind, so the verbatim frame is the whole event. Either way the verbatim frame is
/// emitted, because an update's `content` and `locations` REPLACE the previous collections rather
/// than appending to them — a consumer reconstructing tool state needs the frame as sent.
fn tool_call_update_kinds(update: &ToolCallUpdate) -> Vec<EventKind> {
    let mut events = Vec::new();
    if let Some(is_error) = update.fields.status.and_then(terminal_status) {
        events.push(tool_result_kind(
            &update.tool_call_id.to_string(),
            update.fields.raw_output.as_ref(),
            update.fields.content.as_deref().unwrap_or_default(),
            is_error,
        ));
    }
    events.push(update_raw("tool_call_update", update));
    events
}

/// Whether a tool-call status is terminal, and if so whether it is an error.
///
/// `pending` (the status ACP assumes when the field is omitted) and `in_progress` are not terminal,
/// so no [`ActivityEventKind::ToolResult`] is fabricated for a call that has not finished.
fn terminal_status(status: ToolCallStatus) -> Option<bool> {
    match status {
        ToolCallStatus::Completed => Some(false),
        ToolCallStatus::Failed => Some(true),
        _ => None,
    }
}

/// Builds the neutral [`ActivityEventKind::ToolResult`] for a finished tool call.
///
/// The output is the ACP `rawOutput` when the agent supplied one — that is the tool's actual return
/// value — and otherwise the `content` collection the agent reported instead. An agent that supplied
/// neither yields a JSON `null` output, which is honest: the call finished and reported nothing.
fn tool_result_kind<T: Serialize + std::fmt::Debug>(
    call_id: &str,
    raw_output: Option<&Value>,
    content: &[T],
    is_error: bool,
) -> EventKind {
    let output = match raw_output {
        Some(output) => output.clone(),
        None if content.is_empty() => Value::Null,
        None => to_value_or_debug(&content),
    };
    (
        ActivityEventKind::ToolResult {
            call_id: call_id.to_owned(),
            output,
            is_error,
        },
        false,
    )
}

/// The transcript event recording an inbound `session/request_permission` — what the agent ASKED.
///
/// Emitted before the answer is written to the wire, so the transcript order is ask-then-decide even
/// if the run dies between the two.
#[must_use]
pub fn permission_request_kind(request: &RequestPermissionRequest) -> EventKind {
    raw_kind(protocol::METHOD_SESSION_REQUEST_PERMISSION, request)
}

/// The transcript event recording how a permission request was ANSWERED, and by which policy.
///
/// A policy that auto-approves is exactly the thing an operator must be able to audit after the
/// fact, so the decision is a first-class transcript entry naming the policy, the tool call it
/// applied to, the outcome sent, and the options that were on offer — enough to reconstruct why that
/// option was the one selected.
#[must_use]
pub fn permission_decision_kind(
    policy: PermissionPolicy,
    tool_call_id: &str,
    options: &[PermissionOption],
    outcome: &RequestPermissionOutcome,
) -> EventKind {
    let value = json!({
        "policy": policy.label(),
        "toolCallId": tool_call_id,
        "offered": to_value_or_debug(&options),
        "outcome": to_value_or_debug(outcome),
    });
    (
        ActivityEventKind::Raw {
            source: format!("{}/decision", protocol::METHOD_SESSION_REQUEST_PERMISSION),
            value,
        },
        false,
    )
}

/// The transcript event recording the terminal `session/prompt` Response **verbatim**.
///
/// Every other frame this adapter handles gets a verbatim `Raw` twin; the one that decides the
/// activity's result is no exception. Three things live only in the frame as sent:
///
/// - `PromptResponse._meta`, which ACP reserves for agent-supplied metadata and which the schema
///   does carry — an agent reporting a turn's cost there would otherwise lose it from both the
///   transcript and the payload,
/// - the exact JSON-RPC `error` object of a failed turn, whose `data` member the neutral
///   [`StopKind::Error`] message cannot carry,
/// - the raw `result` of a frame that does not decode as a [`PromptResponse`], which is precisely
///   the frame an operator diagnosing a broken agent needs to read.
///
/// [`PromptResponse`]: agent_client_protocol_schema::v1::PromptResponse
#[must_use]
pub fn prompt_response_kind(response: &JsonRpcResponse) -> EventKind {
    raw_kind(protocol::METHOD_SESSION_PROMPT, response)
}

/// The transcript event recording a peer request this adapter refused as an unadvertised method.
///
/// An `fs/*` or `terminal/*` request from an agent we told `false` is a protocol-conformance problem
/// on the agent's side, and it changes what the agent can do — so it belongs on the transcript, not
/// only in a log line an operator will never read next to the run.
#[must_use]
pub fn refused_request_kind(method: &str, params: Option<&Value>) -> EventKind {
    (
        ActivityEventKind::Raw {
            source: format!("refused/{method}"),
            value: json!({
                "method": method,
                "params": params.cloned().unwrap_or(Value::Null),
                "code": protocol::method_not_found_code(),
                "reason": "this client does not advertise the capability serving this method",
            }),
        },
        false,
    )
}

/// A durable [`ActivityEventKind::Raw`] carrying `value` verbatim under `source`.
///
/// Serialisation of an ACP type can fail in one narrow case — a non-finite `f64`, reachable through
/// a `usage_update`'s cost `amount` — so a failure is recorded WITH the value's `Debug` rendering
/// rather than swallowed. Nothing reaches the transcript as a bare "encoding failed".
#[must_use]
pub fn raw_kind<T: Serialize + std::fmt::Debug>(source: impl Into<String>, value: &T) -> EventKind {
    (
        ActivityEventKind::Raw {
            source: source.into(),
            value: to_value_or_debug(value),
        },
        false,
    )
}

/// [`raw_kind`] with the `session/update` source prefix applied to `suffix`.
///
/// The recorded value is the update's own PAYLOAD, not the tagged `SessionUpdate` envelope: the
/// variant is already named by the source label, so repeating the `sessionUpdate` discriminator
/// inside the value would store the same fact twice.
#[must_use]
pub fn update_raw<T: Serialize + std::fmt::Debug>(suffix: &str, value: &T) -> EventKind {
    raw_kind(format!("{UPDATE_SOURCE_PREFIX}/{suffix}"), value)
}

/// Serialises `value`, falling back to a structured record of the failure plus the value's `Debug`
/// rendering so no information is lost when JSON cannot represent it.
fn to_value_or_debug<T: Serialize + std::fmt::Debug>(value: &T) -> Value {
    match serde_json::to_value(value) {
        Ok(encoded) => encoded,
        Err(source) => json!({
            "acpEncodeError": source.to_string(),
            "debug": format!("{value:?}"),
        }),
    }
}

#[cfg(test)]
mod tests {
    use agent_client_protocol_schema::ProtocolVersion;
    use agent_client_protocol_schema::v1::{
        AvailableCommand, AvailableCommandsUpdate, ConfigOptionUpdate, ContentBlock, ContentChunk,
        CurrentModeUpdate, PermissionOption, PermissionOptionKind, Plan, PlanEntry,
        PlanEntryPriority, PlanEntryStatus, RequestPermissionOutcome, RequestPermissionRequest,
        SessionInfoUpdate, SessionUpdate, StopReason, TextContent, ToolCall, ToolCallStatus,
        ToolCallUpdate, ToolCallUpdateFields, ToolKind, UsageUpdate,
    };
    use aion_core::{
        ActivityEventKind, ActivityId, InterventionPrimitive, MessageRole, ProgressDetail, RunId,
        StopKind, WorkflowId,
    };
    use aion_integrations::HarnessError;
    use aion_integrations::jsonrpc::{JsonRpcId, JsonRpcResponse};
    use serde_json::json;
    use uuid::Uuid;

    use super::{
        EventIdentity, EventKind, gate_protocol_version, is_success, negotiated_capabilities,
        permission_decision_kind, permission_request_kind, prompt_response_kind,
        refused_request_kind, stop_kind, stop_reason_label, update_to_kinds,
    };
    use crate::messages::MessageAccumulator;
    use crate::permission::PermissionPolicy;

    /// The `source` of a `Raw` kind, or `None` for another kind.
    fn raw_source(event: &EventKind) -> Option<&str> {
        match &event.0 {
            ActivityEventKind::Raw { source, .. } => Some(source.as_str()),
            _ => None,
        }
    }

    /// Whether any event in `events` is a `Raw` with exactly `source`.
    fn has_raw(events: &[EventKind], source: &str) -> bool {
        events.iter().any(|event| raw_source(event) == Some(source))
    }

    fn text_chunk(text: &str) -> ContentChunk {
        ContentChunk::new(ContentBlock::Text(TextContent::new(text)))
    }

    /// Translate one update with a fresh accumulator.
    fn kinds(update: &SessionUpdate) -> Vec<EventKind> {
        let mut accumulator = MessageAccumulator::new();
        update_to_kinds(update, &mut accumulator)
    }

    #[test]
    fn the_matching_protocol_version_passes_the_gate() -> Result<(), HarnessError> {
        gate_protocol_version(ProtocolVersion::V1)
    }

    #[test]
    fn a_mismatched_protocol_version_is_refused_naming_both_versions() -> Result<(), String> {
        // The spec's rule: a client that cannot speak the echoed version must disconnect. The
        // refusal has to say which versions were involved or an operator cannot act on it.
        let error = gate_protocol_version(ProtocolVersion::V0)
            .err()
            .ok_or("a version mismatch must be refused")?;
        assert!(
            matches!(error, HarnessError::Protocol { .. }),
            "a version mismatch is a protocol error, got {error:?}"
        );
        let message = error.to_string();
        assert!(
            message.contains("version 1") && message.contains("version 0"),
            "the refusal names the expected AND received versions: {message}"
        );
        Ok(())
    }

    #[test]
    fn only_cancel_is_advertised() {
        let capabilities = negotiated_capabilities();
        assert!(capabilities.supports_primitive(InterventionPrimitive::Cancel));
        for unsupported in [
            InterventionPrimitive::InjectMessage,
            InterventionPrimitive::PauseResume,
            InterventionPrimitive::UpdateBudget,
            InterventionPrimitive::RespondToApproval,
        ] {
            assert!(
                !capabilities.supports_primitive(unsupported),
                "{unsupported:?} has no ACP mechanism and must not be advertised"
            );
        }
        assert_eq!(capabilities.supported.len(), 1);
    }

    #[test]
    fn only_end_turn_is_a_completed_run() {
        assert!(is_success(StopReason::EndTurn));
        for failure in [
            StopReason::MaxTokens,
            StopReason::MaxTurnRequests,
            StopReason::Refusal,
            StopReason::Cancelled,
        ] {
            assert!(
                !is_success(failure),
                "{failure:?} is not a completed run and must never be reported as success"
            );
        }
    }

    #[test]
    fn every_stop_reason_maps_to_a_kind_and_a_wire_label() {
        let cases = [
            (StopReason::EndTurn, StopKind::EndTurn, "end_turn"),
            (StopReason::MaxTokens, StopKind::LimitReached, "max_tokens"),
            (
                StopReason::MaxTurnRequests,
                StopKind::LimitReached,
                "max_turn_requests",
            ),
            (
                StopReason::Refusal,
                StopKind::Other {
                    reason: "refusal".to_owned(),
                },
                "refusal",
            ),
            (StopReason::Cancelled, StopKind::Cancelled, "cancelled"),
        ];
        for (reason, expected_kind, expected_label) in cases {
            assert_eq!(stop_kind(reason), expected_kind, "kind for {reason:?}");
            assert_eq!(stop_reason_label(reason), expected_label);
        }
    }

    #[test]
    fn an_agent_message_chunk_streams_an_ephemeral_delta() -> Result<(), String> {
        let events = kinds(&SessionUpdate::AgentMessageChunk(text_chunk("hi")));
        let [only] = events.as_slice() else {
            return Err(format!("one delta per chunk, got {events:?}"));
        };
        assert!(only.1, "a per-token chunk is ephemeral");
        assert!(matches!(only.0, ActivityEventKind::Delta { .. }));
        Ok(())
    }

    #[test]
    fn a_user_message_chunk_accumulates_into_a_durable_user_message() -> Result<(), String> {
        // Accumulated, not one durable event per chunk: `ContentChunk` is a stream type and an
        // echoing agent must not be able to write N durable events for one message.
        let mut accumulator = MessageAccumulator::new();
        let streamed = update_to_kinds(
            &SessionUpdate::UserMessageChunk(text_chunk("do the thing")),
            &mut accumulator,
        );
        assert!(
            streamed.is_empty(),
            "a user chunk emits nothing until its boundary, got {streamed:?}"
        );
        let closed = accumulator.finish();
        let [only] = closed.as_slice() else {
            return Err(format!("one message per user turn, got {closed:?}"));
        };
        assert!(!only.1, "a user turn is durable");
        match &only.0 {
            ActivityEventKind::Message { role, text } => {
                assert_eq!(*role, MessageRole::User);
                assert_eq!(text, "do the thing");
            }
            other => return Err(format!("expected Message, got {other:?}")),
        }
        Ok(())
    }

    #[test]
    fn an_agent_thought_chunk_streams_a_note_and_survives_durably() -> Result<(), String> {
        // The ruling: the flood argument justifies "not one event per token", never "no durable
        // record". Reasoning streams live AND accumulates.
        let mut accumulator = MessageAccumulator::new();
        let streamed = update_to_kinds(
            &SessionUpdate::AgentThoughtChunk(text_chunk("weighing it")),
            &mut accumulator,
        );
        let [only] = streamed.as_slice() else {
            return Err(format!("one note per thought chunk, got {streamed:?}"));
        };
        assert!(only.1, "the live reasoning stream is ephemeral");
        match &only.0 {
            ActivityEventKind::Progress {
                detail: ProgressDetail::Note { text },
            } => assert_eq!(text, "weighing it"),
            other => return Err(format!("expected Progress::Note, got {other:?}")),
        }

        let closed = accumulator.finish();
        let [durable] = closed.as_slice() else {
            return Err(format!("one durable reasoning message, got {closed:?}"));
        };
        assert!(!durable.1, "the accumulated reasoning is durable");
        match &durable.0 {
            ActivityEventKind::Message { role, text } => {
                assert_eq!(*role, MessageRole::Assistant);
                assert_eq!(text, "weighing it");
            }
            other => return Err(format!("expected Message, got {other:?}")),
        }
        assert_eq!(
            accumulator.completed_text(),
            "",
            "reasoning is never the activity's result"
        );
        Ok(())
    }

    #[test]
    fn the_terminal_response_is_recorded_verbatim_with_its_meta() -> Result<(), String> {
        // `_meta` is the schema's own extensibility member and lives nowhere else once the payload
        // is formed, so the terminal frame is recorded as sent.
        let result = json!({ "stopReason": "end_turn", "_meta": { "costUsd": 0.04 } });
        let event = prompt_response_kind(&JsonRpcResponse::success(JsonRpcId::number(9), result));
        assert_eq!(raw_source(&event), Some("session/prompt"));
        match &event.0 {
            ActivityEventKind::Raw { value, .. } => {
                assert_eq!(value["result"]["_meta"]["costUsd"], json!(0.04));
                assert_eq!(value["id"], json!(9));
            }
            other => return Err(format!("expected Raw, got {other:?}")),
        }
        assert!(!event.1, "the terminal frame is durable");
        Ok(())
    }

    #[test]
    fn a_tool_call_yields_the_classified_event_and_the_verbatim_frame() -> Result<(), String> {
        let call = ToolCall::new("t1", "Read src/main.rs")
            .kind(ToolKind::Read)
            .raw_input(json!({ "path": "src/main.rs" }));
        let events = kinds(&SessionUpdate::ToolCall(call));
        let [classified, verbatim] = events.as_slice() else {
            return Err(format!("a classified event plus the frame, got {events:?}"));
        };
        match &classified.0 {
            ActivityEventKind::ToolCall {
                tool,
                call_id,
                input,
            } => {
                assert_eq!(tool, "Read src/main.rs");
                assert_eq!(call_id, "t1");
                assert_eq!(input, &json!({ "path": "src/main.rs" }));
            }
            other => return Err(format!("expected ToolCall, got {other:?}")),
        }
        // The verbatim frame is what preserves `kind`, which the neutral ToolCall cannot carry. The
        // variant is named by the source label, so the value is the update's own payload.
        assert_eq!(raw_source(verbatim), Some("session/update/tool_call"));
        match &verbatim.0 {
            ActivityEventKind::Raw { value, .. } => {
                assert_eq!(value["kind"], json!("read"));
                assert_eq!(value["toolCallId"], json!("t1"));
                assert_eq!(value["title"], json!("Read src/main.rs"));
            }
            other => return Err(format!("expected Raw, got {other:?}")),
        }
        Ok(())
    }

    #[test]
    fn a_completed_tool_call_update_yields_a_tool_result() -> Result<(), String> {
        let update = ToolCallUpdate::new(
            "t1",
            ToolCallUpdateFields::new()
                .status(ToolCallStatus::Completed)
                .raw_output(json!({ "bytes": 42 })),
        );
        let events = kinds(&SessionUpdate::ToolCallUpdate(update));
        let [result, verbatim] = events.as_slice() else {
            return Err(format!("a result plus the frame, got {events:?}"));
        };
        match &result.0 {
            ActivityEventKind::ToolResult {
                call_id,
                output,
                is_error,
            } => {
                assert_eq!(call_id, "t1");
                assert_eq!(output, &json!({ "bytes": 42 }));
                assert!(!is_error);
            }
            other => return Err(format!("expected ToolResult, got {other:?}")),
        }
        assert_eq!(
            raw_source(verbatim),
            Some("session/update/tool_call_update")
        );
        Ok(())
    }

    #[test]
    fn a_failed_tool_call_update_marks_the_result_as_an_error() -> Result<(), String> {
        let update = ToolCallUpdate::new(
            "t2",
            ToolCallUpdateFields::new().status(ToolCallStatus::Failed),
        );
        let events = kinds(&SessionUpdate::ToolCallUpdate(update));
        let Some(first) = events.first() else {
            return Err("a failed update yields events".to_owned());
        };
        match &first.0 {
            ActivityEventKind::ToolResult {
                output, is_error, ..
            } => {
                assert!(is_error, "a failed status is an error result");
                assert_eq!(
                    output,
                    &serde_json::Value::Null,
                    "no output reported is a null output, not a fabricated one"
                );
            }
            other => return Err(format!("expected ToolResult, got {other:?}")),
        }
        Ok(())
    }

    #[test]
    fn a_non_terminal_tool_call_update_is_the_verbatim_frame_alone() -> Result<(), String> {
        // `in_progress` is not a result: fabricating a ToolResult for it would tell the console the
        // call had finished.
        let update = ToolCallUpdate::new(
            "t3",
            ToolCallUpdateFields::new().status(ToolCallStatus::InProgress),
        );
        let events = kinds(&SessionUpdate::ToolCallUpdate(update));
        let [only] = events.as_slice() else {
            return Err(format!("only the frame, got {events:?}"));
        };
        assert_eq!(raw_source(only), Some("session/update/tool_call_update"));
        Ok(())
    }

    #[test]
    fn the_remaining_variants_each_reach_the_transcript_verbatim() {
        // Every one of these is a real `session/update` variant with no faithful neutral kind. Each
        // must still produce exactly one event, under a source naming the variant.
        let cases: Vec<(SessionUpdate, &str)> = vec![
            (
                SessionUpdate::Plan(Plan::new(vec![PlanEntry::new(
                    "read the code",
                    PlanEntryPriority::High,
                    PlanEntryStatus::Pending,
                )])),
                "session/update/plan",
            ),
            (
                SessionUpdate::AvailableCommandsUpdate(AvailableCommandsUpdate::new(vec![
                    AvailableCommand::new("create_plan", "draft a plan"),
                ])),
                "session/update/available_commands_update",
            ),
            (
                SessionUpdate::CurrentModeUpdate(CurrentModeUpdate::new("architect")),
                "session/update/current_mode_update",
            ),
            (
                SessionUpdate::ConfigOptionUpdate(ConfigOptionUpdate::new(vec![])),
                "session/update/config_option_update",
            ),
            (
                SessionUpdate::SessionInfoUpdate(SessionInfoUpdate::new().title("a title")),
                "session/update/session_info_update",
            ),
            (
                SessionUpdate::UsageUpdate(UsageUpdate::new(1_024, 200_000)),
                "session/update/usage_update",
            ),
        ];
        for (update, source) in cases {
            let events = kinds(&update);
            assert!(
                has_raw(&events, source),
                "{update:?} must reach the transcript under `{source}`, got {events:?}"
            );
        }
    }

    #[test]
    fn a_plan_reaches_the_transcript_with_all_its_entries() -> Result<(), String> {
        // A plan is a COMPLETE replacement, so a partial mapping would leave a stale plan on
        // screen. The verbatim entries are what make replacement possible.
        let plan = Plan::new(vec![
            PlanEntry::new("read", PlanEntryPriority::High, PlanEntryStatus::Completed),
            PlanEntry::new("write", PlanEntryPriority::Low, PlanEntryStatus::Pending),
        ]);
        let events = kinds(&SessionUpdate::Plan(plan));
        let [only] = events.as_slice() else {
            return Err(format!("one event for a plan, got {events:?}"));
        };
        match &only.0 {
            ActivityEventKind::Raw { value, .. } => {
                let entries = value["entries"]
                    .as_array()
                    .ok_or("the plan entries survive as an array")?;
                assert_eq!(entries.len(), 2);
                assert_eq!(entries[0]["status"], json!("completed"));
            }
            other => return Err(format!("expected Raw, got {other:?}")),
        }
        Ok(())
    }

    #[test]
    fn a_permission_request_and_its_decision_are_both_recorded() -> Result<(), String> {
        let options = vec![
            PermissionOption::new("a1", "Allow once", PermissionOptionKind::AllowOnce),
            PermissionOption::new("r1", "Reject", PermissionOptionKind::RejectOnce),
        ];
        let request = RequestPermissionRequest::new(
            "sess-1",
            ToolCallUpdate::new("t1", ToolCallUpdateFields::new().title("rm -rf".to_owned())),
            options.clone(),
        );
        let asked = permission_request_kind(&request);
        assert_eq!(raw_source(&asked), Some("session/request_permission"));

        let outcome = PermissionPolicy::Deny.decide(&options);
        let decided = permission_decision_kind(PermissionPolicy::Deny, "t1", &options, &outcome);
        assert_eq!(
            raw_source(&decided),
            Some("session/request_permission/decision")
        );
        match &decided.0 {
            ActivityEventKind::Raw { value, .. } => {
                assert_eq!(value["policy"], json!("deny"));
                assert_eq!(value["toolCallId"], json!("t1"));
                assert_eq!(value["outcome"]["outcome"], json!("selected"));
                assert_eq!(value["outcome"]["optionId"], json!("r1"));
                assert_eq!(
                    value["offered"]
                        .as_array()
                        .map(Vec::len)
                        .ok_or("the offered options are recorded")?,
                    2
                );
            }
            other => return Err(format!("expected Raw, got {other:?}")),
        }
        assert!(!asked.1 && !decided.1, "an audit record is never ephemeral");
        Ok(())
    }

    #[test]
    fn a_cancelled_decision_is_recorded_as_cancelled() -> Result<(), String> {
        let decided = permission_decision_kind(
            PermissionPolicy::AllowOnce,
            "t9",
            &[],
            &RequestPermissionOutcome::Cancelled,
        );
        match &decided.0 {
            ActivityEventKind::Raw { value, .. } => {
                assert_eq!(value["outcome"]["outcome"], json!("cancelled"));
                assert_eq!(value["policy"], json!("allow_once"));
            }
            other => return Err(format!("expected Raw, got {other:?}")),
        }
        Ok(())
    }

    #[test]
    fn a_refused_peer_request_is_recorded_with_its_code() -> Result<(), String> {
        let params = json!({ "sessionId": "s1", "path": "/etc/passwd" });
        let event = refused_request_kind("fs/read_text_file", Some(&params));
        assert_eq!(raw_source(&event), Some("refused/fs/read_text_file"));
        match &event.0 {
            ActivityEventKind::Raw { value, .. } => {
                assert_eq!(value["method"], json!("fs/read_text_file"));
                assert_eq!(value["code"], json!(-32601));
                assert_eq!(value["params"]["path"], json!("/etc/passwd"));
            }
            other => return Err(format!("expected Raw, got {other:?}")),
        }
        Ok(())
    }

    #[test]
    fn stamping_fills_the_run_identity_and_the_acp_attribution() {
        let identity = EventIdentity {
            workflow_id: WorkflowId::new(Uuid::nil()),
            run_id: RunId::new(Uuid::from_u128(0x11)),
            activity_id: ActivityId::from_sequence_position(4),
            attempt: 3,
        };
        let event = identity.stamp(
            7,
            (
                ActivityEventKind::Stop {
                    reason: StopKind::EndTurn,
                },
                false,
            ),
        );
        assert_eq!(event.attempt, 3);
        assert_eq!(event.worker_seq, 7);
        assert_eq!(event.store_seq, None);
        assert!(!event.ephemeral);
        // ACP reports no producer identity; the constants say so rather than inventing one.
        assert_eq!(event.agent_id, Uuid::nil());
        assert_eq!(event.agent_role, "agent");
    }
}
