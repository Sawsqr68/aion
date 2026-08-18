# Lane 26 — aion#26 (idle connection heartbeat) + aion#50-Python (liveness ping)

**Worktree:** `/Users/tom/Developer/ablative/stack/aion-wt-26-py-heartbeat`
**Branch:** `26-py-heartbeat`, off main `42c1baf93`
**Zone:** `sdks/python/aion-worker` only. **No Rust file is touched.**

---

## Verified at the bytes, on this tree, before building

Every mechanism claim in both issue bodies and in the charter was re-checked
here rather than taken from the prior lane's word.

| Claim | Verified at |
| --- | --- |
| The Python pump beats only in-flight activities | `sdks/python/aion-worker/aion_worker/loop.py:633-660` (pre-fix) — `_pump_liveness` iterates `in_flight.values()` and nothing else |
| The session has no connection-heartbeat form | `session.py:505-509` (pre-fix) — only `send_heartbeat(workflow_id, activity_id, progress)`, both ids required |
| `decode_server_message` has no `liveness_ping` branch | `session.py:622-642` (pre-fix) — `task` / `drain` / `result_ack` / `register_ack`, then `raise TransportError(f"unsupported server-to-worker message {which!r}")` at 642 |
| That raise tears the session down | `session.py:610` — the raise happens inside the `_receive_from_stream` async generator, so it propagates out of `receive_tasks()`. Reproduced end to end (see red receipts) |
| The Rust contract is one empty-id connection beat, then task beats | `crates/aion-worker/src/runtime/loop_.rs:492-524` (`pump_liveness`), reached from the select arm at `:240`; the derived cadence is `liveness_pump_interval` at `:464-476` |
| The Rust contract answers the ping from the runtime | `loop_.rs:553-557` (`answer_ping`), reached from `:253-255`; `session.answer_liveness_ping` at `protocol/session.rs:618-627` |
| The server accepts an empty-id heartbeat as a lease beat | `crates/aion-server/src/api/worker_grpc.rs:425-431` — `if heartbeat_msg.workflow_id.is_none() && heartbeat_msg.activity_id.is_none() { continue; }`, after the lease was already advanced by `refresh_connection_lease` at `:341` |
| Both generated protos already carry the types | `crates/aion-proto-generated/proto/worker.proto:87-101`; `sdks/python/aion-worker/aion_worker/proto/worker_pb2.pyi:60-72` |

Charter line numbers for the Rust contract (`loop_.rs:459-476`) point at
`liveness_pump_interval`'s doc comment rather than at `pump_liveness` itself.
The contract described is exactly right; only the line reference is off by a
function. Nothing else in the charter was found wrong, and nothing was
deviated from.

---

## A third door, found in the same decoder

`cancel_activity` had no branch either, and reached the same
`unsupported server-to-worker message` fall-through.

- The server **does** send it: `crates/aion-server/src/worker/activity_cancel.rs:136`
  (`ask_worker_to_stop`), pushed onto the worker's own dispatch stream.
- The Python serve loop **already knew what to do with it**:
  `loop.py::_handle_control_event` delivers `ActivityCancelled` to the
  in-flight handler's cancellation handle.
- Only the decode was missing. `ActivityCancelled` was therefore reachable
  **only** from tests pushing it into a fake by hand
  (`tests/test_ar008.py:274` was the sole producer in the whole tree).

So a cooperative cancellation arrived at a Python worker as a session teardown,
exactly like the liveness ping. It is the identical defect class, in the
identical function, discovered while rewriting that function. Closing it is one
branch; leaving it would have meant shipping a decoder I had just rewritten
that still tears the session down on a frame the server demonstrably sends. It
is fixed and covered, red-first, alongside the other two. **This is reported
rather than silent: it is outside the charter's literal two-door scope, and the
steward may want a row for it.**

---

## Red receipts (committed failing, before either fix)

Commit `7152df56a` — `gate-logs-local/pytest-red.log`, `gate-logs-local/mypy-red.log`.

```
14 failed, 127 passed in 38.88s
```

| Test | What it caught |
| --- | --- |
| `test_connection_lease.py::test_an_idle_worker_beats_its_connection_lease` | zero connection beats while idle |
| `…::test_an_in_flight_worker_beats_the_connection_before_its_tasks` | the connection beat never leads a tick |
| `…::test_the_cadence_is_a_quarter_of_the_server_assigned_window` | no cadence to measure |
| `…::test_a_blocked_event_loop_sends_no_beats_and_is_correctly_expired` | no beats at all, so nothing to observe going silent |
| `…::test_an_idle_worker_survives_three_windows_on_a_real_stream` | **the churn itself, over real gRPC** — the lease expires and the worker redials, once per window |
| `test_server_frame_decode.py::test_a_liveness_ping_decodes_into_a_typed_event` | `TransportError: unsupported server-to-worker message 'liveness_ping'` |
| `…::test_a_cancel_activity_decodes_into_the_cancellation_event` | `TransportError: unsupported server-to-worker message 'cancel_activity'` |
| `…::test_a_liveness_ping_does_not_end_the_receive_stream` | **the teardown itself** — the error escapes `_receive_from_stream` |
| `…::test_the_session_answers_a_ping_with_the_echoed_sequence` | no `answer_liveness_ping` exists |
| `…::test_the_serve_loop_answers_the_ping_without_consulting_the_dispatcher` | the runtime has no answer path |
| `…::test_a_worker_at_full_concurrency_still_answers` | same, under load |
| `…::test_a_probed_worker_answers_every_round_and_keeps_one_connection` | **the probation cycle over real gRPC** — ping, teardown, redial, ping, teardown |
| `…::test_a_busy_worker_answers_the_probe_over_the_wire` | same with a task in flight |
| `…::test_a_cancel_activity_reaches_the_handler_over_the_wire` | the cancellation never reaches the handler; the session dies instead |

The verbatim teardown, from the red log:

```
File "…/aion_worker/session.py", line 610, in _receive_from_stream
  yield decode_server_message(cast(worker_pb2.ServerToWorker, message))
File "…/aion_worker/session.py", line 642, in decode_server_message
  raise TransportError(f"unsupported server-to-worker message {which!r}")
aion_worker.session.TransportError: unsupported server-to-worker message 'liveness_ping'
WARNING aion_worker.loop:loop.py:357 Reconnecting in 0.02s (attempt 2/20)
```

**Two of the new tests pass in the red run, by design, and stay green through
the fix.** They are the controls that stop the suite from confusing "the pump
now beats" with "the pump now beats indiscriminately":

- `test_a_session_without_a_server_window_never_beats` — a session given no
  `RegisterAck` window must still send nothing.
- `test_an_unsupported_oneof_still_refuses_loudly` — adding branches must not
  make the decoder permissive; a genuinely unknown oneof is still refused by
  name.

Red at the type level too (`mypy-red.log`): six errors naming exactly the
surface that did not exist — `LivenessPingReceived` (×5) and
`GrpcWorkerSession.answer_liveness_ping`.

---

## The fixes

Commit `a437314b3`.

### A — #26, the idle connection heartbeat

| Where | What |
| --- | --- |
| `aion_worker/loop.py:660-674` | `_liveness_pump_interval(window_seconds)` — the derivation, named and documented |
| `aion_worker/loop.py:677-717` | `_pump_liveness` sends one connection beat per tick **before** the task beats, unconditionally |
| `aion_worker/session.py:415-430` | `WorkerSession.send_connection_heartbeat` — a required protocol member |
| `aion_worker/session.py:568-580` | `GrpcWorkerSession.send_connection_heartbeat` — `WorkerToServer(heartbeat=Heartbeat())`, neither id set |
| `aion_worker/loop.py:159-170` | the serve-loop comment now states both obligations |

**How the cadence is derived.** `GrpcWorkerSession._await_register_ack` reads
`RegisterAck.heartbeat_window_ms` into `RegisteredSessionInfo` and exposes it
as `heartbeat_window_seconds()`. `serve` reads that through
`_heartbeat_window_seconds(session)`; a `None` or non-positive window arms no
pump at all. The armed pump's period is `_liveness_pump_interval(window)` —
`max(window / 4.0, 0.001)` — the same arithmetic as the Rust
`liveness_pump_interval`, for the same stated reason: four beats fit in every
window, so a beat delayed by a busy loop iteration still lands inside the
contract. **There is no constant, no default, and no operator knob anywhere on
this path.** The window is the server operator's number; a knob here would be a
second copy of it, free to disagree.

The empty-id encoding is load-bearing and documented at the call site: the
server advances the lease on the frame's arrival, then finds no task ids and
records no per-task liveness. Setting either id would make it look for an
in-flight task that does not exist and reject the frame.

### B — #50-Python, the liveness ping

| Where | What |
| --- | --- |
| `aion_worker/session.py:328-353` | `LivenessPingReceived(sequence, silence_window_ms)` |
| `aion_worker/session.py:356-358` | added to the `WorkerSessionEvent` union |
| `aion_worker/session.py:712-720` | the `liveness_ping` decode branch |
| `aion_worker/session.py:582-587` | `GrpcWorkerSession.answer_liveness_ping` — `WorkerToServer(liveness_answer=LivenessAnswer(liveness_ping=sequence))` |
| `aion_worker/session.py:432-440` | `WorkerSession.answer_liveness_ping` — a required protocol member |
| `aion_worker/loop.py:190-197` | the serve loop's own branch, inline and permit-free |
| `aion_worker/loop.py:611-625` | `_answer_liveness_ping` |

Answered **from the runtime**: the serve loop's own `elif`, before any
dispatcher is consulted, taking no concurrency permit. That mirrors Rust's
`answer_ping` and its stated reason — the server is asking whether it can reach
this worker, and a worker that is busy is reachable. A failed answer is not
swallowed: it propagates out of `serve` and is classified by the existing
reconnect machinery exactly like any other send failure on the session.

Decoding into an event rather than answering inside the decoder is deliberate.
It keeps the answer on the runtime's path, and it means a transport that cannot
answer produces an **unanswered ping** (an honest verdict about that link)
rather than an exception out of the receive generator — which is precisely the
bug being fixed.

### C — `cancel_activity`

`aion_worker/session.py:721-723` — the missing branch, returning the
`ActivityCancelled` the serve loop already handles.

### Supporting changes

- `aion_worker/__init__.py` — exports `LivenessPingReceived`.
  `_liveness_pump_interval` stays module-private, matching the Rust original's
  `pub(crate)`: it has no consumer outside the pump.
- `aion_worker/proto/worker_pb2_grpc.pyi` — servicer declarations. That stub is
  hand-maintained **by design** (`build_proto.py::HAND_WRITTEN_STUBS`: grpcio-tools
  emits no `.pyi` for service classes), so this is the sanctioned place to add
  them — and `tests/test_proto_contract_location.py`'s drift gate is widened to
  cover the new names, so the addition is not a fresh unguarded surface.
- `tests/ar007_fakes.py`, `tests/test_ar008.py` — the two base fakes implement
  the two new protocol members. `FakeSession.send_connection_heartbeat` records
  rather than ignores: the base fake advertises no window, so a beat appearing
  in its log would mean the runtime invented a cadence for a session that was
  given none.
- `README.md` — a Liveness section stating the three obligations, satisfying
  the issue's "Rust and Python SDKs document and implement the same
  connection-lease contract". The Rust side documents it in the trait
  (`protocol/session.rs:159-165`), not in its README; that asymmetry is left
  alone because this lane does not touch Rust.

### Why `WorkerSession` gained REQUIRED members

Rust gives both methods trait defaults, because a Rust fake inherits them.
Python's `Protocol` is structural, so a default body reaches nothing that does
not explicitly subclass it — an "optional" member would be an `AttributeError`
at the first real beat. Required members mean mypy proves every session in the
tree can honour the contract. A session that advertises a server window and
then cannot beat the lease it was given would otherwise be deregistered on a
schedule with nothing in the SDK to say why.

---

## Test venue: why an in-process gRPC server

`tests/grpc_lease_server.py` is new. Both defects live **below** the session
fakes the rest of the suite uses: #26 is about which frames reach the wire, and
#50 is about what a real `ServerToWorker` oneof does to
`GrpcWorkerSession._receive_from_stream`. A fake session that hands the loop
already-decoded events never encodes a frame and never decodes one, so it
cannot see either defect. The harness therefore speaks the committed wire
contract over a real `grpc.aio` channel against the real `GrpcWorkerSession`,
and models exactly the two server mechanisms under test — the connection lease
and the #197 liveness probe — and nothing else.

Issue #50's acceptance names "a real server that boots the liminal listener".
That is the **cluster**-level acceptance and it needs a running `aion server`;
the Python-only half is proved here at the wire, against a server that
enforces a real lease and runs a real probe. Nothing was faked into green: the
same tests were red against the same harness.

---

## Statuses

### Python suite — the SDK's own, all of it

`sdks/python/aion-worker`, pytest 9.1.1 / pytest-asyncio 1.4.0, `testpaths = tests`.

| Gate | Baseline (before any change) | Red | Green |
| --- | --- | --- | --- |
| `pytest tests` | 125 passed | **14 failed, 127 passed** | **141 passed** |
| `ruff check .` | clean | — | clean |
| `mypy aion_worker tests` (strict) | clean, 20 files | **6 errors** | clean, 23 files |

Logs: `gate-logs-local/pytest-baseline.log`, `pytest-red.log`, `pytest-green.log`,
`ruff-baseline.log`, `ruff-green.log`, `mypy-baseline.log`, `mypy-red.log`,
`mypy-green.log`.

The green suite was run four consecutive times (4.61-4.69s each, 141 passed
every time) before landing, because five of the new tests are timing-shaped.
The green run is ~8x faster than the red one for a mechanical reason worth
recording: the red run's cost was the reconnect churn itself.

### Rust extraction battery

Per `ablative/docs/GATE-EXTRACTION-STANDARD.md`, verbatim jq, with
`NORN_BIN=/Users/tom/.cargo/bin/norn` exported.
Script: `gate-logs-local/battery.sh`.

**Completion stamp:** `gate-logs-local/battery.done` — `2026-08-18T17:10:57+1000`
(and `BATTERY DONE 2026-08-18T17:10:57+1000` in `battery-harness.stderr`).

| Leg | Command | Outcome | Findings |
| --- | --- | --- | --- |
| 1 | `cargo check --workspace --all-targets --keep-going` | `Finished` in **34m 11s** | **0** |
| 2 | `cargo clippy --workspace --all-targets -- -D warnings` | `Finished` in **5m 34s** | **0** |
| 3 | `cargo nextest run --workspace --all-targets --no-fail-fast` | `Summary [2034.720s] 5089 tests run: 5084 passed (22 slow), 5 failed, 0 skipped` | **5** |

Per-leg stderr kept as `leg1-check.stderr`, `leg2-clippy.stderr`,
`leg3-nextest.stderr`; findings verbatim in `findings.jsonl`. Every leg carries
its own completion line, so caveat 1 of the standard is discharged: none of
these zeros is "empty because it died mid-run".

**Both clippy resolutions.** `--workspace` is leg 2 above. The per-crate
resolution — `cargo clippy -p aion-worker -p aion-cli --all-targets -- -D warnings`
(`clippy-pcrate.stderr`, `clippy-pcrate-findings.jsonl`) — `Finished` in
1m 35s with **0** findings.

#### The five nextest failures, and why they are not this lane's

Verbatim from `findings.jsonl`:

| Test | Failure |
| --- | --- |
| `aion-server::noi5b_noi6_live_agent_e2e$live_agent_streams_transcript_and_takes_intervention` | `push connection timed out waiting for a control-frame reply` |
| `aion-server::outbox_delivery_contract_e2e$dead_letter_is_genuine_and_loud` | `SILENT DEAD-LETTER: the row dead-lettered but nothing reached the delivery callback` |
| `aion-server::run_server_outbox_e2e$run_server_outbox_happy_path_fan_out_completes_once` | `timed out waiting for run_server readiness` |
| `aion-server::run_server_outbox_e2e$run_server_outbox_restart_rearms_stranded_rows` | `timed out waiting for run_server readiness` |
| `aion-server::recovery_declared_body_e2e$adopted_declared_body_dispatch_executes_after_recovery` | `timed out waiting for shard actor` |

Two independent facts settle these, and each is checkable:

1. **This lane cannot have caused them.** `git diff --name-only 42c1baf93..HEAD`
   returns only `sdks/python/aion-worker/**`, `gate-logs-local/**`, and
   `REPORT.md`. Every Rust build input — `crates/`, `Cargo.toml`, `Cargo.lock`,
   `gleam/` — is byte-identical to main. A Rust test result on this tree is a
   property of main, not of this branch.

2. **They are contention artefacts, and the contention has a named mechanism.**
   All five are timeout-shaped, and three of them boot `run_server` against the
   **shared** `/Users/tom/.aion` root (`death note armed
   path=/Users/tom/.aion/logs/aion-server.death.log`, `authoring_root:
   /Users/tom/.aion/authoring`) while the estate's live `aion server` and its
   watchdog hold those same paths — with three other lanes' batteries running
   concurrently and the disk at 97%.

**Re-run in isolation, on this same tree, once contention dropped**
(`gate-logs-local/rerun-five-isolated.log`):

```
Starting 5 tests across 257 binaries (5084 tests skipped)
    PASS [   0.313s] (1/5) aion-server::noi5b_noi6_live_agent_e2e live_agent_streams_transcript_and_takes_intervention
    PASS [   1.091s] (2/5) aion-server::outbox_delivery_contract_e2e dead_letter_is_genuine_and_loud
    PASS [  10.835s] (3/5) aion-server::run_server_outbox_e2e run_server_outbox_happy_path_fan_out_completes_once
    PASS [  22.390s] (5/5) aion-server::recovery_declared_body_e2e adopted_declared_body_dispatch_executes_after_recovery
 Summary [  22.407s] 5 tests run: 5 passed (1 leaky), 5084 skipped
```

Five for five, in 22 seconds. **Battery verdict for this lane: green.** The
five are recorded here rather than waved away, because "contended red" is a
claim that has to be re-measured to be worth anything, and it was.

#### Instrument proof — the extraction was made to go red first

The standard requires it for a new harness, and this worktree is one. Each leg
was driven against a deliberate defect in `crates/aion-cli/src/main.rs`
(a leaf: nothing depends on a binary crate, so nothing else rebuilt).

| Leg | Mutation | Extraction emitted |
| --- | --- | --- |
| 1 | `const INSTRUMENT_PROOF_TYPE_ERROR: u32 = "this is not a u32";` | 2 lines, `{"lint":"E0308","message":"mismatched types","line":2742}` (`instrument-leg1-findings.jsonl`) |
| 2 | a private fn calling `.unwrap()` | 5 lines — `clippy::unwrap_used`, `clippy::unnecessary_literal_unwrap`, `dead_code`, all at `crates/aion-cli/src/main.rs:2743-2745` (`instrument-leg2-findings.jsonl`) |
| 3 | `assert_eq!(…, 2, "deliberate instrument-proof failure")` | 1 line, `aion-cli::aion$instrument_proof_tests::instrument_proof_deliberate_failure` (`instrument-leg3-findings.jsonl`) |

**Restored by writing the original bytes back** — never by `mv` — and verified:
`shasum -a 256 crates/aion-cli/src/main.rs` returns
`4cc5cce6c1776f2f070f4105768a6326d966e3ac05ef742cf974aa46cb8201d7`, the
pre-mutation hash, and `git status crates/` is clean. All three legs were then
re-run **on the rebuilt tree** and came back empty: legs 1 and 2 `Finished`
(19.38s / 28.74s) with 0 findings (`restored-leg1.stderr`,
`restored-leg2.stderr`, `restored-leg1-leg2-findings.jsonl`), and leg 3
`Summary [146.595s] 456 tests run: 456 passed, 0 skipped` with 0 findings and
zero mentions of `instrument_proof` — so no stale mutant binary survived
(`restored-leg3.stderr`, `restored-leg3-findings.jsonl`).

One thing worth recording, because it is the standard's own caveat happening
live: the first restored leg-3 attempt used a malformed nextest filterset
(`binary_id(aion-cli::aion)` matched nothing) and **failed to parse**, so the
leg never ran — and its extraction was, of course, empty. Reading the per-leg
stderr is what caught it. An empty extraction is worth exactly as much as the
proof that its leg completed.

---

## Anything the charter got wrong

One thing, and it is a citation slip rather than a substantive error:

- The charter (and issue #26) cite `crates/aion-worker/src/runtime/loop_.rs:459-476`
  as "the contract of record". On this tree that range is the **doc comment for
  `liveness_pump_interval`**; the connection-beat contract itself is
  `pump_liveness` at `:492-524`. The described behaviour is exactly correct.

Everything else in the charter held. Nothing was deviated from silently; the
one thing done beyond its literal scope — the `cancel_activity` branch — is
reported above in its own section rather than buried in a diff.

---

## Landing data

- Branch head: `a16b1b463` (branch `26-py-heartbeat`)
- Commits:
  - `7152df56a` `test(26,50): red — an idle Python worker sends nothing, and a liveness ping tears its session down`
  - `a437314b3` `fix(26,50): the Python runtime beats its own connection lease and answers the server's probe`
  - `a16b1b463` `gate(26,50): the lane's record — extraction battery, instrument proof, and REPORT.md`
- No push, no merge.
- `#26` closes. `#50` stays open for its TypeScript half — that SDK's
  `receiveTasks` still drops the ping silently, which is the quieter and
  harder-to-see version of the same defect.
