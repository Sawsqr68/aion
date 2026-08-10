//// Pins the Gleam-side canonical agent-outcome record (`aion/agent`) to the
//// exact wire shape the Rust harnesses emit from `wait_result` — a JSON
//// object of `text` + `stop_reason`, snake_case, nothing else. The decode
//// side is exercised against harness-authored bytes (key order the Rust
//// serializer produces) rather than only against this codec's own encode, so
//// the pin cannot degenerate into a self-consistency check.

import aion/agent.{AgentOutcome}
import gleam/string
import gleeunit/should

pub fn the_record_round_trips_through_its_codec_test() {
  let codec = agent.outcome_codec()
  let outcome = AgentOutcome(text: "the answer", stop_reason: "end_turn")
  let wire = codec.encode(outcome)
  codec.decode(wire)
  |> should.equal(Ok(outcome))
}

pub fn the_harness_wire_bytes_decode_test() {
  // Bytes as the Rust side writes them (serde field order: text first).
  let codec = agent.outcome_codec()
  codec.decode("{\"text\":\"done\",\"stop_reason\":\"end_turn\"}")
  |> should.equal(Ok(AgentOutcome(text: "done", stop_reason: "end_turn")))
}

pub fn an_empty_text_on_a_completed_run_is_honest_test() {
  let codec = agent.outcome_codec()
  codec.decode("{\"text\":\"\",\"stop_reason\":\"end_turn\"}")
  |> should.equal(Ok(AgentOutcome(text: "", stop_reason: "end_turn")))
}

pub fn a_bare_string_is_refused_by_the_decoder_test() {
  // The pre-#181 wire shape must not decode: the record is the contract now.
  let codec = agent.outcome_codec()
  let assert Error(failure) = codec.decode("\"just text\"")
  failure.reason
  |> string.is_empty
  |> should.be_false
}
