# SEAT RESUME — marker BC (2026-08-06)

**Supersedes BB for STATE.** Vesper Lynd, session
`5849e0d8-4802-4869-8e0f-9f7fd187e198`. BB's *findings* stand; its "order of
work" is now discharged through item 7.

# STATUS: review-5's blockers 1 and 2, majors 3 and 4, moderates 5 and 6, minor 7 — ALL FIXED AND MUTATION-PROVEN. Not yet re-reviewed, not yet committed.

---

## What was done, and how each was proven

### BLOCKER 1 — the finding-5 guard stranded every reopened run. FIXED.

**Reproduced at my own hand before touching the fix.** Wrote
`a_reopened_run_that_finishes_again_records_its_terminal` against the unfixed
tree and watched it fail with `None` where a terminal must be — the run
projecting `Running` forever, the doorbell unrung, `Ok` returned so nothing
logged. That is the regression proof and it exists in the suite permanently.

**The remedy is lease identity, not history.** `crates/aion/src/lifecycle/completion.rs`:

```rust
let superseded_by_a_newer_lease = context
    .registry
    .get(handle.workflow_id(), handle.run_id())
    .ok()
    .flatten()
    .is_some_and(|current| current.pid() != handle.pid());
if matches!(*progress, TerminalProgress::Recorded) || superseded_by_a_newer_lease {
```

Conservative by construction: stands down only when a **different live lease is
positively observed**. An absent or unreadable registry entry falls through to
pre-existing behaviour, so this cannot regress anything by *inventing* a
stand-down out of missing evidence.

`run_segment_was_reopened` is **deleted**. The call-site comment is rewritten
against the real mechanism and now states, in the code, why the history
predicate is unrepairable rather than merely wrong.

`WorkflowHandle::pid()` confirmed at `crates/aion/src/registry/handle.rs:295`
(`pub const fn pid(&self) -> u64`); `Registry::get` at `table.rs:159`.

**Both tests kept, and the fixtures now carry the discriminator explicitly:**
- `a_reopened_run_is_not_appended_into_by_a_monitor_that_never_recorded` —
  registers a **successor handle** (`respawned_lease`, pid+1) under the same
  `(workflow, run)` key, modelling `reopen::respawn_and_register`, and asserts
  `assert_ne!` on the pid before running. Without that assertion the test could
  pass for the wrong reason.
- `a_reopened_run_that_finishes_again_records_its_terminal` — asserts
  `assert_eq!` that the registry holds THIS handle, i.e. it has not been
  superseded.

### MODERATE 6 — the false compaction justification. FIXED, AND THE DISJUNCT IS NOW COVERED.

**Mutation B, first pass: dropping `progress == Recorded` turned NOTHING red
across the whole `lifecycle::` module (97 passed, 0 failed).** So the disjunct
was either unreachable or untested, and BB's own rule said: if nothing goes red,
delete it.

Before deleting I read `reopen.rs`. It is **reachable**, and the window is
precise: reopen `registry.remove`s the terminal-cached handle (`reopen.rs:111`)
BEFORE appending `WorkflowReopened` (`:130`) and only registers the successor at
`respawn_and_register` (`:148`). In that interval the registry holds **nothing**
for the key — `get` → `None`, `superseded_by_a_newer_lease` → false, correctly —
and history holds no terminal, because the reopen superseded it. A retry attempt
landing there having already recorded its own terminal would append a **second**
one into a run mid-revival. `progress` is the only thing that still knows.

So: wrote `a_recorded_monitor_stands_down_inside_the_reopen_window`, which calls
`handle_process_exit_attempt` directly with `progress = Recorded`, terminal +
reopen in history, and the registry entry removed — asserting both halves of the
window as fixture controls first.

**Mutation B, second pass: exactly 1 RED, the new test, `left: 2 right: 1`.**

The false sentence ("survives a compaction that the history scan would not") is
gone and its retraction is written into the code, naming that compaction does
not exist in this codebase.

### BLOCKER 2 — the `not_owner` arm matched a shape production never makes. FIXED.

`crates/aion-client/src/transport/embedded.rs`. Both shapes now route through
**one** classifier, `store_error_class`, which is the only place the store rule
is written:

```rust
aion::EngineError::Store(store)
| aion::EngineError::Durability(aion::durability::DurabilityError::Store(store)) => {
    store_error_class(store, error)
}
```

Full store family classified: `NotOwner` → `not_owner`, `NotFound` →
`not_found`, `SequenceConflict`/`Backend`/`Serialization` → `backend`, matching
`aion-server`'s `wire_from_store` composed with `ClientError::from_wire_error`.

The test table now carries the **production** shape
(`Durability(Store(NotOwner))`), plus `Durability(Store(NotFound))` and a
store-family control (`Durability(Store(SequenceConflict))` → `backend`) so a
store fault cannot acquire a caller-facing class just by arriving through the
same helper.

### MAJOR 3 — the query family. FIXED.

`query_error_class` mirrors `aion-server`'s `query_wire`: `UnknownQuery` →
`unknown_query`, `Timeout` → `query_timeout`, `NotRunning`/`ReplyDropped` →
`not_running`, `Unknown` → `not_found`, `HandlerFailed` → `query_failed`,
`Engine` → `backend`. All six rows in the table.

### MAJOR 4 — `ContractIdentity` / `NoQueueDeclaration`. FIXED.

Both → `invalid_state`, matching the server. Clippy's `match_same_arms` then
correctly required them merged with the four existing `invalid_state` variants;
**merged in code, comment rewritten to cover all six** — not suppressed.

Required re-exporting `ContractIdentityError` from `aion` (it is the `source`
field of `EngineError::ContractIdentity`, so no downstream can construct or
destructure that variant without it — the same real API gap as `ContentHash`,
and the lib.rs comment now names both).

### MODERATE 5 — the divergence I introduced. FIXED BOTH WAYS.

`ShuttingDown` → **`not_running`** (was `unavailable`), matching the server.
`unavailable` is the class the SDK also raises for a dial failure, so the old
mapping told an in-process caller its endpoint was unreachable when the engine
it holds was shutting down.

`EngineTaskEpochClosed` **arm deleted**. Constructed at exactly one site
(`completion.rs:124`, the terminal-append boundary on the internal process-exit
path), reachable by the completion monitor and by none of the eight transport
operations. The deletion is stated in the comment as a claim about what this
surface can return.

The doc comment's over-claim is **retracted in place**: it now says explicitly
that the dependency runs server → client, that these expectations are a
TRANSCRIPTION rather than a check, that a rule in two places with nothing
forcing agreement has already drifted, and that the durable fix is a shared
classifier needing a crate both can depend on — out of scope here, but named
rather than implied away.

### MINOR 7 — counts. FIXED.

`~58 variants` → **52** (counted from `error.rs`; both mentions corrected).
"four return points" → five and "six call sites" → eight were marker-BA prose,
not code; corrected here by amendment rather than by editing the historical
marker.

---

## Mutations run this session (all restores sha-verified)

| # | mutation | result |
|---|---|---|
| A | completion: drop the lease disjunct | 1 RED — `a_reopened_run_is_not_appended_into…` (`left: 2 right: 1`) |
| B1 | completion: drop `progress == Recorded` (before the new test) | **0 RED across `lifecycle::` — the finding** |
| B2 | same, after the new test | 1 RED — `a_recorded_monitor_stands_down_inside_the_reopen_window` |
| 1 | embedded: drop the `Durability(Store)` half | RED — `backend` vs `not_owner` |
| 2 | embedded: drop the `Query` arm | RED — `backend` vs `unknown_query` |
| 3 | embedded: drop `NoQueueDeclaration` from the state arm | RED — `backend` vs `invalid_state` |
| 4 | embedded: `ShuttingDown` back to `unavailable` | RED — `unavailable` vs `not_running` |
| 5 | `store_error_class`: `NotOwner` → `server` | RED — `backend` vs `not_owner` |

`completion.rs` restored to `40e00b7da9b66fa03f90b6455942a8ba3b1b9bcc`;
`embedded.rs` restored to `2b52321a0b1c48c75a86d8897f3e639dcccf9fd3` before the
later refactor.

---

## Gates measured at my own hand

    cargo fmt --all                                        (write, whole tree)
    cargo clippy --workspace --all-targets -- -D warnings  CLIPPY_EXIT=0 (read BARE)
    cargo test -p aion-client --lib                        62 / 0
    cargo test -p aion-rs --lib lifecycle::completion      24 / 0
    cargo test -p aion-rs --lib lifecycle::                97 / 0
    cargo test -p aion-rs (full)   ⏳ RUNNING — started 08:06:55Z, nohup+disown,
                                   log <scratchpad>/aion-rs-full.log

Clippy fired twice and both were **fixed in code, never suppressed**:
`match_same_arms` (merged the six `invalid_state` variants) and `too_many_lines`
(the case table lifted out of the test body, then split into
`admission_and_run_state_cases` + `store_and_query_cases`). Table now carries
**19 rows**, verified by count after the split.

🔴 One expected red, **not mine**: `outbox_out_of_order_completions_still_settle_with_n_terminals`
= **#85** (libsql 0.9.30 double-close), structurally attributed. Anything else
red is mine. **Contended-red-is-not-red** if anything else ran alongside.

---

## STILL TO DO

1. Read the full `aion-rs` battery result.
2. `cargo test -p aion-server --lib` (was 796/0 before this change; the
   `aion` lib.rs re-export touches it).
3. **Opus sub-agent review** — CLAUDE.md mandates it, and this round has earned
   it twice over. Give the reviewer BB (the findings) and this marker (the
   remedies), and tell it the tree ALSO carries unreviewed `engine/api.rs`,
   `engine/builder.rs`, the `nif_child_tasks.rs → engine_tasks.rs` rename, and
   four new untracked source files.
4. Commit naming files EXPLICITLY (never `git add -A`), message via
   `git commit -F -` with a quoted heredoc. Then push.
5. Tell Tom plainly — he asked twice about real fixes and was right to.
6. Then the remaining tutorial-self-run issues: **#5** (spec banked,
   `SPEC-5-AWL-SUM-COMBINATOR.md`), **#1** (`workflow.now` history-tail read),
   **#6** (`max_concurrency`; note the third unfiled defect — worker selection
   is `min_by_key(worker.id)`, so there is **no load balancing within a pool**),
   **#9** (reply only, past Waffles).
7. **Owed to Tom:** dry run of the #146 tutorial + one-page crib sheet, without
   touching his pids.

## Files changed in this session's remedy

    crates/aion/src/lifecycle/completion.rs
    crates/aion/src/lib.rs
    crates/aion-client/src/transport/embedded.rs

(on top of the tree BB describes — see marker BA for the full explicit file list
for the commit)

---

## Tom

Messaged at ~07:5xZ with the plan, in plain language: the regression found and
fixed, the false comment killed, the five remaining items in order, and that the
tutorial is ready NOW and does not wait on any of this. He replied with approval
to go ahead and to keep things merged as I go.

---

## Standing constraints (carry verbatim)

- **NO WORKTREES** in aion. One mutating lane. Unmarked dirty = STOP.
- ⛔ **NEVER `git add -A`** — it swallows the NO-SWEEP untracked dirs and hangs.
  Name files explicitly. Commit message via `git commit -F -` with a **quoted**
  heredoc (zsh eats backticked words from `-m`); verify with `git log`.
- ⛔ Never bare `cargo clean`; no custom `CARGO_TARGET_DIR`; no launchd.
- ⛔ **NO-SWEEP** untracked holds: `.aion-accepted-binary-20260731/` (**SOLE copy
  of Tom's accepted build — nothing may touch it**),
  `.aion-jjuabk-snapshot-20260731/`, `.aion-data-preswap-20260804/`,
  `../aion-pre-scrub-backup-20260804.git`.
- ⛔ No `git gc`/`prune`/`--prune`; no `--force`/`--mirror`/`--delete` on push.
  **DELETE NOTHING** while the estate-wide hold stands.
- ⛔ `git stash push` never with `-u`; stash by explicit path.
- 🔴 **Tom's live pids: server 45425, worker 48399**, started from repo root.
  **DO NOT TOUCH.** Untouched all session.
- 🔴 **WORKER REBUILDS FROZEN** estate-wide until the #139 live cut.
- 🔴 beamr JIT blocker (Artemis): keep long workflows off until fixed beamr lands.
- 🔴 `aion server` must start FROM THE REPO ROOT (store is cwd-relative `./aion-data`).
- 🔴 **NEVER PUBLISH ARTIFACTS FOR TOM.** Deliverables are local files.
- 🔴 CLAUDE.md mandates an **Opus** sub-agent review before work lands.
- 🔴 Do not commit while a battery is in flight (the #123 build-identity pin).
- Never mention prospekt to Tom.
- Estate diff driver emits no ±: `git -c diff.external= diff --no-ext-diff`.
- zsh: no `PIPESTATUS` (lowercase `$pipestatus`, 1-indexed); `status` is
  read-only — name it `rc`. **Never cite an exit code off a pipeline tail.**
- BSD `sed` has no `\s` — use `/usr/bin/python3`. `~/.local/bin/python3` cannot exec.
- Foreground `sleep` is blocked. Use `until <check>; do sleep N; done` or
  `nohup … & disown` (plain background bash dies in waves).
- Box is AEST (+1000): always `-t "%Y-%m-%dT%H:%M:%S%z"`.
- **Mutation discipline:** back up, mutate, observe RED, restore, prove with
  `shasum -a 1`, re-run green.
- Autonomous `/loop`, dynamic pacing — **every tick MUST end with
  `ScheduleWakeup` carrying the literal sentinel `<<autonomous-loop-dynamic>>`.**
