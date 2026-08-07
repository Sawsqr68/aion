# SEAT RESUME — marker BL (2026-08-06)

Supersedes **BK**. **REVIEW 11 REFUSED.** Reviews 6–11 have all refused.
Nothing is committed. Nothing lands until a review passes.

🔴 **BK CONTAINS TWO FALSE CLAIMS OF MINE. Both are recorded below. Do not
re-read BK for its verdicts — read this file.**

---

## BLOCKER 1 — the O5 obligation-proof is FLAKY. REPRODUCED AT MY OWN HAND.

`crates/aion/src/lifecycle/completion.rs:2010`
`at_the_ceiling_every_failing_attempt_states_itself` — the single test that BK,
`RULING-COMPLETION-RETRY-BOUND-2026-08-06.md` and `config.rs` all name as the
collector of obligation O5 — **fails depending on which siblings run.**

Measured by me, unmodified tree:

| invocation | result |
|---|---|
| `cargo test -p aion-rs --lib -- lifecycle::completion` | **3 of 3 RED** |
| full `cargo test -p aion-rs --lib` (654 tests) | green |
| reviewer: `--test-threads=1` | green |
| reviewer: paired with the wheel-timer test | 3 of 3 RED |

The red is at `completion.rs:2062`: *"fewer than three attempts failed, so no
ladder-silent attempt was ever reached…"* — with **`stated` coming back `[]`**.

🔴 **`[]` means NOTHING WAS CAPTURED AT ALL.** The message blames the emission
rule. The cause is measurement loss. **This is exactly the class BK's own F6
calls "a red that reassigns blame."** The proof of the lane's central obligation
is currently a test whose greenness is a property of which siblings ran.

### Where I had got to when compaction landed

- The capture is `crates/aion/src/log_capture.rs`, installed at
  `completion.rs:2028-2035` via `tracing::subscriber::set_default` (thread-scoped).
- `LogCapture::new` builds `Registry::default().with(CaptureLayer(..))`.
  `CaptureLayer` implements neither `register_callsite` nor `enabled`, so this
  subscriber IS interested in everything.
- **Leading hypothesis (reviewer's, NOT yet confirmed by me): tracing's callsite
  `Interest` cache is PROCESS-GLOBAL while the subscriber is THREAD-SCOPED.** A
  sibling test hitting the same `warn!` callsite from a thread with no
  subscriber can get `Interest::never()` cached for that callsite, after which
  the macro short-circuits and *this* thread's emissions never happen.
- `log_capture.rs:19-28` states the constraint in ONE direction only — "an
  emission made on a DIFFERENT thread … is invisible here". **It misses the
  direction that bites: another thread hitting the same callsite can make THIS
  thread's emissions invisible.** That doc claim needs correcting whatever the fix.

### Next experiment (do this first on resume)

Call `tracing::callsite::rebuild_interest_cache()` immediately after
`set_default` in the O5 test and re-run `cargo test -p aion-rs --lib --
lifecycle::completion` ×3. If it goes green, the mechanism is confirmed and the
fix belongs in `LogCapture` (rebuild on install), NOT in one test.
⚠️ Rebuild-on-install may still race a concurrent poisoner; if so the honest
remedy is a process-wide capture mutex serialising every test that uses
`LogCapture`, or a global subscriber with a test-scoped filter.
**Whatever the remedy: the O5 proof cannot ship as a test that is green only in
some sibling sets.** Confirm the mechanism by measurement before writing any
claim about it.

---

## BLOCKER 2 — B1 IS NOT CLOSED. 🔴 MY BK CLAIM WAS FALSE.

BK says *"Everything this lane touched is green across both runs."* **False.**

`cargo test --workspace` is **fail-fast by default.** Both runs ABORTED at
`tests/outbox_e2e.rs`, which is the **139th and last** `Running` line in both
logs. Everything cargo had not yet reached never ran:

- 🔴 **`aion_store` appears NOWHERE in either log.**
  `crates/aion-store/src/testing/refusing_reads.rs` — one of the exact two files
  review 10's B1 named — **was never executed by the run I declared closed B1.**
  (It is green in `scratchpad/test-aion-store.log`, 64 passed, on unchanged
  bytes — but BK did not cite that run, and the mechanism it DID cite is false.)
- 🔴 **`aion-server` never ran at all.** The lane edits
  `crates/aion-server/src/error.rs` (two new match arms). Its lib-test binary was
  COMPILED (the log carries `aion-server (lib test) generated 1 warning`) but
  never executed, and none of its 44 integration tests ran. **No test has run
  against that file since it changed.**
- `aion-toolchain` and `aion-worker` integration tests: none ran.
- 19 `crates/aion/tests/*.rs` sorting after `outbox_e2e` never ran in either
  workspace run. *That* gap is separately covered by
  `scratchpad/battery-aion-rs.log` (1113/0, all 59 targets, final tree). The
  four bullets above are NOT covered.

🔴 **THE METHOD FAILURE IS THE ONE BK ITSELF PREACHES.** I compared 139 vs 138
verdict lines *between* the two runs and never compared recorded-against-declared
*within* one run. I wrote "a leg that never runs cannot fail" into BK's own
"METHOD WORTH KEEPING" section in the same sitting. Knowing the rule is not
applying it.

### The fix
Re-run with **`cargo test --workspace --no-fail-fast`** so #85's known red does
not mask everything behind it, then **reconcile the target count against the
declared target list** — not just tally passed/failed. Expect #85 to fail again;
that is fine and attributed. Do it on the FINAL tree, after Blocker 1 is fixed.

---

## FINDING 3 — 🔴 B2's REPLACEMENT CLAIM IS FALSE. I DELETED THE TRUE COPY.

`crates/aion/src/runtime/config.rs:180` now states, in bold: **"a failing
attempt spends exactly one history read."** **That is false.**

Past the append, `handle_process_exit_attempt` calls:
- `upsert_workflow_visibility` → `crates/aion/src/lifecycle/visibility.rs:25`
  `event_store.read_history(...)`
- `reconcile_terminal_registry` → `crates/aion/src/lifecycle/completion.rs:542`
  `context.store.read_history(id)`

Both can fail `StoreError::Backend`, which `completion_retry.rs`
`store_error_is_transient` classifies **Retryable**. So an attempt on the
recorded-then-bookkeeping-failed path spends **2–3 history reads, every attempt,
unboundedly.**

🔴 **The lane's OWN `crates/aion/src/store_faults.rs:63-67` says exactly this:**
*"One operation can read history several times — the attempt's own read, then
the visibility upsert's, then the registry reconcile's."*

Review 10's B2 was that the doc contradicted itself. **I closed it by deleting
the TRUE copy and keeping the FALSE one.** The contradiction now spans two files
in the same lane instead of two lines in one.

Worse: `config.rs:185` names the O5 count assertion as what "will catch it if it
stops being true." **It cannot.** The O5 fixture makes the FIRST read fail, so
the attempt never reaches the post-append reads — the assertion is **invariant
to the quantity it is offered as a guard on**. (And per Blocker 1 it is not
reliably green either.)

### The fix
State the TRUE per-attempt cost: one read on the fail-fast path, 2–3 on the
recorded-then-bookkeeping-failed path. Re-derive the 125/s and 2/min magnitudes
against the honest number. **Delete the false claim that the O5 test guards it**,
or build a guard that can actually see the post-append reads.

---

## FINDING 4 (minor) — F6 is mitigated, not eliminated. BK overstates it.

`crates/aion/src/engine/builder.rs:1013-1041`: the fixture guard reads
`before_drop` then performs an **awaited** `store.read_history` before
`drop(engine)`, so a window remains between guard and drop. The in-code comment
is honest ("cannot plausibly exhaust"); **BK's "Fixed" is the overstatement.**
Reviewer's judgement, which I accept: the guard **cannot** mask the defect (with
`Drop` broken nothing has fired yet, so the guard passes and the post-sleep
assertion fires), and the ~6 s of sleeping is justified. **Accept with caveat —
soften BK/BL wording, no code change.**

## FINDING 5 (minor) — out-of-scope public surface

`crates/aion/src/lib.rs:96` newly re-exports
`aion_package::{ContentHash, ContractIdentityError}` — a public API change riding
in a completion-retry lane. Justified in comment, but out of scope for #7.
Decide: keep and declare, or split out.

---

## VERIFIED POSITIVELY BY REVIEW 11 — SETTLED, DO NOT RE-LITIGATE

- **F1 CLOSED.** Every tokio citation checked by hand against vendored 1.52.3:
  240, 287, 303, 331, 334, 500, 503, 547-550 — **all exact**; all four ordering
  properties hold; `Cargo.lock:5262` pins 1.52.3.
- **F8 CLOSED.** Exactly one construction site (`completion.rs:307`); the 🔴 note
  sits where the change would be made.
- **F7 CLOSED.** The named executable mutation genuinely reconstructs the prior
  key's collapse and is not built from the fix's new machinery.
- **F4 CLOSED.** Doc split; both tests carry their own.
- **F9 — reviewer AGREES with my call not to split `handle.rs`.** Its 500 count
  still includes `#[cfg(test)]` accessors; CLAUDE.md excludes tests, so
  production code is under 500. Every other file comfortably under.
- **#85 attribution SOUND** (`64aa16c99`, 2026-08-01, five days pre-lane).
- **Invariant 3** upheld: no new `EventStore::append`; terminal appends go
  through the `Recorder` under its lock, epoch gate before the branch.
- **Invariant 4** upheld: no stored status field.
- **No bypasses** anywhere in the diff: zero `unwrap()`/`expect(`/`panic!`/
  `todo!`/`#[allow]`/`#[expect]`/`#[ignore]` on any added line.
- **No invented limits**; `worker_threads(1)` is pre-existing at HEAD.
- Test scaffolding correctly gated (`#[cfg(test)]`; `refusing_reads` behind
  aion-store's `test-support`, an aion-client **dev**-dependency only).
- `-p aion-rs` on the final tree: 60 verdict lines, 1113 passed / 0 failed.

## REVIEW 11 DID NOT REACH

- `cargo clippy --workspace --all-targets`, `cargo fmt --check` (disk) — my
  EXIT 0 stands untested by it.
- Whether `api.rs`→`api_workflow_ops.rs` and `builder.rs`→`builder_assembly.rs`
  are pure moves.
- ~500 new lines of `aion-client` transport tests, beyond `map_engine_error`.
- Blocker 1's MECHANISM is "most probable", not certain. **The repro is fact;
  the cause is mine to confirm.**

---

## ORDER OF WORK ON RESUME

1. **Blocker 1** — confirm the mechanism by experiment, fix in `LogCapture` not
   in one test, correct the one-directional doc claim, and prove the fix by
   running the previously-red filter combinations.
2. **Finding 3** — state the true per-attempt read cost; kill the false guard claim.
3. **Finding 4/5** — soften the F6 wording; rule on the lib.rs re-export.
4. **Blocker 2** — `cargo test --workspace --no-fail-fast` on the FINAL tree,
   then reconcile recorded-against-declared targets.
5. `cargo fmt --all`, clippy, `-p aion-rs`.
6. **Review 12** on Opus, dispatched with BL as claims to falsify.

## STANDING CONSTRAINTS (unchanged)

- Nothing committed. On a PASS: files named EXPLICITLY, never `git add -A`.
  Manifest in `scratchpad/commit-manifest-7-lane.txt` — **re-derive before use**.
- Tom's pids **45425** (server) / **48399** (worker): do not touch. Verified
  alive and untouched through both workspace batteries.
- Disk ~22 GiB free. No full cold builds. Athena freed 10.41 GiB earlier and has
  nothing further of her own to give.
- ⛔ Three untracked `.aion-*` dirs in the repo root are NO-SWEEP holds.

## OWED TO TOM

Reviews 6–11 have ALL refused. Say so plainly, and say that reviews 10 and 11
each found a REAL gate hole the earlier ones missed — 11 found that my own
declared discharge of 10's blocker was false. Then: #146 tutorial dry run +
crib sheet (without touching his pids), then #5, #1, #6, #9.
