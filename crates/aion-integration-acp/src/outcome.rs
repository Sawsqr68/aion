//! Interpretation of the terminal `session/prompt` Response as the activity's result.
//!
//! ACP's prompt Response carries a **stop reason and nothing else**. Two things follow, and both are
//! decided here:
//!
//! 1. **Which stop reasons are a completed run.** Exactly one — `end_turn`. `refusal`, `max_tokens`,
//!    and `max_turn_requests` are the agent stopping short of the work, and `cancelled` is a
//!    cancellation. Reporting any of them as a completed result would hand a workflow a refused or
//!    truncated answer as though it were the real one.
//! 2. **Where the answer itself comes from.** Not the Response — it has no output field. The agent's
//!    words were streamed as `agent_message_chunk` updates and reassembled by
//!    [`crate::messages::MessageAccumulator`], so both the accumulated text and the last completed
//!    answer travel here alongside the Response and become the payload's `text` and `final_message`.
//!
//! # An empty `text` on a completed turn is honest, not a bug
//!
//! A turn that ends `end_turn` having emitted only tool calls — no `agent_message_chunk` at all —
//! yields a SUCCESSFUL result whose `text` is `""`. That is the truth of what happened: the agent
//! finished its turn and said nothing, and the work it did is on the transcript as tool events. The
//! alternative — failing a turn the agent completed, or substituting the tool output for words the
//! agent never spoke — would each report something that did not happen. A caller that requires text
//! checks for it; this module will not invent it.

use agent_client_protocol_schema::v1::{PromptResponse, SessionId, StopReason};
use aion_integrations::jsonrpc::{JsonRpcError, JsonRpcResponse};
use aion_integrations::{AgentOutcome, HarnessError, Payload};

use crate::protocol;
use crate::translate;

/// The most of a non-completed turn's partial text that rides in the harness-error message.
///
/// This bounds an ERROR MESSAGE, not the data: the turn has already failed, and the same text is on
/// the transcript in full. It only keeps an enormous partial answer from bloating the persisted
/// failure text.
///
/// **The number is a matched precedent, not a fresh choice.** `aion-integration-norn` bounds the
/// same thing — a failed run's partial output riding its terminal error message — with
/// `PARTIAL_OUTPUT_MESSAGE_BYTES = 4000` (`aion-integration-norn/src/translate.rs`). Two first-party
/// adapters truncating an operator-facing failure message at two different lengths would make the
/// same failure read differently depending on which harness ran it, so this adapter takes the
/// established value rather than picking its own.
const PARTIAL_TEXT_MESSAGE_BYTES: usize = 4000;

/// How the reader pump's turn ended — the ONE signal [`AgentSession::wait_result`] receives.
///
/// The pump has no error channel of its own, and a dropped sender cannot say WHY it was dropped.
/// Naming the three ways a turn can end keeps `wait_result` from asserting a cause that did not
/// happen: before this existed, every non-answer was reported as "the agent closed the channel",
/// which is false for a reader that hit an I/O failure and false again for a pump that died with an
/// answer already in hand.
#[derive(Debug)]
pub(crate) enum PumpEnd {
    /// The id-matched `session/prompt` Response arrived and was interpreted.
    ///
    /// Boxed because the response frame it carries dwarfs the other two variants, and this enum
    /// travels through a `oneshot` channel whose buffer is sized by the largest variant.
    Answered(Box<PromptOutcome>),
    /// The reader reached end-of-stream — the agent closed its stdout — with the prompt unanswered.
    StreamEnded,
    /// The reader hit an unrecoverable transport failure with the prompt unanswered.
    TransportFailed {
        /// The transport error as the reader reported it.
        detail: String,
    },
}

/// The terminal outcome of the prompt turn, assembled by the pump and consumed by
/// [`AgentSession::wait_result`].
///
/// The accumulated text travels WITH the response rather than through shared state, so there is no
/// window in which `wait_result` could read a buffer the pump had not finished flushing.
#[derive(Debug)]
pub(crate) struct PromptOutcome {
    /// The id-matched `session/prompt` Response, exactly as the agent sent it.
    pub(crate) response: JsonRpcResponse,
    /// Every agent answer completed during the turn, joined — the run's whole output.
    pub(crate) text: String,
    /// The LAST completed agent answer alone, exactly as the agent streamed it.
    pub(crate) final_message: String,
}

impl PumpEnd {
    /// Interprets a pump end as the activity's [`Payload`], or the honest error it is.
    ///
    /// `session_id` is the session the turn ran in — minted by `session/new` or restored by
    /// `session/load` — and becomes the outcome record's `session_id`: the handle of the
    /// conversation that produced this result.
    ///
    /// # Errors
    ///
    /// [`HarnessError::Transport`] for a turn that never got an answer, naming which of the two
    /// unanswered endings happened; otherwise every error [`outcome_into_payload`] can return.
    pub(crate) fn into_payload(self, session_id: &SessionId) -> Result<Payload, HarnessError> {
        match self {
            Self::Answered(outcome) => outcome_into_payload(*outcome, session_id),
            Self::StreamEnded => Err(HarnessError::transport(
                "the ACP agent closed its stdout while session/prompt was still outstanding — it \
                 exited without answering, so the turn has no terminal result (the agent's own \
                 diagnostics are on this process's stderr)",
            )),
            Self::TransportFailed { detail } => Err(HarnessError::transport(format!(
                "the ACP agent's stdout failed while session/prompt was still outstanding, so the \
                 turn has no terminal result: {detail}"
            ))),
        }
    }
}

/// Interprets the terminal outcome as the activity's [`Payload`], or the honest error it is.
///
/// - **`end_turn`** → the completed run. The payload is the canonical [`AgentOutcome`] record,
///   `{"text": <all completed answers>, "final_message": <the last completed answer>,
///   "stop_reason": "end_turn", "session_id": <the ACP session>}` —
///   ACP's native stop reason mapped through the adapter's existing neutral
///   [`translate::stop_kind`] mapping and spelled by the one canonical
///   [`StopKind::canonical_stop_reason`](aion_integrations::StopKind::canonical_stop_reason)
///   vocabulary, and the session the agent itself minted (or reloaded) as the record's
///   conversation handle. ACP's Response carries ONLY a stop reason, so the answer itself has to
///   come from the accumulated `agent_message_chunk` text; a payload of the stop reason alone
///   would hand the workflow an activity result with no result in it.
/// - **`refusal`, `max_tokens`, `max_turn_requests`** → [`HarnessError::Harness`] naming the reason.
///   None of them is a completed run: the agent stopped short. The partial text rides along,
///   labelled and bounded, so a caller's accept-the-partial policy stays reachable.
/// - **`cancelled`** → [`HarnessError::Harness`] whose message starts with
///   [`protocol::CANCELLED_DETAIL_PREFIX`], so a cancellation is distinguishable from a failure.
/// - an **error Response** → [`HarnessError::Harness`]; code `-32000` names the authentication the
///   agent is demanding, and `-32800` is reported as a cancellation.
/// - a Response with **neither** `result` nor `error` → [`HarnessError::Protocol`]. A broken frame.
pub(crate) fn outcome_into_payload(
    outcome: PromptOutcome,
    session_id: &SessionId,
) -> Result<Payload, HarnessError> {
    let PromptOutcome {
        response,
        text,
        final_message,
    } = outcome;
    if let Some(error) = response.error {
        return Err(prompt_error(&error, &text));
    }
    let Some(result) = response.result else {
        return Err(HarnessError::protocol(
            "the session/prompt response carried neither result nor error",
        ));
    };
    let prompt: PromptResponse = serde_json::from_value(result.clone()).map_err(|source| {
        HarnessError::protocol(format!(
            "the session/prompt result is not an ACP PromptResponse ({source}): {result}"
        ))
    })?;
    let label = translate::stop_reason_label(prompt.stop_reason);
    if !translate::is_success(prompt.stop_reason) {
        return Err(non_completion(prompt.stop_reason, label, &text));
    }
    AgentOutcome::new(
        text,
        final_message,
        &translate::stop_kind(prompt.stop_reason),
        session_id.to_string(),
    )
    .into_payload()
    .map_err(|source| {
        HarnessError::protocol(format!("the terminal payload is not encodable: {source}"))
    })
}

/// The honest error for a turn that ended on a stop reason other than `end_turn`.
fn non_completion(reason: StopReason, label: &str, text: &str) -> HarnessError {
    let mut message = if matches!(reason, StopReason::Cancelled) {
        format!("{}: stopReason: {label}", protocol::CANCELLED_DETAIL_PREFIX)
    } else {
        format!("the prompt turn did not complete: stopReason: {label}")
    };
    append_partial(&mut message, text);
    HarnessError::harness(message)
}

/// The honest error for a `session/prompt` answered with a JSON-RPC error object.
fn prompt_error(error: &JsonRpcError, text: &str) -> HarnessError {
    let mut message = if error.code == protocol::auth_required_code() {
        format!(
            "the agent requires authentication before it will run (code {}): {} — an unattended \
             activity cannot complete an interactive `authenticate` flow; configure the agent's \
             credentials in the harness environment",
            error.code, error.message
        )
    } else if error.code == protocol::request_cancelled_code() {
        format!(
            "{}: the agent answered session/prompt with code {} ({}) instead of \
             stopReason `cancelled`",
            protocol::CANCELLED_DETAIL_PREFIX,
            error.code,
            error.message
        )
    } else {
        format!(
            "session/prompt failed (code {}): {}",
            error.code, error.message
        )
    };
    append_partial(&mut message, text);
    HarnessError::harness(message)
}

/// Appends the turn's partial text to a failure message, labelled and bounded.
///
/// Nothing is appended for an empty partial: fabricating a `partial output:` label for text that does
/// not exist would read as though the agent had produced something.
fn append_partial(message: &mut String, text: &str) {
    if text.is_empty() {
        return;
    }
    message.push_str("; partial output: ");
    message.push_str(&bounded_for_message(text));
}

/// Bounds `rendered` to [`PARTIAL_TEXT_MESSAGE_BYTES`], appending an explicit truncation marker when
/// it was cut. The cut lands on a char boundary so the message stays valid UTF-8.
fn bounded_for_message(rendered: &str) -> String {
    if rendered.len() <= PARTIAL_TEXT_MESSAGE_BYTES {
        return rendered.to_owned();
    }
    let mut end = PARTIAL_TEXT_MESSAGE_BYTES;
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

#[cfg(test)]
mod tests {
    use agent_client_protocol_schema::v1::SessionId;
    use aion_integrations::jsonrpc::{JsonRpcError, JsonRpcId, JsonRpcResponse};
    use serde_json::json;

    use super::{PARTIAL_TEXT_MESSAGE_BYTES, PromptOutcome, outcome_into_payload};
    use crate::protocol;
    use aion_integrations::{ContentType, HarnessError, Payload};

    /// The session every test turn runs in — the id the completed record must carry.
    const TEST_SESSION: &str = "sess-outcome-test";

    /// Runs the terminal interpretation over a success Response carrying `result`.
    fn payload_of(result: serde_json::Value, text: &str) -> Result<Payload, HarnessError> {
        outcome_into_payload(
            PromptOutcome {
                response: JsonRpcResponse::success(JsonRpcId::number(1), result),
                text: text.to_owned(),
                final_message: text.to_owned(),
            },
            &SessionId::from(TEST_SESSION.to_owned()),
        )
    }

    /// Runs the terminal interpretation over an error Response.
    fn error_of(code: i64, message: &str, text: &str) -> Result<Payload, HarnessError> {
        outcome_into_payload(
            PromptOutcome {
                response: JsonRpcResponse::failure(
                    JsonRpcId::number(1),
                    JsonRpcError::new(code, message),
                ),
                text: text.to_owned(),
                final_message: text.to_owned(),
            },
            &SessionId::from(TEST_SESSION.to_owned()),
        )
    }

    #[test]
    fn end_turn_yields_the_canonical_agent_outcome_record() -> Result<(), Box<dyn std::error::Error>>
    {
        let payload = payload_of(json!({ "stopReason": "end_turn" }), "the answer")?;
        assert_eq!(payload.content_type(), &ContentType::Json);
        assert_eq!(
            payload.to_json()?,
            json!({
                "text": "the answer",
                "final_message": "the answer",
                "stop_reason": "end_turn",
                "session_id": TEST_SESSION,
            }),
            "the completed payload is the canonical AgentOutcome record — ACP's response \
             carries only a stop reason, so the text must come from the stream, and the \
             session the turn ran in is the record's conversation handle"
        );
        Ok(())
    }

    #[test]
    fn refusal_is_never_reported_as_success() -> Result<(), String> {
        let error = payload_of(json!({ "stopReason": "refusal" }), "I will not")
            .err()
            .ok_or("a refusal must not be a completed result")?;
        assert!(
            matches!(error, HarnessError::Harness { .. }),
            "a refusal is a harness failure, got {error:?}"
        );
        let message = error.to_string();
        assert!(
            message.contains("refusal"),
            "the error names the stop reason: {message}"
        );
        assert!(
            message.contains("partial output: I will not"),
            "the partial answer rides the message, labelled: {message}"
        );
        Ok(())
    }

    #[test]
    fn each_non_completing_stop_reason_is_a_failure_naming_itself() -> Result<(), String> {
        for label in ["max_tokens", "max_turn_requests", "refusal"] {
            let error = payload_of(json!({ "stopReason": label }), "")
                .err()
                .ok_or_else(|| format!("{label} must not be a completed result"))?;
            assert!(matches!(error, HarnessError::Harness { .. }));
            let message = error.to_string();
            assert!(
                message.contains(label),
                "the error names `{label}`: {message}"
            );
            assert!(
                !message.starts_with(protocol::CANCELLED_DETAIL_PREFIX),
                "{label} is not a cancellation: {message}"
            );
            assert!(
                !message.contains("partial output"),
                "an empty partial must not be fabricated: {message}"
            );
        }
        Ok(())
    }

    #[test]
    fn cancelled_is_distinguishable_from_any_other_failure() -> Result<(), String> {
        let error = payload_of(json!({ "stopReason": "cancelled" }), "half an ans")
            .err()
            .ok_or("a cancellation is not a completed result")?;
        let message = error.to_string();
        assert!(
            message.contains(protocol::CANCELLED_DETAIL_PREFIX),
            "a cancellation carries the documented prefix so callers can match on it: {message}"
        );
        assert!(message.contains("partial output: half an ans"));
        Ok(())
    }

    #[test]
    fn an_auth_required_error_names_the_authentication_it_needs() -> Result<(), String> {
        let error = error_of(protocol::auth_required_code(), "log in first", "")
            .err()
            .ok_or("an error response is not a result")?;
        let message = error.to_string();
        assert!(
            message.contains("requires authentication"),
            "the -32000 code is translated into an actionable message: {message}"
        );
        assert!(message.contains("log in first"));
        Ok(())
    }

    #[test]
    fn a_request_cancelled_error_is_reported_as_a_cancellation() -> Result<(), String> {
        // A conforming agent answers `stopReason: cancelled`, never this code. One that does not
        // must still be reported as a cancellation rather than an opaque failure.
        let error = error_of(protocol::request_cancelled_code(), "aborted", "")
            .err()
            .ok_or("an error response is not a result")?;
        assert!(
            error.to_string().starts_with(&format!(
                "harness reported failure: {}",
                protocol::CANCELLED_DETAIL_PREFIX
            )),
            "got: {error}"
        );
        Ok(())
    }

    #[test]
    fn an_ordinary_error_response_surfaces_its_code_and_message() -> Result<(), String> {
        let error = error_of(-32603, "the model exploded", "")
            .err()
            .ok_or("an error response is not a result")?;
        let message = error.to_string();
        assert!(message.contains("-32603") && message.contains("the model exploded"));
        Ok(())
    }

    #[test]
    fn a_response_with_neither_result_nor_error_is_a_protocol_error() -> Result<(), String> {
        let response: JsonRpcResponse =
            serde_json::from_value(json!({ "jsonrpc": "2.0", "id": 1 }))
                .map_err(|source| format!("the bare frame must decode: {source}"))?;
        let error = outcome_into_payload(
            PromptOutcome {
                response,
                text: String::new(),
                final_message: String::new(),
            },
            &SessionId::from(TEST_SESSION.to_owned()),
        )
        .err()
        .ok_or("a broken frame is not a result")?;
        assert!(
            matches!(error, HarnessError::Protocol { .. }),
            "got {error:?}"
        );
        assert!(
            error.to_string().contains("neither result nor error"),
            "the error names the broken frame: {error}"
        );
        Ok(())
    }

    #[test]
    fn a_result_that_is_not_a_prompt_response_is_a_protocol_error() -> Result<(), String> {
        let error = payload_of(json!({ "stopReason": "teleported" }), "")
            .err()
            .ok_or("an unknown stop reason must not decode")?;
        assert!(
            matches!(error, HarnessError::Protocol { .. }),
            "an off-contract result is a protocol error, got {error:?}"
        );
        Ok(())
    }

    #[test]
    fn an_enormous_partial_answer_is_bounded_with_an_explicit_marker() -> Result<(), String> {
        let huge = "x".repeat(PARTIAL_TEXT_MESSAGE_BYTES * 3);
        let error = payload_of(json!({ "stopReason": "max_tokens" }), &huge)
            .err()
            .ok_or("max_tokens is not a completed result")?;
        let message = error.to_string();
        assert!(message.contains("partial output: "));
        assert!(
            message.contains("truncated for this message"),
            "the cut is explicit"
        );
        assert!(
            message.len() < PARTIAL_TEXT_MESSAGE_BYTES + 500,
            "the message stays bounded, got {} bytes",
            message.len()
        );
        Ok(())
    }
}
