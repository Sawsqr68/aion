# beamr `build_stacktrace` heap-allocation divergence

**Status:** Confirmed against beamr 0.16.2 on 2026-07-24.

## Contract at risk

A raising workflow query handler must produce `QueryError::HandlerFailed` and
must not terminate the workflow. Query diagnostics are non-durable and cannot
change that outcome.

Erlang/OTP and beamr disagree when the handler is wrapped by a three-part
`try` catch:

```erlang
try Handler(QueryId) of
    Outcome -> Outcome
catch
    Class:Reason:Stacktrace -> handle(Class, Reason, Stacktrace)
end
```

OTP enters `handle/3`. Under the heap state reproduced by Aion's real workflow
runtime, beamr exits the process before the catch body runs.

## Minimal repository reproduction

The existing from-source fixture is sufficient; it registers a `boom` query
handler that executes:

```gleam
panic as "deliberate handler failure"
```

The controlling regression is
`crates/aion/tests/sdk_sleep_query_e2e.rs::failing_query_handler_never_kills_the_run`.
It builds `crates/aion/tests/fixtures/sleep_query` from the current
`gleam/aion_flow` source and runs it on the embedded beamr runtime.

With `aion_flow_query_pump:run_handler/2` using
`Class:Reason:Stacktrace`:

```text
cargo test -p aion-rs --test sdk_sleep_query_e2e \
  failing_query_handler_never_kills_the_run -- --nocapture
```

produced:

```text
Err(Query(ReplyDropped))
workflow process 1 exited: Error: VM execution error:
heap full: requested 2 words with 1 available
```

The resulting history contained `WorkflowStarted`, `TimerStarted`, then
`WorkflowFailed`. Enabling the existing tracing subscriber produced no query
exception diagnostic, proving that execution did not reach the first catch-body
call.

Changing only that catch to `Class:Reason`, while retaining the diagnostic and
error-reply calls, made the same e2e pass. The tracing subscriber then observed:

```text
category="query_reply_exception"
class=error
reason=deliberate handler failure
```

The unchanged Gleam/OTP harness passed with the three-part catch and observed
both its diagnostic and typed error reply. The discrepancy is therefore a VM
instruction-surface divergence, not a new truthful query classification.

Evidence logs from the resolution run are retained under `target/`:

- `query-catch-real-beamr-trace.log`
- `query-catch-real-beamr-terminal.log`
- `query-catch-two-part-real-beamr.log`
- `query-catch-resolution-e2e.log`

## Bytecode and beamr failure point

Compiling the three-part catch with `erlc -S` emits this sequence before any
catch-body expression:

```text
try_case
move x2
build_stacktrace
```

beamr 0.16.2's interpreter `try_case` exposes class, reason, and trace through
`x0`-`x2`. Its `build_stacktrace` implementation then constructs each frame
with heap-allocating `four_tuple` and `cons` calls. In the reproduced workflow
heap state, that allocation returns `HeapFull` rather than collecting/growing
and retrying. The execution error escapes the handler catch and terminates the
workflow.

A two-part catch does not emit `build_stacktrace`, so beamr reaches the catch
body, sends `reply_query_error`, and the caller receives `HandlerFailed`.

## Why the caller saw `ReplyDropped`

`reply_query_error` is the only path that sends
`QueryError::HandlerFailed` through the pending one-shot sender. Because beamr
terminated the workflow before that NIF call:

1. process-exit cleanup removed the pending sender;
2. the waiting `QueryService` observed the closed one-shot; and
3. `QueryService::query_process` mapped that closure to `ReplyDropped`.

`ReplyDropped` was therefore secondary infrastructure noise caused by VM
termination, not the truthful handler outcome.

## Aion mitigation

`aion_flow_query_pump.erl` now uses two-part catches for both handler/reply and
error-reply exceptions. Diagnostics still contain bounded class and reason.
For stack context they contain:

- the source module/function/file/line carried by Gleam panic reason maps, when
  present; and
- the exact pump catch boundary for every exception.

This shape executes identically on OTP and beamr, preserves `HandlerFailed`,
and keeps reply exceptions non-fatal. It intentionally does not claim raw VM
frames that beamr cannot safely build today.

## beamr escalation

beamr should make `build_stacktrace` allocation obey normal process-heap
semantics: ensure capacity, collect/grow as required, and retry frame/list
construction rather than returning a process-fatal `HeapFull`. A regression
should execute a three-part catch with one free heap word and assert that the
catch body receives class, reason, and a stacktrace without terminating the
process.

Once that VM regression is fixed and Aion adopts the corrected beamr version,
the query pump can be reconsidered for full raw VM stack frames. The typed
handler-failure and liveness contract must remain the controlling gate.
