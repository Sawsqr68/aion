# Seat marker BY — 2026-08-07

Supersedes BX. Read this one first.

## §0 Where things stand

**Review 18 returned REFUSE.** Full text is durable at
`/private/tmp/claude-501/-Users-tom-Developer-ablative-stack-aion/5849e0d8-4802-4869-8e0f-9f7fd187e198/tasks/aa243913d9208de2c.output`
— read it before acting on the summaries below. Nine findings. This is the 13th
consecutive refusal (Reviews 6–18), and the first whose blocking findings are
architectural rather than evidential.

Its "found SOUND" section is long and independently derived (the 24-NIF table,
the 13/11 writer split, the four background writers, CLAIM 2/4/5/6, both tests
non-vacuous). That half stands. Do not re-litigate it.

### The two blocking findings, as I read them after checking the code myself

**F1 — the armed deadline.** Mechanism CONFIRMED at my hand. The refusal returns
at `nif_continue_as_new.rs:166-176`, before `retire_run_deadline` at line 186, so
the predecessor's `deadline:{run}` row stays live at its original `fire_at`.
`builder.rs:582` awaits `recover_active_workflows_on_startup`, but that call only
*spawns* recovered processes — replay proceeds asynchronously — and `:594` then
runs `recover_timers_on_startup`, whose `recover_due` (`time/recovery.rs:96`)
fires every overdue timer synchronously. So a deadline can fire while the
recovered predecessor is still replaying, appending `WorkflowTimedOut`.

**Where I part company with the reviewer:** they frame this as strictly worse
than the pre-lane severed chain. I do not think that survives. Pre-lane,
continue-as-new held an *unearned exemption* from deadline expiry — it retired
the deadline as part of a transition that then did not complete. The exemption
was purchased by recording a terminal for something that did not happen. Post-lane
the run carries the same exposure every other in-flight run already carries: a
run parked in an activity when the engine dies also times out if the outage
exceeds its remaining budget. That is what a wall-clock deadline MEANS. And
`WorkflowTimedOut` on a run past its declared deadline is the author's declared
contract, not data loss.

**Where the reviewer is exactly right, and it is a real defect:** the doc says
the opposite of what can happen. `nif_continue_as_new.rs:116` — *"Gating here
costs nothing that was not already lost"* — is false; the armed deadline is a
cost. The refusal message at line 172 — *"a recovering engine resumes it. No
operator action"* — is conditional and states itself as unconditional. Under this
repo's own standard a confidently wrong comment is a correctness bug. **Fix the
comments, keep the gate.**

**F2 — the process that no longer dies.** CONFIRMED and this one needs CODE, not
prose. `cancel_pid` at `nif_continue_as_new.rs:78` is unreachable on the refusal
path: `?` at line 76 returns first. Pre-lane it always ran. So the workflow
process stays runnable on a scheduler `Engine::drop` deliberately leaves alive,
and every subsequent NIF write it makes is ungated — timers, activities,
`spawn_child`, and `send_signal` into a THIRD workflow's history. That is
invariant 3, and the lane extends the exposure while `engine/api.rs` describes it
as pre-existing.

**The fix:** on `EngineTaskEpochClosed` ONLY, tear the process down before
returning. Not on `HistoryShape` and not on store errors — those are
workflow-visible errors the author may legitimately handle. Only the epoch
refusal means "this engine must stop speaking for this run", and killing the
process is what makes the refusal message's own promise ("the run stays live in
history and a recovering engine resumes it") true rather than a race.

### The remaining findings

- **F3** — `wheel_torn_down_before_teardown_cancel`'s new sentence is
  conditionally false. `Engine::cancel` calls `cancel_inflight_timers` at
  `api_workflow_ops.rs:163` BEFORE `terminate::cancel` at `:164`, so at the
  moment the cancel is refused the run is `Running`, not terminal. Rewrite.
  The test at `nif_timer_bridge_tests.rs:582` asserts the wording, so it cannot
  see this — it measures text, not truth.
- **F4** — the `pub(crate)` narrowing on `WorkflowDeadlineHandler::new` is
  harmless and correct, but its justification is wrong in both directions:
  `register_deadline_handler` is ALREADY `pub(crate)` (`nif_timer_bridge.rs:653`)
  so the described attack was unreachable, while `Recorder::new` IS `pub`
  (`recorder.rs:61`) so the boundary it implies does not exist. Keep the
  narrowing, rewrite the reasoning.
- **F5** — both new `aion-server` mapping arms ship untested, in the very file
  whose sibling test exists because that happened before. Add the tests.
- **F6** — `crates/aion/src/runtime/nif_continue_as_new_tests.rs` is UNTRACKED
  while the working tree's `nif_continue_as_new.rs:190` declares
  `#[path = …] mod tests`. Verified: HEAD does NOT reference it, so HEAD still
  compiles — this is a pre-commit hazard, not present damage. **`git add` it
  explicitly when committing. Never `git add -A` in this tree.**
- **F7** — the NIF adapter is untested, and F2's fix makes that the load-bearing
  half. Needs a test.
- **F8** — the changeset argues both sides of discriminant collapse: `error.rs`
  and `error_engine.rs` deliberately give two variants one label, while
  `nif_timer_fire.rs:123-137` introduces `RefusedAppend` expressly to avoid
  doing that. Needs the argument for why this discriminant is the one that does
  not travel.
- **F9** — the doc charges the pre-lane case for a condition it grants the
  post-lane case for free. Both are equally conditional on a later engine.

## §0b Tom's rulings this session

1. 🔴 **No GitHub Actions at all.** Standing. Discharged by deleting both
   workflow files (commit `9d259b185`). Never re-add one.
2. **"start using Aion workflows as the work" — and soon.** He wants
   demonstrable value to justify continuing. He also said to use norn wherever
   we can. See §0c.
3. He flagged **Artemis is free** — I have re-raised the beamr JIT blocker
   directly with them (details in §0d).
4. **GitHub Pages** — asked, answered, still his call.

## §0c The first "our work runs on Aion" lane — DONE AND PROVEN

`workflows/issue_recheck.awl` + `workflows/README.md` +
`workflows/issue-recheck/README.md`. Untracked; not yet committed.

Proven live on `aion 0.11.0` against an isolated server (pid 82997, ports
8097/50097, `/Users/tom/gate-verify-bx`, NOT Tom's). All three arms exercised:

| Issue | claim | hits | control | Result |
|---|---|---|---|---|
| #150 | `visibility_reconciliation_interval` | 0 | 180 | absent |
| #150 | `signal_delivery` | 0 | 180 | absent |
| #150 | `completion_retry` | 0 | 180 | absent |
| #107 | `list_children` | 0 | 479 | absent across all `crates/` |
| #136 | `.awl` | 0 | 42 | absent from `aion-cli/templates/` |
| #152 | `home_dir` | 1 | 180 | present — sent me to read it |
| control | `WorkerAvailabilityRequest` | 2 | 180 | proves `now_present` reachable |
| inverted | scope `NO-SUCH-DIR` | — | 0 | **refused**, exit 1, no finding |

The last two rows are what make the other six mean anything.

**Two issues confirmed as a side effect of running it:**
- **#151 VERIFIED, reproduced live** — a failing `aion run` answers
  `workflow 2 returned error`. "workflow 2" is an internal counter, and the same
  response carries the real ids in its `details` payload.
- **#152 VERIFIED, BOTH HALVES** — `error.rs:31` tells the operator to move
  `store.data_dir` beneath "`$AION_HOME`, default `~/.aion`", and no expansion
  exists to make that advice work (the only `home_dir` in `aion-server/src` is a
  test helper at `state.rs:2690`).

Language gotchas found the hard way: `count`, `present`, and `absent` are all
reserved AWL words and cannot name an action or an input.

## §0d beamr JIT blocker — re-raised with Artemis today

Still open. Latest published beamr is **0.16.3, dated 28 July** — two days BEFORE
we reported this — so nothing published carries a fix. Tree pins 0.16.2.
Repro + patch on branch `defect/beamr-jit-loses-native-services` in
`ablative-io/aion`. This is the ceiling on how long a demonstration workflow may
run, and I told Tom so.

## §1 The commits, and what blocks them

Three unpushed, in order: `2848ddb0a` (lane), `5775b58a4` (record),
`9d259b185` (Actions withdrawal — Tom-authorised, stuck on top of the others).

🔴 **Do not push until Review 18's findings are dealt with.** Not documented,
not accepted — dealt with.

## §2 Standing constraints (unchanged)

NO WORKTREES in aion; one mutating lane; unmarked dirty = STOP. Never
`git add -A`. Never `git stash -u`. No `git gc`/`prune`/`--prune`. No `--force`,
`--mirror`, `--delete` pushes. Never bare `cargo clean`; no custom
`CARGO_TARGET_DIR`; no launchd. Commit messages carrying backticks go via
`git commit -F -`. `aion server` starts from the repo root. No bypasses:
`#[allow]`, `#[expect]`, `#[deny]`, `#[ignore]`, `_var`, `#[cfg(any())]`.

**Tom's live pids: server 45425, worker 48399, tutorial server 50822 — DO NOT
TOUCH.** My isolated server is 82997 and is mine to stop.

**NO-SWEEP untracked:** `.aion-accepted-binary-20260731/` (sole copy of Tom's
accepted build), `.aion-jjuabk-snapshot-20260731/`, `.aion-data-preswap-20260804/`.

**Teardown owed:** `/Users/tom/gate-verify-bx` (keep until #154 closes), and the
older BT §9 list.
