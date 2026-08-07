# Seat marker CA — 2026-08-07

Supersedes BZ. Read this first. **The tree compiles**; BZ §0's broken-file warning is discharged.

## §1 What LANDED this window (uncommitted, in the working tree)

### Finding A (Review 19, BLOCKING) — FIXED AND MUTATION-PROVEN

`crates/aion/src/runtime/nif_continue_as_new.rs`

- `enum ContinuationOutcome { Complete, TerminalOnly(DurabilityError) }`.
- `outcome_must_end_the_process(&Result<ContinuationOutcome, NifContextError>) -> bool`
  — `Ok(_) => true` (both variants mean the terminal is durable), `Err` => only
  `EngineTaskEpochClosed`.
- `record_continuation` now returns `Result<ContinuationOutcome, DurabilityError>`.
  The deadline retirement's failure travels in the **`Ok`** arm, because the caller's
  next act is deciding whether to end a process and the only fact that decides it is
  whether the terminal landed.
- The NIF's `match` has three arms; `TerminalOnly` emits an operator term.

**The `TerminalOnly` operator text was verified against `lifecycle/deadline.rs`, not
assumed.** My first draft claimed a stray `TimerFired` would be recorded — false. What
actually happens: `decide_disposition` (handle registered) sees the non-timeout terminal,
retires the deadline itself, loses cleanly — the documented re-drive. But a continued
predecessor is cancelled and DEREGISTERED, so a later fire takes
`finalize_timed_out_without_handle`, which returns early for any terminal that is not
`TimedOut` and retires nothing. So the row survives and whole-history recovery re-arms it.
That is what the message now says.

### Test double

`crates/aion/src/store_faults.rs` — added `fail_appends_after(skip, count)`;
`fail_next_appends` is now that with `skip = 0`. One budget, mirroring the existing
`fail_reads_after`. (My earlier note that "no failing-store double exists" was WRONG —
`store_faults.rs` is exactly one and predates me.)

### Tests (4 green, `cargo test -p aion-rs --lib nif_continue_as_new`)

- `a_closed_epoch_refuses_the_continue_as_new_terminal` — control now asserts `Complete`.
- `an_already_terminal_run_reports_its_own_cause_not_the_epoch` — unchanged.
- `the_process_ends_whenever_the_terminal_landed_and_for_one_refusal_besides` — the
  predicate, five cases including `TerminalOnly`.
- `a_deadline_retirement_that_fails_after_the_terminal_reports_terminal_only` — NEW,
  FlakyStore, control + treatment, oracle reads PAST the injector.

### Finding C — FIXED, and my own comment was refuted by measurement

CONTROL 2 in the predicate test now says it is a **type-level** control over a
structurally unreachable input, and names the mutation it ACTUALLY kills.

### Mutations RUN (file restored byte-identical, shasum `3fcd38c03e63e91d5c07edd65ae8f5c48b92ffae`)

| # | mutation | result |
|---|---|---|
| M1 | post-terminal failure propagated as `Err` (pre-fix semantics) | **RED** — 1 failed |
| M2 | `TerminalOnly => false` in the predicate | **RED** — 2 failed |
| M3 | predicate keys on the `Durability` FAMILY | RED at **CONTROL 1**, not CONTROL 2 — this REFUTED the comment I had written |
| M4 | `!matches!(HistoryShape)` — "everything but an already-terminal run" | RED at **CONTROL 2** uniquely |

## §2 🔴 FINDING B IS **NOT** CLOSED — AND THE ATTEMPT FOUND SOMETHING ELSE

Review 19's Finding B: the NIF's `if outcome_must_end_the_process(&recorded)` gate is
measured by nothing. I built a live-process test for it. **It does not work, and I removed
it rather than ship a green test that measures nothing — which is the exact defect Review
19 caught.**

### What was built and then reverted

- `crates/aion/tests/fixtures/aion_continue_fixture.erl` / `.beam` — a fixture that
  retries `aion_flow_ffi:continue_as_new/1` (the retry exists because a test can only seed
  the registry once it holds the pid, which it only holds after spawning).
- `a_closed_epoch_refusal_ends_the_live_workflow_process` in
  `nif_continue_as_new_tests.rs` — spawn, register, close epoch, assert the pid is gone.

### 🔴 WHY IT DOES NOT WORK — MEASURED, THREE STEPS

1. **The mutation survived.** Reverting the gate to `if recorded.is_ok()` (exactly the
   pre-remediation form) left the test GREEN in 0.24s. So the process was dying without
   `cancel_pid`.
2. **First diagnosis was wrong and I checked it.** I assumed the fixture crashed on the
   NIF's return shape, rewrote it to swallow every outcome with `try…catch _:_` and to
   park (never exit) in both branches, and recompiled. **Still green under mutation.**
   Removing the `receive … after 10` delay (the only fixture in the tree that used one)
   changed nothing either.
3. **The attribution control names the real behaviour.** Two processes in one harness:
   - `park_only/0`, which never touches a NIF → **SURVIVES**.
   - the NIF caller, while its run is **UNREGISTERED** → **DIES within 300 ms**.

   So in a bare `RuntimeHandle` harness, calling the `continue_as_new` NIF kills the
   calling process even on the `UnknownProcess` path — a path
   `outcome_must_end_the_process` says must NOT end the process.
4. **The dirty-vs-pure discriminator rules out the obvious explanation.** A third
   process looping on `aion_flow_ffi:workflow_id/0` — registered `pure`, not
   `dirty_side_effectful` — against the same unregistered run **ALSO DIES**. So it is not
   the dirty-call path. What the three processes have in common is that the two that die
   are the two making a NIF call that FAILS, and `try … catch _:_ … end` does not stop it.

### What this means, stated carefully

**This is not yet a defect claim.** Two live hypotheses, neither tested:

- **(a) Harness.** RULED OUT on two counts. The engine NIF table IS registered by
  `RuntimeHandle::new` (`handle.rs:155` → `nif.rs:122`), so this is not an `undef`; and
  `RuntimeConfig::new(Some(1))` is the same runtime `engine/startup_tests.rs` uses, so it
  is not an unusual configuration. The pure-NIF discriminator also rules out a
  dirty-scheduler cause.
- **(b) Real, and now the leading one.** A workflow process does not survive a NIF call
  that fails, whatever the NIF, and Erlang `try…catch` does not intercept it. **If that
  holds in a real engine too, the premise of Review 18's F2 — that a refusal leaves a live
  durable writer — is FALSE, and the `cancel_pid` gate is belt-and-braces rather than
  load-bearing.** That would change the lane's story, so it must be settled, not assumed
  either way. Note the refusal path returns through `Err(term) => Ok(term)` in
  `continue_as_new_impl` — a NORMAL return of an error tuple, which on its face should not
  kill anything. Whatever is killing these processes is therefore not yet understood, and
  saying otherwise would be the confident-wrong-answer this lane keeps catching.

**Where to look first next time:** what beamr does with a NIF that returns an error, and
whether `continue_as_new_impl`'s `Ok(error_term)` return is genuinely reaching the process
as a value. A `tracing` capture around the spawn (the crate has `log_capture`) would show
the exit reason without needing new machinery.

### 🔴 THE NEXT DESIGN (do this, don't rebuild the bare harness)

Use a REAL engine, in a `#[cfg(test)]` unit test (NOT `tests/` — `begin_close()` and
`engine_tasks()` are `pub(crate)`):

- `crates/aion/src/engine/startup_tests.rs` already builds engines with `EngineBuilder`;
  copy that setup. A real engine configures schedulers properly, which settles (a) for
  free.
- Deploy/register a fixture that calls `continue_as_new`, `start_workflow` it, then
  `engine.<runtime>.engine_tasks().begin_close()`, then assert on `is_live(pid)`.
- **Keep both controls — they are the only reason the failure above was caught:**
  a `park_only` process that never calls the NIF, and the unregistered/pre-close phase.
- `runtime.is_live(pid)` is the probe. **NOT `live_processes_for_test()`** — that counts
  processes this test did not spawn (it read 2 with one fixture up). A global gauge cannot
  attribute.

## §3 Remaining from Review 19

- **B** — above. BLOCKING.
- **D** — `nif_continue_as_new.rs`: a failing `cancel_pid` discards the epoch refusal's
  operator text via `?`.
- **E** — `engine/api.rs:216` "asked to continue and was refused is now stopped" is
  unqualified; only the EPOCH refusal stops it.
- **F** — `workflows/issue-recheck/README.md:58-67`: the run table needs a
  `control_symbol` column; without it the rows are unfalsifiable.

Then: clippy both gates (`--workspace` AND `-p aion-rs` — different `preserve_order`
resolutions), the suite **to a file**, Review 20, and only then push `9d259b185`,
`88b010961`, `610ae17f7`.

## §4 Carried forward from BZ (unchanged)

- **Tom's rulings:** no GitHub Actions ever; start using Aion workflows AS the work, soon;
  he wants demonstrable value. GitHub Pages is still his open call.
- **beamr — ONE MOVE, NOT TWO. Superseded during this window; read this, not the earlier
  plan.** I was going to rev-pin main because nothing published carried the
  JIT/NativeServices durability fix. **That was true this morning and is false as of
  2026-08-07:** beamr **0.17.0** published tonight, and Cally measured
  `git merge-base --is-ancestor 4e94cec v0.17.0^{}` → **ANCESTOR**, with two controls
  (4e94cec is NOT an ancestor of v0.16.3; v0.16.3 is NOT an ancestor of v0.17.0). So the
  fix is in a published, citable version.
  **Take 0.17.0. Do NOT rev-pin.** A git SHA in the manifest has no version identity and no
  registry record; a published version is an artefact anyone can check later.
  - It is a **breaking-boundary** move: `^0.16.2` does not admit 0.17.0, so this is a real
    manifest edit and resolution change — the same cost the rev-pin carried, not a new one.
  - 0.17.0 is cut from main so it also carries the 0.16.3 `as_bytes` class. The interim
    `cargo update -p beamr` → 0.16.3 remains available if the lane isn't ready for a
    manifest edit, but it does NOT get the durability fix.
  - **`Scheduler::spawn_link_dirty` is Removed (breaking) in 0.17.0. MEASURED AT AION'S OWN
    BYTES, 2026-08-07: 0 uses** across `crates/`, `gleam/`, `sdks/` — control: `spawn_link`
    in any form appears 45 times in `crates/`, so the grep can find things. Not inherited
    from haematite's count; measured here.
  - ⚠️ **0.17.0 is NOT a security completion.** Its advisory discloses remaining JIT sites
    still open (one named plus two unenumerated classes) under `jit`, which cannot be
    disabled while `threads` is on. It buys the durability fix and the `as_bytes` class and
    nothing more.
  - After the move: re-cut the differential pin and confirm the 1p/1f → 2p/0f flip for
    Artemis — beamr's own wall cannot observe `replay_driver`, so my pin is the instrument.
  Still held behind this lane (manifest/lockfile). Do NOT "fix" main's `0.16.2` manifest
  line as a separate act — the version move replaces it.
- **Stray, flagged not fixed (Cally):**
  `.meridian/workflows/stacked-dev/worker/crates/haematite/Cargo.toml:11` pins
  `beamr = "0.6.1"` — ten minors back. It is a scaffold template, not a shipped artifact,
  but anything it materialises would build something ancient. Judge it when the lane is
  clear.
- **My isolated server: pid 82997, ports 8097/50097, `/Users/tom/gate-verify-bx`.** Mine to
  stop. 🔴 **Tom's pids 45425 / 48399 / 50822 — DO NOT TOUCH.**
- **Settled by Review 19, do not re-litigate:** F1's factual claims and its trade;
  F3/F4/F5/F8/F9; both `EngineTaskEpochClosed` single-construction-site claims; totality of
  every `DurabilityError` match; `EngineTaskRuntime` shared by `Arc`; the new
  `cancel_pid`-then-allocate ordering; the AWL body as injection-safe.

## §5 Standing constraints (unchanged)

NO WORKTREES in aion; one mutating lane; unmarked dirty = STOP. Never `git add -A`. Never
`git stash -u`. No `git gc`/`prune`. No `--force`/`--mirror`/`--delete` pushes. Never bare
`cargo clean`; no custom `CARGO_TARGET_DIR`; no launchd. Commit messages with backticks via
`git commit -F -`. `aion server` starts from the repo root. No bypasses: `#[allow]`,
`#[expect]`, `#[deny]`, `#[ignore]`, `_var`, `#[cfg(any())]`.

**NO-SWEEP untracked:** `.aion-accepted-binary-20260731/` (sole copy of Tom's accepted
build), `.aion-jjuabk-snapshot-20260731/`, `.aion-data-preswap-20260804/`.

## §6 Findings D and E — FIXED (uncommitted)

- **D** `nif_continue_as_new.rs`: a failing `cancel_pid` now carries the CAUSE. A `cause`
  string is built from `recorded` (Complete / TerminalOnly / refused) and folded into the
  termination-failure message, because a failed kill after a landed terminal and a failed
  kill after a refusal need opposite operator responses and this is the only frame holding
  both facts.
- **E** `engine/api.rs`: two paragraphs rewritten. The predicate paragraph now names
  `outcome_must_end_the_process` (the old text still said `refusal_must_end_the_process`,
  which no longer exists) and states that a LANDED terminal also ends the process,
  including the half-completed case. The "what is still open" paragraph now says "refused
  by the EPOCH gate" rather than "asked to continue and was refused", and says why the
  narrower wording matters.

## §7 FINDING F — FIXED. The re-run is complete and the table is rewritten.

The README's run table had no `control_symbol` **and no `scope`** column, and **no
evidence file exists anywhere** — so the rows could not be reproduced, exactly as the
reviewer said. I re-ran them rather than annotate from memory.

**Two corrections fell out of the re-run:**

1. **The #150 rows' `control` count CANNOT BE REPRODUCED — which is the finding, and it
   is weaker than "wrong".** The rows recorded `control` 180. Re-running against
   `/Users/tom/Developer/ablative/stack/aion/crates/aion-server/src` with
   `control_symbol: "pub fn"` gives **77**, and eight candidate symbols tried by hand
   (`fn `, `use `, `pub `, `let `, `self`, `::`, `aion`, `;`, `-> `, `&self`, `Result`)
   give 169/192/123/160/96/192/158/194/174/74/150 against 199 files — none is 180. So I
   cannot say the old number was wrong, only that nothing recorded lets anyone check it.
   That is exactly what the missing column costs. The claims themselves are unchanged:
   `visibility_reconciliation_interval`, `signal_delivery` and `completion_retry` are each
   **0 hits** against a live control.
   **The other two old controls DID reproduce** — #107's 479 (`pub fn` over `<repo>/crates`)
   and #136's 42 (which is `workflow` over the templates dir, 86 files). So the table is
   not uniformly unfalsifiable; one row of it is.
2. **`scope` MUST be absolute.** A relative `crates/aion-server/src` makes the control
   return 0 and the workflow correctly REFUSES (`unmeasured`, exit 1) — the declared
   command runs in the SERVER's cwd, not the caller's. That refusal firing for real is
   itself a live proof of the inverted-control arm. (The README's "Running it" example was
   ALREADY absolute — I wrote that it was relative and that was wrong; nothing needed
   fixing there.)

**Re-run so far (aion 0.11.0, isolated server pid 82997 on :50097, 2026-08-07):**

| issue | scope (absolute) | control_symbol | control_hits | claim | hits |
|---|---|---|---|---|---|
| 150 | `<repo>/crates/aion-server/src` | `pub fn` | 77 | `visibility_reconciliation_interval` | 0 |
| 150 | `<repo>/crates/aion-server/src` | `pub fn` | 77 | `signal_delivery` | 0 |
| 150 | `<repo>/crates/aion-server/src` | `pub fn` | 77 | `completion_retry` | 0 |

**ALL EIGHT ROWS RE-RUN** (aion 0.11.0, isolated server pid 82997 on :50097, 2026-08-07),
and `workflows/issue-recheck/README.md` now carries `scope` and `control_symbol` columns
plus a paragraph saying why the first table failed its own test. Results: #150 x3 = 0 hits
(control 77, `pub fn`); #107 `list_children` = 0 (control 479); #136 `.awl` = 0 (control 42,
`workflow`); #152 `home_dir` = 1 (control 77) — still PRESENT, claim still dead as written;
positive control `WorkerAvailabilityRequest` = 2; inverted control over `<repo>/NO-SUCH-DIR`
REFUSED with exit 1.

**Task #150's note still carries the unreproducible 180 and should read 77 (`pub fn`,
`<repo>/crates/aion-server/src`).** Its three claims are unaffected.

## §8 Gate state at this checkpoint (A/C/D/E/F done, B open)

- `cargo clippy -p aion-rs --all-targets -- -D warnings` → **EXIT 0, zero diagnostics.**
  The `--workspace` run is still owed (it resolves `preserve_order` ON — a different run).
- `cargo test -p aion-rs` → **EXIT 101. 9 verdict lines, 758 passed / 1 failed / 0 ignored.**
  The single failure is `covered::covered_set_is_byte_identical` in the
  `awl_bc4_differential` target: a trail-length divergence in
  `header-types/valid/line_width` (reference=6, direct=7). **That is task #149**, recorded
  before this lane as red on main AND nondeterministic. My diff touches **0** files under
  any `awl`/`bc4` path (measured), and nothing in the continue-as-new path is on that
  target's route.
  **MEASURED RATHER THAN INHERITED — three runs of that target, three different outcomes:**

  | run | exit | verdict | divergence reported |
  |---|---|---|---|
  | 1 (in the full suite) | 101 | 28 passed / 1 failed | `header-types/valid/line_width` event[6], reference=6 direct=7 |
  | 2 | 101 | 28 passed / 1 failed | `dag-fork/valid/fork_named_branches` event[6] |
  | 3 | **0** | **29 passed / 0 failed** | — |

  The failing CASE changes between runs and the target goes fully green on a third. That is
  #149's nondeterminism reproduced directly, and it settles attribution better than a
  clean-tree run would have: a deterministic failure caused by my diff cannot pick a
  different case each time, and cannot pass. **No clean-tree re-run is owed; this is the
  measurement.** #149 itself remains open and is not this lane's to close.
