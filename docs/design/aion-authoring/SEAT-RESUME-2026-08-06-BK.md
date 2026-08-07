# SEAT RESUME — marker BK (2026-08-06)

Supersedes **BJ**. BJ carried review 9's twelve closures. **Review 10 REFUSED**
them on two blockers and six findings. This marker records what review 10 got
right, the one thing it got wrong, and how each item was closed.

Reviews 6, 7, 8, 9 and 10 have all refused. Nothing is committed.

---

## What review 10 verified INDEPENDENTLY (and it matters)

It did not take BJ's word. It rebuilt the two headline measurements in an
**isolated scratch crate**:

- **D-1 reproduces.** Restoring `pub` on both `CompletionRetryConfig` fields
  turns the `compile_fail` twin red with exactly the recorded message.
- **The retracted `compile_fail,E0451` claim is correctly retracted.** On
  rustc 1.97.1 a snippet failing on an unresolved import still passes with the
  code attached. Removing the annotation was right.
- **MA-3 is airtight.** It swept for every door into a degenerate ladder —
  serde, `#[derive(Default)]`, bytemuck, `mem::zeroed`, functional-update
  syntax, the in-crate test module — and found none. The deletion of
  `check_completion_retry` and `EngineError::InvalidCompletionRetry` is clean:
  nothing in crates, `sdks/`, `gleam/`, examples or benches referenced them.

That is the standard. A review that re-derives beats a review that reads.

---

## Blockers — both closed

### B1 — the declared gate did not run the lane's own new tests. **REAL.**

The lane modified `aion-client` (+517 lines, essentially all new tests) and
`aion-store` (`refusing_reads.rs`, 311 lines with its own `#[test]`s), and
declared green on `cargo test -p aion-rs`, **which runs none of them**. Clippy
compiled them; nothing executed them.

**Closed by running `cargo test --workspace`** — see GATES below. Getting there
took a disk decision, recorded because the method is reusable:

- 18 GiB free, `target/` alone 90 GiB. Athena freed 10.41 GiB (her own frame
  build cache — not a git object, outside the deletion hold) → 28.32 GiB.
- She recommended Dean's box, on two measurements of her own: a cold `--workspace`
  frame battery drew 16.29 GiB whole-disk, and ⭐ **only 9.18 of that landed in
  `target` — 44% landed OUTSIDE any target-scoped `du`** (`~/.cargo`,
  `node_modules`, `$TMPDIR`). A ceiling estimated by watching `target`
  under-reserves by about a third.
- I ran it locally anyway, but **under a hard disk floor** rather than on an
  estimate: `scratchpad/guarded-workspace-build.sh` polls `df` every 15 s and
  kills the build at 8 GiB free, exiting **99** — a distinct code, so a disk
  abort can never be read as a compile failure or a pass. `df` cannot attribute
  a change to one consumer, but it is the right instrument for a BOUND.
- Measured increment: ~6.4 GiB. Floor never approached.

### B2 — `config.rs` contradicted itself about the per-attempt read cost. **REAL.**

Line 176 said a failing attempt does "several history reads"; line 183 costed the
30 s ceiling at "two reads a minute", which is **one** read per attempt. The doc
disagreed with itself, and the wrong copy inflated the runaway magnitude that
justifies the ruling.

**Fixed by subtraction.** The per-attempt cost is now stated **once** —
"a failing attempt spends exactly one history read", with the mechanism
(`handle_process_exit_attempt`'s first fallible call past the epoch gate is the
history read) — and both magnitudes are derived from it. It also names the O5
count assertion as the test that will catch it if it stops being true.

⚠️ **BJ's MI-2 entry was false as written.** It said the stale "several reads"
comment "is corrected". One copy was; this one survived. Recorded here because
the lane's whole subject is claims that outrun their evidence.

---

## Findings

### F1 — the tokio citation named a version this workspace has never linked. **REAL.**

`engine_tasks.rs` cited `tokio-1.53.1`. `Cargo.lock` pins **1.52.3**. Every line
number was also wrong, and `shutdown()` was cited at 303 — which is inside
`drop_join_handle_slow` in both versions.

**Verified at my own hand against the vendored 1.52.3 and rewritten.** The
property HOLDS; only the attribution was wrong:

| path | mechanism (1.52.3 line numbers) |
|---|---|
| normal completion | `store_output` (549) sets `Stage::Finished` over `Stage::Future`, dropping it; `complete()` (331) reaches `transition_to_complete()` (334) after |
| cancellation | `shutdown()` (240) → `cancel_task` (500) → `drop_future_or_output()` (503), then `complete()` |
| join-handle drop | `drop_join_handle_slow` (287) drops at 303 |
| panic | `poll_future` routes it into the same `store_output` guard (547-550) |

The comment now states the PROPERTY as the claim and the citations as pinned to
1.52.3, with a 🔴 that a version bump silently invalidates them and nothing in
the build will say so.

### F6 — the wheel-timer tests could go red blaming `Drop` for a timer that fired legitimately. **REAL, and the worst of the set.**

The fixture armed at `now + 150 ms`. The release test has to reach
`drop(engine)` inside that window. On a loaded box the deadline passes first,
`TimerFired` lands honestly, and the decisive assertion reports it as a
surviving task — **a red that reassigns blame**, which is worse than no test.

**Fixed** with a named margin (`WHEEL_TIMER_ARMING_MARGIN`, 2 s) plus an
explicit **fixture guard**: before the drop, assert both that the deadline has
not passed and that no `TimerFired` has landed, with messages that name the
FIXTURE and prescribe widening the margin — explicitly *not* evidence about
cancellation.

**Two mutations, two distinguishable reds:**

| # | Mutation | Result |
|---|---|---|
| W | collapse the margin to 1 ns | RED at the **fixture** assertion, quoting both timestamps |
| A′ | `remove(&key)` without `abort()` (the real defect) | RED at the **cancellation** assertion, past the guard; positive control green |

A′ is the one that matters: it proves the new guard does **not** mask the defect
it sits in front of.

### F4 — the fixture extraction detached a test's entire rationale. **REAL.**

The `///` block documenting `dropping_an_engine_without_shutdown_disarms_the_live_timer_wheel`
had been spliced onto the extracted helper (visible as a missing blank line),
leaving the test with no doc at all. **Split back**, and the reference to "the
test above" replaced with the sibling's actual name, since the helper now sits
between them.

### F7 — the named faithful mutation could not compile. **REAL.**

`engine_tasks.rs` named "deleting `monitor_pid` from `CompletionRetryKey`" as
the faithful mutation. The test constructs the key with that field and the
control reads it, so that mutation does not compile — **a NON-RUN: neither a
kill nor a survivor.** The doc now names the executable equivalent actually run
(keep the field, hand-write `PartialEq`/`Hash` to ignore it) and states why the
literal prior text is unavailable here.

### F3 — BJ's "STILL OPEN" was stale. **ALREADY DISCHARGED.**

Review 10 read a BJ snapshot taken before I extended it. The MI-3 and F-i
sections were added at the end of that tick; the marker it refused already
documented both as closed. No action.

### F8 — client/server error-table divergence. **NOT A DEFECT — verified.**

`EngineTaskEpochClosed` has an arm in `aion-server`'s tables and none in
`aion-client`'s `map_engine_error`. That omission is **deliberate and
documented** at `embedded.rs:552`, on the ground that the variant is reachable
only through the completion monitor. I checked the load-bearing claim rather
than the assertion: `grep` finds **exactly one construction site**
(`completion.rs:296`). The argument holds.

(Review 10 said it falls through to `backend`; it falls through to `server`.
Immaterial to the analysis.)

**But nothing enforced the single-site claim**, so I put it where a change would
be made: a 🔴 block on `refuse_if_epoch_closed` stating that another crate's
deliberate omission depends on this staying the only construction site, and that
a second one reachable from a client-facing operation would silently take the
`_ =>` catch-all.

### F9 — `handle.rs` at exactly 500 code lines. **MEASURED, NOT CHANGED — and here is why.**

The lane took it **468 → 500**. It consumed the whole margin itself.

I did **not** split it. CLAUDE.md says nothing *over* 500; 500 complies. Unlike
MA-3 — where the invariant was genuinely bypassable and a doc edit was rightly
refused as "a documented hole is not a closed one" — **there is no hole here**.
Manufacturing a mechanical move of a 110-line function with delicate teardown
ordering, five reviews deep, would add risk to close nothing. Recorded so the
next person to touch this file knows they must split it first.

### F10 — comment reflow damage from the MA-4 edit. **REAL.**

`cargo fmt` does not reflow comments, so it could not catch these. Three
mid-sentence splices fixed in `handle.rs` (one 128-char line reading as a
splice, plus two more of the same class from the same edit).

---

## GATES AT THE FINAL TREE

- `cargo fmt --all` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — **EXIT 0**.
- `cargo test -p aion-rs` — **1113 passed / 0 failed / 0 ignored**, 60 verdict
  lines, 0 non-ok, **EXIT 0**. (Counted by verdict LINE, not prose.)
- `cargo test --workspace` — **RAN TWICE. Not green, and I am not calling it
  green.** Both runs failed at the same place, and nowhere else:

  | run | verdict lines | passed | failed | outbox_e2e |
  |---|---|---|---|---|
  | 1 | 139 | 2673 | 1 | `Backend("SQLite failure: bad parameter or other API misuse")` |
  | 2 | 138 | 2666 | 0 | **SIGSEGV — binary died, emitted NO verdict line** |

  🔴 **RUN 2 IS THE LESSON. A crashed test binary emits no verdict line, so the
  naive tally reads `0 failed` while a whole binary is dead.** Tallying
  passed/failed alone would have declared run 2 GREEN. The tell is the LINE
  COUNT: 138 where run 1 had 139. Count verdict lines and reconcile them against
  the binaries that were supposed to report — a leg that never runs cannot fail,
  and its silence is indistinguishable from a pass.

- **The failure is #85, not this lane.** Attribution, not inference:
  - `docs/design/aion-authoring/LIBSQL-DOUBLE-CLOSE-85.md`, first committed
    **2026-08-01 (64aa16c99) — five days before this lane** — names an
    *intermittent* failure in `crates/aion/tests/outbox_e2e.rs`, quotes this
    exact signature, and identifies the trigger as all eight tests running
    concurrently under cargo at `worker_threads = 4`.
  - The two faces are one defect: a use-after-free presents as a bogus
    API-misuse error or as a segfault depending on what the freed memory held.
    Two contended runs, two faces, same binary.
  - `outbox_e2e` in ISOLATION: **5 runs, 5 green.** Isolation is a different
    condition, so this does not by itself attribute — it is only consistent.
  - The lane cannot reach it. `outbox_e2e` runs against a real `LibSqlStore`;
    the lane's ONLY `aion-store` change is a doc comment plus two lines
    exporting a new **test-only** module (`refusing_reads`) that `outbox_e2e`
    does not use. Nothing under `outbox` or `libsql` is touched at all.

  ⇒ Everything this lane touched is green across both runs. The one red is a
  documented, root-caused, pre-existing defect with an open task.

- Tom's pids 45425 (server) / 48399 (worker) alive before AND after both
  batteries, elapsed advancing normally — untouched. No test binds :8080; the
  references are parsed config fixtures and CLI e2e tests that rewrite the port
  dynamically.

## STILL OPEN

- Workspace battery result → then **review 11** on Opus.
- On a PASS: commit + push, files named EXPLICITLY (never `git add -A` in this
  tree — it swallows the NO-SWEEP untracked snapshot dirs and hangs). Commit
  message via `git commit -F -` with a quoted heredoc. Derived file list in
  `scratchpad/commit-manifest-7-lane.txt`, **re-derive before using**.

## OWED TO TOM

- Reviews 6–10 all refused; say so plainly, and say that review 10 found a real
  gate hole (B1) that the earlier ones missed.
- #146 tutorial dry run + one-page crib sheet, without touching his pids.
- Then #5 (`SPEC-5-AWL-SUM-COMBINATOR.md`), #1, #6, #9.
- #150 (config server surface) — MA-3 hands that lane a cleaner seam: it parses
  config, calls `try_new`, and maps `InvalidCompletionRetryLadder` to its own
  wire error, rather than inheriting an `EngineError` variant only `build()`
  could raise.

## METHOD WORTH KEEPING

**Athena's gate-scope note, which is the same class as B1.** Frame shipped a
declared gate that ran fewer legs than its config declared, and nobody noticed
for eight days because nothing compared **recorded against declared**. Frame's
runner now asserts that as two explicit numbers *even when they match*. A leg
that never runs cannot fail, and its silence is indistinguishable from a pass —
which is exactly how `-p aion-rs` read green over ~800 lines it never executed.
