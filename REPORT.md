# Lane 25 — the liveness verdict reaches only the transports the probe carries

**Issue:** aion#25 — "Liminal liveness probe makes healthy gRPC workers
permanently ineligible for dispatch."
**Worktree:** `aion-wt-25-liveness`, branch `25-liveness`, off main `1e20aea43`.
**Builder:** Opus. **No push, no merge.**

---

## 1. What the bytes on this tree actually said

The charter told me to verify the mechanism before building on it, and the
mechanism had MOVED since the issue was filed. That is the first thing the
steward needs.

**The issue was filed 2026-08-07. #197 landed 2026-08-11 (`fc98ea1b7`, released
in 0.14.0) and taught the probe to ping gRPC-delivered workers over their task
streams.** There is even a later commit — `dbd11bd3b`, 2026-08-13 — retiring
"the stale dispatch-ineligible-forever claim" on those grounds. So the issue's
literal cause sentence ("a gRPC worker starts with no liminal dispatch-
reachability proof … the probe has no path to test it") describes pre-#197 code,
and the reporter's proposed fix — scope the verdict to
`LiminalConnectionNotifier::liveness_targets()` and check every excluded entry is
`WorkerDelivery::Liminal` — **would be a regression on this tree**: it would drop
gRPC workers out of a verdict that is now entitled to judge them, and it would
also drop any liminal worker whose connection has closed, silently re-admitting a
worker the server demonstrably cannot reach. I did not implement it.

What #197 did NOT do is scope the verdict. `publish_reachability_verdict` still
formed its published set from the whole of `HeartbeatTracker::unreachable_workers()`
— every tracked worker — with no regard for whether the probe held any wire on
which that worker could be asked. Correctness became a property of ONE call site
(`ServerState::spawn_liminal_liveness_probe`) passing `Some` twice, rather than a
property of the probe. `LivenessProbe::new`, documented in-tree as "the
liminal-only construction for callers with no gRPC fleet", still published
exclusions over gRPC workers it could never ping — the original #25 injury
intact, behind a wiring accident.

That is what this lane fixes, and it is red-provable on this tree: see §3.

## 2. The fix

**A verdict carries a worker only if this probe covers the transport that worker
is delivered over.** Coverage is STRUCTURAL — which wires the probe holds (a
liminal notifier, a gRPC answer registry, or both) — never the set of targets a
round happened to enumerate.

Files: `crates/aion-server/src/worker/liminal_liveness.rs` (`covers`,
`within_reach`, `publish_reachability_verdict`, `probe_once`),
`crates/aion-server/src/worker/registry.rs` (`WorkerDelivery::transport`,
`ConnectedWorkerRegistry::transports_of`),
`crates/aion-server/src/worker/liveness.rs`,
`crates/aion-core/src/cluster_event.rs`,
`crates/aion-server/src/stream/cluster_stream.rs`.

Three consequences, all deliberate:

1. **Uncarried transports are left alone.** A probe with no gRPC answer registry
   publishes nothing about gRPC workers; a probe with no liminal notifier
   publishes nothing about liminal workers. An exclusion no answer could ever
   lift is a permanent withdrawal dressed as a measurement.
2. **Every round publishes, including one that pinged nothing.** The old
   early-return reasoned that publishing an empty set "would be indistinguishable
   from clearing one". They are the same act, and skipping it left a stale verdict
   in force over a fleet the round had measured afresh — the issue's fourth
   acceptance criterion. The set comes from the tracker's standing evidence, so it
   is well defined with no targets at all.
3. **A worker that has LEFT the registry is named by no verdict.** It has no
   transport with which to answer the coverage question, selection can no longer
   see it, and guessing one would put a fabricated name in an operator-facing set.
   The tracker keeps its evidence untouched: this is a scope on the VERDICT, not a
   second reaper.

**Why not scope to the round's targets** (the reporter's approach, and the
tempting one): a liminal worker whose connection has closed is enumerated by
nothing, and it is exactly the worker that must stay excluded. That case is
pinned by `the_same_verdict_still_excludes_an_unreachable_worker_of_a_covered_transport`,
which is green both before and after the fix and is the control that forbids the
wrong cure.

### Subtraction taken along the way

`WorkerDelivery::transport()` is now the ONE mapping from a held delivery to its
wire discriminant. Two byte-identical private copies already existed —
`registry.rs` for the WS3 `WorkerConnected` delta and `stream/cluster_stream.rs`
for the snapshot — and a third was about to be written for the verdict. Both are
deleted. Likewise the operator-facing transport WORD now has one table:
`aion_core::WorkerTransport::name()`, which `ProbedTransport::name()` delegates
to, so a liveness withdrawal and a cluster event cannot name the same link
differently. (`ProbedTransport::name` previously reached the word for a liminal
line through `ProbedTransport::Liminal { pid: 0 }.name()` — a fabricated pid used
purely to index a string table.)

## 3. Red-first receipts

Commit `7920deb1b` is tests-only, before any production change.

`gate-logs-local/red-01-transport-scope.log` — 1 passed, 3 FAILED:

| pin | verdict on main |
| --- | --- |
| `a_probe_that_cannot_reach_a_transport_leaves_its_workers_alone` | FAILED — the uncarried transport's worker was excluded |
| `an_empty_round_clears_a_stale_verdict_it_cannot_re_measure` | FAILED — stale gRPC exclusion left standing for ever |
| `a_grpc_worker_stays_dispatchable_across_real_liminal_probe_rounds` | FAILED — the live shape, over real sockets |
| `the_same_verdict_still_excludes_an_unreachable_worker_of_a_covered_transport` | ok (the control; must be green on both sides) |

Green after the fix: `gate-logs-local/green-04-final.log` — 5 passed (the fifth,
`a_worker_that_left_the_registry_is_named_by_no_verdict`, was added with the fix
and is mutation-checked below).

**The live pin is real machinery, not a mock.** A real `liminal-server` listener
on loopback with the aion notifier installed, a real remote `aion-worker` driven
through the production `serve_with_redial` entry point, the real `LivenessProbe`
on its derived cadence and the real `HeartbeatSweeper`, with a gRPC worker
registered into the same registry and tracker and beating its connection lease
exactly as a connected SDK does. It does not wait for a duration — it waits for
PROOF that real rounds concluded (the liminal worker becoming dispatch-reachable,
which only a consecutive run of answered real pings can establish), and it
asserts the gRPC worker is selectable BEFORE and AFTER, so the assertion is a
change and not a standing condition.

**Mutation check on the branch the red did not cover.** The departure rule is new
behaviour, so the literal-prior-text red says nothing about it. Mutating
`is_some_and` → `is_none_or` in `within_reach` (the one line that decides an
absent worker's fate) reddens `a_worker_that_left_the_registry_is_named_by_no_verdict`
and nothing else: `gate-logs-local/mutation-absent-worker.log` (4 passed, 1
FAILED). Restored by writing the bytes back and re-run green on the rebuilt tree:
`gate-logs-local/green-03-restored.log`.

## 4. Acceptance criteria

| # | Criterion | Where it is pinned |
| - | --------- | ------------------ |
| 1 | A registered gRPC worker remains dispatchable across repeated liminal probe rounds | `a_grpc_worker_stays_dispatchable_across_real_liminal_probe_rounds` (real sockets), `a_probe_that_cannot_reach_a_transport_leaves_its_workers_alone` |
| 2 | A genuinely unreachable liminal worker is still excluded | `the_same_verdict_still_excludes_an_unreachable_worker_of_a_covered_transport` |
| 3 | Recovery of a liminal worker restores its eligibility | the live pin: the real liminal worker starts excluded (unserved probation) and becomes selectable only by answering a consecutive run of real pings, with no separate trigger |
| 4 | An empty liminal target set cannot introduce a gRPC exclusion and clears stale transport-specific verdicts safely | `an_empty_round_clears_a_stale_verdict_it_cannot_re_measure` |
| 5 | A regression test runs both worker transports in one server and dispatches to the gRPC worker before and after a liminal probe round | the live pin (both assertions, plus a zero-frames-pushed check proving the eligibility was not bought by quietly growing a gRPC arm) |

Plus a coverage truth table — all four cells, both transports across all four
wire-holdings — in `liminal_liveness.rs`'s unit tests
(`a_probe_covers_exactly_the_transports_it_holds_a_wire_for`). A `covers` that
returned `true` unconditionally IS the pre-fix behaviour and satisfies any test
that checks only the admitting direction, so the denying half is enumerated
explicitly.

## 5. Regression suites re-run

All green, unchanged: `grpc_liminal_one_eligibility_set` (2),
`grpc_worker_liveness_e2e` (6), `dead_man_switch_e2e` (4),
`dead_man_switch_refusal_logs` (1), `heartbeat_sweeper_e2e` (5). Logs under
`gate-logs-local/regress-*.log`.

## 6. Gates

Extraction battery per `ablative/docs/GATE-EXTRACTION-STANDARD.md`, with
`NORN_BIN=/Users/tom/.cargo/bin/norn` exported: `gate-logs-local/battery.sh`,
findings in `gate-logs-local/findings.jsonl`, per-leg stderr in
`gate-logs-local/leg{1,2,3}-*.stderr` (caveat 1 of the standard: the Summary line
and per-leg stderr are kept so "empty because green" is provably different from
"empty because it died mid-run").

**Result: GREEN — empty extraction.** `findings.jsonl` is 0 lines across all
three legs. The two disambiguating artifacts the standard's caveat 1 requires:
leg 1 and leg 2 each end `Finished \`dev\` profile` in their stderr, and leg 3's
stderr carries `Summary [590.131s] 5165 tests run: 5165 passed (7 slow), 0
skipped`. So the empty extraction is empty because everything passed, not
because a run died early. Harness completion stamped in
`gate-logs-local/battery-harness.stderr` (`BATTERY DONE 2026-08-18T13:41:07+1000`).

**Instrument proved able to go red first**, as the standard requires of a new
harness — each leg driven against a deliberate failure:

- leg 1 (`cargo check`) — emitted `{"level":"warning", … "lint":"unused_variables", …}`
- leg 2 (`cargo clippy -D warnings`) — emitted the same finding at `"level":"error"`
- leg 3 (`nextest`) — emitted `{"type":"nextest","test":"aion-server::aion_server$worker::liveness::tests::a_grpc_probe_reports_no_connection_pid","message":"test failed"}`, with `Summary [0.085s] 1 test run: 0 passed, 1 failed`

Both mutations restored by writing bytes back; `git status` clean afterwards.

Both clippy resolutions were run, per the standing note that `--workspace` and
`-p <crate>` resolve features differently: `-p aion-core -p aion-server
--all-targets -- -D warnings` (`gate-logs-local/clippy-pcrate.log`, exit 0) and
the workspace leg inside the battery.

No fmt gate (builders autoformat). `cargo fmt` was run and the resulting churn in
files this lane does not touch — notably `crates/aion-server/src/state.rs`, which
is **lane 15's zone** — was reverted rather than committed, so the lanes do not
collide over formatting.

## 7. FINDING OUTSIDE THIS LANE'S ZONE — the Python and TypeScript worker SDKs cannot answer a liveness ping

This is the #25 injury still live in production today, and it is **not fixable in
`crates/aion-server`**. Reporting rather than deviating, because the files belong
to lane 26.

Since #197 the probe pings every gRPC-delivered worker over its task stream and
withdraws eligibility from any worker that does not answer within the cadence.
The Rust SDK answers (`crates/aion-worker/src/runtime/loop_.rs:253`,
`session.answer_liveness_ping`). The other two do not:

- **Python** (`sdks/python/aion-worker/aion_worker/session.py:626`,
  `decode_server_message`): a `liveness_ping` oneof matches no branch and falls
  to `raise TransportError(f"unsupported server-to-worker message {which!r}")`.
  That is raised out of the `_receive_from_stream` async generator, so the ping
  does not merely go unanswered — it **tears the session down**, and the worker
  reconnects, is pinged again, and tears down again.
- **TypeScript** (`sdks/typescript/aion-worker/src/session.ts:466`,
  `receiveTasks`): a `livenessPing` frame matches no branch and is **silently
  dropped**. No answer, no error, no reconnect — the worker looks perfectly
  healthy while its probation never clears.

Consequence for either SDK, on any server that boots the liminal listener (which
is where the probe is spawned — `run.rs:923`, and it is not opt-in): the worker
registers, runs work until the probe's first rounds conclude, and is then
correctly-by-#197-semantics excluded from dispatch for ever. From the operator's
seat that is exactly the reported symptom — "the worker remains connected but
selection reports no eligible compatible worker and the activity parks" — and it
would survive this lane's fix untouched.

The remedy is in the SDKs, not the server: decode `liveness_ping` and send back
`WorkerToServer.liveness_answer{liveness_ping: <echoed sequence>}` from the
runtime (not from action code), mirroring the Rust runtime's contract. Both
SDKs' generated protos already carry `LivenessPing` and `LivenessAnswer`, so the
wire needs no change.

**Lane 26 already owns `sdks/python/aion-worker/{loop,session}.py`** — the exact
files a Python fix touches — so I did not edit them; two lanes writing those files
would collide. The TypeScript SDK is owned by no lane in this cluster. Steward's
call on routing.

## 8. Anything the charter got wrong

One thing, stated rather than silently worked around: **the charter's prescribed
fix ("scope each verdict to the round's actual liminal targets") is the
reporter's pre-#197 remedy and is wrong on this tree.** Implementing it literally
would re-admit a liminal worker whose connection has closed. I built the
invariant the charter's first clause states — "a transport's liveness verdict may
alter eligibility ONLY for workers carried by that transport" — with coverage
scoped structurally rather than by enumeration, and pinned the enumeration
variant as forbidden with the control test. The empty-round clause is honoured
in full.

Nothing else. All five acceptance criteria are gated by tests; no deferrals, no
`#[allow]`, no `#[ignore]`, no `unwrap`/`expect` in library code, no new
configuration knob, no invented default.
