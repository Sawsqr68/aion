//// The canonical agent-outcome record — the shape every agent harness emits
//// as a completed agent activity's result (task #181's ruling).
////
//// A Gleam-authored workflow that dispatches an agent activity declares its
//// output as this record and decodes it with `outcome_codec`: `text` carries
//// the agent's terminal reply (the honest empty string when the agent
//// finished a round saying nothing), `stop_reason` carries the canonical
//// snake_case stop reason
//// (`end_turn | tool_use | limit_reached | cancelled | error | other`),
//// spelled by the one Rust-side mapping in `aion-core` — this module defines
//// no vocabulary of its own — and `session_id` carries the handle of the
//// conversation that produced the outcome. The handle is a fact, not a
//// promise: it names the session the harness actually ran and says nothing
//// about whether it can be resumed (resumability is the harness's own
//// handshake question). It is meaningful only to the harness that minted it;
//// resume routing pairs it with the harness identity the workflow record
//// already carries.
////
//// This is the Gleam-side single source of the record, mirrored from
//// `aion_core::AgentOutcome`; example packages import it rather than each
//// declaring their own copy that could drift.

import aion/codec.{type Codec}
import gleam/dynamic/decode
import gleam/json

/// The canonical agent-outcome record: the agent's terminal `text`, the
/// canonical snake_case `stop_reason`, and the `session_id` conversation
/// handle.
pub type AgentOutcome {
  AgentOutcome(text: String, stop_reason: String, session_id: String)
}

/// The wire codec for the record: a JSON object of exactly
/// `{"text": <string>, "stop_reason": <string>, "session_id": <string>}`.
pub fn outcome_codec() -> Codec(AgentOutcome) {
  codec.json_codec(
    fn(outcome: AgentOutcome) {
      json.object([
        #("text", json.string(outcome.text)),
        #("stop_reason", json.string(outcome.stop_reason)),
        #("session_id", json.string(outcome.session_id)),
      ])
    },
    {
      use text <- decode.field("text", decode.string)
      use stop_reason <- decode.field("stop_reason", decode.string)
      use session_id <- decode.field("session_id", decode.string)
      decode.success(AgentOutcome(text:, stop_reason:, session_id:))
    },
  )
}
