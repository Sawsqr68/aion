//! The canonical agent-outcome record — the ONE shape every agent harness
//! emits as a completed agent activity's result.
//!
//! Task #181's ruling (docs commit `c223c8c`): the `agent` marker had
//! universalized one harness path's shape (`-> String`, "parse it
//! downstream"), while the harnesses honestly emitted three different ad-hoc
//! JSON shapes. The fix is this record, defined by aion, snake_case and
//! AWL-expressible: every harness maps its native terminal outcome onto it at
//! the integration boundary, and a workflow's `agent` action declares a record
//! of exactly this shape and reads `text` / `stop_reason` / `session_id` as
//! fields.
//!
//! Task #205's ruling widened the record with a third mandatory field,
//! `session_id` — see [`AgentOutcome::session_id`] for its semantics. The
//! widening is a replacement, not an addition: the wire shape is exactly the
//! three keys, a two-field payload no longer decodes, and a two-field AWL
//! declaration no longer checks.
//!
//! # One stop-reason vocabulary, by subtraction
//!
//! The canonical `stop_reason` strings are the snake_case renderings of the
//! existing neutral [`StopKind`] semantic set — no third stop-reason table
//! exists anywhere. [`StopKind::canonical_stop_reason`] is the single mapping,
//! so the transcript channel (which carries `StopKind` itself) and this
//! outcome record cannot drift. `StopKind`'s own serialization is untouched:
//! the transcript wire is live and is a different channel from the result.
//!
//! # The record rides inside a [`Payload`]
//!
//! Nothing about the type-erased result path changes: an activity result is
//! still a [`Payload`] of `{content_type, bytes}`. The record is the JSON the
//! bytes carry for a completed agent activity.

use serde::{Deserialize, Serialize};

use crate::activity_event::StopKind;
use crate::payload::{ContentType, Payload};

impl StopKind {
    /// The canonical `snake_case` `stop_reason` string for this stop kind.
    ///
    /// This is the ONE mapping from the neutral stop vocabulary onto the
    /// [`AgentOutcome::stop_reason`] wire strings. The detail payloads of
    /// [`StopKind::Error`] and [`StopKind::Other`] deliberately do not reach
    /// the canonical string: the canonical vocabulary is closed, and the
    /// detail belongs to the transcript channel that carries the full
    /// `StopKind`.
    #[must_use]
    pub fn canonical_stop_reason(&self) -> &'static str {
        match self {
            Self::EndTurn => "end_turn",
            Self::ToolUse => "tool_use",
            Self::LimitReached => "limit_reached",
            Self::Cancelled => "cancelled",
            Self::Error { .. } => "error",
            Self::Other { .. } => "other",
        }
    }
}

/// The error serializing an [`AgentOutcome`] as its result [`Payload`].
#[derive(Debug, thiserror::Error)]
pub enum AgentOutcomeError {
    /// The record's mandatory `session_id` is the empty string.
    ///
    /// A vacant mandatory field is a harness bug, refused here by name at the
    /// one funnel every harness serializes through — never shipped as a
    /// silently empty handle a workflow would try to resume with.
    #[error(
        "the agent-outcome record's mandatory `session_id` is empty — every harness fills it \
         with the handle of the conversation that produced this outcome, and an empty handle \
         names no conversation"
    )]
    EmptySessionId,
    /// The record could not be encoded as JSON — unreachable for three plain
    /// strings, but never swallowed.
    #[error("the agent-outcome record is not encodable: {0}")]
    Encode(#[from] serde_json::Error),
}

/// The canonical agent-outcome record: what a completed agent activity's
/// result payload carries, from every harness.
///
/// Three fields, all `snake_case`, all mandatory, all AWL-expressible — the
/// AWL checker demands an `agent` action's declared return type be a record of
/// exactly this shape (`text: String`, `stop_reason: String`,
/// `session_id: String`), so a workflow reads `reply.text`,
/// `reply.stop_reason`, and `reply.session_id` as ordinary record fields.
///
/// An empty `text` on a completed run is honest, not a bug: an agent that
/// finished its turn having emitted only tool calls said nothing, and the
/// harness reports that truth rather than inventing words (the work is on the
/// transcript). `stop_reason` is one of the canonical strings produced by
/// [`StopKind::canonical_stop_reason`]:
/// `end_turn | tool_use | limit_reached | cancelled | error | other`.
#[derive(Serialize, Deserialize, ts_rs::TS, Clone, Debug, PartialEq, Eq)]
pub struct AgentOutcome {
    /// The agent's terminal text — the run's actual output.
    pub text: String,
    /// The canonical `snake_case` stop reason
    /// ([`StopKind::canonical_stop_reason`]).
    pub stop_reason: String,
    /// The handle of the conversation that produced this outcome.
    ///
    /// A FACT, not a promise: the id names the session the harness actually
    /// ran — minted, loaded, or derived by that harness — and says nothing
    /// about whether it can be resumed. Resumability is the per-agent question
    /// a harness's own handshake answers (for ACP, the `initialize` response's
    /// `loadSession` capability), never something this field asserts.
    ///
    /// The id is meaningful only to the harness that minted it; resume routing
    /// pairs the id with the harness identity the workflow record already
    /// carries. It is never empty — a harness with a vacant handle is refused
    /// at [`AgentOutcome::into_payload`], not shipped.
    pub session_id: String,
}

impl AgentOutcome {
    /// Builds the record from the agent's terminal text, the neutral stop
    /// kind, and the conversation handle, spelling `stop_reason` through the
    /// one canonical mapping.
    #[must_use]
    pub fn new(text: impl Into<String>, stop: &StopKind, session_id: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            stop_reason: stop.canonical_stop_reason().to_owned(),
            session_id: session_id.into(),
        }
    }

    /// Serializes the record as the activity's JSON result [`Payload`].
    ///
    /// # Errors
    ///
    /// [`AgentOutcomeError::EmptySessionId`] when the mandatory `session_id`
    /// is the empty string — the vacancy is refused by name rather than
    /// shipped. [`AgentOutcomeError::Encode`] when the record cannot be
    /// encoded — unreachable for three plain strings, but never swallowed.
    pub fn into_payload(self) -> Result<Payload, AgentOutcomeError> {
        if self.session_id.is_empty() {
            return Err(AgentOutcomeError::EmptySessionId);
        }
        let bytes = serde_json::to_vec(&self)?;
        Ok(Payload::new(ContentType::Json, bytes))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{AgentOutcome, AgentOutcomeError, StopKind};

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    /// The canonical vocabulary is the closed set the ruling names, spelled by
    /// exactly one mapping. Enumerated over every variant so a new `StopKind`
    /// arm cannot ship without a canonical spelling.
    #[test]
    fn every_stop_kind_has_its_canonical_snake_case_spelling() {
        let cases = [
            (StopKind::EndTurn, "end_turn"),
            (StopKind::ToolUse, "tool_use"),
            (StopKind::LimitReached, "limit_reached"),
            (StopKind::Cancelled, "cancelled"),
            (
                StopKind::Error {
                    message: "boom".to_owned(),
                },
                "error",
            ),
            (
                StopKind::Other {
                    reason: "teleported".to_owned(),
                },
                "other",
            ),
        ];
        for (kind, expected) in cases {
            assert_eq!(kind.canonical_stop_reason(), expected);
        }
    }

    /// The wire form is exactly the three `snake_case` keys the AWL record
    /// declares — this is the shape every harness emits and every workflow
    /// decodes, so it is pinned as JSON, not as a Rust round-trip alone.
    #[test]
    fn the_record_serializes_as_the_three_snake_case_fields() -> TestResult {
        let outcome = AgentOutcome::new("the answer", &StopKind::EndTurn, "sess-7");
        assert_eq!(
            serde_json::to_value(&outcome)?,
            json!({ "text": "the answer", "stop_reason": "end_turn", "session_id": "sess-7" })
        );
        Ok(())
    }

    #[test]
    fn the_record_round_trips_through_its_payload() -> TestResult {
        let outcome = AgentOutcome::new("", &StopKind::EndTurn, "sess-7");
        let payload = outcome.clone().into_payload()?;
        assert_eq!(payload.content_type(), &super::ContentType::Json);
        let decoded: AgentOutcome = serde_json::from_slice(payload.bytes())?;
        assert_eq!(decoded, outcome);
        assert_eq!(
            decoded.text, "",
            "an empty text on a completed run is honest and survives the wire"
        );
        Ok(())
    }

    /// A vacant mandatory `session_id` is refused by name at the serialization
    /// funnel — no harness can ship a silently empty handle.
    #[test]
    fn an_empty_session_id_is_refused_by_name() -> Result<(), String> {
        let error = AgentOutcome::new("the answer", &StopKind::EndTurn, "")
            .into_payload()
            .err()
            .ok_or("an empty session_id must not serialize")?;
        assert!(
            matches!(error, AgentOutcomeError::EmptySessionId),
            "the refusal is the named vacancy, got {error:?}"
        );
        assert!(
            error.to_string().contains("session_id"),
            "the refusal names the field: {error}"
        );
        Ok(())
    }

    /// A two-field payload — the pre-#205 wire shape — no longer decodes: the
    /// widening replaced the record rather than adding beside it, so a stale
    /// emitter fails loudly at the decode boundary instead of producing a
    /// record with an invented handle.
    #[test]
    fn a_two_field_payload_no_longer_decodes() {
        let stale = json!({ "text": "the answer", "stop_reason": "end_turn" });
        let decoded: Result<AgentOutcome, _> = serde_json::from_value(stale);
        assert!(
            decoded.is_err(),
            "the pre-widening shape must be refused, not defaulted"
        );
    }
}
