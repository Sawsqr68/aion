<!-- EXTRACTION: final verdict message of the review-22 subagent, extracted
     2026-08-08 from session transcript agent-ac0791a59bc6e9b0f.jsonl (full JSONL not committed).
     The extraction is the last text block of the transcript, verbatim. -->

Session closed clean: tree byte-identical to start, no `.orig` files, final green re-run 5/0, `cargo fmt --check` EXIT 0.

---

# REVIEW 22 — **REFUSED** (1 BLOCKING / 1 MAJOR / 3 MINOR)

The R21 fixes are real and almost all of them verify. The B1 stale-binary diagnosis holds. But the lane's **central new behaviour is not measured anywhere**, and I proved it with a mutation that survives 671 lib tests plus the continue-as-new e2e.

---

## BLOCKING

### B1 — The NIF gate's `Ok(TerminalOnly)` arm is unmeasured. Mutation RUN and SURVIVING.

**Claim** (`crates/aion/src/runtime/nif_continue_as_new_tests.rs:8-11`): *"The NIF's own gate — the `if` that CONSUMES the predicate, and the `cancel_pid` behind it — is measured separately, at the bottom of this file, against a live beamr process. It has to be: a reviewer reverted that `if` to its pre-remediation form and the entire suite stayed green."*
And `crates/aion/src/runtime/nif_continue_as_new.rs:300`: *"**Every `Ok` ends the process, including the half-completed one.**"*

**Code**: the live test drives exactly two input classes through the NIF — `Err(Store)` raised pre-terminal (CONTROL 2, spared) and `Err(EngineTaskEpochClosed)` (treatment, killed). `register_live_run` (`nif_continue_as_new_tests.rs:513-548`) arms **no deadline**, so `retire_run_deadline` has nothing to append and `Ok(TerminalOnly)` can never arise on the live path. The half-completed case is proven only against the *pure predicate* (`:282-291`, hand-built value) and against `record_continuation`'s return (`:424-429`, which calls `outcome_must_end_the_process` directly, not through the NIF). Nothing executes the `if` at `nif_continue_as_new.rs:96` with an `Ok(TerminalOnly)` in hand.

**Mutation, run at my hand** — restore the exact defect the lane exists to fix, at the gate:

```rust
if outcome_must_end_the_process(&recorded)
    && !matches!(&recorded, Ok(ContinuationOutcome::TerminalOnly(_)))
{
```

| run | result |
|---|---|
| `cargo test -p aion-rs --lib runtime::nif_continue_as_new` | **EXIT 0**, 5 passed / 0 failed, 10.37 s |
| `cargo test -p aion-rs --lib` | **EXIT 0**, 671 passed / 0 failed |
| `cargo test -p aion-rs --test continue_as_new` | **EXIT 0**, 3 passed / 0 failed |

Restored via `mutate.py restore`, shasum back to `ef9a764b8fd637c5471eba2055c71279f512dca6`, session ended with a green re-run of the rebuilt tree (5/0) and `cargo fmt --check` EXIT 0.

**Why blocking**: `ContinuationOutcome` exists *only* for this decision. With the mutation live, a workflow process whose `WorkflowContinuedAsNew` is durable stays runnable and keeps writing timers, activities, children and signals into a closed history — concurrently with the successor the sweep starts. That is load-bearing invariant 3, and it is the exact exposure the lane's own doc enumerates. All six recorded mutations are caught either by the epoch direction (M5/M6/M7, a/b) or by the predicate function in isolation (c); **none** exercises the NIF's consumption of the new `Ok` arm. The file's header claims that gap is closed. It is not.

**Remedy**: add a third live condition to `the_nif_gate_ends_a_live_process_for_the_epoch_refusal_and_spares_the_others`. Have `register_live_run` seed a deadline (`record_timer_started` with `crate::time::deadline_timer_id(&run_id)`, exactly as `seed_running_run` does at `:53`/`:67-69`), arm `fail_appends_after(<seeding appends>, 1)` so the refusal lands on the *retirement* rather than the terminal, and assert all three facts that separate this death from every other one: the process DIES, `continued(&history)` is true, and `!retired_a_timer(&history)`. Then re-run the mutation above and require it RED.

---

## MAJOR

### M2 — The justification for sparing `HistoryShape` is false, and contradicts the rule this lane states three times.

**`crates/aion/src/runtime/nif_continue_as_new.rs:316-321`** (new in this diff):

> *"`HistoryShape` (this run already recorded a terminal *before* the call) and a store error raised *before* the terminal both leave history unmoved and **the run genuinely live**. Terminating for those would turn a recoverable authoring mistake or a transient backend **into a dead run**"*

`HistoryShape` is raised at `nif_continue_as_new.rs:451-459` exactly when `terminal_outcome_from_history(&history, parent_run_id).is_some()`, and that function (`lifecycle/completion.rs:628-655`) is reopen-aware — *"A WorkflowReopened after a terminal supersedes it, so a reopened run reports no terminal outcome until it terminates again."* So `HistoryShape` ⟹ the run holds a live, unsuperseded terminal ⟹ **the run is not live, and cannot be "turned into a dead run": it already is one.** The sentence collapses two cases and is true of only the store-error one.

It also contradicts, in the same lane:
- `nif_continue_as_new.rs:300-305` — *"Leaving it alive after a landed terminal is strictly worse than leaving it alive before one"*;
- the test name at `:274` — `the_process_ends_whenever_the_terminal_landed_and_for_one_refusal_besides`;
- `engine/api.rs:206-207` (edited by this diff) — *"the question that decides this is 'did the terminal land', not 'was there an error'."*

By that stated rule `HistoryShape` **is** a landed terminal and the process should end. The code spares it, and the doc papers the contradiction over with a false premise rather than naming the real reason. The old pre-diff text did not make this claim — the lane introduced it.

**The decision is right, for a reason the doc does not give**: every writer that can record a terminal while this process is still resolvable owns the teardown itself — `terminate::cancel` calls `cancel_pid` (`lifecycle/terminate.rs:139`); `terminate::complete`/`fail` (`:63`, `:96`) call `registry.remove`, after which every durable NIF fails to resolve a `NifContext`; the deadline handler cancels the pid and deregisters LAST (`lifecycle/deadline.rs:380`). A second `cancel_pid` from here races those and turns a successful teardown into a "termination failed" report.

Duplicated wrong in two further places: `nif_continue_as_new_tests.rs:310-311` and `engine/api.rs:201-204`.

**Remedy**: state that `HistoryShape` is a landed terminal, that the stated rule would therefore end the process, and that it is spared anyway because *this seam does not own that teardown* — naming the three writers. Fix all three sites together.

---

## MINOR

### m3 — `crates/aion/src/lib.rs:78` still calls the fixture a retry fixture, which is exactly what this diff rewrote `store_faults.rs` to retract.

`lib.rs:78`: `/// Fault-injecting stores shared by the crate's retry proofs.`
`store_faults.rs:8-16` (new): *"**Not every consumer is a retry path, and calling this a retry fixture was wrong.** … A fixture described as serving retries only would look inapplicable to exactly the case that needs it most."*

One truth in two places, and the two now disagree — in the place rustdoc shows first. **Remedy**: change `lib.rs:78` to match (e.g. "shared by the crate's durable-refusal proofs").

### m4 — `nif_continue_as_new.rs:229-231` names a registration boundary that does not exist.

*"the predecessor's handle stays registered **until** `reconcile_terminal_registry`"*. That function (`lifecycle/completion.rs:541-552`) calls `Registry::reconcile` (`registry/table.rs:230-243` — replaces the cached projected status only) and `replace_residency(Suspended)` (`:207-219` — sets residency only). Neither removes the handle, and `completion.rs:216-218` says so outright: *"This monitor's own `reconcile_terminal_registry` is the exception: it suspends rather than removes."* Nothing on the NIF continue-as-new path calls `registry.remove` for the predecessor at all.

The load-bearing half is true and verified (retirement at `:404` precedes reconcile at `:499`), so the conclusion holds *a fortiori* — but as written a reader concludes the handle path closes at reconcile, after which a racing fire would take `finalize_timed_out_without_handle`, which for a non-`TimedOut` terminal is a documented no-op (`lifecycle/deadline.rs:429-439`) and repairs nothing. **Remedy**: "…is still registered when the monitor performs the retirement — `reconcile_terminal_registry` runs after it and only SUSPENDS residency, it does not deregister —".

### m5 — `engine/api.rs:263` — "the paragraph 46 lines above" is a hardcoded line-distance nothing enforces.

It currently lands inside the intended paragraph (`api.rs:211-216`), but any edit above silently falsifies it. **Remedy**: name the section, not a count.

---

## Verified at my own hand vs. taken on trust

**Judgement on the B1 stale-binary mechanism — ACCEPTED, and independently corroborated.**
The mechanism fully accounts for the observations, and I closed the loop myself rather than reading the logs:
- I ran the `if true` gate mutation: **RED at `nif_continue_as_new_tests.rs:706`, CONTROL 2's survival assert, in 0.05 s** — bit-identical to R21's five "nondeterministic" failures (0.03–0.06 s, same assert). The signature *is* that mutation.
- The timing is the discriminator and it is decisive: with the correct gate, a pre-terminal store fault produces no `cancel_pid` at all, so a genuine CONTROL 2 death cannot happen on the first 20 ms probe. The green runs take 10.1–10.4 s (CONTROL 2 spending its full `SETTLE_BUDGET`); the stale-mutant runs take 0.05 s. Those are different populations, not one flaky one.
- Base rate: ~30 runs at proven-fresh binaries across R21's post-rebuild set, the six-mutation baseline/final, and my own 5 runs + 2 mutation baselines — **0 red**. All ~5 red observations sit at proven-stale-binary conditions.
- The R21 evidence logs are internally coherent: `b1-mut.log` (22.19 s compile → RED 0.06 s), `b1-stale.log` (**0.74 s "Finished", no recompile** → same RED at proven-clean source), `b1-touched.log` (22.25 s recompile → EXIT 0 in 10.30 s).
- The hardened harness closes it: `mutate.py restore` writes bytes (fresh mtime), `apply` refuses to clobber a backup (exit 3), the driver pins the expected baseline rather than blessing found bytes, and the session must end green on the rebuilt tree. I used it for four mutations and it behaved as documented every time. **Residual, stated**: a destroyed observation cannot be re-caught, so this is a sufficiency argument, not a proof of the negative. What makes that acceptable is M4's diagnosis — if it ever recurs, the next observer gets attribution instead of a mystery (I confirmed the diagnosis fires with real content, below).

**Verified at my own hand:**
- **The fixture artifact matches its source.** Recompiled `aion_continue_fixture.erl` with `erlc` and compared chunks via `beam_lib`: **`Code` chunk byte-identical**. (Note: my first oracle, `abstract_code`, returned `no_abstract_code` for *both* files — a vacuous comparison. The `Code` chunk is the real proof.) Exports are `park_only/0`, `await_then_continue/0`, `module_info/0,1`; `wait_registered(0) -> park()` confirmed in source, so budget exhaustion cannot fall through to the call.
- **R21-B2, all four repair-chain claims, each read at the site named**: `completion.rs:404` (`retire_run_deadline` under the recorder lock, `?`, in the resume arm) precedes `start_continuation_replacement` at `:506`; `time/deadline.rs:110-118` states the resume contract verbatim as quoted; `startup_sweeps.rs:268-283` (`Utc::now() + 3_652_500 days`, `expired_timers(horizon)`, `SweepScope::{ColdBoot,Adoption}`) with `retire_orphaned_terminal_deadline_independent` at `:348-400` doing `SequenceConflict` re-evaluation; call sites at `startup.rs:79` (cold boot, before repopulation and before `sweep_continued_as_new_replacements`) and `:135` (adoption); `lifecycle/deadline.rs:256-279` `Some(_)` arm retires and loses cleanly, reached only when `registry.get` returns `Some` (`:150`).
- **R21-B3**: `time/recovery.rs:150-172` — `outstanding_future_timers` filters `fire_at > now`; the "D5 resurrection hazard" regression at `:415-496` asserts both halves.
- **Store claims**: no backend deletes timer rows (grepped `aion-store`, `-libsql`, `-haematite`); haematite scopes `expired_timers` to `owned_shard_scope()` (`store.rs:2908-2919`); "outstanding-in-history is the fact every consumer checks" holds — `fire_timer_guarded` gates on `timer_is_live` → `live_timers_in_active_segment` (`timer_service.rs:250,335-342`).
- **R21-M4**: the diagnosis fires with real content. Under the `if true` mutation the panic printed `terminal in history = false`, `unspent append failures = 0`, the full history dump, and the stale-binary hint. No `#[allow]`/`#[expect]`/`#[ignore]`/`_var`/`#[cfg(any())]` anywhere in the diff (grepped); the only `unwrap_or` added is `unwrap_or(Term::NIL)`, the file's existing convention.
- **R21-M5**: `engine/api.rs:211-216` states the price; the "cost nothing" sentence is gone.
- **R21-m6**: re-derived the README decomposition — full tree **460**, tracked **356**, git-ignored **104** = **92** `build/packages/gleam_*` + **12** `build/dev/erlang/*/_gleam_artefacts/*.erl`, zero unaccounted. Then re-ran **every** table row with the workflow's own command (`grep -rlF -- "$1" "$2" | wc -l`): 77, 42, 0, 0, 0, 1, 2, 0, 0, 0 — all reproduce exactly.
- **R21-m7/m8/m9**: the `expired_timers` claim is gone from the message; `TerminalOnly` now says "not the only witness, the earliest one"; the `Err` arm is `error!`, matching `completion_retry.rs:687-712` where the unretryable (epoch-closed) path logs at `error!`.
- **Type-level control claim**: `NifContext::block_on_recorder` returns `Result<T, NifContextError>` from a closure yielding `Result<_, DurabilityError>` widened by the single `#[from]` (`nif_context.rs:21-40, 239-253`) — `Durability` is indeed the only structurally reachable variant. I also checked both mutations named in the CONTROL 2 comment answer as claimed.
- **Enumerations**: exactly three `FlakyStore` consumers (`nif_child_watch`, `lifecycle/completion`, this lane); `start_continuation_replacement` has exactly one non-test call site; `store_faults` is `#[cfg(test)]`-gated (`lib.rs:76-80`); no stale references to `refusal_must_end_the_process` remain; no god files (`api.rs` = 257 non-test code lines).
- **Live-test ordering**: the fault is armed before `register_live_run`, whose single `record_workflow_started` append consumes the skip; the fixture is released only by the `registry.insert` that follows it; `take_failure` uses `checked_sub` so no budget underflows; `fail_next_appends` now stores 0 into the new skip budget, preserving its old behaviour.

**Gates and mutations I re-ran** (all output to files, verdict lines counted):

| what | result |
|---|---|
| `cargo fmt --check` | **EXIT 0** (twice: mid-review and at close) |
| `cargo clippy -p aion-rs --all-targets -- -D warnings` | **EXIT 0** |
| `cargo test -p aion-rs --lib runtime::nif_continue_as_new` ×5, after `touch` + rebuild | **EXIT 0** ×5 — 5/0 each, 10.07–10.35 s |
| `cargo test -p aion-rs` (full) | **EXIT 0 on my first run** — **60 verdict lines** (58 `Running` + 2 from `Doc-tests`, reconciled), **1130 passed / 0 failed**. `covered::covered_set_is_byte_identical` passed here. |
| mutation `if true` | **RED**, CONTROL 2 at `:706`, **0.05 s**; restored `ef9a764b…` |
| mutation `if false` | **RED**, treatment at `:744`, **20.08 s**; restored `ef9a764b…` |
| mutation *gate skips `Ok(TerminalOnly)`* | **🔴 SURVIVED** — module 5/0, `--lib` 671/0, `--test continue_as_new` 3/0; restored `ef9a764b…` |
| final green re-run of the rebuilt tree | **EXIT 0**, 5/0 |

The `if false` result is worth naming separately: it closes the confound that the treatment's process might be dying because a NIF call after `begin_close()` is fatal rather than because of `cancel_pid`. With the gate disabled the treatment **survives** for the full 20 s, so the death in the green run is attributable to `cancel_pid`. That was the one attribution gap I could not close by reading.

**Taken on trust:** the `cargo clippy --workspace --all-targets` result (I ran the `-p aion-rs` form only — note the memory rule that `--workspace` and `-p` resolve features differently, so the workspace form still needs its own run); the accuracy of the "13 of 24 NIFs write durably" enumeration in `api.rs` (unchanged context, outside this diff); that `cancel_pid` reliably drives the process-exit callback (established pre-existing design, exercised by `tests/continue_as_new.rs`).

Nothing committed, pushed, or deleted. `git status` is identical to session start; both pinned shasums hold; no `.orig` files remain; all scratch under the session scratchpad.
