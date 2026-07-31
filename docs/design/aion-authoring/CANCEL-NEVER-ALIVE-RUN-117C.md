# #117(c) — cancelling a run that was never alive

**Ruling: YES, narrowly, for never-alive runs only, under five binding requirements.
Mechanism (B) — a registry-held terminal-writer reservation — confirmed as the discharge
of requirement (1).** Waffles, relayed by Cally, 2026-08-01.

**Status: requirement (5) DISCHARGED as the seam test, before the feature.** The design
below is written and not yet built.

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

## Not yet built

The reservation type, its mutual exclusion with `insert`, the cancel branch, requirements
(1)–(4). Stated as outstanding rather than implied complete.

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
