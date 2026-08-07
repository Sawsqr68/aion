# SEAT RESUME — marker AY, 2026-08-06

Supersedes **AX** for STATE. AW holds the third-review findings F1–F15 and why
each mattered. Latest letter wins.

---

## #7: ALL FIFTEEN FINDINGS BUILT. Three mutation legs written AND proven decisive.

AX left three legs owed. All three are now written, run, and — the part that
matters — **proven decisive by breaking the fix and watching the right test go
red while its siblings stayed green.**

### F1 — the resume arm

`the_resume_arm_refuses_to_append_once_the_epoch_is_closed`
(`lifecycle/completion.rs` tests).

    green on the fixed tree                                          -> ok
    mutation: refuse_if_epoch_closed moved back inside the else arm
      the_resume_arm_refuses_to_append_once_the_epoch_is_closed      -> FAILED
      an_attempt_refuses_to_append_once_the_epoch_is_closed          -> ok

**That divergence IS the finding.** The old test kept passing against the defect
because it drove the fresh-append arm; the gate could sit in one arm of two and
look fully covered. Restored, verified byte-identical to pre-mutation.

### F2 — the backoff ceiling

`a_backoff_ceiling_below_the_floor_is_refused_before_anything_is_started`
(`engine/builder.rs` tests). Two shapes — a zero ceiling (drives the interval to
zero on first advance) and a ceiling below the floor (the wait ratchets DOWN as
the outage lengthens) — plus a ceiling EQUAL to the floor as the accept control,
because a fixed interval is a legitimate ladder.

    mutation: the max_backoff arm of check_completion_retry made unreachable
      a_backoff_ceiling_below_the_floor_...                          -> FAILED
      a_zero_initial_backoff_is_refused_...                          -> ok
      the_configured_backoff_ladder_governs_when_a_retry_lands       -> ok
      backoff_delays_grow_and_cap                                    -> ok

### F3 — teardown continues past a failing step

`a_failing_teardown_step_does_not_skip_the_ones_after_it` (`engine/api.rs`
tests), using the new `#[cfg(test)]` seam `force_process_exit_drain_failure`.

    mutation: keep(self.runtime.shutdown()) -> keep(Ok(self.runtime.shutdown()?))
      a_failing_teardown_step_does_not_skip_the_ones_after_it        -> FAILED on (c)
      all 17 other tests matching "shutdown"                         -> ok

🔴 **It failed on assertion (c) — the decisive one — exactly as predicted.** (a)
"the error reaches the caller" and (b) "the epoch is closed" are BOTH true of the
defect too; only `installed_workflow_catalog().is_none()` can tell "the later
steps ran" from "the function returned early". A test asserting only (a) would
have passed against the defect.

🔴 **AND THE CONTROL EARNED ITS KEEP.** The first cut of F3 asserted the catalog
was cleared without checking it was ever installed — and it wasn't:
`engine_with_loaded_workflow` calls `Engine::new` directly, while only
`EngineBuilder::build` installs the NIF seams. The `is_none()` would have passed
on an engine that never had a catalog. **A pass measuring nothing.** The test now
installs the seam explicitly and says why.

---

## Gates on the resulting tree, at my own hand

    cargo fmt --all                                        run, whole tree
    cargo clippy --workspace --all-targets -- -D warnings  EXIT 0   (read BARE)
    cargo test -p aion-client                              61+1+6+2 passed / 0 failed
    cargo test -p aion-server --lib                        796 passed / 0 failed
    cargo test -p aion-rs         39 verdict rows, 985 passed / 1 FAILED

🔴 **The tree is RED, and the red is #85, not mine.** The failure is
`outbox_out_of_order_completions_still_settle_with_n_terminals` with
`SQLite failure: bad parameter or other API misuse`. Attribution is **structural,
not statistical**:

- The mechanism is already root-caused in dep source — `LIBSQL-DOUBLE-CLOSE-85.md`,
  libsql 0.9.30 double-closes every connection.
- A prior seat measured this exact target and error string at ~37.5% per battery
  run (`SEAT-RESUME-2026-08-01-E.md`), and `RECOVERY-LOOP-GUARDS-121.md` records
  the SIBLING test failing with the identical string — both **before this work
  existed**.
- It passes **4/4** when `--test outbox_e2e` runs alone, and 3/3 when the single
  test runs alone. It only appears under full-battery contention.
- This change set touches neither libsql, the store, nor the outbox.

Do not settle it statistically and do not re-investigate it as a regression.

🔴 **Clippy caught one of my own errors this round**: `engine.runtime().clone()`
on a `&RuntimeHandle` — `RuntimeHandle` is not `Clone`, so it copied the
reference and did nothing. Removed; the assertions read through `engine.runtime()`
directly.

---

## 🔴 STILL TO DO

1. **Opus review in flight** (CLAUDE.md mandates it before landing; three prior
   reviews each found real majors, so this is not a formality). Fix whatever it
   returns.
2. **COMMIT — naming files EXPLICITLY.** Never `git add -A` in this tree.
   🔴 Four files are UNTRACKED and load-bearing; committing without them lands a
   NON-COMPILING build:
       crates/aion/src/store_faults.rs               (lib.rs declares it)
       crates/aion/src/lifecycle/completion_retry.rs
       crates/aion/src/engine/builder_assembly.rs
       crates/aion/src/engine/api_workflow_ops.rs
   Message via `git commit -F -` with a QUOTED heredoc (zsh eats backticks from
   `-m`). Then push.
3. Tom's remaining four: **#9** operator fact in `details`, **#6**
   `max_concurrency` ignored on liminal, **#5** AWL `sum` (must bump
   `Keyword::ALL`'s `[Self; 57]` in the SAME change), **#1** `workflow.now()`
   history tail vs replay position.
4. **Owed to Tom:** a dry run of the #146 tutorial walk + a one-page crib sheet
   (every command in order, what he should see after each). Derive it from the
   tutorial doc and the proven self-run — **his server pid and worker pid are not
   to be touched.**
5. Then: #139 live cut, the 0.11.1 decision, #142 held on that cut.

---

## Tom comms this session

06:04Z he asked "how are we going?" — answered.
06:32Z he pushed back harder: *how much longer, how close to the bottom, "you told
me this was all good to go a month ago", "we really need to have stuff to show
tomorrow morning."*

I answered plainly and did not argue the premise: this one lands within the hour;
four small ones left, all expected today; and on the month-ago point — what
shipped did work, but some of my fixes only fixed the front door and my own tests
walked in the front door too, so they passed and I believed them. What changed
this week is that I break every fix on purpose now. Told him the tutorial walk is
showable today and offered the dry run + crib sheet. **No decision was asked of
him.** He has not replied to either message.

---

## Standing constraints (unchanged, carry verbatim)

NO WORKTREES in aion · ONE mutating lane · unmarked dirty = STOP · never
`git add -A` · never `git stash -u` · NO `git gc`/`prune`/`--prune` until the
estate deletion hold lifts · **DELETE NOTHING** · no `--force`/`--mirror`/
`--delete` on any push · ⛔ never bare `cargo clean`, no custom
`CARGO_TARGET_DIR`, no launchd · 🔴 **Tom's pids 45425 (server) / 48399 (worker)
— DO NOT TOUCH** (untouched all session) · worker rebuilds FROZEN estate-wide
until the #139 live cut · NO-SWEEP untracked dirs: `.aion-accepted-binary-20260731/`
(SOLE copy of Tom's accepted build), `.aion-jjuabk-snapshot-20260731/`,
`.aion-data-preswap-20260804/` · beamr JIT blocker with Artemis: keep long
workflows off until the fixed beamr lands · do not edit `CLAUDE.md` · never
mention prospekt to Tom · **NEVER publish artifacts for Tom** — deliverables are
local files · CLAUDE.md mandates an **Opus** sub-agent review before work lands ·
`<<autonomous-loop-dynamic>>` wakeup must be re-armed every tick.

Box: AEST (+1000) — always `-t "%Y-%m-%dT%H:%M:%S%z"`. zsh: no `PIPESTATUS`
(lowercase `$pipestatus`, 1-indexed), `status` is a READ-ONLY alias for `$?` so
name the variable `rc`, no word-splitting on unquoted expansion. BSD `sed` has
no `\s` — use `/usr/bin/python3` (NOT `/Users/tom/.local/bin/python3`, it cannot
exec). Foreground `sleep` is blocked — use `until <check>; do sleep N; done` with
an extended `timeout`, or `run_in_background`. Estate diff driver emits no ±
prefixes — `git -c diff.external= diff --no-ext-diff`.

🔴 **Never cite an exit code off a pipeline tail.** Run the gate bare, read `$?`.
