# SEAT RESUME — marker BS — 2026-08-07

**Supersedes BR.** BR carries the Review-14 verdict table and the tutorial fixes;
BQ + amendments carry the previous battery/clippy record. Both still readable.

---

## 1. REVIEW 14'S THREE OPEN FINDINGS ARE ALL CLOSED

BR §4 left findings 3 and 5 plus the `RefusingReadStore` residue open. All three
are now fixed, and each fix is mutation-proven.

### Finding 3 — the second chimera, `crates/aion-cli/tests/error_rendering.rs`

**Verified at my hand, then fixed.** A query against a terminal run cannot carry
`WorkflowTerminal`. Real shapes, read off the code:

| situation | route | wire type | detail |
|---|---|---|---|
| query vs finished run | **`Ok`** + `QueryResponse.outcome.error` oneof | `QueryNotRunning` | `query error: workflow <id> is not running` |
| signal/cancel vs finished run | `Err(Status)` + status details | `WorkflowTerminal` | `workflow <id> has already reached terminal state Completed` |
| engine stopping | `Err(Status)` + status details | `ShuttingDown` | `the engine is shutting down and is not accepting work` |

`QueryError::NotRunning` displays `workflow {0} is not running`; `EngineError::Query`
wraps it as `query error: {0}`; `query_wire()` hands the wire `source.to_string()`.
`signal_terminal_error` / `cancel_terminal_error` (`api/handlers/error.rs:35`, `:42`)
build the middle row and are called only from `workflows.rs:182` / `:278`.

What changed:
- `"terminal"` query case → `Ok(outcome_error(not_running_with_type("QueryNotRunning", …)))`,
  detail derived by `query_not_running_detail()`.
- `signal` handler is now **keyed on `signal_name`**: `"approve"` → `WorkflowNotFound`
  (unchanged), `"terminal"` → the real `WorkflowTerminal`.
- `typed_error_type_from_status_details_reaches_stderr` repointed at `"shutdown"`,
  which genuinely rides status details.
- NEW `typed_error_type_from_query_outcome_reaches_stderr` — the oneof route.
- NEW `signal_against_a_terminal_run_renders_workflow_terminal`, incl. a negative
  assertion that a finished run is **not** reported as a shutdown.
- The 🔴 comment block rewritten; it had asserted the false thing.

**7/7 green.**

### Finding 5 — `crates/aion/src/engine/api.rs`
**Verified at my hand.** Two doc blocks were concatenated above
`a_second_failing_engine_shutdown_step_is_reported_not_swallowed`: the 39-line
teardown-ordering block (ending "The decisive observable is (c), not (a)") plus
the correct "F5:" block. The `(a)`/`(b)`/`(c)` assertions it names live in
`a_failing_teardown_step_does_not_skip_the_ones_after_it`, ~60 lines below.
Moved the block onto the test it describes. "the sibling below" still reads true.
`cargo test -p aion-rs --lib engine::api` → 19/19.

### Finding 7 residue — the store double
**Verified at my hand and the reviewer is right.** While armed it disclaimed
EVERY shard (`is_current_owner → false`) yet still accepted an `append`. No
backend can occupy that: a node that does not own the shard cannot write to it.

Fixed by **fencing history in both directions**, and then made honest end to end:
- `crates/aion-store/src/testing/refusing_reads.rs` → **`fenced_history.rs`**
- `RefusingReadStore` → **`FencedHistoryStore`**
- `refuse_reads`/`allow_reads` → **`arm_fence`/`disarm_fence`**
- `self.fence()?` added to `append` and `append_with_outbox`
- module doc, type doc, method docs and the client test's teardown comment all
  corrected. Listings/timers/packages still delegate and the doc now says so
  and says why (a fixture must still be able to observe what was written).
- Call sites updated: `aion-store/src/testing/mod.rs`, `aion-client/src/transport/embedded.rs`.

**The fence-on-append was UNMEASURED when added** — nothing on today's path
writes while armed. So a new test `appends_are_fenced_as_well_as_reads` was
written with a control (unarmed append succeeds), the refusal, and survival
(disarm restores). 3/3 green in that module.

---

## 2. MUTATIONS RUN THIS TURN — all restored `shasum`-identical

| # | file | mutation | result |
|---|---|---|---|
| M10 | `aion-client/.../convert.rs` | `decode_wire_error`: `error_type: value.error_type` → `None` | 🔴 oneof test RED, **status-details test still GREEN** — proves the two decode paths are distinct and the old fixture could never have covered the oneof. `decode_wire_error` has exactly ONE caller. |
| M11 | `aion-cli/src/render.rs` | widen the `ShuttingDown` guard to every `not_running` | 🔴 BOTH the new signal-terminal test and the end-to-end table RED; mutant output is the exact operator harm ("the server is shutting down" for a finished run) |
| M12 | `aion-store/.../fenced_history.rs` | delete `self.fence()?;` from `append` | 🔴 `appends_are_fenced_as_well_as_reads` RED at the armed assertion |

Restore hashes verified for all three:
`convert.rs 235a8f75…`, `render.rs da66dd0d…`, `fenced_history.rs fa3cf096…`.

M11 is the INVERSE of the already-proven "delete the guard" mutation — deleting
it kills the shutdown case, widening it kills the terminal cases. Both directions
now have a killer.

---

## 3. GATES THIS TURN

- **Clippy: 5 runs, all exit 0, zero diagnostics.**
  `--workspace --all-targets -- -D warnings`, plus `-p` for `aion-cli`,
  `aion-store`, `aion-client`, `aion-rs` (⚠️ `--workspace` resolves
  `preserve_order` ON and `-p` off — different runs, both required).
  Logs: `scratchpad/clippy-p-*.log`.
### 🔴 BATTERY RESULT — **GREEN**, reconciled (landed after this marker was first written)

```
sentinel RC_WS=0
declared : 223 distinct binaries (keyed on target/debug/deps/<name>-<hash>)
         +  20 doc-test targets   = 243
verdicts : 247        DEFICIT = 0  (surplus +4, and a surplus cannot hide a crash)
totals   : 4214 passed / 0 failed / 0 ignored
```
**Surplus fully attributed by re-derivation, not by recall** — exactly 243
targets emitted at least one verdict, and the 4 extra lines come from two
targets that spawn child cargo processes:
`aion_server-5ce111d9e042313f` emits 4, `Doc-tests aion` emits 2.

🔴 **The #85 libsql red did NOT fire this run.** Do **not** read that as #85
being fixed or gone — it is intermittent and contention-dependent (task #85:
2/2 contended runs reproduced, 5/5 isolated green). This run simply did not hit
it. #85 stays open and unchanged. **A green that a known intermittent failed to
appear in is weaker evidence than a green with the failure attributed** — say so
rather than banking it.

- **Previous battery** (for comparison, superseded): `cargo test --workspace
  --no-fail-fast` → `scratchpad/bs-workspace.log`, completion sentinel
  `scratchpad/bs-battery.done` carries `RC_WS=`.
  🔴 **On resume, reconcile it the same way as BQ amendment 2**: count DISTINCT
  binaries by path under `target/debug/deps/` (never the `Running <label>` text,
  which collides), add doc-test targets, compare against verdict-line count.
  A DEFICIT means a crashed binary; a surplus must be attributed. Previous run:
  243 declared / 247 verdicts / deficit 0 / 4210 passed / 1 failed.
- `cargo fmt` run on the tree (autoformat, never format-check).

---

## 4. TASK #153 FILED
`validate_document_name` accepts a leading `_` the AWL lexer rejects. Repro and
the subtraction cure are in the task body. This was BR §5's outstanding filing.

---

## 5. TOM
Answered his 23:27Z check-in at 2026-08-07 (Meridian DM
`dm:c9255b2a-5731-4d17-8124-e3bfa2224186`). Plain language, two parts: the
tutorial's `timeout` lie is gone and the deck was re-measured in a clean world;
the fix lane's three findings are closed with deliberate-break proof, battery
running, one review to go. Told him nothing is needed from him.

🔴 **He is mid-tutorial and outranks everything.**

**23:36Z — Tom applied time pressure**: "can you get it ready to go... and in not
very long". I told him the tutorial is ready to walk NOW and the code lane is
not, named the three remaining steps, and said I would not wave a review through
to hit a deadline. Compressed what was compressible: Review 15 dispatched to run
**in parallel with the battery** rather than after it.

## 5a. 🔴 WAFFLES' BACKING — ON THE RECORD *BEFORE* REVIEW 15 RETURNED

Reported the pressure to Waffles at 23:42Z, deliberately before the verdict
landed rather than after. His ruling (23:43Z, `dm:896955e1-86dd-4d6a-9c25-f3f978b189a9`),
verbatim in substance:

> "You have my backing, on the record, before Review 15 returns. A fifteenth
> refusal that finds something real is a **result, not a delay** — the arc
> you've described (fixes introducing defects) is precisely the arc where waving
> a review through costs the most. If Tom presses again, the answer is the one
> you already gave him, and you can point at this message. **The deadline bends,
> the bar doesn't.**"

He also put the **aion working-tree hold on his board**: nothing at his seat or
under his dispatch touches this repo — no checkout, reset, stash, clean or
sweep — until the lane lands. Cally's sweep report stands corrected on the
branch-clean vs tree-clean distinction.

🔴 **If a future seat reads this while under pressure to pass a review: the
ruling above is the standing answer. Do not re-litigate it. Point at it.**

---

## 6. ON RESUME — in this order

1. **Tom first if he has said anything.**
2. ✅ **BATTERY DONE AND GREEN — see §3.** Nothing to re-run. 4214/0/0,
   deficit 0. #85 did not fire; it stays open regardless.
3. 🔴 **REVIEW 15 IS IN FLIGHT ON OPUS — its verdict is the ONLY thing left
   before commit.** It was dispatched ~23:39Z with brief
   `scratchpad/review-15-brief.md` (read-only, told not to run cargo). If its
   result is lost to the compaction and no notification arrives, **re-dispatch
   rather than assume a pass** — a review whose verdict nobody read is a review
   that did not happen. If it REFUSES: fix every finding, mutation-prove each,
   re-run the affected suites, and only then commit. Waffles' standing ruling
   (§5a) covers the time cost.
   *(Superseded dispatch note, kept for context:)* Give it BS, BR, BQ, BP and
   `scratchpad/review-14-brief.md`. Tell it reviews 6–14 all REFUSED and that
   14's findings were all correct — its job is to find the fifteenth thing.
   NEW SURFACE for it to attack: the renamed store double, the two new CLI
   tests, and whether `signal_terminal_detail()` / `query_not_running_detail()`
   being shared between mock and assertion hides anything (my reasoning: the
   message is an INPUT passing through, the subject under test is the CLI's
   rendering — challenge that).
4. **Commit in three lots, paths named EXPLICITLY, never `git add -A`.**
   🔴 **DERIVE the file lists at commit time — do not reuse a list from a
   marker, including this one.** The two commands:
   ```
   git -c diff.external= diff --no-ext-diff --name-status | sort
   git ls-files --others --exclude-standard -- crates docs | sort
   ```
   (Scope the untracked one to `crates docs`. An unscoped `git ls-files
   --others` returns ~8 MB because it walks the no-sweep backup directories
   `.aion-accepted-binary-20260731/`, `.aion-jjuabk-snapshot-20260731/`,
   `.aion-data-preswap-20260804/` — **none of which may ever be swept or
   committed**.)

   Lot shapes:
   (a) tutorial — `docs/authoring/build-tutorial.py`, `first-workflow.html`,
       `tutorial-examples/*.awl` + the 4 deletions;
   (b) the #7 fix lane once review 15 passes — note it now includes the
       **file rename** `refusing_reads.rs` → `fenced_history.rs` (the file was
       UNTRACKED, so `git mv` failed and a plain `mv` was used; git will see
       add-only);
   (c) markers.
   **KEEP EXCLUDED**: `docs/design/aion-authoring/SPEC-5-AWL-SUM-COMBINATOR.md`.
5. Then #5, #1, #6, #9.

---

## 7. LIVE STATE — TEARDOWN OWED (unchanged from BR §7)

- 🔴 **Tom's production pids 45425 / 48399 — DO NOT TOUCH.**
- Tom's tutorial server **pid 50822**, `~/my-first-flow`, `:8099`/`:50099` — HIS.
- 🆕 **My clean world: pid 92685, `~/tut-clean`, `:8098`/`:50098`.**
  **TEAR DOWN BY PORT** — `lsof -tiTCP:8098 -sTCP:LISTEN` — never a pgrep
  pattern (that has written a confident false success twice). Then
  `rm -rf ~/tut-clean`.
- Proving clones to remove: `~/gate-clean-3`, `~/gate-clean-4`, `~/gate-clean-5`,
  `~/gate-clean-red`, `~/gate-work-3`, `~/gate-work-4`, `~/gate-work-5`,
  `~/gate-work-red`.
- `scratchpad/shimbin/aion` is a FAKE `aion` stub (positive control). Do not
  leave it on PATH.

## 8. HOUSEKEEPING NOTE
A **stale `.git/index.lock`** was cleared this turn — zero bytes, dated
2026-08-06 19:47 (around the history scrub), no git process alive, nothing
holding it open via `lsof`. It was blocking every index write including Tom's
own commits. A lock file is not a git object, so the estate deletion hold does
not cover it. Recorded here because removing anything under `.git/` should
never be silent.

---

## §9 — REVIEW 15 (2026-08-07): REFUSED, three findings, ALL CLOSED

Review 15 (Opus, agentId `ad6520ea5b433526c`) refused the lane. It found
**nothing wrong with the engine work** — it traced every fixture in
`error_rendering.rs` to its construction site and confirmed no chimera, cleared
the moved `api.rs` doc block "correct in every particular", confirmed the store
double's append fence measures, and re-verified invariant 3 (the only
`EventStore::append` added anywhere in the diff is a test seed fixture). It also
independently confirmed the `compile_fail` doctest on `CompletionRetryConfig`
DOES run — it lands in a second unlabelled merged-doctest section, which reads
at first glance as dropped.

### F1 — the deck promised fourteen words over a table of thirteen · CLOSED

`build-tutorial.py` lede read *"Fourteen words, in four groups."* The table
explains **13 distinct** words (`failure` appears in two rows) in 4 groups. The
build's own `gate_keyword_coverage` had been printing `all 13 keyword(s)` the
whole time — one truth in two places, only one maintained. `timeout` left the
examples, its row went with it, the sentence did not move.

**Cure is SUBTRACTION, not a gate.** `KEYWORD_TABLE` is hoisted to module level
and `keyword_table_shape()` reads both numbers off it; the lede is an f-string
over them. The sentence can no longer disagree with the table it describes.

- Rendered: `first-workflow.html:692` → `13 words, in 4 groups.`
- **Tracking probe**: added a probe group + a `zzz` row → lede moved to
  `14 words, in 5 groups`. Restored, deck rebuilt, all gates green.
- 🔴 **AMENDED 2026-08-07 (Review 16 minor 2). THE SHASUM RECORDED HERE WAS
  WRONG.** This section claimed the restored `build-tutorial.py` hashed
  `e69a87f0…`; the delivered file hashed
  `3d880bf9bf730f38974114972c765f49f6f9b3808a525527aa949ba1de0fbe7b`. Cause: the
  hash was taken BEFORE the F4 docstring edit, which lands in the same file — so
  it was a true reading of an earlier artifact, recorded as if it described the
  delivered one. **A withdrawal is proven against the ARTIFACT, not against the
  claim**, and a hash captured at the wrong moment is exactly the failure that
  law names. The SUBSTANCE is independently verified and stands: Review 16
  regenerated the whole deck to a scratch path and got a file byte-identical to
  the committed `first-workflow.html` (`e3f97508…`), with no `zzz` residual and
  four group rows. Re-confirmed at this seat 2026-08-07 after the derived-number
  work below — scratch render still `e3f97508…`, byte-identical. The old hash is
  struck rather than re-asserted.
- **Documented gap, deliberately not closed**: `gate_keyword_coverage` forbids a
  word the examples USE from being unexplained; nothing forbids the reverse (a
  row explaining a word no example contains). The probe above demonstrated it —
  the `zzz` row passed the gate. Stated in `keyword_table_shape`'s docstring
  rather than left as a hidden assumption.

### F2 — one refusal, false at one of its three sites · CLOSED (+ one half REFUTED)

`wheel_torn_down` was shared by the pre-arm gate, the post-insert retraction,
and the append boundary, on the reasoning that differing wording teaches the
operator the causes differ when they do not. Sound reasoning, **false premise**:
at the append boundary the timer HAD been armed, HAD elapsed and HAD fired —
what is refused there is the append. And the boxed error in the blocking body
left the closing `map_err` nothing to switch on, so the teardown was reported as
`EngineSeamError::Recorder`.

Split into `wheel_torn_down_before_arming` / `wheel_torn_down_after_firing`, and
a typed `TimerAppendError { WheelTornDown, Append }` replaces the boxed channel
so the seam variant survives.

**The PLAUSIBLE secondary was REFUTED by writing the test.** Review 15 suspected
a torn-down deadline would grind the 6-attempt ladder and emit `tracing::error!`
for an orderly shutdown. Written as a deadline test first, it failed with
*"no deadline handler registered"* — because `fire_timer_guarded` demuxes a
reserved `deadline:{run}` timer to `fire_deadline` BEFORE the generic
record-then-deliver path, so a deadline **never reaches the append boundary that
raises the refusal**. The ladder was never exposed to it. An ORDINARY timer is
the path that does reach it; `is_wheel_teardown` + the early return serve that
path and are measured there. The refutation is written into the code
(`is_wheel_teardown`'s doc and the test's doc), not just here.

### F3 — the file this lane grew was the one it left over the cap · CLOSED

`nif_timer_bridge.rs`: HEAD **459** production lines → worktree **520**, against
a 500 cap the lane had just enforced on two neighbours (`api.rs` 554→257,
`builder.rs` 597→419). Split at a real seam — everything that runs *after* a
timer is due moved to `runtime/nif_timer_fire.rs`.

- `nif_timer_bridge.rs` **436** · `nif_timer_fire.rs` **139** · both under 500.
- ⚠️ **Measurement defect caught in my own counter**: splitting on the first
  `#[cfg(test)]` truncated at the inline test seam at :409 and reported **26**
  production lines. Absurdity was visible, which is the only reason it was
  caught. The counter now splits on the literal `\n#[cfg(test)]\n#[path` module
  marker. **Any future line-count verdict must use that marker.**

### F4 — minor · CLOSED
`cli_crate_and_version() -> tuple[str, str]` returns a 3-tuple; annotation and
"Both are READ" docstring corrected to three.

### Mutations (both restored byte-identical)

| # | mutation | result |
|---|---|---|
| M13 | `into_seam_error`: `WheelTornDown` → `Recorder` (the literal prior flattening) | 🔴 `stand_down_is_not_a_fault` RED, **other 3 green** |
| M14 | `wheel_torn_down_after_firing` → the literal prior "was not armed" text | 🔴 `a_torn_down_wheel_refuses_to_append` RED, **other 3 green** |

Orthogonal by construction: M13 kills only the type assertion, M14 only the
wording assertion. Restore hash `nif_timer_fire.rs`
`ae31f9165007ffcab69998a33c5cbdcbc0b7df455e4c35c209380816a4190e4e`.

### Gates after F1–F4
- `cargo test -p aion-rs --lib runtime::` → **215 passed / 0 failed**
- `cargo clippy -p aion-rs --all-targets -- -D warnings` → exit 0, zero diagnostics
- tutorial build → exit 0, all gates announced green
- 🔴 **AMENDED 2026-08-07 (Review 16 minor 3): this line was STALE when Review 16
  read it.** It said "⏳ full workspace battery + workspace clippy: RE-RUN
  REQUIRED on this tree" — but both had already been re-run green on that very
  tree, and §9a below records them (243 declared / 247 verdicts / 243 emitted /
  4215 passed / 0 failed, `RC_WS=0`; clippy exit 0, zero diagnostics). A pending
  marker left standing after the work is done is not a harmless leftover: the
  next reader either re-runs an hour-long battery for nothing, or starts
  discounting the ⏳ marks in this file, and the second is the expensive one.
  **Both gates were also re-run again on the F-A/F-B tree — see BU §6.**

### §9a — BATTERY ON THE POST-REVIEW-15 TREE: GREEN, reconciled

```
sentinel RC_WS = 0
declared : 223 distinct binaries + 20 doc-test targets = 243
verdicts : 247                                   surplus +4
targets emitting >=1 verdict : 243 / 243   ZERO-verdict targets: NONE
totals   : 4215 passed / 0 failed / 0 ignored
```

Surplus re-derived, not recalled: `aion_server-5ce111d9e042313f` emits 4 and
`Doc-tests aion` emits 2 — both spawn child cargo processes — and (4−1)+(2−1)
is exactly the +4. The load-bearing line is **ZERO-verdict targets: NONE**;
a crashed binary emits no `test result:` line at all, so a surplus can never
rule one out and only the per-target check can.

**4215 vs the previous run's 4214 is +1 — exactly the one test added
(`stand_down_is_not_a_fault`).** Unplanned cross-check, taken.

Workspace clippy on the same tree: exit 0, **zero diagnostics**.

🔴 **#85 DID NOT FIRE AGAIN. TWO CONSECUTIVE ABSENCES ARE STILL NOT EVIDENCE.**
It is contention-dependent (2/2 contended runs reproduce, 5/5 isolated green).
A green where a known intermittent merely failed to appear is WEAKER evidence
than a green with it present and attributed. #85 stays open, unchanged.

### §9b — 🔴 A SECOND COMFORTABLE-NUMBER DEFECT, 30 MINUTES AFTER THE FIRST

The reconciliation regex read `^\s*Running\s+\S+\s+\((target/\S+)\)$`, which
requires exactly ONE token between `Running` and the paren. Cargo emits
`Running unittests src/lib.rs (target/...)` — **two** tokens — so every unit-test
binary was silently dropped. Declared came back **200** instead of 243, turning
the deficit into a reassuring **surplus of +27**.

It was caught only because 200 ≠ the 223 the same tree shape produced an hour
earlier. **There was no error, no exception, and the wrong answer pointed in the
safe direction.** This is the same family as the line-counter that read 26 for a
520-line file, and it is now twice in one session.

**Standing rule for this estate:** a measurement tool must be run against a
KNOWN answer before its verdict is quoted, and a number that makes a problem
disappear is a trigger to check the TOOL, not a finding. Correct anchor here:
`^\s*Running\s+.*?\((target/\S+)\)\s*$`.

### §9c — Review 16 dispatched (delta only)

Scoped strictly to the F1–F4 fixes, NOT a whole-lane re-review — Review 15
explicitly cleared the engine work. Brief: `scratchpad/review-16-brief.md`.
Dispatched because the base rate demands it: nine consecutive refusals, and
twice the findings were defects introduced while fixing the previous ones.
**If its verdict is lost to a compaction and no notification arrives,
re-dispatch rather than assume a pass.**
