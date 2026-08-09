//// Typed query handler registration and replies.

import aion/codec.{type Codec}
import aion/error
import aion/internal/ffi
import gleam/json
import gleam/string

/// Register a typed read-only query handler with AT's query service.
///
/// The handler's argument and return types are fixed by the two `Codec`s
/// supplied at registration. When the engine dispatches a query with this
/// name, the SDK decodes the caller's argument document with
/// `arguments_codec`, invokes `reply` with the decoded value, encodes the
/// returned value with `value_codec`, and replies on the engine-provided
/// one-shot reply channel. Neither direction crosses the FFI boundary without
/// a codec.
///
/// A caller that supplies no arguments sends the JSON `null` document, so a
/// query that takes no input registers [`codec.nil_codec`] as its arguments
/// codec and ignores the `Nil` its handler receives.
/// Arguments that `arguments_codec` cannot decode never reach `reply`: the
/// failure is replied as a typed query-handler failure naming the decode
/// error, so a caller learns its request was malformed instead of receiving a
/// silently defaulted answer.
///
/// Registration stores the encoding handler in the workflow process
/// dictionary (the engine-contract location the yield-point query pump reads
/// from) and then registers the name with the engine. Register before the
/// first yield point — `workflow.sleep`, `workflow.receive`, `workflow.run`,
/// `child.await`, `workflow.all`/`workflow.race` — that should answer the
/// query; awaits reached earlier cannot service it.
/// Because workflow code re-executes from the top on replay, re-registration
/// after recovery is automatic: a recovered workflow answers queries without
/// any extra author code.
///
/// Queries bind to AT's read-only query service: they append no workflow `Event`,
/// are answered at engine yield points, and never block workflow progress. By
/// type this callback only returns a value; by workflow-author convention it must
/// not call activity-dispatch primitives such as `workflow.run` or otherwise
/// mutate workflow state — the engine refuses recording calls made while a
/// query is being serviced, surfacing them as a typed handler failure.
pub fn handler(
  name: String,
  arguments_codec: Codec(arguments),
  value_codec: Codec(value),
  reply: fn(arguments) -> value,
) -> Result(Nil, error.QueryError) {
  let encoded_reply = fn(query_id, raw_arguments) {
    case arguments_codec.decode(raw_arguments) {
      Ok(arguments) -> {
        let encoded = value_codec.encode(reply(arguments))
        ffi.reply_query(query_id, encoded)
      }
      Error(decode_error) ->
        ffi.reply_query_error(query_id, arguments_failure(name, decode_error))
    }
  }

  ffi.register_query_handler(name, encoded_reply)
  case ffi.register_query(name, register_config()) {
    Ok(_) -> Ok(Nil)
    Error(raw_error) -> Error(query_error(raw_error))
  }
}

/// Render an arguments decode failure as the handler-failure message the
/// querying caller receives.
fn arguments_failure(name: String, decode_error: codec.DecodeError) -> String {
  let path = case decode_error.path {
    [] -> ""
    segments -> " at " <> string.join(segments, ".")
  }
  "query "
  <> name
  <> " could not decode its arguments: "
  <> decode_error.reason
  <> path
}

/// Dispatch a typed query, with its arguments, through the
/// in-engine/client-side binding.
///
/// This helper exists for callers already inside the engine boundary and for the
/// pure Gleam test harness. It encodes `arguments` with `arguments_codec`, asks
/// AT's query service for the named handler, then decodes the encoded reply
/// with `value_codec`. An unregistered name
/// returns `Error(UnknownQuery(name))`; malformed replies return
/// `Error(QueryDecodeFailed(_))`. Query dispatch records no workflow event.
pub fn dispatch(
  name: String,
  arguments_codec: Codec(arguments),
  arguments: arguments,
  value_codec: Codec(value),
) -> Result(value, error.QueryError) {
  let encoded_arguments = arguments_codec.encode(arguments)
  case ffi.dispatch_query(name, dispatch_config(encoded_arguments)) {
    Ok(raw_payload) -> {
      case value_codec.decode(raw_payload) {
        Ok(value) -> Ok(value)
        Error(decode_error) -> Error(error.QueryDecodeFailed(decode_error))
      }
    }
    Error(raw_error) -> Error(query_error(raw_error))
  }
}

fn register_config() -> String {
  json.object([]) |> json.to_string
}

/// The engine's dispatch config carries the caller's argument document as the
/// `payload` field: JSON text inside a JSON string, matching the sentinel's
/// own arguments encoding.
fn dispatch_config(encoded_arguments: String) -> String {
  json.object([#("payload", json.string(encoded_arguments))])
  |> json.to_string
}

/// Map the engine's raw query error strings to the typed taxonomy.
///
/// The engine emits exactly: `unknown:<name>` (unregistered query name),
/// `timeout` (reply deadline expired), `not_running:<workflow_id>` (terminal
/// or non-resident target), `handler_failed:<message>` (the handler ran and
/// reported failure — the `query_failed` wire code), and engine-fault strings
/// (`unknown_workflow:`, `reply_dropped`, `engine:`) which surface as
/// `QueryEngineFailure`.
fn query_error(raw: String) -> error.QueryError {
  case raw {
    "unknown:" <> name -> error.UnknownQuery(name: name)
    "timeout" -> error.QueryTimedOut(error.TimedOut(message: raw))
    "not_running:" <> workflow_id ->
      error.QueryNotRunning(workflow_id: workflow_id)
    "handler_failed:" <> message -> error.QueryHandlerFailed(message: message)
    _ -> error.QueryEngineFailure(raw)
  }
}
