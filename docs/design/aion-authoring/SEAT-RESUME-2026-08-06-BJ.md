# SEAT RESUME — marker BJ (2026-08-06)

Supersedes **BI**. BI carried review 9's twelve open items. This marker records
which of them are now closed, how each closure was measured, and what is left.

Nothing is committed. Nothing lands until a review passes.

---

## Closed this tick, each with its mutation

### MA-3 — the real code hole. `check_completion_retry` was bypassable.

The ladder validator sat in `EngineBuilder::build`, and every step around it was
`pub`: `CompletionRetryConfig::new` with `pub` fields, `RuntimeConfig::
with_completion_retry`, `RuntimeHandle::new`. m2 closed it last tick with a doc
edit. A documented hole is not a closed one.

**Fix — subtraction.** The invariant now lives on the type:

- `CompletionRetryConfig`'s fields are **private**.
- `new` is gone; `try_new(initial, max) -> Result<Self, InvalidCompletionRetryLadder>`
  replaces it and does the validation.
- `initial_backoff()` / `max_backoff()` accessors.
- `InvalidCompletionRetryLadder` — a `thiserror` enum, two arms, exported from
  `runtime::mod` and `lib.rs`.
- `check_completion_retry` **deleted** from the builder, along with its call in
  `build()`, `EngineError::InvalidCompletionRetry`, and that variant's two
  mappings in `aion-server/src/error.rs`. A gate that can no longer be reached is
  zombie code.
- The two builder refusal tests **moved** to `runtime::config::
  completion_retry_ladder_tests`, where the refusal now lives.

`Default::default` is the one path that writes the fields directly (it cannot
fail, and this crate does not `unwrap` in library code). That gap is closed by
`the_default_ladder_is_one_the_constructor_would_accept`, which reads whatever
the default holds and puts it back through `try_new` — it never names 1 ms or
30 s, so a future change to the ruled numbers is checked rather than escaping.

**Mutations — all four isolating (each kills exactly one test, siblings green):**

| # | Mutation | Killed |
|---|---|---|
| M-a | delete the zero-floor arm of `try_new` | `a_zero_initial_backoff_cannot_be_named` |
| M-b | delete the ceiling arm of `try_new` | `a_ceiling_below_the_floor_cannot_be_named` |
| M-c | make the shipped default degenerate | `the_default_ladder_is_one_the_constructor_would_accept` |
| D-1 | restore `pub` on both fields (**literal prior text**) | the `compile_fail` doctest |

D-1 is the one that matters: privacy is what makes the constructor the only
door, and no unit test can observe privacy — a test that could name a degenerate
ladder would not compile, which is a NON-RUN, not a failure. Two doctests hold
it, identical but for one expression: a control that must compile and run, and a
`compile_fail` twin that swaps the constructor call for a struct literal. Under
D-1 the twin reported *"Test compiled successfully, but it's marked
`compile_fail`"*.

🔴 **A false claim was caught and removed on the way.** The first draft annotated
the twin `compile_fail,E0451` and said the code pinned the reason. **Measured:
rustdoc does not enforce it on this toolchain** — a snippet edited to fail on an
unresolved import still passed with the code attached. The annotation was
decoration reading as a gate. It is gone, and the doc now states what actually
protects the twin (the shared shape with the control catches a rename; a
hand-edit that breaks only the failing copy would not be caught) and names the
measurement.

### MI-1 — the M1 assertion claimed the TASK survives; it measured the MAP.

`shutdown_timer_wheel` both removes the entry and aborts the handle.
`armed_wheel_timers()` reads `pending_timers.len()`, which sees only the removal
— so deleting `pending.handle.abort()` left every test green, and **no test
anywhere covered that abort.**

**Fix.** The deadline moved from +1 hour to +150 ms, the fixture was extracted
into one shared helper (`arm_resident_run_with_wheel_timer`), and the release
test now also waits past the deadline and asserts no `TimerFired` reached the
store. A new sibling, `a_live_wheel_timer_fires_when_the_engine_is_not_released`,
is its positive control — same fixture, no drop, `TimerFired` present. An absence
is only evidence when its presence has been shown reachable by the same means.

**Mutation A** — `remove(&key)` without `abort()`: release test RED
("must CANCEL its wheel tasks, not merely forget them"), control green.

### MI-2 — the O5 test measured no-gaps, not no-truncation.

`[1,2,3]` is contiguous whether the loop failed three times or sixty-four and
fell silent after the third. Added `assert_eq!(attempts.len(), failing_reads)`,
**derived** from the outage budget: every failing attempt spends exactly one
history read, and the assertion measures that claim rather than assuming it.
The stale comment saying an attempt performs "several reads" was wrong and is
corrected.

**Mutation T** — `if (at_ceiling || first_sight) && attempts <= 10`: a pure
suffix cut. Contiguity stayed satisfied (`[1..10]`); only the new count killed it
(left: 10, right: 64). Isolating.

### MI-8 — `attempt_number` split captured lines on `|`.

A backend error string can contain `|`, and the captured `error=` field is the
value most likely to. **Fix — subtraction.** `LogCapture` no longer stores a
joined string: a `CapturedEvent` is a level and an unjoined field list, with
`field(name)` (exact prefix; tracing field names cannot contain `=`) and
`mentions(needle)`. The delimiter now exists only in `Display`, which no
assertion parses. The helper is deleted. Both call sites updated; the O5 field
checks got *stronger* on the way — `at_ceiling` is now `== Some("true")` rather
than a substring match.

### MA-4 · MI-5 · MI-6 · MI-7 · MI-9 — the comment and emission fixes

- **MA-4** `handle.rs`: the third stale enumeration of what `Drop for Engine`
  does is gone. It no longer repeats the list — the doc on `Drop for Engine` is
  the one authority — and states the shared PROPERTY the argument actually needs
  (none of them await; none run while the caller still holds the engine).
- **MI-5** `completion_retry.rs`: "Once the terminal event is in history the run
  IS terminal" was false after a `ContinuedAsNew` or a reopen. Narrowed to *was
  observed durable*, with the retraction stated and the distinction the variant
  exists to draw shown to survive it.
- **MI-6**: the duplicated tail-read paragraph — one copy deleted.
- **MI-7**: two misindented `monitor_pid` lines inside macro bodies (`cargo fmt`
  does not reach them).
- **MI-9**: `monitor_pid` added to the three emissions that lacked it — the
  engine-released abandonment warn and both `Invariant` arms. An abandonment is
  the message an operator most needs to tie back to a process.
- **MI-4**: the at-ceiling log rate IS the ceiling, so `try_new(1ms, 2ms)` is a
  legitimate config emitting ~500 warn/s. A throttle is forbidden (invented cap);
  the coupling is now STATED at the emission, with the number worked out.

---

### MI-3 — the control fired before the decisive assertion.

In `a_reopened_runs_successor_lease_can_arm_its_own_completion_retry`
(`engine_tasks.rs`), the control was `assert_ne!(predecessor, successor)` — the
same claim the decisive assertion tests. Under the faithful defect (a
`CompletionRetryKey` that stops distinguishing leases) the keys collapse, the
control fires first, and the decisive assertion never executes.

**Fix.** The control is now a statement about the FIXTURE — the two monitor pids
differ, and the workflow/run are the same — which survives the mutation.

**Mutation K** — hand-written `PartialEq`/`Hash` ignoring `monitor_pid` (field
kept so the literals still compile, so it is a run and not a NON-RUN): the test
now fails at **"THE DECISIVE ONE"** (`left: AlreadyArmed, right: Armed`), not at
the control.

### F-i — "the arguments are exactly the ones `arm_completion_retry` passes".

Held by diligence. The one argument that could silently differ is the ladder, so
the O5 test now reads it from `active.context.runtime.completion_retry()` — the
same expression production reads — instead of handing the loop the local it
configured the fixture with. The doc's over-broad claim is retracted and the rest
are named as what they are: this exit's own inputs. The test still reaches 64
attempts in 0.17 s, which is itself the proof the two sources agree.

## GATES AT THE FINAL TREE — BOTH GREEN

- `cargo fmt --all` clean.
- `cargo clippy --workspace --all-targets -- -D warnings` → **EXIT 0**.
- `cargo test -p aion-rs` → **1113 passed / 0 failed, EXIT 0**
  (1109 → 1113: −2 builder refusal tests moved out, +3 ladder tests, +1 wheel
  control, +2 doctests).
- ⚠️ `cargo test --workspace` NOT run at this tree. `--workspace` resolves
  `preserve_order` ON and `-p <crate>` off — different runs. Review 10 was asked
  whether it matters.

## STILL OPEN

- **REVIEW 10 IS RUNNING ON OPUS** (dispatched with BJ and BI as *claims to
  check*, not facts, and told the most valuable thing it can do is find one of
  them false). Reviews 6, 7, 8 and 9 all refused. Nothing lands until one passes.
- On a PASS: commit + push, files named EXPLICITLY (never `git add -A` in this
  tree — it swallows the NO-SWEEP untracked snapshot dirs and hangs). Commit
  message via `git commit -F -` with a quoted heredoc.
- On a REFUSE: fix properly, mutation-prove, re-gate, review 11.

## OWED TO TOM

- Reviews 6–9 all refused; say so plainly.
- #146 tutorial dry run + one-page crib sheet, without touching his pids
  (server 45425, worker 48399).
- Then #5 (`SPEC-5-AWL-SUM-COMBINATOR.md`), #1, #6, #9.
- #150 (config server surface) awaits Waffles' dispatch round — note that MA-3
  hands that lane a cleaner seam: it will parse config and call `try_new`,
  mapping `InvalidCompletionRetryLadder` to its own wire error, rather than
  inheriting an `EngineError` variant only `build()` could raise.
