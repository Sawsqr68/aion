# Delivery report — aion #36 follow-on lane (legs 1–5)

- **Worktree**: `/Users/tom/Developer/ablative/stack/aion/.worktrees/36-follow-on`
- **Branch**: `36-follow-on` (never pushed, never merged; no git config touched)
- **Base**: `e05d3e3b4` — the landed aion main tree named by the charter
- **Head**: `ded583b7e` (this report is the commit after it)
- **Charter**: `gate-logs/resume-adoption/FOLLOW-ON-BRIEF-DRAFT.md` (r1, with the
  battery amended in-flight to the estate standard at
  `/Users/tom/Developer/ablative/docs/GATE-EXTRACTION-STANDARD.md`)
- **Execution mode**: plain builder. No aion harness was used to do this work.

## Leg disposition

| Leg | Claim | Disposition |
|---|---|---|
| 1 — cross-process re-attach arm | worker killed under a live agent; replacement worker resumes the SAME session, exactly one session after adoption | **PROVEN**, re-verified by breaking it |
| 2 — issue #36 verbatim e2e | live builder → pause → server restart → resume → completes with ONE agent session, isolated in-worktree server | **PROVEN**, both faces (Paused and Running) |
| 3 — supersession absence + adoption event | no `superseded:server-death` while the holder lives; an explicit adoption event through the run's ONE Recorder | **PROVEN** at engine level and server level, and **CORRECTED**: the inherited version applied it to every activity class |
| 4 — Running-path truth fix | recovery offers dangling remote-agent attempts to adoption for Running exactly as for Paused | **PROVEN** at engine level and server level |
| 5 — readiness-pin hardening | the decorative constant-helper test replaced by one that fails against the literal prior defect | **PROVEN** |

Every leg carries a mutation receipt: a run in which the fix under it was
removed and the test went red. No leg rests on "the test passes and we believe
it is watching something".

## What I inherited, and what I did with it

The predecessor had gone further than the dispatch note said: six commits
(`a88008ae0`..`707104f0e`) covering legs 1, 3, 4 and 5, plus **one uncommitted
edit** in `crates/aion-server/src/stream/selector.rs`.

**Judgement on the uncommitted edit: KEPT, committed as `eb1b8da4c`.** It was
not optional — it was the missing half of a landed change. `d9d3e50dd` added
`Event::ActivityAdoptionOffered` without giving `aion-server`'s exhaustive
`event_status` match an arm, so **the branch head I inherited did not build**.
Receipt, taken by stashing the edit and compiling the inherited head:

```text
crates/aion-server/src/stream/selector.rs:113:11: error[E0004]: non-exhaustive patterns: `&aion_core::Event::ActivityAdoptionOffered { .. }` not covered
error: could not compile `aion-server` (lib) due to 1 previous error
```

The "in-progress" test file the note mentioned
(`crates/aion-integration-norn/tests/cross_process_reattach_e2e.rs`) was already
committed, not in progress.

**On the shape of the inherited red/fix pairs.** For legs 1 and 5 the production
fix was already landed on the base, so the predecessor's "red" commits are
mutations — the landed fix removed, the new test failing against it, the fix
restored. `git diff e05d3e3b4 HEAD -- crates/aion-integration-norn/src/spawn_guard.rs`
is **empty**: leg 1 added no production code at all. That is the right technique
for an acceptance leg over already-fixed code, but it must not be read as "this
lane fixed the spawn gate". Leg 1's contribution is the *evidence*.

## THE FINDING THAT MATTERS: the inherited change was over-applied

The workspace battery on the final tree — the first one anyone in this lane
had run — found **five gates red**, none of them mine, all of them from the
inherited leg-3/4 change:

```text
aion-rs::settled_fanout_e2e settled_fanout_inverts_completion_captures_failure_and_replays
  assertion `left == right` failed: recovery supersedes exactly the two
  started-but-unsettled members, each at its dangling attempt
aion-rs::nested_workflows_e2e recovery_mid_leaf_activity_redispatches_without_respawning_children
  Error: "timed out waiting for leaf re-dispatch recorded"
aion-rs::nested_workflows_e2e self_spawning_recursion_stops_at_depth_three_and_recovers
  Error: "timed out waiting for deepest re-dispatch recorded"
aion-server::graceful_shutdown_park_e2e drain_timeout_parks_and_exits_success_and_restart_completes
  assertion `left == right` failed: the post-restart re-dispatch must CONTINUE
  the recorded attempt trail   left: 1   right: 2
aion-server::graceful_shutdown_park_e2e graceful_drain_parks_in_flight_activity_and_restart_completes
  (identical)
```

**These are not stale pins.** `d9d3e50dd` deleted #266 Defect B's supersession
record outright and gave EVERY dangling attempt the adoption treatment. #36 is a
ruling about *remote-agent* attempts; #266 Defect B is a live ruling with its own
incident about ordinary ones — an orphaned attempt gets an honest non-terminal
record so the run stops reading as silently still working, and the re-dispatch
continues at the NEXT attempt. The graceful-drain case makes it sharp: the drain
PARKS the activity, so nothing survives, and calling that a retained live
execution is a new falsehood pointing the other way.

Both rulings stand. The fork between them is the DECLARED CLASS — which is the
charter's own wording ("recovery must offer dangling **remote-agent** attempts to
the adoption path") and the only signal at the dispatch seam that can tell them
apart, because the engine cannot observe a harness process and must not guess.

Fixed in `ed04c529b`:

- `WorkflowCatalog::declares_agent_action` — the sibling of
  `declares_advisory_action`, off the same authority (the `.v4` contract the run
  is pinned to, resolved by `(workflow_type, loaded_version)` so a hot redeploy
  never reclasses an in-flight run).
- `runtime::nif_activity_agent` — the class at the seam, argument written out.
  Every absence answers `false`, which is the #266 posture: an unresolvable class
  degrades to the record that says something happened, never to the silence that
  says nothing did.
- The seam forks: agent + dangling → adoption offer, attempt RETAINED; ordinary +
  dangling → supersession record, attempt ADVANCED; nothing dangling → unchanged.
- One detector (`dangling_attempt`) with two consumers, so the two paths can never
  disagree about which attempt is dangling.

All five gates restored, and the lane's own gates still green.

**Why it got through**: the inherited `recovery_adoption_e2e` exercised only the
new behaviour, so it could not tell "adoption arrived" from "supersession was
deleted". It is now a PAIR whose documents differ in exactly one token — `agent`
— the shape the advisory-class exhibit uses, with a fresh-ordinal control on the
other side. It also builds through `compile_and_assemble_awl` rather than a Gleam
example, so it needs no external toolchain.

```text
test a_fresh_dispatch_receives_neither_record ... ok
test an_ordinary_dangling_attempt_is_superseded_not_adopted ... ok
test paused_recovery_records_adoption_without_supersession ... ok
test running_recovery_records_adoption_without_supersession ... ok
test adoption_does_not_duplicate_an_earlier_attempt_failure ... ok
test the_adoption_offer_is_budget_inert ... ok
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.62s
```

## Leg 1 — cross-process re-attach

`crates/aion-integration-norn/tests/cross_process_reattach_e2e.rs` (red
`e40da95ec`, lint-adjusted `707104f0e`; assertions unchanged across that pair).
Real `norn 0.1.0`, isolated `HOME`/XDG, a loopback mock provider holding the
agent's call open, worker one killed under the live agent, worker two redelivering
the same `(workflow_id, activity_id)`.

Verified on this tree (`gate-logs/follow-on/leg1-run.txt`):

```text
test replacement_worker_reattaches_the_surviving_norn_session ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.18s
```

**Mutation control** (`gate-logs/follow-on/leg1-mutation-control.txt`) — the spawn
gate's `same_execution` adoption branch removed:

```text
Error: Occupied { detail: "spawn refused: live sibling pid 24337 holds this tree (workflow f10dd4f7-4305-460d-b307-9e25cfba7a4e, activity activity:1)" }
Error: "timed out waiting for the replacement worker to reattach at …/second.ready"
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 30.15s
```

Branch restored; the orphaned mock-provider agent the failed control left behind
was reaped by pid.

## Leg 2 — issue #36 verbatim, on an isolated in-worktree server

`crates/aion-cli/tests/issue36_pause_restart_resume_e2e.rs` (`9cfc5f114`, Running
face `5c25a4c88`, decomposed `0fff34d71`). Driven entirely through the product's
own commands: `aion server`, `aion deploy <doc.awl>`, `aion worker agent`,
`aion start`, `aion pause`, `aion resume`, `aion describe`.

### Binary provenance (charter amendment 2)

Server, CLI and worker are all `env!("CARGO_BIN_EXE_aion")` — the binary cargo
builds from **this** worktree for **this** test target:

```text
/Users/tom/Developer/ablative/stack/aion/.worktrees/36-follow-on/target/debug/aion
built from the tree under test (head ded583b7e at the final battery)
```

`~/.cargo/bin/aion` is never named, invoked, or on any path this test uses. The
only installed binary used is the **agent** (`NORN_BIN=~/.cargo/bin/norn`,
`norn 0.1.0`), which is the external tool under adoption, not the code under test.

### Isolation

Created fresh per test under `CARGO_TARGET_TMPDIR` — inside this worktree's
`target/`: the server's `AION_HOME` and haematite store, three reserved loopback
ports (HTTP/gRPC/liminal), the agent's `HOME` + `XDG_CONFIG_HOME` +
`XDG_DATA_HOME`, the agent's working tree. The provider is a loopback socket this
test binds. The estate server, `~/.aion`, the estate's `aion-data/`, live workers
and sibling worktrees were not touched at any point in this lane.

### The recorded result

```text
test a_paused_run_resumed_across_a_server_death_adopts_its_live_builder ... ok
test a_running_run_recovered_across_a_server_death_adopts_its_live_builder ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 13.52s
```

The durable trail the gate reads back (Paused face):

```text
WorkflowStarted, SearchAttributesUpdated, TimerStarted, ActivityScheduled,
ActivityStarted{attempt 1}, WorkflowPaused, ActivityAdoptionOffered{attempt 1},
WorkflowResumed, ActivityCompleted{attempt 1}, WorkflowCompleted, TimerCancelled
```

Asserted: status `Completed`; the literal bytes `superseded:server-death` absent
from the whole payload; exactly one `ActivityAdoptionOffered`; exactly one
`ActivityCompleted` and **its attempt is 1** (the retained identity, not a
successor); the pinned session id present in the run's own record; and exactly one
persisted agent session whose `created_at` is unchanged from the capture taken
while the builder was live mid-call.

That last one answers the coinciding-answers problem: the session id is templated
on the workflow id, so a re-created session answers the same id. The **open time**
does not coincide.

**Mutation control** (`gate-logs/follow-on/leg2-mutation-control.txt`) — #36's own
seq-6 record restored in `nif_activity_dispatch.rs`:

```text
test a_paused_run_resumed_across_a_server_death_adopts_its_live_builder ... FAILED
test a_running_run_recovered_across_a_server_death_adopts_its_live_builder ... FAILED
a live builder must never be recorded as superseded by server death:
a live builder must never be recorded as superseded by server death:
test result: FAILED. 0 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 12.29s
```

Both faces fail independently.

## Leg 3 — supersession absence and the adoption event

`crates/aion/tests/recovery_adoption_e2e.rs` (red `601563a98`, fix `d9d3e50dd`,
**corrected and re-cast** `ed04c529b` — see the finding above). Six tests, green,
covering the agent face (Paused and Running), the one-token ordinary control, the
fresh-ordinal control, the earlier-failure case and budget inertness.

**Single-writer law, checked rather than assumed.** The append path is
`nif_activity_dispatch` → `NifContext::record_activity_adoption_offered` (which
locks the run's one `Recorder`) → `Recorder::append_with`. Grepping the lane's
entire diff for an introduced `EventStore::append` / `.append(` returns nothing.

**Projection sweep.** Every site that classifies activity events was checked for
one left behind by the new variant. The exhaustive matches are semantically right:
the resolver groups the offer with `ActivityStarted` as *not a command outcome*;
`continue_as_new` counts the ordinal as still pending; `delegated` files it under
the Activity family; the name maps name it. One site reached it through a
catch-all — `aion-core`'s current-step fold — addressed below. The ops-console
wire bindings were a second such site and had not been regenerated (below).

## Leg 4 — the Running path

- Engine: `running_recovery_records_adoption_without_supersession` asserts
  identically to the Paused case.
- Server: `a_running_run_recovered_across_a_server_death_adopts_its_live_builder`
  runs the whole incident with **no pause anywhere** and no operator action after
  the restart — recovery alone — and asserts the same record.
- Both faces go red under the same mutation, independently.

## Leg 5 — readiness pin

`recovered_pending_await_has_no_false_error_deadline` asserted
`pending_await_deadline() == None` — a constant helper on no mutation path. It is
replaced by `a_live_recovered_process_may_reach_its_await_after_the_old_deadline`,
which runs the production `wait_for_pending_await` against a real recovered
process that reaches its await **after** the old budget would have expired, with a
fixture control asserting the process is still alive at that moment.

Its red (`a88008ae0`) is the literal prior defect restored:

```text
runtime::handle::readiness::tests::a_live_recovered_process_may_reach_its_await_after_the_old_deadline ... FAILED
Runtime { reason: "recovered workflow process 1 did not reach its pending await within 400ms" }
test result: FAILED. 0 passed; 1 failed; EXIT=101
```

Green on this tree:

```text
test runtime::handle::readiness::tests::a_live_recovered_process_may_reach_its_await_after_the_old_deadline ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 689 filtered out; finished in 0.70s
```

## Work beyond the five legs

1. **`eb1b8da4c`** — the server stream selector arm; the inherited head did not
   build without it.
2. **`26b31c674` + `34b4d55c2`** — `aion-core`'s current-step fold reached the new
   event through its catch-all. Behaviour was already right; nothing said so, in a
   module whose docs enumerate what every activity event does to the fold. Now the
   docs name it and a pin fails against both mistakes (retiring the ordinal, or
   re-opening it):

```text
---- current_step::tests::an_adoption_offer_leaves_the_ordinal_exactly_as_dispatched stdout ----
Error: "the adopted ordinal is still the current step"
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 116 filtered out; finished in 0.00s
```

   `34b4d55c2` corrects my own first attempt: I wrote the arm out with an empty
   body and clippy called it (`match_same_arms`) — correctly. The guard is the
   pin, not the arm.
3. **`ed04c529b`** — the agent-scoping correction (above), plus two collateral
   repairs the same battery surfaced:
   - `apps/aion-ops-console/src/types/generated/index.ts` regenerated. The new
     event is a wire type and the change had landed at one end only: *"the
     committed ops-console bindings are STALE … (committed 46957 bytes, exporter
     produces 47276 bytes)"*.
   - `crates/aion-integration-norn/tests/norn_e2e.rs` repaired — see F6.
4. **`0fff34d71`, `ded583b7e`** — two decompositions, both demanded by clippy
   (`too_many_lines`) and both fixed at the source.
5. **`5b4c14034`** — the canonical extraction battery script, copied verbatim, and
   the proof it can go red.

## Findings

**F1 — the cross-generation redelivery is adopted at the session layer, not by the
in-process single-flight.** In the leg-2 run the worker process survives the
server's death but the liminal connection does not. The first execution's dispatch
dies with the connection; the redelivery arriving on the new connection is a fresh
execution, which reaches the spawn gate, finds the marker naming its own
`(workflow_id, activity_id)` held by a live agent, adopts it, and re-attaches with
`--resume-if-exists`. Observable consequence: the provider sees **two** calls and
the tree briefly holds **two** `norn` processes — but exactly **one session**, and
one completion at the retained attempt. This is why the gate asserts one SESSION
rather than one provider call: asserting the latter would assert something the
design does not promise. Lane 1's `AgentFlights` single-flight covers the
same-connection face, which is a different one.

**F2 — the mutated run still Completed.** Under leg 2's mutation control, with the
false `superseded:server-death` back in place, the run still finished
successfully. The falsehood is invisible in the operator's outcome and visible
only in history — exactly the state leg 4 names, and the argument for gating on
the RECORD rather than the verdict. A gate asserting only `Completed` would have
passed against the defect.

**F3 — recovery re-offers on every generation, by construction.**
`history_needs_activity_adoption` treats `ActivityAdoptionOffered` as
non-settling, so a run crossing two server deaths receives two offers. Correct and
truthful, but it means the "exactly one adoption offer" assertion is about a
**single** crossing, not an invariant of the event class.

**F4 — nothing about leg 1 was prohibitively hard.** The charter asked that
difficulty be written up rather than absorbed as delay. The honest answer is the
opposite of the freeze-window hypothesis: the fixture is cheap and I re-verified
it plus a mutation control in under two minutes of machine time. What was genuinely
absent from lane 1 was leg 2, and that took the bulk of this lane's effort — mostly
in mapping the real serving path (`aion worker agent` plus the AWL `harness`
section), not in anything being hard.

**F5 — the five-gate regression** (above). The general lesson is the one the
charter already states and lane 1 did not execute: the battery is not a formality
at the end, it is the only thing that sees a change's blast radius. A lane that
never runs it can land a correct-looking fix that deletes a ruling.

**F6 — a protocol change that landed at one end, red and invisible since
2026-08-14.** `norn_e2e::norn_harness_drives_a_real_run_end_to_end` required a
completed run's payload to be a bare JSON string. #205 (`bc6703ef4`, "an agent
outcome names the conversation that produced it") widened it to the canonical
record and changed `translate.rs`/`session.rs` **without touching this reader**.
The test has therefore been failing for anyone with `NORN_BIN` set ever since, and
passing (as an announced skip) for everyone else:

```text
Error: "a schema-less completed run's output is a JSON string, got {\"text\":\"hello from the mock provider\",\"final_message\":\"hello from the mock provider\",\"stop_reason\":\"end_turn\",\"session_id\":\"cebbc59f-…\"}"
```

Repaired against the contract `AgentSession::wait_result` documents. The wider
point: an env-gated test is invisible to the default battery, so its red can
survive indefinitely — the gate that catches this class must run with the gate
variables SET, which is why every battery in this lane ran with `NORN_BIN`.

**F7 — a differential gate that disagrees under load.**
`awl_bc4_differential::covered::covered_set_is_byte_identical` failed in the full
battery with ten divergences in `dag-fork/valid/after_multi_diamond`, all of the
shape `event[9] /data/activity_id: reference=2 direct=1` — the two engines
assigned the fork's ordinals in opposite order. It passes solo (70.61s). So it is
not a regression from this lane, but it is a real fragility: a byte-identity
differential over a parallel fork can be decided by scheduling, and on a loaded box
it is. Worth a board row against whoever owns BC4.

**F8 — the norn e2e litters a spawn marker into the source tree.** Running
`aion-integration-norn`'s e2es with `NORN_BIN` set leaves
`crates/aion-integration-norn/.aion-held-execution.json` behind — untracked, NOT
gitignored, so a careless `git add -A` would commit a spawn-gate lock into the
repository. Its pid is dead by then, and the gate cleans a dead marker on the next
acquire, so it is harmless in operation and a hazard only in the repo. I removed
it before every commit. A one-line `.gitignore` entry, or a tempdir workspace like
the cross-process test uses, would close it.

## Isolation and hygiene ledger

- No estate server, `~/.aion`, estate `aion-data/`, live worker or sibling worktree
  was read or written. My isolated servers bind ports reserved by the test process
  itself; the estate server's downtime window (12:04:39Z–12:10:49Z) could not have
  been noticed by them and none of my failures point at it.
- Every server, worker and agent this lane started was created under this
  worktree's `target/` and reaped. After the leg-1 mutation control left one orphan
  (the failed run never reached its teardown), I identified it by its mock-provider
  argv and pid and killed it; a `ps` sweep afterwards showed no `norn` on the box.
- A sibling lane's battery (`wa-envelope-delta`) was running concurrently. It was
  identified by its cwd and left strictly alone.
- **Freeze windows disclosed by the coordinator** (disk emergency, not a fault of
  this lane): #1 11:45:59Z–11:46:18Z (19s), #2 11:59:31Z–12:12:16Z (~12.7 min). My
  final batteries both started AFTER window #2 closed (12:17:55Z and 13:29:58Z), so
  no result in this report spans a freeze. The three suspect nextest failures in the
  12:17:55Z run were nonetheless re-run solo before being dismissed, per the
  standing instruction — all three passed (F7 covers the interesting one).
- No `#[allow]`, `#[expect]`, `#[ignore]`, `#[cfg(any())]` or lint bypass of any
  kind was added. Four clippy findings landed on my own code and all four were
  fixed at the source (`34b4d55c2`, `0fff34d71`, `ded583b7e` ×2).
- No allocation instrumentation was run at any point.
- `cargo fmt --all` run before committing; no fmt gate was run as a gate.
- Never pushed, never merged, no git config touched.
- Files owned by other live lanes were **not** edited. `aion-integrations` (the
  shared mock-harness territory named in the dispatch) was READ ONLY, to learn how
  a harness reads its prompt; no file in it appears in this lane's diff.

## Non-claims

- **The costly proofs did not run.** The repository gates them behind
  `AION_RUN_COSTLY_TESTS` and announces each skip by name; the battery ran in the
  repository's default posture, so the 3-node cluster kill-9 failover and
  parked-timer resume proofs are announced-but-unproven by this run. Untouched by
  this lane.
- **The house battery (`scripts/battery.sh`) was not run.** The amended charter
  replaces it with the extraction standard below, so the Python/TypeScript SDK
  proto-drift gates, the posture controls, the wire-type gates and the all-features
  test gate are not established by this run.
- **No review was performed.** This is a builder's report.
- **The leg-2 gate is runtime-gated on `NORN_BIN`.** Unset, it announces
  `ISSUE36_E2E_SKIPPED (PROVES NOTHING)` on the real stderr handle and returns
  `Ok`, as the crate's existing agent e2es do. Every result quoted here was produced
  with `NORN_BIN=/Users/tom/.cargo/bin/norn` set, including both full batteries. A
  default `cargo nextest run` does not establish leg 2.
- **Agent-session continuity is proven for `norn` only.** The ACP adapter's
  equivalent path is untested by this lane.
- **The agent/ordinary fork is proven at the class boundary, not at every call
  site.** `declares_agent_action` reads the pinned contract; a package that carries
  no contract answers `false` and keeps the #266 posture. That degradation is
  deliberate and documented, but it means a contract-less legacy package's agent
  activity would be superseded rather than adopted.

## Battery — the canonical extraction standard

Per the in-flight amendment, the battery is
`/Users/tom/Developer/ablative/docs/GATE-EXTRACTION-STANDARD.md`, copied verbatim
to `gate-logs/follow-on/gate-extraction.sh` (the copy's body was checked
byte-identical to the doc's fenced block, not eyeballed) and run from the worktree
root at the final tree, with `NORN_BIN` set so the agent gates actually execute.
**Green = EMPTY extraction.**

### The instrument was proven able to go red first

One deliberate compile finding and one deliberate failing test, both reverted
immediately after (`gate-logs/follow-on/instrument-proof.txt`, commit
`5b4c14034`):

```text
{"type":"clippy","level":"warning","file":"crates/aion-core/src/current_step.rs","line":93,"column":9,"lint":"unused_variables","message":"unused variable: `deliberate_instrument_probe`"}
{"type":"clippy","level":"warning","file":"crates/aion-core/src/current_step.rs","line":93,"column":9,"lint":"unused_variables","message":"unused variable: `deliberate_instrument_probe`"}
{"type":"clippy","level":"error","file":"crates/aion-core/src/current_step.rs","line":93,"column":9,"lint":"unused_variables","message":"unused variable: `deliberate_instrument_probe`"}
{"type":"clippy","level":"error","file":"crates/aion-core/src/current_step.rs","line":93,"column":9,"lint":"unused_variables","message":"unused variable: `deliberate_instrument_probe`"}
{"type":"nextest","test":"aion-core::aion_core$current_step::tests::deliberate_instrument_probe_must_fail","stdout":"\nthread 'current_step::tests::deliberate_instrument_probe_must_fail' (2475667) panicked at crates/aion-core/src/current_step_tests.rs:423:5:\nassertion `left == right` failed: the extraction instrument must report this test\n  left: 1\n right: 2\n","message":"test failed"}
```

All three legs emit: check and clippy each reported the compile finding, and
nextest reported the failing test with its panic text. An empty extraction from
this script is therefore evidence.

### Run 1 — 12:17:55Z, head `5b4c14034`: six findings

```text
{"type":"clippy","level":"error","file":"crates/aion/src/runtime/nif_activity_dispatch.rs","line":222,"column":1,"lint":"clippy::too_many_lines","message":"this function has too many lines (112/100)"}
{"type":"clippy","level":"error","file":"crates/aion/src/runtime/nif_activity_dispatch.rs","line":222,"column":1,"lint":"clippy::too_many_lines","message":"this function has too many lines (112/100)"}
{"type":"clippy","level":"error","file":"crates/aion-integration-norn/tests/norn_e2e.rs","line":153,"column":7,"lint":"clippy::too_many_lines","message":"this function has too many lines (105/100)"}
{"type":"nextest","test":"aion-cli::aion$worker_shell::tests::cancellation_reaps_the_entire_spawned_process_group","stdout":"Error: ParseIntError { kind: Empty }\n","message":"test failed"}
{"type":"nextest","test":"aion-rs::aion$runtime::nif_query::nif_query_tests::delivery_queues_pending_query_and_wakes_the_workflow_process","stdout":"Error: Delivery { reason: \"query enqueue failed: … process 1 is not ready\" }\n","message":"test failed"}
{"type":"nextest","test":"aion-rs::awl_bc4_differential$covered::covered_set_is_byte_identical","stdout":"… 10 DIVERGENCES … DIVERGENCE dag-fork/valid/after_multi_diamond event[9] /data/activity_id: reference=2 direct=1 …","message":"test failed"}
```

Disposition: the two clippy findings are mine and were fixed at the source
(`ded583b7e`). The three nextest findings were re-run SOLO, as required, and all
three passed — contention artifacts on a box carrying three concurrent batteries
(F7 records the one worth a board row):

```text
worker_shell::tests::cancellation_reaps_the_entire_spawned_process_group ... ok  (1 passed)
runtime::nif_query::nif_query_tests::delivery_queues_pending_query_and_wakes_the_workflow_process ... ok  (1 passed)
covered::covered_set_is_byte_identical ... ok  (1 passed, 70.61s)
```

### Run 2 — the FINAL tree

> **Collector's addendum (Vesper Lynd, 2026-08-17, at collection).** The builder
> died mid-report to API weather — three consecutive 529 deaths after the battery
> finished — with the work complete and this section unwritten. Everything below
> this note is the collector's words, written from the artifacts; everything above
> it is the builder's.

Run 2 started 13:29:58Z at head `ded583b7e` ("refactor(36): split the two
functions the canonical battery named") and completed ~13:41Z (the extraction
artifact's final write). The extraction (`gate-logs/follow-on/battery-final.txt`)
contains **exactly one finding**:

- `aion-rs::awl_bc4_differential$covered::covered_set_is_byte_identical` — ten
  divergences, all in `dag-fork/valid/after_multi_diamond` events 9/10, the two
  engines assigning the fork's `activity_id` ordinals (and the matching result
  bytes) in opposite order. This is the known load-sensitive fork-ordering flake
  (board row aion#42; F7 above records this lane's first sighting). This is its
  fourth sighting overall.

**Disposition — solo re-run at collection, per the standing instruction**
(`gate-logs/follow-on/battery-final-solo-rerun.txt`):

```text
PASS [  71.380s] (1/1) aion-rs::awl_bc4_differential covered::covered_set_is_byte_identical
Summary [  71.384s] 1 test run: 1 passed (1 slow), 28 skipped
```

Contention artifact, dismissed with a receipt; the underlying fragility remains
aion#42's to close.

**Completion-evidence caveat, stated honestly:** the run-2 stderr artifact is
zero bytes (the extraction script's per-leg redirects) and the builder died
before capturing the nextest Summary line, so run 2 carries no independent
completion proof. It is not the battery of record: the collector's post-rebase
battery on the landed tree — run with completion evidence kept — supersedes it.
