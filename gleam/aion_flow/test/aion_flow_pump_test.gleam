//// Query pump loop tests against the in-process FFI double.
////
//// The scripted await (`test/aion_pump_script.erl`) plays the engine's role
//// at a yield point: enqueued `aion_query:` sentinels simulate pending
//// queries surfaced by a suspending await, and the final non-sentinel
//// result simulates the await's own resolution. The harness
//// (`test/aion_flow_ffi.erl`) records every `reply_query` /
//// `reply_query_error` attempt so tests can assert the reply channel was
//// used without touching recorded observations (queries never write
//// history).

import aion/activity
import aion/child
import aion/codec
import aion/error
import aion/internal/ffi
import aion/internal/pump
import aion/query
import aion/testing
import aion/workflow/concurrency
import gleam/dynamic/decode
import gleam/json
import gleam/string
import gleeunit
import gleeunit/should

pub fn main() {
  gleeunit.main()
}

@external(erlang, "aion_pump_script", "reset")
fn script_reset() -> Nil

@external(erlang, "aion_pump_script", "enqueue")
fn enqueue(result: Result(String, String)) -> Nil

@external(erlang, "aion_pump_script", "take")
fn scripted_await() -> Result(String, String)

@external(erlang, "aion_flow_ffi", "testing_query_replies")
fn testing_query_replies() -> Result(String, String)

@external(erlang, "aion_flow_ffi", "testing_query_diagnostics")
fn testing_query_diagnostics() -> Result(String, String)

@external(erlang, "aion_flow_ffi", "testing_enqueue_collect_result")
fn enqueue_collect(result: Result(String, String)) -> Result(String, String)

fn fresh_env() -> Nil {
  let assert Ok(_) = testing.new()
  script_reset()
}

fn query_replies() -> String {
  let assert Ok(replies) = testing_query_replies()
  replies
}

fn query_diagnostics() -> String {
  let assert Ok(diagnostics) = testing_query_diagnostics()
  diagnostics
}

fn reply_with(payload: String) -> fn(String, String) -> Result(String, String) {
  fn(query_id, _arguments) { ffi.reply_query(query_id, payload) }
}

/// Echo the caller's raw argument document back as the reply, so a test can
/// prove the arguments reached the handler rather than a default.
fn reply_with_arguments() -> fn(String, String) -> Result(String, String) {
  fn(query_id, arguments) { ffi.reply_query(query_id, arguments) }
}

pub fn pump_passes_resolved_await_through_untouched_test() {
  fresh_env()
  enqueue(Ok("resolved-value"))

  pump.run(scripted_await)
  |> should.equal(Ok("resolved-value"))

  query_replies()
  |> should.equal("[]")
}

pub fn pump_passes_non_sentinel_error_through_untouched_test() {
  fresh_env()
  enqueue(Error("timeout:deadline expired"))

  pump.run(scripted_await)
  |> should.equal(Error("timeout:deadline expired"))

  query_replies()
  |> should.equal("[]")
}

pub fn pump_services_sentinel_then_reenters_await_test() {
  fresh_env()
  ffi.register_query_handler("pump-state", reply_with("{\"answer\":42}"))
  enqueue(Error(
    "aion_query:{\"query_id\":\"q-1\",\"name\":\"pump-state\",\"arguments\":\"null\"}",
  ))
  enqueue(Ok("await-resolved"))

  pump.run(scripted_await)
  |> should.equal(Ok("await-resolved"))

  query_replies()
  |> should.equal("[\"ok:q-1:{\\\"answer\\\":42}\"]")
}

pub fn pump_services_queued_sentinels_in_order_test() {
  fresh_env()
  ffi.register_query_handler("first", reply_with("{}"))
  ffi.register_query_handler("second", reply_with("{}"))
  enqueue(Error(
    "aion_query:{\"query_id\":\"q-a\",\"name\":\"first\",\"arguments\":\"null\"}",
  ))
  enqueue(Error(
    "aion_query:{\"query_id\":\"q-b\",\"name\":\"second\",\"arguments\":\"null\"}",
  ))
  enqueue(Ok("await-resolved"))

  pump.run(scripted_await)
  |> should.equal(Ok("await-resolved"))

  query_replies()
  |> should.equal("[\"ok:q-a:{}\",\"ok:q-b:{}\"]")
}

pub fn handler_raise_replies_error_and_workflow_survives_test() {
  fresh_env()
  ffi.register_query_handler("pump-boom", fn(_query_id, _arguments) {
    panic as "handler exploded"
  })
  enqueue(Error(
    "aion_query:{\"query_id\":\"q-raise\",\"name\":\"pump-boom\",\"arguments\":\"null\"}",
  ))
  enqueue(Ok("survived"))

  pump.run(scripted_await)
  |> should.equal(Ok("survived"))

  let replies = query_replies()
  replies
  |> string.contains("error:q-raise:")
  |> should.be_true()
  replies
  |> string.contains("handler exploded")
  |> should.be_true()

  let diagnostics = query_diagnostics()
  diagnostics
  |> string.contains("query_reply_exception:operation=reply_query_exception")
  |> should.be_true()
  diagnostics
  |> string.contains("class=error reason=handler exploded stacktrace=")
  |> should.be_true()
  diagnostics
  |> string.contains(
    "aion_flow_pump_test:handler_raise_replies_error_and_workflow_survives_test@",
  )
  |> should.be_true()
  diagnostics
  |> string.contains(";aion_flow_query_pump:run_handler/3")
  |> should.be_true()
}

pub fn missing_handler_replies_error_and_workflow_survives_test() {
  fresh_env()
  enqueue(Error(
    "aion_query:{\"query_id\":\"q-ghost\",\"name\":\"ghost\",\"arguments\":\"null\"}",
  ))
  enqueue(Ok("survived"))

  pump.run(scripted_await)
  |> should.equal(Ok("survived"))

  let replies = query_replies()
  replies
  |> string.contains("error:q-ghost:")
  |> should.be_true()
  replies
  |> string.contains("no handler registered")
  |> should.be_true()
}

pub fn failed_reply_after_caller_timeout_is_non_fatal_test() {
  fresh_env()
  ffi.register_query_handler("pump-late", reply_with("{}"))
  enqueue(Error(
    "aion_query:{\"query_id\":\"dropped-1\",\"name\":\"pump-late\",\"arguments\":\"null\"}",
  ))
  enqueue(Ok("survived"))

  pump.run(scripted_await)
  |> should.equal(Ok("survived"))

  query_replies()
  |> should.equal("[\"failed:dropped-1\"]")
  query_diagnostics()
  |> string.contains(
    "query_reply_failed:operation=reply_query query_id=dropped-1 reason=unknown_query_id",
  )
  |> should.be_true()
}

pub fn failed_error_reply_after_caller_timeout_is_non_fatal_test() {
  fresh_env()
  // No handler for "ghost": the pump's reply_query_error attempt itself
  // fails (dropped caller) and must still not crash the workflow.
  enqueue(Error(
    "aion_query:{\"query_id\":\"dropped-2\",\"name\":\"ghost\",\"arguments\":\"null\"}",
  ))
  enqueue(Ok("survived"))

  pump.run(scripted_await)
  |> should.equal(Ok("survived"))

  query_replies()
  |> should.equal("[\"failed:dropped-2\"]")
  query_diagnostics()
  |> string.contains(
    "query_reply_failed:operation=reply_query_error query_id=dropped-2 reason=unknown_query_id",
  )
  |> should.be_true()
}

pub fn reply_exception_is_diagnosed_and_falls_back_to_error_reply_test() {
  fresh_env()
  ffi.register_query_handler("pump-reply-exception", reply_with("{}"))
  enqueue(Error(
    "aion_query:{\"query_id\":\"exception-1\",\"name\":\"pump-reply-exception\","
    <> "\"arguments\":\"null\"}",
  ))
  enqueue(Ok("survived"))

  pump.run(scripted_await)
  |> should.equal(Ok("survived"))

  query_replies()
  |> string.contains("error:exception-1:query handler raised error")
  |> should.be_true()
  let diagnostics = query_diagnostics()
  diagnostics
  |> string.contains(
    "operation=reply_query_exception query_id=exception-1 class=error",
  )
  |> should.be_true()
  diagnostics
  |> string.contains(
    "reason=simulated reply exception stacktrace=aion_flow_query_pump:run_handler/3",
  )
  |> should.be_true()
}

pub fn error_reply_exception_is_diagnosed_and_non_fatal_test() {
  fresh_env()
  enqueue(Error(
    "aion_query:{\"query_id\":\"error-exception-1\",\"name\":\"ghost\","
    <> "\"arguments\":\"null\"}",
  ))
  enqueue(Ok("survived"))

  pump.run(scripted_await)
  |> should.equal(Ok("survived"))

  query_replies()
  |> should.equal("[]")
  let diagnostics = query_diagnostics()
  diagnostics
  |> string.contains(
    "operation=reply_query_error_exception query_id=error-exception-1 class=error",
  )
  |> should.be_true()
  diagnostics
  |> string.contains(
    "reason=simulated error reply exception stacktrace=aion_flow_query_pump:reply_error/2",
  )
  |> should.be_true()
}

pub fn sentinel_name_with_escaped_characters_resolves_handler_test() {
  fresh_env()
  ffi.register_query_handler("say \"hi\"\tnow", reply_with("{}"))
  enqueue(Error(
    "aion_query:{\"query_id\":\"q-esc\",\"name\":\"say \\\"hi\\\"\\u0009now\","
    <> "\"arguments\":\"null\"}",
  ))
  enqueue(Ok("await-resolved"))

  pump.run(scripted_await)
  |> should.equal(Ok("await-resolved"))

  query_replies()
  |> should.equal("[\"ok:q-esc:{}\"]")
}

pub fn malformed_sentinel_without_query_id_is_skipped_test() {
  fresh_env()
  let payload = string.repeat("a", 300)
  enqueue(Error("aion_query:" <> payload))
  enqueue(Ok("survived"))

  pump.run(scripted_await)
  |> should.equal(Ok("survived"))

  // No reply channel is reachable without a query id; the engine-side
  // caller times out and the workflow keeps running.
  query_replies()
  |> should.equal("[]")

  let diagnostics = query_diagnostics()
  diagnostics
  |> string.contains("payload_bytes=300")
  |> should.be_true()
  diagnostics
  |> string.contains("prefix=" <> string.repeat("a", 256))
  |> should.be_true()
  diagnostics
  |> string.contains(string.repeat("a", 257))
  |> should.be_false()
  diagnostics
  |> string.contains("decode_reason=missing_query_id")
  |> should.be_true()
}

pub fn sentinel_missing_name_replies_malformed_error_test() {
  fresh_env()
  enqueue(Error("aion_query:{\"query_id\":\"q-noname\",\"arguments\":\"null\"}"))
  enqueue(Ok("survived"))

  pump.run(scripted_await)
  |> should.equal(Ok("survived"))

  let replies = query_replies()
  replies
  |> string.contains("error:q-noname:")
  |> should.be_true()
  replies
  |> string.contains("malformed query sentinel")
  |> should.be_true()
}

pub fn sentinel_missing_arguments_replies_malformed_error_test() {
  // The engine always emits an arguments field, so a sentinel without one is
  // a broken carrier. The pump must say so rather than run the handler with
  // an invented empty request — a handler must never mistake a protocol
  // break for a caller who asked for nothing.
  fresh_env()
  ffi.register_query_handler("pump-noargs", reply_with("{}"))
  enqueue(Error(
    "aion_query:{\"query_id\":\"q-noargs\",\"name\":\"pump-noargs\"}",
  ))
  enqueue(Ok("survived"))

  pump.run(scripted_await)
  |> should.equal(Ok("survived"))

  let replies = query_replies()
  replies
  |> string.contains("error:q-noargs:")
  |> should.be_true()
  replies
  |> string.contains("malformed query sentinel: missing_arguments")
  |> should.be_true()
  // The handler never ran: no successful reply was recorded.
  replies
  |> string.contains("ok:q-noargs")
  |> should.be_false()
}

pub fn sentinel_arguments_reach_the_handler_unescaped_test() {
  // Arguments cross the sentinel as an escaped JSON string, so a document
  // containing quotes and backslashes must arrive byte-identical — and must
  // not be able to forge the sentinel's own keys.
  fresh_env()
  ffi.register_query_handler("pump-echo", reply_with_arguments())
  enqueue(Error(
    "aion_query:{\"query_id\":\"q-args\",\"name\":\"pump-echo\",\"arguments\":"
    <> "\"{\\\"note\\\":\\\"\\\\\\\"query_id\\\\\\\":\\\\\\\"forged\\\\\\\"\\\",\\\"n\\\":3}\"}",
  ))
  enqueue(Ok("await-resolved"))

  pump.run(scripted_await)
  |> should.equal(Ok("await-resolved"))

  let replies = query_replies()
  // The reply is keyed to the real query id, not the one embedded in the
  // caller-controlled arguments.
  replies
  |> string.contains("ok:q-args:")
  |> should.be_true()
  replies
  |> string.contains("forged")
  |> should.be_true()
  replies
  |> string.contains("\\\"n\\\":3")
  |> should.be_true()
}

pub fn child_await_services_pending_query_before_resolving_test() {
  // `child.await` is a yield point like activity/signal/timer awaits: a
  // query sentinel surfaced while the parent is parked on a child terminal
  // must be serviced and the await re-entered, not surfaced as a bogus
  // child failure. The `queried-child` double yields one sentinel before
  // resolving.
  fresh_env()
  ffi.register_query_handler("child-state", reply_with("{\"pending\":1}"))

  let string_codec = codec.json_codec(json.string, decode.string)
  let assert Ok(handle) =
    child.spawn(
      "queried-child",
      fn(_input) { Ok("type-anchor-only") },
      "ignored-input",
      string_codec,
      string_codec,
      string_codec,
    )

  child.await(handle)
  |> should.equal(Ok("queried-child-receipt"))

  query_replies()
  |> should.equal("[\"ok:q-child:{\\\"pending\\\":1}\"]")
}

/// A typed echo activity over the harness mock registry: the raw mock
/// passes the encoded input payload straight through as the output.
fn echo_activity(input: String) -> activity.Activity(String, String) {
  let string_codec = codec.json_codec(json.string, decode.string)
  activity.new("echo", input, string_codec, string_codec, fn(value) {
    Ok(value)
  })
}

fn register_echo_mock() -> Nil {
  let assert Ok(_) =
    ffi.testing_register_activity_mock("echo", fn(raw_input) { Ok(raw_input) })
  Nil
}

pub fn collect_all_services_pending_query_before_resolving_test() {
  // `concurrency.all` is a yield point like the other awaits: a query
  // sentinel surfaced while the fan-out is parked must be serviced and the
  // same collect re-entered, not surfaced as a bogus activity failure.
  fresh_env()
  register_echo_mock()
  ffi.register_query_handler("collect-state", reply_with("{\"pending\":2}"))
  let assert Ok(_) =
    enqueue_collect(Error(
      "aion_query:{\"query_id\":\"q-all\",\"name\":\"collect-state\","
      <> "\"arguments\":\"null\"}",
    ))

  concurrency.all([echo_activity("alpha"), echo_activity("beta")])
  |> should.equal(Ok(["alpha", "beta"]))

  query_replies()
  |> should.equal("[\"ok:q-all:{\\\"pending\\\":2}\"]")
}

pub fn collect_race_services_pending_query_before_resolving_test() {
  fresh_env()
  register_echo_mock()
  ffi.register_query_handler("race-state", reply_with("{\"pending\":1}"))
  let assert Ok(_) =
    enqueue_collect(Error(
      "aion_query:{\"query_id\":\"q-race\",\"name\":\"race-state\","
      <> "\"arguments\":\"null\"}",
    ))

  concurrency.race([echo_activity("alpha"), echo_activity("beta")])
  |> should.equal(Ok("alpha"))

  query_replies()
  |> should.equal("[\"ok:q-race:{\\\"pending\\\":1}\"]")
}

pub fn collect_non_sentinel_error_passes_through_the_pump_test() {
  // A real collect failure must pass through `pump.run` untouched and map
  // through the activity error taxonomy, never loop the pump.
  fresh_env()
  register_echo_mock()
  let assert Ok(_) = enqueue_collect(Error("timeout:deadline expired"))

  concurrency.all([echo_activity("alpha")])
  |> should.equal(
    Error(error.ActivityTimedOut(error.TimedOut("deadline expired"))),
  )

  query_replies()
  |> should.equal("[]")
}

pub fn query_handler_registration_answers_through_pump_test() {
  fresh_env()
  let int_codec = codec.json_codec(json.int, decode.int)

  query.handler("typed-state", codec.null_codec(), int_codec, fn(_) { 42 })
  |> should.equal(Ok(Nil))

  enqueue(Error(
    "aion_query:{\"query_id\":\"q-typed\",\"name\":\"typed-state\","
    <> "\"arguments\":\"null\"}",
  ))
  enqueue(Ok("await-resolved"))

  pump.run(scripted_await)
  |> should.equal(Ok("await-resolved"))

  query_replies()
  |> should.equal("[\"ok:q-typed:42\"]")
}

pub fn typed_handler_answers_from_its_decoded_arguments_test() {
  // The answer is derived from the arguments, so a handler that ignored them
  // (or received a default) cannot produce it.
  fresh_env()
  let int_codec = codec.json_codec(json.int, decode.int)

  query.handler("typed-double", int_codec, int_codec, fn(n) { n * 2 })
  |> should.equal(Ok(Nil))

  enqueue(Error(
    "aion_query:{\"query_id\":\"q-double\",\"name\":\"typed-double\","
    <> "\"arguments\":\"21\"}",
  ))
  enqueue(Ok("await-resolved"))

  pump.run(scripted_await)
  |> should.equal(Ok("await-resolved"))

  query_replies()
  |> should.equal("[\"ok:q-double:42\"]")
}

pub fn typed_handler_refuses_arguments_its_codec_cannot_decode_test() {
  // Undecodable arguments must never reach the handler body: the caller
  // learns its request was malformed instead of receiving an answer computed
  // from an invented default.
  fresh_env()
  let int_codec = codec.json_codec(json.int, decode.int)

  query.handler("typed-strict", int_codec, int_codec, fn(n) { n * 2 })
  |> should.equal(Ok(Nil))

  enqueue(Error(
    "aion_query:{\"query_id\":\"q-strict\",\"name\":\"typed-strict\","
    <> "\"arguments\":\"\\\"not-an-int\\\"\"}",
  ))
  enqueue(Ok("await-resolved"))

  pump.run(scripted_await)
  |> should.equal(Ok("await-resolved"))

  let replies = query_replies()
  replies
  |> string.contains("error:q-strict:")
  |> should.be_true()
  replies
  |> string.contains("query typed-strict could not decode its arguments")
  |> should.be_true()
  replies
  |> string.contains("ok:q-strict")
  |> should.be_false()
}

pub fn dispatch_carries_typed_arguments_to_the_registered_handler_test() {
  // The in-engine dispatch helper is the caller-side twin of `handler`: what
  // it encodes is exactly what the handler decodes.
  fresh_env()
  let int_codec = codec.json_codec(json.int, decode.int)
  let string_codec = codec.json_codec(json.string, decode.string)

  query.handler("dispatch-echo", string_codec, int_codec, fn(text) {
    string.length(text)
  })
  |> should.equal(Ok(Nil))

  query.dispatch("dispatch-echo", string_codec, "say \"hi\"", int_codec)
  |> should.equal(Ok(8))
}
