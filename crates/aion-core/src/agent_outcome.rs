//! The canonical agent-outcome record — the ONE shape every agent harness
//! emits as a completed agent activity's result.
//!
//! Task #181's ruling (docs commit `c223c8c`): the `agent` marker had
//! universalized one harness path's shape (`-> String`, "parse it
//! downstream"), while the harnesses honestly emitted three different ad-hoc
//! JSON shapes. The fix is this record, defined by aion, snake_case and
//! AWL-expressible: every harness maps its native terminal outcome onto it at
//! the integration boundary, and a workflow's `agent` action declares a record
//! of exactly this shape and reads `text` / `stop_reason` as fields.
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

/// The canonical agent-outcome record: what a completed agent activity's
/// result payload carries, from every harness.
///
/// Two fields, both `snake_case`, both AWL-expressible — the AWL checker demands
/// an `agent` action's declared return type be a record of exactly this shape
/// (`text: String`, `stop_reason: String`), so a workflow reads
/// `reply.text` and `reply.stop_reason` as ordinary record fields.
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
}

impl AgentOutcome {
    /// Builds the record from the agent's terminal text and the neutral stop
    /// kind, spelling `stop_reason` through the one canonical mapping.
    #[must_use]
    pub fn new(text: impl Into<String>, stop: &StopKind) -> Self {
        Self {
            text: text.into(),
            stop_reason: stop.canonical_stop_reason().to_owned(),
        }
    }

    /// Serializes the record as the activity's JSON result [`Payload`].
    ///
    /// # Errors
    ///
    /// Returns the `serde_json` error when the record cannot be encoded —
    /// unreachable for two plain strings, but never swallowed.
    pub fn into_payload(self) -> Result<Payload, serde_json::Error> {
        let bytes = serde_json::to_vec(&self)?;
        Ok(Payload::new(ContentType::Json, bytes))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{AgentOutcome, StopKind};

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

    /// The wire form is exactly the two `snake_case` keys the AWL record
    /// declares — this is the shape every harness emits and every workflow
    /// decodes, so it is pinned as JSON, not as a Rust round-trip alone.
    #[test]
    fn the_record_serializes_as_the_two_snake_case_fields() -> TestResult {
        let outcome = AgentOutcome::new("the answer", &StopKind::EndTurn);
        assert_eq!(
            serde_json::to_value(&outcome)?,
            json!({ "text": "the answer", "stop_reason": "end_turn" })
        );
        Ok(())
    }

    #[test]
    fn the_record_round_trips_through_its_payload() -> TestResult {
        let outcome = AgentOutcome::new("", &StopKind::EndTurn);
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
}
