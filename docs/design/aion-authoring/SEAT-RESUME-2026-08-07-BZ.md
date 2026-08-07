# Seat marker BZ — 2026-08-07

Supersedes BY. Read this first.

## §0 🔴 THE TREE IS MID-EDIT AND DOES NOT COMPILE

I was interrupted partway through the Review 19 remediation. **`crates/aion/src/runtime/nif_continue_as_new.rs` is in a broken intermediate state.** First job on resume: finish it or revert it. Do not run a battery until it builds.

What I already did in that file:
- **Added** `enum ContinuationOutcome { Complete, TerminalOnly(DurabilityError) }`.
- **Replaced** `refusal_must_end_the_process(&NifContextError) -> bool` with
  `outcome_must_end_the_process(&Result<ContinuationOutcome, NifContextError>) -> bool`
  (`Ok(_) => true`; `Err` => only `EngineTaskEpochClosed`).

**What is NOT yet done and is why it will not compile:**
1. `record_continuation` still returns `Result<(), DurabilityError>`. It must return
   `Result<ContinuationOutcome, DurabilityError>`, with the tail becoming:
   ```rust
   recorder.record_workflow_continued_as_new(...).await?;
   match crate::time::retire_run_deadline(recorder, &history, parent_run_id).await {
       Ok(()) => Ok(ContinuationOutcome::Complete),
       Err(error) => Ok(ContinuationOutcome::TerminalOnly(error)),
   }
   ```
   The pre-terminal checks (already-terminal `HistoryShape`, epoch `EngineTaskEpochClosed`) stay `Err`.
2. The NIF body still calls `.err().is_none_or(refusal_must_end_the_process)` — that
   symbol no longer exists. It becomes `if outcome_must_end_the_process(&recorded) {`.
3. The NIF's final `match recorded` must handle three cases: `Ok(Complete)` → `Ok(())`,
   `Ok(TerminalOnly(e))` → an error term naming the half-completed transition,
   `Err(e)` → `Err(context_error_term(...))`.
4. `nif_continue_as_new_tests.rs` still imports `refusal_must_end_the_process` and its
   test `only_the_epoch_refusal_ends_the_workflow_process` still calls it. Both must move
   to the new signature. The control test's `record_continuation(...).await?` now yields a
   `ContinuationOutcome` — assert it is `Complete` (strengthens the control).

## §1 Review 19 — REFUSE. Full text is durable at

`/private/tmp/claude-501/-Users-tom-Developer-ablative-stack-aion/5849e0d8-4802-4869-8e0f-9f7fd187e198/tasks/a70962032134afdac.output`

**Read it before acting.** Its F1–F9 table says: F1 CLOSED, **F2 PARTIAL**, F3 CLOSED,
F4 CLOSED, F5 CLOSED, F6 CLOSED, **F7 NOT CLOSED**, F8 CLOSED, F9 CLOSED.

### Finding A — BLOCKING. VERIFIED AT MY OWN HAND.

`record_continuation` performs **two independent durable appends**: I confirmed
`record_workflow_continued_as_new` (`recorder.rs:470`) and `record_timer_cancelled`
(`recorder.rs:825`, reached from `time/deadline.rs:138`) are each their own
`append_with`. If the terminal lands and the deadline retirement then fails with a
store error, the old predicate returned **false**, the process was **not** killed, and:
- the run is terminal-`ContinuedAsNew` with a live process still writing into it;
- **no successor is started** — `start_continuation_replacement` (`completion.rs:506`)
  is reached only from the process-exit monitor, so no exit means no successor;
- two writers on one history once the sweep later starts the successor. Invariant 3.

Pre-existing behaviour, NOT a regression — but the remediation wrapped it in a named
predicate whose doc asserted "any store error … are ordinary failures a workflow author
may legitimately catch", which is false for the post-terminal case. The `ContinuationOutcome`
work above is the fix.

### Finding B — BLOCKING, NOT YET STARTED. The F2 wiring is measured by nothing.

The reviewer mutated the NIF's `if` back to pre-remediation behaviour
(`.err().is_none()`) and got **669 passed / 0 failed** on `--lib` plus **13 passed / 0
failed** on the `continue_as_new` and `workflow_timeout_e2e` integration targets. They
restored and verified byte-identity. **The mutation is the harness** — a new test must
drive `continue_as_new_impl` against a LIVE beamr process with a CLOSED epoch and fail
when that wiring is reverted.

Note `crates/aion/tests/continue_as_new.rs` (231 lines) drives `Engine::continue_as_new`
with the epoch OPEN; there are three tests at lines 13, 78, 184. No existing test closes
the epoch against a live process.

**There is NO failing-store double anywhere in `crates/aion` or `crates/aion-store`** — I
grepped. One must be written to test the Finding A path (terminal lands, second append
fails). ~40 lines wrapping `InMemoryStore` and failing appends after a counter.

### Findings C, D, E, F — smaller, none started

- **C** (`nif_continue_as_new_tests.rs:244-252`): "CONTROL 2" says `RecorderPoisoned` is
  "reached through a DIFFERENT variant" — false. `block_on_recorder` maps via the only
  `From` impl, so **`Durability` is the sole variant reachable at that call site**;
  `UnknownProcess`/`TermEncoding`/`RecorderPoisoned` are structurally unreachable there.
  Keep the control (it kills a mutation) but say what it is.
- **D** (`nif_continue_as_new.rs`, the `cancel_pid` arm): a failing `cancel_pid` discards
  the epoch refusal's operator text via `?`.
- **E** (`engine/api.rs:216`): "asked to continue and was refused is now stopped" is
  unqualified; only the EPOCH refusal stops the process.
- **F** (`workflows/issue-recheck/README.md:58-67`): the run table records a control count
  with **no `control_symbol` column**, so the rows are unfalsifiable — in a document whose
  own thesis is that an attributed number needs its owner. Add the column.
  The reviewer independently reproduced every `aion-server/src` row exactly and confirmed
  the AWL body is injection-safe and correctly reasoned.

## §2 Commits — TWO UNPUSHED, PLUS THE ACTIONS WITHDRAWAL

- `9d259b185` chore(ci): withdraw both GitHub Actions workflows — **Tom-authorised**, stuck on top
- `88b010961` fix(7): the Review 18 remediation — **now refused by Review 19**
- `610ae17f7` feat(workflows): the first work-on-Aion lane + the issue record

🔴 **Nothing is pushed.** Do not push until Review 20 clears.

## §3 What is settled and must NOT be re-litigated

Review 19 independently verified as SOUND: F1's factual claims (deadline never retired by
the refusal; `recover_active_workflows_on_startup` only spawns; `recover_timers_on_startup`
fires overdue timers synchronously and awaited, after the spawn pass;
`sweep_continued_as_new_replacements` enumerates `ContinuedAsNew` only so nothing repairs
`TimedOut`); the F1 trade itself as defensible; F3/F4/F5/F8/F9's corrections; both
`EngineTaskEpochClosed` single-construction-site claims; totality of every `DurabilityError`
match (`error.rs:504`, `error_engine.rs:82`, `completion_retry.rs:826,879`);
`EngineTaskRuntime` shared by `Arc` so NIF gate and `Engine::drop` read one flag; the new
ordering (`cancel_pid` then term allocation) as no new hazard.

One residual to weigh, not blocking: the reviewer thinks "unearned exemption" is
rhetorically strong — the old path's deadline retirement was the *normal* semantics of a
continuation, and what the old path faked was the successor, not the retirement. Worth
softening in the doc.

## §4 Other state

- **Tom's rulings:** no GitHub Actions ever; start using Aion workflows AS the work, soon;
  he wants demonstrable value. The `workflows/` lane answers the last one.
- **beamr:** Artemis confirmed the JIT/NativeServices defect is FIXED ON MAIN at `4e94cec`
  (30 Jul), one day AFTER tag `v0.16.3` (`9d0d0e0`, 29 Jul) — so nothing published carries
  it. **Rev-pin unblocks long runs today.** I told Artemis I will do it once this lane
  vacates, because it touches the manifest/lockfile and would invalidate a review in
  flight. Their caveat: beamr's own wall canNOT observe `replay_driver` (zero on both
  sides of a set equality), so **my differential pin is the instrument** — re-cut it and
  confirm the 1p/1f → 2p/0f flip. Do NOT "fix" main's `0.16.2` manifest line; it is ruled.
- **Verified issues today:** #150 (all three knobs), #107, #136, #151 (reproduced live),
  #152 (both halves), #153. Recorded in
  `docs/design/aion-authoring/OPEN-ISSUES-WHOLE-PICTURE-2026-08-07.md` and in the task
  subjects.
- **My isolated server: pid 82997, ports 8097/50097, `/Users/tom/gate-verify-bx`.** Mine to
  stop. 🔴 **Tom's pids 45425 / 48399 / 50822 — DO NOT TOUCH.**
- **Method defect recorded as memory** `truncating-a-suite-destroys-the-failure`: piping a
  suite through `head` throws away the `failures:` block. Suite output goes to a FILE.

## §5 Standing constraints (unchanged)

NO WORKTREES in aion; one mutating lane; unmarked dirty = STOP. Never `git add -A`. Never
`git stash -u`. No `git gc`/`prune`. No `--force`/`--mirror`/`--delete` pushes. Never bare
`cargo clean`; no custom `CARGO_TARGET_DIR`; no launchd. Commit messages with backticks via
`git commit -F -`. `aion server` starts from the repo root. No bypasses: `#[allow]`,
`#[expect]`, `#[deny]`, `#[ignore]`, `_var`, `#[cfg(any())]`.

**NO-SWEEP untracked:** `.aion-accepted-binary-20260731/` (sole copy of Tom's accepted
build), `.aion-jjuabk-snapshot-20260731/`, `.aion-data-preswap-20260804/`.
