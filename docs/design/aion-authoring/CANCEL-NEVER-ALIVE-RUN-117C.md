# #117(c) — cancelling a run that was never alive

**Ruling: YES, narrowly, for never-alive runs only, under five binding requirements.
Mechanism (B) — a registry-held terminal-writer reservation — confirmed as the discharge
of requirement (1).** Waffles, relayed by Cally, 2026-08-01.

**Status: BUILT. All five requirements discharged.** Requirement (5) landed first, as the
seam test, before the feature existed (`ff9431bc`); (1)–(4) follow below.

---

## The problem

`lifecycle::terminate::cancel` (`terminate.rs:105`) opens with `registered_handle()` then
`handle.recorder()`. No residency ⇒ no Recorder ⇒ `WorkflowNotFound`. That is load-bearing
invariant #3 working exactly as designed. The hole is that there is no branch for a run
that can **never** obtain a writer: its pinned package no longer loads, so it can never be
made resident, so it can never be cancelled, so it is `Running` forever — and cancel, the
operator's only lever, is the one thing refused.

## What was eliminated, by measurement, before anyone reached for it

The natural fix is "give it a handle so it has a Recorder." **It is dead.**
`WorkflowHandleParts` (`registry/handle.rs:97`) requires `pid: u64` and
`loaded_version: ContentHash` — precisely the two things an unrecoverable run cannot
supply. Fabricating them corrupts three independent subsystems that read those fields as
*facts*:

| site | what a fabricated field does |
|---|---|
| `engine/startup.rs:186` | the recovery loop skips any run with a live pid — the skip becomes permanent |
| `engine/reload.rs:310`, `engine/admission.rs:181` | a handle's `loaded_version` enters the live-version set gating package unload — a fabricated hash mis-pins a version (#101) |
| `runtime/handle/delivery.rs:615` | routing believes there is a process to deliver to |

> **The cheap route is worse than the expensive one.** Worth establishing before the
> expensive one has to justify itself.

## The mechanism — (B), ruled

A **terminal-writer reservation** in the registry. Not a handle: no pid, no
`loaded_version`, no residency, invisible to `live_pid`. Keyed by `(id, run)` and
**mutually exclusive with a handle for that pair**.

The registry stays the single arbiter of who holds a writer, so *exactly one writer* is
true **by construction** for the terminal case rather than argued from circumstance. The
race that option (A) would have caught after the fact never opens.

Option (A) — minting a Recorder directly against the store — is foreclosed by requirement
(1)'s own words: a second mint path at the store is not the same primitive. Its safety
argument resolves at `SequenceConflict`, which yields the design law this produced:

> **An alarm is never a load-bearing component.** A design whose correctness argument
> routes through `SequenceConflict` is asking the detector that exists to *report* the bug
> to also *prevent* it.

## The five requirements, and where each lands

1. **Same writer-exclusion primitive; preconditions re-proven under lock, measured not
   cited; STOP if reload succeeds.** The reservation acquisition IS the under-lock point.
   Both preconditions — non-resident, and *reload refuses right now* — are re-measured at
   reservation time, not carried in from an earlier observation. If the package loads, the
   run is recoverable and cancellation goes through the ordinary path.
2. **Terminal-append-only, citing the engine's remembered non-viability reason.** The
   reservation licenses exactly one append and then ends. The reason comes from
   `registry.unrecoverable().get(&id)` — the typed verdict landed in `7ead877a`'s sibling
   `7c9a2329`.
3. **The verb names the true state.** Never "not found." The run exists; it cannot be run.
4. **Rehearse on a fixture copy; first live case is `0756ecd5`.**
5. **Boot honours cancellation before minting a writer.** ✅ **DISCHARGED BELOW.**

## ✅ Requirement (5), discharged first

`a_cancelled_run_forced_into_the_active_list_never_has_a_writer_minted`
(`engine/startup_tests.rs`).

Mechanism (B) produces a history shape nothing else produces: `WorkflowStarted` then
`WorkflowCancelled`, with no process ever having existed. If boot made such a run resident,
the next restart would undo the cancellation and the operator's only lever would have been
a no-op. So this had to hold **before** the writer that creates the shape exists at all —
which is why the ruled discharge order puts it first.

An existing test already proves this at the **store filter**. This proves it one layer
deeper, at the guard the filter normally hides (#121): the cancelled run is forced into
`list_active` by `StaleActiveListStore` and the recovery loop must refuse it on its own
account.

**Control, run:** with `if false && projected_status.is_terminal()` the test fails on its
own assertion. `startup.rs` restored to sha256
`0759b697afe80fae80a5758c37b251b8eec3923baff4b750b8e334ed5613f3d7`, matching its pin, with
no `if false` remaining.

The instrument that made this reachable was built an hour earlier for #121, for an
unrelated reason. Worth noting: **the guard-layer test for requirement (5) was not
buildable before #121 existed** — through an honest store, a cancelled run never reaches
the loop.

## ✅ Waffles' design note, verified rather than asserted

> *The reservation should be process-local and non-durable — a crash between reservation
> and append must leave the system exactly where it was.*

**It is, by construction, and the registry needs no change to make it so.** `Registry`
(`registry/table.rs:39`) is `#[derive(Debug, Default)]` over three in-memory mutexes —
`handles`, `index`, `unrecoverable` — and holds **no store handle and no persistence of any
kind**. It is rebuilt empty at every boot by `Default`.

So a crash between reservation and append leaves: no reservation (the registry is gone) and
no append (the store never received one). Atomicity lives entirely at the store append,
which is already atomic under `expected_seq`. **A reservation that could itself strand would
be the defect class reborn one level up** — that is exactly the risk the note names, and the
registry's existing semantics close it.

## ✅ Requirements (1)–(4), built

### (1) The exclusion is structural, not a check

`registry::terminal_writer::TerminalWriterReservation` — an RAII guard borrowed from the
registry. No pid, no `loaded_version`, no residency; invisible to `live_pid`,
`live_run_pid`, `get`, and `list`.

The load-bearing change is in `registry/table.rs`: the handle map and the reservation map
now live in ONE `Slots` struct behind ONE mutex (the old `handles` mutex, renamed `slots`).
Two mutexes would have made the exclusion a rule two critical sections co-operate to keep.
One makes it a property of every operation that can create either, because no path can
observe or mutate one without holding the other's lock too. `insert` and `insert_if_absent`
refuse with `TerminalWriterHeld`; `reserve_terminal_writer` refuses with
`TerminalWriterUnavailable`.

**The exclusion is WORKFLOW-scoped, not `(workflow, run)`-scoped.** The ruling keys the
reservation by `(id, run)` — it does — but a `Recorder` writes the WORKFLOW's event stream,
so a handle on any run of the same workflow is still a second writer. A `(workflow, run)`
check would have waved that through, and it is reachable: the continue-as-new window holds
two runs at once. `Slots::any_handle_for` therefore scans the handle map rather than
consulting the `index`, because the index has one slot per workflow and legitimately goes
empty while an older handle is still live.

**Release is `Drop`, and that is the point.** A reservation that could be taken and never
released would wedge the workflow against every future writer for the life of the process —
this defect class reborn one level up, which is exactly the risk Waffles' design note named.
There is no release call for a caller to forget.

### (2) Preconditions re-measured, and the STOP

`lifecycle/cancel_never_alive.rs`. Non-residency is proven by the registry under its own
lock — the reservation IS that proof. The package probe runs through
`durability::pinned_package_if_loaded`, which shares its resolution steps with
`ActiveWorkflowRecoverySeamImpl::recover_active_workflow` via one extracted
`resolve_pinned_package`, so the question asked is the one recovery answers rather than a
lookalike that could drift silently. If the package resolves, the path STOPS with
`RunIsRecoverable`.

One honest window: the catalog read and the append are not atomic with each other. It
cannot produce a second writer — the reservation holds for both — so its worst case is
cancelling a run that became recoverable a microsecond ago, which is what the operator
asked for. The dangerous failure is closed structurally; this one is not dangerous.

**Stated limitation:** the probe is RESOLUTION, not a spawn. A run unrecoverable because
spawning fails will resolve and be refused. That is the conservative direction, and
deliberate — making the probe a real spawn would leave a resident process behind on a
cancellation path. `0756ecd5`'s measured cause is resolution failure.

### (3) Exactly one terminal transition

`record_cancelled` **consumes** the reservation, and there is no other way to reach the
recorder it holds. The type licenses one transition and ends. That transition is the same
shape every other terminal writer records: `WorkflowCancelled`, then the run's outstanding
declared-timeout deadline retired under the same recorder — a cancelled run must not leave
an armed deadline behind.

### (4) The verb names the true state — in BOTH directions

Four typed refusals, each mapped at the server to `INVALID_STATE` (gRPC `FailedPrecondition`
/ HTTP 409) and never to 404, because in every one of them the run exists and its history is
readable:

| refusal | what is true |
|---|---|
| `NoResidencyVerdict` | the run exists; this engine recorded no reason it is not resident |
| `RunIsRecoverable` | its pinned package resolves now — use the ordinary path |
| `TerminalWriterUnavailable` | the workflow already has a writer |
| `TerminalWriterHeld` | a reservation holds the slot (transient, seen by `insert`) |

Accuracy is a demand in both directions. A workflow id nothing was ever started under is
**still** `WorkflowNotFound` — renaming that would be the same dishonesty pointed the other
way. And a run that already recorded a terminal is told exactly that, ahead of everything
else, because it is the most informative true statement about it.

## What an existing test caught, and the change it forced

`deadline_and_cancel_race_records_exactly_one_terminal` went red. Not a flake — a real
consequence: a resident run whose deadline fires while a cancel is in flight deregisters
between the cancel's registry lookup and this branch, so the cancel now arrives at a
TERMINATED run with no handle. It was getting `NoResidencyVerdict`, which is true but
useless.

The fix went in the code, not the assertion: the triage read now refuses on a recorded
terminal **before** asking about verdicts. The race's honest answer is *"already recorded a
terminal event"* — strictly better than the `WorkflowNotFound` it got before this lane
existed.

**Then that assertion flaked too, and the second answer was the more interesting one.**
Pinning the single label passed 600 isolated iterations and failed under a loaded
`--all-targets` run: there is a real window in which the handle is gone and the winning
teardown's terminal is not yet visible to this reader, and in that window
`NoResidencyVerdict` is the true statement. Both refusals are honest; which one you get
encodes this box's scheduling, not a property of the system.

So the test now pins the **invariant**: the loss is one of those two refusals, it is
**never** `WorkflowNotFound` — a run whose history the next statement reads is not missing —
and exactly one terminal is recorded either way.

> A hardcoded expected value encodes the harness's timing. **Assert the invariant, not the
> enumeration** — and a label that only holds on an unloaded box is an enumeration.

## The controls, run

Seven inverted controls, each RUN, each killing **exactly** its own tests and leaving the
rest green:

| mutation | what died |
|---|---|
| `any_handle_for` always `None` | the workflow-scoped exclusion tests (3, incl. the cancel branch) |
| `refuse_if_terminal_writer_held` no-op | `a_reservation_and_a_handle_are_mutually_exclusive` |
| `Drop` release removed | 5 tests — the leak wedges the workflow, exactly as predicted |
| `if false && loaded.is_some()` | the STOP test only; its no-package control stayed GREEN |
| `if false && triage.is_empty()` | `a_workflow_that_never_existed_is_still_not_found` |
| `reject_if_wrong_run` result discarded | the stale-run-id test |
| triage terminal guard discarded | the deadline race test, and only it |

The STOP pair is the sharpest: the two tests differ **only** in whether the catalog holds
the pinned version, so a `cancel` that refused everything would pass one and fail the other.

Pins re-verified byte-identical after every restore; `grep "if false"` returns only doc
comments.

## Files

| file | what |
|---|---|
| `crates/aion/src/registry/terminal_writer.rs` | NEW — the guard |
| `crates/aion/src/registry/table.rs` | `Slots` behind one lock; reserve/release; both `insert` paths refuse; 5 tests |
| `crates/aion/src/lifecycle/cancel_never_alive.rs` | NEW — the branch, four conditions |
| `crates/aion/src/lifecycle/cancel_never_alive_tests.rs` | NEW — 8 tests |
| `crates/aion/src/lifecycle/terminate.rs` | `cancel` routes on no-handle; `catalog` on the context |
| `crates/aion/src/durability/recovery.rs` | `resolve_pinned_package` extracted; `pinned_package_if_loaded` |
| `crates/aion/src/error.rs` | 4 new typed refusals |
| `crates/aion-server/src/error.rs` | wire mapping — all four `INVALID_STATE`, never 404 |

## ✅ The operator-facing surface — BUILT in its own increment

`GET /workflows/unrecoverable`. See
[`UNRECOVERABLE-RUNS-SURFACE-117.md`](UNRECOVERABLE-RUNS-SURFACE-117.md).

This lane gave an operator who knows the id a working lever; that one gives them the id.
Between them #117 is closed, with one limitation stated there: the surface is HTTP-only —
no gRPC method and no CLI verb, because the CLI reaches the server over gRPC and that means
a proto change wider than the increment.

---

# 📍 RESUME HERE — written at compaction #3 (2026-08-01)

## Tree state

**21 commits ahead of `origin/main`, all local and unpushed. No uncommitted crate code.**

| commit | what |
|---|---|
| `a1782ce9` | #122 closed — dependents gate green, three red runs made durable in-repo |
| `2ea67fef` | **#121 CLOSED** — both recovery-loop guards reached; `StaleActiveListStore` built |
| `9ccee6f9` | #88 — deploy-refusal built, measured at 14 broken tests, **REVERTED** |
| `ff9431bc` | **#117(c) requirement (5)** discharged as the seam test |

## 🔴 THE ACTIVE LANE — #117(c), RULED AND UNBLOCKED

Waffles ruled via Cally 2026-08-01: **YES narrowly for never-alive runs, five binding
requirements, mechanism (B) confirmed.** (A) and (C) are FORECLOSED — do not re-open them.

**Done:** requirement (5) (the seam test, ruled to come first) and the process-local /
non-durable verification Waffles asked for. Both above, both committed.

**Next, in order — requirements (1)–(4):**

1. The **terminal-writer reservation** type in `crate::registry`. Not a handle: no pid, no
   `loaded_version`, no residency, invisible to `live_pid`. Keyed by `(id, run)` and
   **mutually exclusive with a handle for that pair** — that mutual exclusion IS the
   discharge of requirement (1), so it must be enforced by the registry, not by a check.
2. Preconditions re-proven **at reservation time, under the lock, measured not cited**:
   non-resident, AND *reload refuses right now*. **STOP if reload succeeds** — then the run
   is recoverable and cancellation goes the ordinary route.
3. Exactly ONE terminal append, citing `registry.unrecoverable().get(&id)` (landed
   `7c9a2329`), then the reservation ends.
4. The verb names the true state — **never "not found."**
5. Fixture rehearsal; first live case is `0756ecd5`.

## Method notes that cost something to learn tonight

- **The only thing that has ever caught a vacuous test here is an inverted control that was
  actually RUN.** Every guard test in these commits has one, and each kills exactly its own
  test.
- **`startup.rs` pin: `0759b697afe80fae80a5758c37b251b8eec3923baff4b750b8e334ed5613f3d7`.**
  Re-verify after any mutation; `grep "if false"` must return nothing.
- **A denominator inherits the scope of the thing you counted in.** #85 is **3 red in 8 on
  the Block G final tree (37.5%)** — NOT 11, which pooled four trees. Corrected against me.
- **A correct caveat does not survive being added to a wrong total.**
- **A fix that is option-independent in its EFFECT can still be option-deciding in its
  BLAST RADIUS** (#88, above).
- **An alarm is never a load-bearing component.**
- Truncation is a lossy channel: it can delete evidence, never fabricate it. **Completeness
  is required to certify a PASS, not a FAILURE** — but a red licenses "this failed", never
  a rate.
- The background-task wrapper reported **exit 0 for a cargo that exited 101, twice
  tonight.** Index on the in-log `EXIT=` line before anything else.
- **Declare regimes to Athena up front** (any `--workspace` or unusual command); she clears
  them in a line. Twice tonight she had to chase an undeclared pid.

## Board after this

#88 (needs its (a)/(b) ruling — the floor I tried was not option-independent), #71, #76,
#117's operator-facing surface (own increment, unblocked in every branch), #85 (open
investigation, `sqlite3_errmsg` lead, evidence in-repo at
`evidence/85-outbox-flake/`).

---

# 📍 RESUME HERE — written at compaction #4 (2026-08-01)

## Tree state

**#117(c) is BUILT and COMMITTED.** Requirements (1)–(5) all discharged.

Gates at the commit, each indexed by its own exit code:

```
cargo clippy --workspace --all-targets -- -D warnings     EXIT 0
cargo fmt --all --check                                   EXIT 0
cargo test -p aion-rs --lib --no-fail-fast                EXIT 0, 614 passed / 0 failed
cargo test -p aion-rs --all-targets --no-fail-fast        EXIT 0, 58 result lines, all ok
```

⚠️ The `--all-targets` run went RED once before the race-assertion fix (the flake described
above) and green after. The background-task wrapper reported **"exit code 0" for that red
run** — third time that trap has appeared. The `EXIT=` line inside the log is the authority.

## Board after this

- **#117's operator-facing surface** — list the unrecoverable runs and why. Own increment,
  unblocked in every branch. This lane gives an operator with the id a working lever; it
  does not yet give them the id.
- **#88** — still needs its (a)/(b) ruling. The floor I tried was not option-independent.
- **#71**, **#76**.
- **#85** — open investigation, `sqlite3_errmsg` connection-level-state lead, evidence in
  `evidence/85-outbox-flake/`.

## Method notes worth keeping

- **`-p <crate>` and `--workspace` resolve different feature sets AND different crates.**
  Adding four `EngineError` variants compiled clean under `-p aion-rs` and broke
  `aion-server`'s two exhaustive matches. Only the workspace run saw it.
- **Fix the code, not the lint.** `too_many_lines` fired on `engine_trace_fields`; the
  answer was extracting `durability_trace_fields`, which was a nested question inside a flat
  dispatch anyway — not an `#[allow]`.
- **A refusal path must release its reservation too.** The STOP test asserts
  `terminal_writer_run == None` after the refusal; without it a STOP would wedge the
  workflow, which is this defect class reborn.
