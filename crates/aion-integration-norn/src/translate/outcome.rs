//! Translation of Norn's terminal stop envelope into the canonical agent outcome.

use aion_core::{AgentOutcome, StopKind};
use aion_integrations::HarnessError;
use serde_json::Value;

use crate::protocol;

/// Interprets a `run/execute` success result — the versioned stop envelope — into the canonical
/// [`AgentOutcome`] record, or the honest error a non-completion (or an off-contract output) is.
///
/// - `stop.reason == "completed"` with a **String** `output` → `Ok(AgentOutcome)`: the string is
///   both the record's `text` and `final_message`. This equality follows norn's own
///   DRIVEN-PROTOCOL.md "Stop envelope" contract: `output` holds the run's final value. The
///   duplication is therefore honest, not an adapter shortcut. `stop_reason` is the canonical
///   spelling of the completed run (see [`completed_stop_kind`] — norn's real `stop.reason`, mapped,
///   never invented), and
///   `session_id` is the envelope's own `session_id` — the session norn actually persisted,
///   read as a fact, never derived by this adapter. A completed envelope whose `session_id` is
///   `null` (a `--no-session` invocation — the one case the stop-envelope contract names) is a
///   deterministic [`HarnessError::Contract`] refusal: the agent-outcome record's `session_id`
///   is mandatory, and a worker configured to discard its session cannot fill it honestly. An
///   empty-string or non-string `session_id` is [`HarnessError::Protocol`] — off the
///   stop-envelope contract entirely.
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
             contract (a record of `text: String`, `final_message: String`, \
             `stop_reason: String`, and `session_id: String`): the envelope's `output` is {}, not \
             the String that becomes both `text` and `final_message` — a norn run serving an \
             `agent` action must end with terminal text (a \
             structured output schema has no place in the agent seam)",
            json_kind(output)
        )));
    };
    // The envelope's own `session_id` is the record's conversation handle — the session norn
    // actually persisted, never a value this adapter derives. The contract allows `null` there
    // ("no session was opened" — a `--no-session` invocation), but the agent-outcome record's
    // `session_id` is mandatory, so a completed run without a handle is a deterministic
    // configuration-class refusal: the same worker configuration discards its session every
    // time, and retrying would burn live agent runs against a permanent misconfiguration.
    let session_id = match result.get(protocol::SESSION_ID_KEY) {
        Some(Value::String(id)) if !id.is_empty() => id.clone(),
        Some(Value::String(_)) => {
            return Err(HarnessError::protocol(format!(
                "the run completed but the envelope's `{}` is the empty string — an id that \
                 names no session is off the stop-envelope contract (id or null, never \"\")",
                protocol::SESSION_ID_KEY
            )));
        }
        Some(Value::Null) | None => {
            return Err(HarnessError::contract(format!(
                "the run completed but its envelope carries no `{}` — the canonical \
                 agent-outcome record's `session_id` is mandatory (the handle of the \
                 conversation that produced the outcome), and norn reports null only when no \
                 session was opened; a norn worker serving an `agent` action must persist its \
                 session (do not pass `--no-session`)",
                protocol::SESSION_ID_KEY
            )));
        }
        Some(other) => {
            return Err(HarnessError::protocol(format!(
                "the run completed but the envelope's `{}` is {}, not the non-empty string the \
                 stop-envelope contract promises",
                protocol::SESSION_ID_KEY,
                json_kind(other)
            )));
        }
    };
    Ok(AgentOutcome::new(
        text.clone(),
        text.clone(),
        &completed_stop_kind(),
        session_id,
    ))
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
