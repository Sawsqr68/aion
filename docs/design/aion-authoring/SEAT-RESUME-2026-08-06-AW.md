# SEAT RESUME — marker AW, 2026-08-06

Supersedes AV for STATE. AV holds the closed sweep row + push receipts (DONE,
nothing owed). AT holds the original 15 #7 findings. Latest letter wins.

---

# 🔴 THE THIRD OPUS REVIEW CAME BACK: **NOT SAFE TO LAND**

Every claim was re-verified at the bytes at my own hand before any edit. All
five substantive findings CONFIRMED. Three of them are *incomplete prior fixes* —
each re-opens the exact hazard the earlier fix was written to close, and each is
invisible to the mutation board because **the mutation sits on the covered branch
while the hazard lives on the uncovered one.**

## Independently re-measured before building

    cargo clippy --workspace --all-targets -- -D warnings   → exit 0
    cargo test -p aion-rs --lib                             → exit 0, 642/0
    cargo fmt --check                                       → exit 1   ← RED

## F1 (MAJOR) — the epoch gate covered ONE of three durable-write paths. FIXED.

`is_epoch_open()` sat inside the `else` (no-terminal-yet) arm only. The
`Err(existing)` resume arm appended `retire_run_deadline` (a real
`TimerCancelled`) with **no gate at all**, and `start_continuation_replacement`
appends `WorkflowStarted` **and spawns a beamr process** after the block. The
comment claiming "THE APPEND BOUNDARY. Checked here and nowhere earlier" was
false as written.

**Built:** new `refuse_if_epoch_closed(context, handle)` — one rule, one
function. Called (a) immediately after the recorder lock is taken, BEFORE the
branch, so it covers every append in the critical section, and (b) at the top of
`start_continuation_replacement`, which is outside the lock and writes a
different run's history.

## F2 (MAJOR) — `max_backoff` reproduced the hot loop m-2 claims to close. FIXED.

`check_completion_retry` refused only `initial_backoff == 0`. `max_backoff:
ZERO` passes it, and `sleep_backoff` assigns `ceiling` whenever doubling exceeds
it ⇒ interval is 0 from the first advance, forever, with no attempt budget.
Same for any ceiling below the floor (ladder ratchets DOWN).

**Built:** the gate also refuses `max_backoff < initial_backoff`, with a
`format!` reason naming both values. Refusing a self-contradictory pair; it
invents no floor, cap or budget.

## F3 (MAJOR) — C-1's fix sat one level below the API the server calls. FIXED.

`RuntimeHandle::shutdown` was corrected, but `Engine::shutdown` — what
`aion-server` actually calls — still short-circuited: `close_and_wait()?`
returned before the epoch was ever gated (registry poison), and it is a condvar
wait with **NO timeout**, so a lifecycle op stuck on a degraded store (exactly
what arms completion retries) blocks *before* the close. Then
`self.runtime.shutdown()?` skipped `shutdown_timer_wheel()` and
`clear_engine_seams()`.

**Built:** `self.runtime.engine_tasks().begin_close();` is now the FIRST
statement — cannot fail, idempotent, so no path leaves retries armed. Then a
`keep(...)` closure captures the first error across `close_and_wait` and
`runtime.shutdown` so every teardown step runs and the first error returns last.

## F4 — `cargo fmt --check` RED (19 diffs, 5 files). NOT YET DONE.
## F5 — `crates/aion/src/store_faults.rs` is UNTRACKED. NOT YET DONE.
`lib.rs` declares it and `completion.rs` uses it in 9 places, so committing
without it lands a NON-COMPILING test build. **`git add` it EXPLICITLY** (never
`git add -A` in this tree).

## Minors built so far

- **F6/F7/F8/F9 doc-theft ×4** — a doc block inserted between an existing doc
  and its item, stealing it. `retry_process_exit` (doc restored, now also names
  the reopen exit), `claim_owned_shards`/`apply_owned_shards` (order swapped),
  `force_shutdown_failure`/`begin_shutdown` (split), and a test carrying TWO
  contradictory doc blocks in `engine_tasks.rs` (merged).
  🔴 **That last edit ATE the `#[test]` attribute** (it sat between the two doc
  blocks) — caught by reading the result, restored. A silently un-run test is
  exactly the damage this class does. Clippy cannot see any of these: the items
  are private, so `missing_docs` never fires.
- **F10 FaultReporter flood** — "report on change" is trivially satisfied by a
  store alternating between the two transient discriminants ⇒ one warn per
  attempt, the #94 flood wearing the transition rule's name. Replaced
  `reported: Option<&str>` with `seen: Vec<&'static str>`: a fault is stated when
  genuinely NEW (at most once per discriminant) or when the ladder says it is due.
- **F11 reopen race** — once `progress == Recorded` the run is terminal so
  `reopen` is admissible; a reopen during the retry window made the next attempt
  re-append and hand the operator a `SequenceConflict` (THE double-writer
  indicator) for a benign event. `TerminalProgress` is now threaded ACROSS
  attempts (`retry_process_exit` takes `arming_progress`, `complete_process_exit`
  takes `prior`), and an attempt that finds no terminal while `progress ==
  Recorded` abandons quietly with an `info!`.
- **F12 misleading test failure** — `shutdown_closes_the_epoch_even_when_a_drain_fails`
  now returns `Box<dyn Error>` (the convention everywhere else in that file) and
  `ok_or`s a MESSAGE, not the injected error shape.

## 🔴 STILL TO DO — NOTHING HERE IS OPTIONAL

1. **F13 god file.** `completion.rs` is 544 code lines (limit 500). Extract the
   retry machinery — `TerminalIntent`, `CompletionFailure`, `TerminalProgress`,
   `FaultReporter`, `failure_discriminant`, `store_error_is_transient` — into
   `crates/aion/src/lifecycle/completion_retry.rs`. `builder.rs` is 625 (was 596
   BEFORE this change, so it is pre-existing but I made it worse).
2. **F14** — one sentence on `TerminalIntent` naming the `exit_time` reuse
   ordering consequence (a late retry's `recorded_at` can precede its
   predecessor's). NOT a determinism violation; no consumer found to break, but
   the survey was never written down.
3. **F15** — `crates/aion-client/src/transport/embedded.rs:458` `map_engine_error`
   has a `_ =>` wildcard, so the two new variants land in the generic bucket and
   the compiler will not surface the next one. Unreachable today; it is the
   "one rule in two places" shape.
4. **NEW MUTATION LEGS — decisive, on-path, one per major:** F1 (drive the
   RESUME arm with a closed epoch and assert nothing is appended), F2 (zero
   `max_backoff` accepted), F3 (`Engine::shutdown` gating + first-error capture).
   The board currently has 15 legs and would pass with all three defects present.
5. Re-run on the resulting tree: `cargo fmt` (autoformat, never format-check),
   `clippy --workspace --all-targets -D warnings`, `cargo test -p aion-rs`,
   full mutation board.
6. Then commit — **explicitly naming files, including `store_faults.rs`** — and
   push.

## Standing constraints (unchanged)

NO WORKTREES · never `git add -A` · never `git stash -u` · NO `git gc`/`prune`
until the estate deletion hold lifts · DELETE NOTHING · Tom's pids 45425 / 48399
untouched · worker rebuilds frozen until the #139 live cut ·
`<<autonomous-loop-dynamic>>` wakeup re-armed every tick · commit messages with
backticks via `git commit -F -` and a quoted heredoc.

## Sweep row: CLOSED, nothing owed

140/140, nothing lost; 139/140 pins pushed and verified object-for-object; the
140th ruled to stay put (blocked by a 105 MB pre-scrub blob, folds into Tom's
off-box-mirror item). Detail in marker AV.
