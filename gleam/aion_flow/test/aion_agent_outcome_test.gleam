//// Pins the Gleam-side canonical agent-outcome record (`aion/agent`) to the
//// exact four-key wire shape the Rust harnesses emit from `wait_result`.
//// Decoding uses harness-authored field order rather than only this codec's own output.

import aion/agent.{AgentOutcome}
import gleam/string
import gleeunit/should

pub fn the_record_round_trips_through_its_codec_test() {
  let codec = agent.outcome_codec()
  let outcome =
    AgentOutcome(
      text: "narration\nthe answer",
      final_message: "the answer",
      stop_reason: "end_turn",
      session_id: "sess-roundtrip",
    )
  codec.decode(codec.encode(outcome))
  |> should.equal(Ok(outcome))
}

pub fn the_harness_wire_bytes_decode_test() {
  let codec = agent.outcome_codec()
  codec.decode(
    "{\"text\":\"narration\\nthe answer\",\"final_message\":\"the answer\",\"stop_reason\":\"end_turn\",\"session_id\":\"sess-wire-9\"}",
  )
  |> should.equal(
    Ok(AgentOutcome(
      text: "narration\nthe answer",
      final_message: "the answer",
      stop_reason: "end_turn",
      session_id: "sess-wire-9",
    )),
  )
}

pub fn empty_text_and_final_message_are_honest_test() {
  let codec = agent.outcome_codec()
  codec.decode(
    "{\"text\":\"\",\"final_message\":\"\",\"stop_reason\":\"end_turn\",\"session_id\":\"sess-quiet\"}",
  )
  |> should.equal(
    Ok(AgentOutcome(
      text: "",
      final_message: "",
      stop_reason: "end_turn",
      session_id: "sess-quiet",
    )),
  )
}

pub fn a_bare_string_is_refused_by_the_decoder_test() {
  let codec = agent.outcome_codec()
  let assert Error(failure) = codec.decode("\"just text\"")
  failure.reason
  |> string.is_empty
  |> should.be_false
}

pub fn a_three_field_payload_is_refused_by_the_decoder_test() {
  // The pre-#241 shape cannot decode: final_message is mandatory, never defaulted.
  let codec = agent.outcome_codec()
  let assert Error(failure) = codec.decode(
    "{\"text\":\"done\",\"stop_reason\":\"end_turn\",\"session_id\":\"sess-stale\"}",
  )
  failure.reason
  |> string.is_empty
  |> should.be_false
}
