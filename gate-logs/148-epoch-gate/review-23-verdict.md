<!-- EXTRACTION: final verdict message of the review-23 subagent, extracted
     2026-08-08 from session transcript agent-a99681274f8aac0ce.jsonl (full JSONL not committed).
     The extraction is the last text block of the transcript, verbatim. -->

Tree restored and verified pristine. Here is the verdict.

---

# REVIEW 23 — **REFUSED** (1 BLOCKING · 0 MAJOR · 4 MINOR)

The code is correct and, at my own hand, genuinely measured — including the R22 blocker, which is closed. The refusal is for the M2 fix: the replacement justification is wrong in the same way the text it replaced was wrong, and this time it contradicts a correct enumeration that already exists 400 lines away in the same crate.

## BLOCKING

**B1 — The M2 replacement justification names two writers that cannot fire in production, mis-states what they do, and omits the one writer that can. Asserted at all three M2 sites, with a hard count at a fourth.**

Sites: `crates/aion/src/runtime/nif_continue_as_new.rs:327-334` · `crates/aion/src/runtime/nif_continue_as_new_tests.rs:304-316` · `crates/aion/src/engine/api.rs:201-206`

Exact claim (`nif_continue_as_new.rs:327-334`):
> "Every writer that can record a terminal while this process is still resolvable ends the process itself: `terminate::cancel` calls `cancel_pid` directly, `terminate::complete` and `terminate::fail` remove the handle from the registry — after which every durable NIF this process makes fails to resolve a context — and the deadline handler cancels the pid and deregisters LAST. A second `cancel_pid` from here would race a teardown already in flight and report a successful one as 'termination failed'."

Exact code:

1. **`terminate::complete` and `terminate::fail` have no production caller anywhere in the workspace.** All six call sites are `crates/aion/src/engine/api.rs:1313, 1344, 1563, 1607, 1642, 1673`, and `#[cfg(test)] mod tests {` opens at `api.rs:774-775` and runs to EOF (1700) with no top-level item after it (verified with `awk 'NR>774 && /^[a-zA-Z#]/'` — only `mod tests {`). There is no `Engine::complete_workflow`/`fail_workflow`. So two of the three named writers are the justification for a *production* sparing decision and cannot occur in production.

2. **Neither of them "ends the process".** `terminate.rs:38-66` and `:75-98` contain no `cancel_pid`; they `registry.remove` at `:64` and `:96`. The process stays runnable indefinitely — it merely stops resolving a NIF context. And the concluding sentence is false for them: there is no `cancel_pid` in those paths to race, so a `cancel_pid` from this seam would simply *succeed*, not report a successful teardown as a failure. (`terminate::cancel` at `:139` and the deadline handler at `lifecycle/deadline.rs:343`/`:380` are the only two where the race sentence is true.)

3. **The enumeration omits `crate::lifecycle::continue_as_new::continue_as_new`** (`crates/aion/src/lifecycle/continue_as_new.rs:53-152`), reached from the live public API `Engine::continue_as_new` (`crates/aion/src/engine/api_workflow_ops.rs:269-297`). It requires a registered handle (`:59`), records `WorkflowContinuedAsNew` at `:93`, retires the deadline at `:107`, **spawns the successor**, and only then `registry.remove`s at `:150`. It never calls `cancel_pid`. It is exactly a writer that records a terminal while this process is still resolvable — and it is the one omitted.

   This is not merely a missing name. The window `:93 → :150` spans `start_workflow_with_options`, so the successor run is already live while the predecessor process is **still registered** — its durable NIFs resolve and write. That is the two-writers-on-one-history condition the predicate's own doc calls load-bearing invariant 3, and the comment tells a maintainer the opposite is guaranteed.

4. **The crate already carries the correct enumeration and this one drifted from it.** `crates/aion/src/lifecycle/completion.rs:213-220` states: *"Five ordinary terminal paths remove the entry the moment they finish — `terminate::complete` (`terminate.rs`), `terminate::fail`, `terminate::cancel`, `continue_as_new`, and the deadline teardown's 'deregister LAST' step."* Five, not three; it includes `continue_as_new`; and it characterises them as **removers**, not process-enders. That sibling paragraph also carries its own retraction of an earlier over-generalisation. The new text contradicts it.

5. `nif_continue_as_new_tests.rs:306` hard-codes the wrong count — *"see the predicate's doc for the **three** writers that each end the process at their own hand"* — so the error is asserted as a number in a second file.

`engine/api.rs` carries, in this same diff, *"🔴 THAT LIST IS A CLAIM ABOUT DURABLE WRITERS AND IT IS ONLY AS GOOD AS ITS ENUMERATION — four times proven."* This is the fifth.

**Remedy.** Rewrite all three sites to the shape that is actually uniform: this seam does not own the teardown, and the owners fall into two kinds — (a) those that kill the pid (`terminate::cancel:139`; the deadline handler `tear_down` step 2 at `lifecycle/deadline.rs:343`, deregistering last at `:380`), where a second `cancel_pid` genuinely races a teardown in flight; and (b) those that only deregister (`lifecycle::continue_as_new:150`, and `terminate::complete:64`/`fail:96` — noting these are presently test-only), where the process is left runnable and only loses context resolution once removal lands. Name `lifecycle::continue_as_new` explicitly and state the `:93→:150` window honestly. Fix the "three writers" count at `nif_continue_as_new_tests.rs:306`. If the word "Every" is kept, attach the derivation the api.rs list already demands (e.g. "non-`cfg(test)` callers of the `record_workflow_*` terminal recorders") so the next reader can re-derive rather than trust — and reconcile against `completion.rs:213-220` rather than restating the same rule in a second place.

*(Observation, not charged to this lane: `terminate::complete`/`fail` being production-dead is itself a CLAUDE.md "no zombie code" question, pre-existing and outside this blast radius.)*

## MINOR

- **m1 — `docs/design/aion-authoring/REVIEW-20-FINDINGS.md:338`** records *"Suite run 3 (supplementary, load 9.16 at entry)"*, but `scratchpad/r22-battery.transcript` contains no suite-3 step: it goes from `GATE suite run 2` straight to `MUTATION BASELINE`, and `r22b-suite-3.log`'s mtime (21:48) is 21 minutes after the transcript's `SESSION CLOSED GREEN`. The log itself is genuine (I counted 60 verdict lines / 1130 passed / 0 failed), but the record's "load recorded beside every gate" property does not hold for that row. Remedy: mark it as a manual post-battery run, or re-run inside the harness.
- **m2 — `scratchpad/r22b-clippy-p.log` is a cache replay, not a run.** 72 bytes, `Finished dev profile in 0.90s`, no `Checking aion-rs` line, while `r22b-clippy-ws.log` did the work (1m41s). The record cites it as the `clippy -p` gate's evidence. Harmless here (the workspace run covers `aion-rs`, and I re-ran `clippy -p` myself with a real `Checking aion-rs` line, EXIT 0), but a future battery should not cite a cache hit as a gate.
- **m3 — spliced-in doc text left ragged mid-paragraph breaks** at `crates/aion/src/engine/api.rs:207` (`/// actually landed,`) and `:268` (`/// there and holds here: the`). `cargo fmt` does not reflow doc comments, so it will not self-correct. Remedy: reflow.
- **m4 (weakest) — `GateHarness` has no `Drop`,** so `harness.runtime.shutdown()?` at `nif_continue_as_new_tests.rs:983` is skipped on any failing assertion, leaking a beamr runtime for the rest of the binary. Same pattern as the file's siblings; NIF state is per-`RuntimeHandle` so there is no cross-test contamination. Noted for completeness.

---

## VERIFIED AT MY OWN HAND vs TAKEN ON TRUST

**Verified (with how):**

| Claim | How |
|---|---|
| TREATMENT A's fault arithmetic (`fail_appends_after(3,1)` skips started + deadline seed + terminal, refuses the retirement) | Counted the appends (`Recorder::record_*` each make exactly one `append_with`, recorder.rs:793, :477, :140; `upsert_visibility_projection_nonfatal` is a no-op with no `visibility` set), traced `take_failure` short-circuit (`store_faults.rs:190-198`, `checked_sub`), **and confirmed empirically**: mutation X's failure output printed `terminal in history = true`, `timer retired = false`, `unspent append failures = 0` |
| Nothing else spends the budget in the measured window | `aion_continue_fixture.erl` `call_once/0` makes exactly one call then `park()`s; `wait_registered/1` polls a pure NIF (a read, not an append); arming precedes registration in both CONTROL 2 (`:727` before `:728`) and TREATMENT A (`:812` before `:813`); no timer service in the harness |
| TREATMENT B's witness is genuine | `retire_run_deadline` (`time/deadline.rs:136-141`) appends `TimerCancelled` only when `outstanding_deadline_timer` is `Some`, so `SeededDeadline::None` makes `TerminalOnly` unreachable and `retired_a_timer` really does separate the two arms |
| The four processes are independent | Four separate `spawn_continuer` calls; `register_live_run` mints a fresh `WorkflowId`/`RunId` each time; `is_live` is per-pid; every history read is keyed by that condition's own returned workflow id |
| The doc's e2e refutation ("`tests/continue_as_new.rs` never drives this NIF") | Read `crates/aion/tests/continue_as_new.rs`: three tests, all driving `engine.continue_as_new(...)` — the API path, not the NIF. **True.** |
| M2 ownership claim: `terminate::cancel` calls `cancel_pid` | `terminate.rs:139` ✓ |
| M2 ownership claim: `complete`/`fail` remove the handle | `terminate.rs:64`, `:96` ✓ — but see B1 for what the surrounding sentence gets wrong |
| M2 ownership claim: deadline handler cancels the pid and deregisters LAST | `lifecycle/deadline.rs:343` (step 2) and `:380` (step 6) ✓ |
| m4 claim: `reconcile_terminal_registry` only suspends residency | `completion.rs:541-551` calls `registry.reconcile` (`table.rs:230-244`, replaces projected status only) then `replace_residency(Suspended)` (`table.rs:207-220`). Neither removes. ✓ |
| The terminal-attempt path retires the deadline before the successor starts, with `?` | `completion.rs:403-405` (`if !matches!(existing, TimedOut) { retire_run_deadline(...).await?; }`), `start_continuation_replacement` called later at `:500` ✓ |
| The racing-fire repair | `deadline.rs:256-279` `Some(_)` branch retires the deadline under the recorder lock and returns `LoseCleanly` ✓ |
| The cold-boot sweep's ten-millennium horizon | `engine/startup_sweeps.rs:272-275` — `Duration::days(3_652_500)`, `expired_timers(horizon)` over all rows ✓ |
| `Err` from `record_continuation` means nothing durable landed | `append_one` (`recorder.rs:1011-1022`) appends then calls `mark_append_success` (`seq.rs:49-60`), which can only fail on `u64` overflow of the sequence head — unreachable. R20's acceptance stands. |
| The `.beam` matches its `.erl` | Recompiled with `erlc` and ran `beam_lib:cmp/2`: `chunks_different "Dbgi"` only — code chunks identical |
| The CB §1 citation | `SEAT-RESUME-2026-08-07-CB.md:16` — "Seven process shapes in ONE harness, measured" ✓ |
| Every README `issue-recheck` row | Reproduced all ten at my own hand: tracked control `git grep -lF 'pub fn' -- crates` = **356**; full-tree = **460** (matches the recorded number exactly); 77; 42; `home_dir` 1; `WorkerAvailabilityRequest` 2; and all five zero-claims = 0 |
| `store_faults` "three consumers" | Grepped: `nif_child_watch.rs:980`, `completion.rs:737`, `nif_continue_as_new_tests.rs:33` (plus a doc mention in `runtime/config.rs:224`) ✓ |
| m5's quote anchor | `"cost, stated because it is not zero"` exists verbatim at `api.rs:205` ✓ |
| The seat's suite evidence | Counted from the log files directly: `r22b-suite-1` and `r22b-suite-3` each **60 verdict lines / 1130 passed / 0 failed / 0 `FAILED` strings**; `r22b-suite-2` = 39 lines, 1 failure, `failures:` block names `outbox_out_of_order_completions_still_settle_with_n_terminals` with `Backend("SQLite failure: bad parameter or other API misuse")` in `tests/outbox_e2e.rs` — untouched by the lane, task #85. Attribution accepted. |
| The battery transcript's precondition samples and per-step loads | Present: three samples (4.38/4.07/4.04 vs 10 cores, 50.05 GiB) and a load figure beside every gate and every mutation ✓ — except suite run 3, see **m1** |

**Taken on trust:** that a failure from `retire_run_deadline` reaching `handle_process_exit_attempt`'s `?` lands in the completion-retry machinery specifically (I verified the `?` propagates; I did not trace the retry wiring end to end); the haematite `expired_timers` owned-shard scoping named in `terminal_only_account`; the seat's account of battery run 1 and of the load-probe incident (I read the record, not the events).

---

## GATES AND MUTATIONS I RE-RAN

Box sequencing honoured: at entry, Apollo Biscuit's haematite battery was live at load 58 on 10 cores — I ran **nothing** until it exited. All work below ran with zero foreign `cargo`/`rustc` processes (`pgrep -lf 'cargo (test|build|clippy)'` and `pgrep -lf battery` both empty; `ps -Ao comm | grep -cE '^(cargo|rustc)$'` = 0), disk 40 GiB free. I killed nothing.

| Step | Result | 1-min load |
|---|---|---|
| `cargo fmt --check` | **EXIT 0** | 6.88 |
| `cargo clippy -p aion-rs --all-targets -- -D warnings` | **EXIT 0**, real work (`Checking aion-rs`, 1m15s) — not a cache replay | 6.88 → 10.00 |
| `cargo test -p aion-rs --lib runtime::nif_continue_as_new` (green baseline) | **EXIT 0**, 5 passed / 0 failed, 10.37s | 9.68 |
| **Mutation X** (mine, not in the seat's eight): `Err(error) => Ok(ContinuationOutcome::TerminalOnly(error))` → `Err(error) => Err(error)` at `nif_continue_as_new.rs:503` — reverting the lane's central fix | **RED, EXIT 101**, 2 failed: `a_deadline_retirement_that_fails_after_the_terminal_reports_terminal_only` **and** `the_nif_gate_ends_a_live_process_for_each_outcome_that_must_end_it_and_spares_the_rest`; killed at TREATMENT A with diagnosis `terminal=true, retired=false, unspent=0` | 9.7 |
| **Mutation Y** (R22's blocker, re-proven at my hand): `&& !matches!(&recorded, Ok(ContinuationOutcome::TerminalOnly(_)))` appended to the gate `if` at `:96` | **RED, EXIT 101**, 1 failed — solely the live gate test, 20.10s | ~10 |
| Final green re-run of the **rebuilt** tree | **EXIT 0**, 5 passed / 0 failed | 7.53 |
| `cargo test -p aion-rs` (full crate, to a file) | **EXIT 0**, **60 verdict lines, 1130 passed, 0 failed**, zero `FAILED` strings — neither #85 nor #149 appeared | 7.63 → 27.82 (self-decay) |

Mutation X is the one worth carrying forward: it is the only run that puts the lane's *own* defect back and demands the suite notice, and it does — in two independent tests, with TREATMENT A's diagnosis fields empirically confirming the `fail_appends_after(3,1)` arithmetic rather than my reading of it.

**Restore proof.** Both mutations restored by **writing bytes** (fresh mtime, forces rebuild — no `mv`), backup at `scratchpad/r23-nif.bak`:

```
8431b5729df2d773a47ba36ecf5aac3723b17a92  crates/aion/src/runtime/nif_continue_as_new.rs
938c41e70eefb5746ee32036a3cd3eb69bc552a8  crates/aion/src/runtime/nif_continue_as_new_tests.rs
5c85ee5233fd5ea3a9dcda3ea82e5337f816123a  crates/aion/src/engine/api.rs
7aae39838e74f025ef690a72acc773daaa7f7023  crates/aion/src/store_faults.rs
10d042007c6594bbd32225e36a2e777a102fb989  crates/aion/src/lib.rs
```

All match the pinned baselines. `git status --porcelain` shows exactly the lane's 6 modified files and its untracked fixtures/docs — no `Cargo.toml`, no `Cargo.lock`, no `.orig`, no `.rej`. Nothing committed, pushed, or deleted; no server started; no process touched; all scratch in the session scratchpad.

---

**Bottom line for the seat:** B1 is a documentation fix at three sites plus one count, not a code change — the gate itself is sound and now measured on all four input classes, and mutation X proves the fix's core is pinned. But the wrong justification is the *second* attempt at that same paragraph and it now contradicts a correct in-tree enumeration at `completion.rs:213-220`, which is precisely the drift the file's own banner warns about. Fix it against that sibling paragraph rather than writing a third independent version, then re-run fmt + both clippy gates + the suite and re-pin.
