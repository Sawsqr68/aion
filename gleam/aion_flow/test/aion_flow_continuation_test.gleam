//// Continue-as-new SDK contract tests against the in-process FFI double.

import aion/codec
import aion/testing
import aion/workflow
import gleam/dynamic/decode
import gleam/json
import gleam/string
import gleeunit
import gleeunit/should

pub fn main() {
  gleeunit.main()
}

pub fn continue_as_new_encodes_input_and_reaches_test_double_test() {
  let assert Ok(env) = testing.new()
  let string_codec = codec.json_codec(json.string, decode.string)

  workflow.continue_as_new("next-run", string_codec)
  |> should.equal(Ok(Nil))

  let assert Ok(observations) = testing.observations(env)
  observations
  |> string.contains("continue_as_new:\\\"next-run\\\"")
  |> should.be_true()
}
