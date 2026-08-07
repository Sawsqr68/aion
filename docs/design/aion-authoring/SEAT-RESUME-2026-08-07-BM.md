# SEAT RESUME — marker BM (2026-08-07)

Supersedes **BL**. Nothing is committed. Reviews 6–11 have all refused; review 12
has not been dispatched.

BL's Blocker 1, Finding 3 and Finding 5 are **CLOSED WITH CODE**, each proven.
BL's Blocker 2 is **IN FLIGHT**. Everything BL recorded as VERIFIED POSITIVELY
still stands and is not re-litigated here.

---

## BLOCKER 1 — CLOSED. The mechanism was NOT what BL guessed.

BL's leading hypothesis was "a sibling caches `Interest::never()` and our
`set_default` cannot heal it". Half right, and the half that was wrong mattered:
BL proposed `rebuild_interest_cache()` as the fix. **Measured: it does not
work** — 2 green of 5. The proposal would have shipped a remedy that leaves the
defect live 60% of the time.

### What was measured, in order

1. **Reproduced pristine**: `cargo test -p aion-rs --lib -- lifecycle::completion`
   → RED, RED, ok. Genuinely flaky, not deterministic.
2. **Discriminating probe** (canary `warn!` in the test's own file + capture
   totals) → 6 of 6 runs: `max_level=TRACE`, `total=1`, `canary=true`. The
   subscriber, the thread and the global level are all FINE. **Only the canary
   was captured; `lifecycle::completion_retry` emitted nothing.**
3. **`rebuild_interest_cache()` after `set_default`** → 2 of 5 green. Not a fix.
4. **`eprintln!` probe inside the loop itself** → 3 of 3 runs: same `ThreadId` as
   the install, 64 attempts genuinely run, `at_ceiling=true`,
   `first_sight=true` on 1, 2 and 64 — and
   **`tracing::event_enabled!(Level::WARN)` = `false` on every attempt.**

The loop is correct and asks to speak. Tracing refuses at the callsite.

### The mechanism, read out of the vendored source

`tracing-core-0.1.36/src/callsite.rs`:

- `rebuild_callsite_interest` (line ~490) ends
  `interest.unwrap_or_else(Interest::never)` — **an empty dispatcher registry
  caches `never`, process-wide, for that callsite.**
- `DefaultCallsite::register` (line ~308) takes its `DISPATCHERS.rebuilder()`
  snapshot BEFORE it stores the result, so a sibling's first hit of a callsite
  can **overwrite** an `always` that our own `Dispatch::new` rebuild had just
  written. Last writer wins.

That is why it is flaky, why it is green alone and at `--test-threads=1`, and
why a rebuild at install time only moves the odds.

### The fix — `crates/aion/src/log_capture.rs`, by SUBTRACTION

`InterestFloor`: a global-default subscriber registered once per process that
**captures nothing and decides nothing** — `register_callsite` →
`Interest::sometimes()`, `enabled` → `false`. The registry is then never empty,
so no fold can reach the `never` default in any interleaving. `sometimes` refers
every event back to the EMITTING thread's own subscriber, which is exactly the
thread scoping the module already promised. `LogCapture::new` is now fallible
(`Result<_, String>`) because installing the floor is — a caller must never be
handed a capture that can silently record nothing.

Both call sites threaded: `lifecycle/completion.rs`, `loader/declared_queues_tests.rs`.

The module header's one-directional claim is corrected: thread scope cuts BOTH
ways, and **the inward direction — another thread making THIS thread's emissions
invisible — is the one that bit.**

### Proof

| tree | `-- lifecycle::completion` | paired with the wheel-timer test |
|---|---|---|
| pristine (defect) | RED, RED, ok | reviewer: 3/3 RED |
| **fixed** | **10 / 10 green** | **6 / 6 green** |
| **mutant** (floor + rebuild deleted, literal prior behaviour) | **4 of 6 RED** | — |

The mutation is the literal prior text, not built from the fix's new machinery:
the `Result` wrapper stays, only `ensure_interest_floor()?` and the rebuild are
removed. `crates/aion/src/log_capture.rs` restored and verified by `shasum -c`.

⚠️ One residual window is named IN THE CODE rather than hidden: a callsite whose
one-and-only registration began before the floor existed and stores after the
rebuild. That is a race inside `tracing-core`, at most once per callsite per
process, and it fails LOUD — an empty capture asserts.

---

## FINDING 3 — CLOSED. The false claim is gone and the true range is stated.

`crates/aion/src/runtime/config.rs` no longer says "a failing attempt spends
exactly one history read". It states the measured range and where each number
comes from:

- **one** on the fail-fast path — `lifecycle/completion.rs:382`;
- **up to three** on the recorded-then-bookkeeping-failed path —
  `lifecycle/visibility.rs:25` and `lifecycle/completion.rs:546`, both
  classified retryable by `store_error_is_transient`.

Magnitudes re-derived: 125 attempts/s is 125 reads/s on the fail-fast path and
**up to 375** on the other; the 30 s ceiling is two attempts a minute and
therefore **two to six** reads a minute.

🔴 The false guard claim is deleted and replaced with what is actually true:
**the O5 proof guards the first number and CANNOT SEE the second** — its fixture
fails the FIRST read, so the loop never reaches the post-append reads and the
assertion is invariant to their cost. `FlakyStore::fail_reads_after` is named as
the shape that *can* reach that path, so the gap is stated as testable-but-untested
rather than papered over.

The same over-broad claim was scoped in three more places inside the O5 test
itself (its budget comment, its truncation comment, its assertion message) —
BL only knew about the `config.rs` copy.

---

## FINDING 5 — RULED: KEEP. The premise was right, the scoping was not.

`crates/aion/src/lib.rs` re-exporting `aion_package::{ContentHash,
ContractIdentityError}` is **load-bearing for this lane**, not out of scope:
`aion-client` declares no `aion-package` dependency at all, and this lane's own
`transport::embedded` error-mapping tests construct
`aion::ContentHash` / `aion::ContractIdentityError` directly. Without those two
lines the crate cannot build the `EngineError` values it exists to translate.
The derivation is now written into the comment so nobody redoes it.

## FINDING 4 — accepted with the caveat, no code change.

F6 is **mitigated, not eliminated** — an awaited `read_history` sits between the
fixture guard and `drop(engine)`. BK's "Fixed" was the overstatement. The guard
cannot mask the defect and the sleeping is justified; the wording is corrected
here and BK is superseded.

---

## BLOCKER 2 — CLOSED. Reconciled recorded-against-declared WITHIN one run.

`cargo test --workspace --no-fail-fast`, log
`scratchpad/battery-workspace-3.log`. The declared list was derived from the
same tree first (`cargo test --workspace --no-run --message-format=json`,
`scratchpad/declared-binaries.txt`) so the comparison is inside one run rather
than between two — the exact method failure that produced BK's false claim.

| quantity | value |
|---|---|
| declared test binaries | **223** |
| `Running` lines (binaries STARTED) | **223 — exact match** |
| doc-test targets | 20 → **243 top-level targets** |
| `test result:` verdict lines | **247** |
| totals | **4199 passed / 1 failed / 0 ignored**, exit **101** |

The 4-line surplus is accounted for, not waved past: three are `aion_server`'s
lib-test binary re-invoked as a CHILD process by its own gate tests (`1 passed;
795 filtered out`) and one is a nested `Doc-tests aion` run. **Every surplus is
an extra report under an existing header; no header is missing a report.** That
is the direction that matters — a crashed binary emits no verdict line at all,
and none is absent here.

🔴 **`aion_store` and `aion_server` BOTH RAN.** Those are the two BK falsely
declared green without executing. `aion_store`'s `refusing_reads.rs` and
`aion-server`'s edited `error.rs` are now covered by a run that reached them.

**The single failure is #85 and is attributed:**
`outbox_failure_fails_fast_and_cancels_unresolved_siblings`,
`crates/aion/tests/outbox_e2e.rs`, `Store(Backend("SQLite failure: `bad
parameter or other API misuse`"))` — the libsql double-close signature, first
committed `64aa16c99` on 2026-08-01, five days before this lane, and isolated
5/5 green. This lane touches nothing under outbox or libsql. No SIGSEGV face
this run.

## GATES ON THE FINAL TREE

- `cargo test -p aion-rs --no-fail-fast` → **60 verdict lines, 1113 passed / 0
  failed, exit 0** (`scratchpad/battery-aion-rs-2.log`). Run separately from
  `--workspace` on purpose: feature resolution differs, so both are required.
- `cargo test --workspace --no-fail-fast` → above.
- `cargo clippy --workspace --all-targets -- -D warnings` → **exit 0, zero
  error/warning lines** (`scratchpad/clippy-workspace.log`).
- `cargo fmt --all` run over the whole tree.

---

# 🔴 REVIEW 12 REFUSED. 5 blockers, 14 findings. Reviews 6–12 have ALL refused.

It verified 12 of BM's claims as true — including, checked against vendored
source rather than trusted, that `Interest::sometimes()` survives the `and` fold
in **both** orders and that `set_global_default` leaks the `Arc` so the floor's
registrar upgrades forever. It independently re-measured 10/10 and 5/5 green,
and confirmed the whole Blocker-2 battery arithmetic. It also caught **three
false numbers written by me in this same tick.**

## CLOSED THIS PASS

**B5 — variant count. CONFIRMED FALSE, FIXED.** `EngineError` has **51**
variants, not 52. Counted at my own hand two ways (variant declarations inside
the enum body; `#[error]` attributes), no duplicates, not `#[non_exhaustive]`.
`aion-client/src/transport/embedded.rs` corrected in both places. The count is
the entire justification for the `_` wildcard, so a wrong count is a wrong
justification.

**B1 — the read-cost range was STILL FALSE. CONFIRMED, FIXED.** I wrote "one to
three". The `ContinuedAsNew` branch spends **four to six**, and I verified every
site by reading the code:

| # | site | reached via |
|---|---|---|
| 1 | `completion.rs:382` | the attempt's own read |
| 2 | `visibility.rs:25` | `completion.rs:491` (skipped only for `TimedOut`) |
| 3 | `completion.rs:546` | `reconcile_terminal_registry` |
| 4 | `completion.rs:571` | `start_continuation_replacement`'s own read |
| 5 | `start_admission.rs:122` | `workflow_identity` via `start.rs:158` |
| 6 | `visibility.rs:25` | the successor's own upsert, `start.rs:190` |

This is ordinary continue-as-new, not an edge case, so under a sick store EVERY
attempt of that run's loop pays it. Magnitudes re-derived: **125/s → up to
750/s**; **2/min → up to 12/min**. 🔴 Twice now I have given a read-cost number
arrived at by reading the first branch and stopping. The doc now carries the
enumeration and says so.

**B3 — the third `refuse_if_epoch_closed` site had no test. CONFIRMED, FIXED.**
New test `the_continuation_start_refuses_once_the_epoch_is_closed`. It calls
`start_continuation_replacement` directly, and that is forced: the gate at the
top of `handle_process_exit_attempt` refuses first, so the window this site
exists for — the epoch closing BETWEEN the two gates — is unreachable through
the arrival path.

| tree | new test | the two PRE-EXISTING epoch tests |
|---|---|---|
| fixed | **ok** | ok |
| gate at `completion.rs:567` deleted | **FAILED** | **still ok (2 passed)** |

🔴 That second column is the positive control and it is the finding: the two
existing tests are **green with the gate deleted**, so they never reached the
site. Review 12's B3 is exactly right. Restore proven by `shasum -c`.

**B2 — O5's mutation evidence. PARTLY UPHELD, DISCHARGED BY RE-RUNNING.**
Review 12 is right that a red landing on `attempts.len() >= 3` is
indistinguishable from the flake, so **mutation A was NOT citable**. It is wrong
about **B** and **T**: their recorded verdicts carry captured values
(`[1,2,4,8,16,32,64]`, `left: 10`) that an empty capture could not produce.
All three re-run on the fixed tree under the harsh filtered invocation
(`-- lifecycle::completion`), restore `shasum -c`-proven after each:

| mutation | verdict on the fixed tree |
|---|---|
| **A** — emission unreachable (`if false && …`) | **RED**, `[]`, killed at the vacuity control |
| **B** — `at_ceiling \|\| first_sight` → `first_sight` | **RED**, left `[1,2,4,8,16,32,64]` vs right `[1..7]` |
| **T** — `&& attempts <= 10` suffix cut | **RED**, left `10` vs right `64` |

A's kill is citable **now and was not before**, and the reason is the whole
point: the unmutated baseline in that same invocation is 10/10 green, so `[]`
is attributable to the mutation rather than to a lost capture.

**B4 — a 1.1 MB raw agent transcript was on the commit list. CONFIRMED,
EXCLUDED.** `docs/design/aion-authoring/SPEC-5-AWL-SUM-COMBINATOR.md` is
1,125,284 bytes of JSONL under a `.md` path — measured: the only untracked doc
in the tree whose first byte is `{`, and 25× the next largest. It is issue #5's
artefact. It is now in the manifest's own **EXCLUDED — OUT OF LANE** section
with the reason written in, and rewriting it is #5's first job. Not deleted:
nothing is deleted under the estate hold.

## STILL OPEN FROM REVIEW 12 — F1..F14

Not yet addressed. The substantive ones: **F3** (the two new `aion-server`
arms have no test), **F9** (`query_error_class`'s `Engine(_)` arm has no table
row, so a mutation survives green), **F1/F2/F4** (stated reasons that name a
retry policy and a wire class the code does not have), **F6** (`log_capture`'s
"one copy, deliberately" is false — `durability/recorder.rs:1142` carries a
third capture), **F7** (the O5 failure message still misattributes), **F5**
(`InterestFloor`'s `max_level_hint` pins the process `LevelFilter` at TRACE,
undocumented), **F8/F10/F13/F14**.

## NOT REACHED BY ANY REVIEW — DO NOT READ AS CLEARED

🔴 **Whether `engine/api_workflow_ops.rs` (539 lines) and
`engine/builder_assembly.rs` (302 lines) are pure moves is UNVERIFIED.** Review
12 dispatched an audit for it that never returned. It did establish that
`Drop for Engine` is entirely NEW and `Engine::shutdown` was REWRITTEN inside
that same diff — both touching invariant 3 — so "it's a split" is already known
to be false in part. This is the largest unchecked surface in the lane.

## ORDER OF WORK ON RESUME

1. F1..F14.
2. Audit the two "split" files against `api.rs`/`builder.rs` at HEAD.
3. Re-run both batteries + clippy on the final tree.
4. **Review 13** on Opus.
5. On a PASS only: commit with files named EXPLICITLY (never `git add -A`).

## STANDING CONSTRAINTS (unchanged)

- Tom's pids **45425** (server) / **48399** (worker): do not touch. Verified
  alive at the start of this tick.
- Disk ~20 GiB free. No cold builds. The guarded build script exits **99** on a
  disk abort so it can never be read as a compile failure.
- ⛔ Three untracked `.aion-*` dirs in the repo root are NO-SWEEP holds.

## OWED TO TOM

Reviews 6–11 have ALL refused. Reviews 10 and 11 each found a real gate hole the
earlier ones missed — 11 found that my own declared discharge of 10's blocker was
false. This tick found that BL's own proposed remedy for Blocker 1 would not have
worked, and only measurement caught it. Then: #146 tutorial dry run + crib sheet
(without touching his pids), then #5, #1, #6, #9.
