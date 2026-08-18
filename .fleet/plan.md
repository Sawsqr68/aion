# fleet-51 — plan

**Issue:** ablative-io/aion #51 — *Reopen misreports a single reopen as concurrent when terminal
reconciliation lags the result future.*
**Investigation verdict:** Reproduced (investigator confidence: high).
**Measured tree:** `bb3b770538d5a117a2ff63119fcdfc70d94bc368`.
**This tree's HEAD:** `bb3b770538d5a117a2ff63119fcdfc70d94bc368` — **identical**. Every citation was
nonetheless re-read by hand in this tree; results in §2, including the three places where the
investigation's citation and the mechanism it names come apart.

One leg: [`.fleet/legs/reopen.md`](legs/reopen.md).

---

## 1. The defect, in this tree

A run that fails while resident terminates through
`lifecycle::completion::handle_process_exit_attempt`. That function performs, strictly in this
order:

1. appends `WorkflowFailed` through the handle's recorder — `completion.rs:418-424`;
2. rings the completion doorbell — `handle.completion().notify(terminal);` at `completion.rs:519`;
3. `upsert_workflow_visibility(...)` — `completion.rs:530-536`;
4. `reconcile_terminal_registry(...)` — `completion.rs:537`, which reads history, calls
   `Registry::reconcile` and `Registry::replace_residency(.., Suspended)` (`completion.rs:541-552`).

`Engine::result` returns as soon as step 2 is observed (`api_workflow_ops.rs:520-524`), because
`CompletionNotifier::notify` is a `watch::Sender::send_replace` (`handle.rs:72-74`) and the waiter
returns on the first `Some`. So between step 2 and step 4 there is a real, caller-observable
interval in which:

- durable history **already** projects `Failed`, and
- the still-registered resident handle **still** caches `Running` (nothing between the handle's
  registration and step 4 rewrites `cached_status` — the only production `Registry::reconcile` call
  sites are `transition.rs:193`, `completion.rs:547`, `startup.rs:321`, `startup.rs:396` and
  `api_workflow_ops.rs:563`, none of which is on this path).

`lifecycle::reopen::reopen` validates history first and correctly concludes *reopenable-terminal*
(`reopen.rs:97-98`). It then, independently, consults the handle's **cached status** as its proof of
whether a concurrent reopen is running (`reopen.rs:109-119`):

```rust
if let Some(existing) = context.registry.get(id, run)? {
    if existing.cached_status().is_terminal() {
        context.registry.remove(id, run)?;
    } else {
        return Err(EngineError::InvalidState {
            reason: format!(
                "workflow {id} run {run} was already reopened and is Running (concurrent reopen)"
            ),
        });
    }
}
```

In the interval above, `cached_status()` is `Running` while history says `Failed`, so one prompt,
legitimate, *single* reopen takes the `else` arm and is refused with the quoted concurrent-reopen
`InvalidState`. History is the correct authority for admitting this case; the cached status is a
lagging projection being used as a lock.

**Why the naive repair is wrong.** Simply calling `Registry::reconcile`/`Registry::remove` by key
would clobber a genuine winner: between this caller's `read_history` (`reopen.rs:88`) and the
mutation, another reopen can append `WorkflowReopened` and register a brand-new handle, and an
unconditional remove-by-key would evict it. The removal must be **identity-checked against the exact
handle instance this caller observed**.

**What actually serializes two writers.** `Recorder::record_workflow_reopened`
(`recorder.rs:501-515`) is the sole append path for `Event::WorkflowReopened`; it routes through
`Recorder::append_with` → `Recorder::append_one` (`recorder.rs:1023-1043`), which reads
`self.sequence.current()` as `expected_seq` and hands it to `EventStore::append`. Two reopens built
at the same history head therefore both compute the same `expected_seq`; the store admits exactly
one and refuses the other with `StoreError::SequenceConflict` (`aion-store/src/memory.rs:414`),
which surfaces as `EngineError::Durability(DurabilityError::Store(..))` (`error.rs:119`). The
registry check is an *early, courteous* rejection; the recorder is the actual serializer. This is
what makes an identity-checked removal safe: a caller that passes admission on a stale snapshot
still cannot double-append.

---

## 2. Citation re-verification at this tree's HEAD

HEAD equals the measured commit, so no citation has *moved*. Each was still re-read by hand. Three
carry substantive corrections that the leg brief must act on — flagged **⚠ DRIFT**.

| # | Citation | Verified? | Note |
|---|---|---|---|
| 1 | `crates/aion/src/lifecycle/completion.rs:418` | ✅ verbatim | `TerminalIntentOutcome::Failed(error) => { recorder.record_workflow_failed(...).await?; ... }` at 418-424; the quoted "DURABLE POINT 2" comment is at 425-429. |
| 2 | `crates/aion/src/lifecycle/completion.rs:519` | ✅ verbatim | `handle.completion().notify(terminal);` at 519; `upsert_workflow_visibility(...)` at 530-536; `reconcile_terminal_registry(...)` at 537. The ordering the issue depends on is exactly as claimed. |
| 3 | `crates/aion/src/lifecycle/completion.rs:541` | ✅ verbatim | `reconcile_terminal_registry` at 541-552: `read_history` → `registry.reconcile` → `replace_residency(.., Suspended)`. Confirms the handle is **reconciled, not removed** — so a terminal-cached leftover is the normal post-failure state. |
| 4 | `crates/aion/src/engine/api_workflow_ops.rs:520` | ✅ verbatim | `let mut receiver = handle.completion().subscribe();` at 520, the `loop { if let Some(outcome) = receiver.borrow().clone() { return ... } }` at 521-524. `Engine::result` also short-circuits on already-terminal history at 505-508. |
| 5 | `crates/aion/src/registry/handle.rs:67` | ✅ mechanism verified, **line off by 5** | `CompletionNotifier::notify` with `drop(self.sender.send_replace(Some(outcome)));` is at **72-74**; line 67 is inside the `subscribe` doc comment. Same function, same semantics. Cosmetic. |
| 6 | `crates/aion/src/lifecycle/reopen.rs:93` | ✅ verbatim | The history-first comment at 93-96, `run_segment`/`validate_and_compute_reopened` at 97-98, and the defective handle check at 109-119, quoted-string included. This is the defect site. |
| 7 | `crates/aion/src/registry/table.rs:222` | ✅ verbatim | "The projected status always wins." at 224; `Registry::reconcile` at 230-244. `Registry::remove` (169-187) and its index compare-and-delete (180-185) are the primitives the new one must sit beside. |
| 8 | `crates/aion/src/durability/recorder.rs:490` | ✅ verbatim | The "sole append path … single-writer sequence discipline" sentence is at 493-495, inside the `record_workflow_reopened` doc block; the fn is at 501-515. |
| 9 | `crates/aion/src/durability/recorder.rs:207` | ⚠ **DRIFT** | The cited `let expected_seq = self.sequence.current(); … store.append(…, expected_seq) … mark_append_success(batch.len())` **is** at 207-211 — but it is inside `record_workflow_started_with_attributes`, a two-event batch path that **reopen never takes**. The discipline reopen actually relies on is the single-event twin `Recorder::append_one` at **`recorder.rs:1032-1043`**, reached via `append_with` (1023-1030). Identical rule, different function. The brief points the builder at `append_one`. |
| 10 | `crates/aion/tests/activity_retry_e2e.rs:224` | ⚠ **DRIFT (operational)** | The cited result → assert-`Failed` → `reopen_workflow` sequence is present in `exhausted_retries_fail_the_run_and_reopen_continues_the_trail`. But this target **is red at HEAD on this box, before any change**, for issue #64 — see §5. It cannot serve as a gate. |

### ⚠ DRIFT 3 — the prescribed test location is pure-unit at HEAD

The investigation prescribes adding
`reopen_after_result_before_terminal_registry_reconcile_succeeds` to
`crates/aion/src/lifecycle/reopen_tests.rs`. That file (360 lines) is at HEAD **entirely pure unit
tests**: it builds `Vec<Event>` by hand and calls `validate_and_compute_reopened` and
`rearmable_timers` directly. It has no engine, no registry, no runtime, and no live run. Taken
naively, "drive a resident run to `WorkflowFailed`" reads as *needs a real Gleam workflow*, which on
this box means *permanently skipped* (§5) — a green that proves nothing.

**Grounded resolution — it can be done in-lib, with no Gleam and no new injection seam.** Three
facts in this tree make it work, and the brief spells all three out:

1. `lifecycle::completion::handle_process_exit_attempt` is **`pub(super)`** (`completion.rs:321`),
   i.e. visible throughout `crate::lifecycle` — so `lifecycle::reopen`'s own `mod tests` can call it
   directly. So are `TerminalIntent`, `TerminalIntentOutcome` and `TerminalProgress`
   (`completion_retry.rs:39/121/183`). No visibility change is required.
2. `completion.rs`'s own test fixture `active_workflow_over` (`completion.rs:758-807`) already
   builds a complete `ProcessExitContext` + `Registry` + registered `WorkflowHandle`
   (`cached_status: Running`) over an `InMemoryStore`, with **no BEAM workflow process and no
   Gleam**. That is the shape the red test's setup copies.
3. A real reopen — `respawn_and_register` → `recover_active_workflow` → a genuinely spawned BEAM
   process — is drivable in-lib via the repository's existing pair
   `RuntimeHandle::register_waiting_test_module` (`runtime/handle/test_support.rs:105`) and
   `WorkflowCatalog::note_loaded_workflow_for_test` (`loader/catalog_test_support.rs:21`), exactly
   as `engine/startup_tests.rs:705-745` and `lifecycle/continue_as_new.rs:318-320` already do. The
   spawned module's exported function is a bare `Wait` instruction, so it parks. So the test can
   **require reopen to fully succeed**, as the investigation specifies — not merely to avoid one
   error variant.

### ⚠ DRIFT 4 — there is no existing "hold point inside a production function body" idiom

Searched: there is no prior art in `crates/aion` for pausing a production path from a test. The two
nearest idioms, both of which the design borrows from rather than inventing something new:

- **`#[cfg(test)] #[path = "…"] mod test_support;`** carrying an `impl` block on a production type —
  `runtime/handle.rs:912`, `loader/catalog.rs:729`, `runtime/process_exit.rs:505`. This is how the
  crate exposes test-only *arming* APIs without fragmenting the production type.
- **An `Option<Arc<dyn Trait>>` seam field on an operation context**, `None` in production —
  `StartupRecoveryContext.recovery` (`engine/builder.rs:138`, `engine/api.rs:583`).

The second is rejected here: `ProcessExitContext` is constructed at `start.rs:557` and
`startup.rs:505` and rebuilt at `completion_retry.rs:269`, and threading a field from `EngineBuilder`
through `StartWorkflowContext` to reach it would touch far more production code than the defect
warrants. The design uses the first: an always-compiled module whose *state* is empty in production
(so the call site is an unconditional, inert lookup) and whose *arming* API is `#[cfg(test)]` (so no
production caller can arm it). Details in the brief.

---

## 3. The fix

Red tests first, then the fix. Full specification, with code shapes, in
[`.fleet/legs/reopen.md`](legs/reopen.md). Summary:

**R1 — `Registry::remove_if_pid`, an identity-checked removal trichotomy**
(`crates/aion/src/registry/table.rs`, beside `remove` at 169-187). Returns
`Removed` / `Absent` / `Replaced(WorkflowHandle)`, decided under the single `slots` lock, comparing
the occupant's `pid()` against the caller's expected pid. Reuses the same index compare-and-delete
rule as `remove` (`table.rs:180-185`) by extraction, not by a second copy.

**R2 — `reopen` stops trusting `cached_status`** (`reopen.rs:109-119`). After
`validate_and_compute_reopened` has established the snapshot is `Failed` or `Cancelled`, the handle
returned by `Registry::get` is disposed of by identity: `Removed` and `Absent` proceed to
`Recorder::record_workflow_reopened`; `Replaced` returns the concurrent-reopen `InvalidState`
without disturbing the new occupant.

**R3 — the runtime-armed gate** in a new
`crates/aion/src/lifecycle/terminal_reconcile_gate.rs`, called unconditionally from
`handle_process_exit_attempt` immediately after `handle.completion().notify(terminal)`
(`completion.rs:519`) and before `upsert_workflow_visibility` (`completion.rs:530`). Keyed by
`(WorkflowId, RunId)` so a sibling test's run is never held. Inert in production: the map is empty,
the call is one `DashMap` miss. Arming is `#[cfg(test)]` and returns an RAII guard that releases on
`Drop`, so a panicking assertion cannot wedge the process-exit callback thread.

**R4 — the red-first pins**, in `crates/aion/src/lifecycle/reopen_tests.rs`:
`reopen_after_result_before_terminal_registry_reconcile_succeeds` (the issue, verbatim) and
`two_reopens_from_one_terminal_head_admit_exactly_one_writer` (the true-concurrency twin).

**R5 — realign the two e2e tests whose premise the fix deliberately changes.** This is not optional
cleanup; without it the fix cannot land green. See §4.

---

## 4. The consequence that will bite: `reopen_e2e` encodes the old rule

`crates/aion/tests/reopen_e2e.rs` is green at HEAD on this box (9 tests, 0.54s — it is Gleam-free,
building its package from the checked-in Erlang fixture
`crates/aion/tests/fixtures/aion_fixture_workflow.beam`). Two of its tests sit directly on the code
being rewritten, and its fixture `lingering_terminal_handle` (`reopen_e2e.rs:356-381`) registers
handles with **`pid: 0`**.

- `reopen_clears_the_lingering_handle_of_a_run_that_failed_while_resident` (384-420) — inserts a
  leftover with `cached_status: Failed`, pid 0, over `Failed` history, and requires reopen to
  succeed. Under R2 this becomes `remove_if_pid(id, run, 0) → Removed → proceed`. **Still passes,
  unchanged.**
- `reopen_still_rejects_a_genuinely_running_concurrent_reopen` (423-468) — inserts a handle with
  `cached_status: Running`, pid 0, over history that is still `Failed`, and asserts `InvalidState`
  with zero `WorkflowReopened` appended. **This test IS the bug.** Its premise — a non-terminal
  cached status over reopenable history proves a concurrent reopen — is exactly the inference R2
  removes. Under the fix it becomes `Removed → proceed → append`, and both of its assertions fail.

  It must be **rewritten, not deleted**, onto the two rejections that are genuinely real under the
  new rule: (a) the winner has already appended `WorkflowReopened`, so history projects `Running`
  and `validate_and_compute_reopened` refuses — which is what `double_reopen_of_a_running_run_is_
  rejected` (225-257) already proves end-to-end and which survives the fix untouched; and (b) the
  observed handle was replaced by a **different pid** between `get` and the removal, so
  `remove_if_pid` returns `Replaced` and reopen refuses without disturbing the new occupant. (b) is
  the new coverage and is where the retained "(concurrent reopen)" wording belongs.

- `reopen_rejects_non_reopenable_states` (174-222) asserts a `Completed` rejection names `Completed`
  and does **not** say "already Running". History-first validation still runs first, so this is
  unaffected — but the new `Replaced` message must keep saying "concurrent reopen" without saying
  "already Running" for a non-Running case, or this test's sibling assertion becomes fragile.

---

## 5. Gate

`.fleet-51-gate.sh` at the repo root (committed). POSIX sh, binds
`PATH="$HOME/.cargo/bin:/usr/local/bin:/usr/bin:/bin"` explicitly — tonight's earlier gate red was
an unbound PATH with `cargo` off it — probes `command -v cargo` first, and exits nonzero naming the
failing command.

Steps, with baselines measured at HEAD on this box:

| Step | Command | Baseline at HEAD |
|---|---|---|
| 1 | `cargo test -p aion-rs --lib` | **706 passed, 0 failed** — the reopen unit tests live in-lib; this is the step that adjudicates the fix |
| 2 | `cargo test -p aion-rs --test reopen_e2e` | **9 passed, 0 failed** — Gleam-free reopen regression battery |
| 3 | `cargo clippy -p aion-rs --all-targets -- -D warnings` | **clean, exit 0** |

### Declared exclusion, by name

The Gleam-dependent e2e targets are excluded because the box toolchain (gleam 1.18.1 / OTP 27) emits
a `gleam_stdlib` beam that beamr's validator refuses:

> `Load { reason: "runtime rejected deployed module gleam@dynamic@decode$691ac8d3… after 31 staged
> registrations: runtime error: BEAM module validation failed: instruction 392: Y register index 5
> is outside frame size 1" }`

That is issue **#64**, owned upstream in beamr, unrelated to #51. They are covered by the Mac
landing battery. The full excluded set is named in the script's header comment.

**`activity_retry_e2e` is one of them, and is excluded — a deliberate departure from the brief's
literal instruction, grounded here.** It is the target the investigation cites, so its absence must
not read as an oversight. Measured at HEAD on this box, unmodified tree: **8 passed, 3 failed** —
`exhausted_retries_fail_the_run_and_reopen_continues_the_trail`,
`in_process_dispatch_miss_is_typed_recorded_and_fails_the_run`, and
`retryable_flakes_cost_retries_not_the_run`, all three dying on the #64 validator error above.
Note that `gleam_test_support::skip_if_unavailable()` does **not** rescue it: that helper only
probes `gleam --version` (`tests/test_support/gleam.rs:171-188`), gleam *is* installed here, so the
skip never fires and the run proceeds into beamr's refusal. Including it would pin the gate
permanently red for a cause this change can neither fix nor regress, destroying the gate's ability
to signal anything about #51.

`reopen_e2e` takes its place as step 2 and is the better substitute on the merits: it is the reopen
regression battery, it owns both tests §4 identifies as premise-changed, and it actually executes on
this box.

---

## 6. Repository laws binding this change

- No `unwrap`/`expect`/`panic` in library code — including the new gate module. The gate uses
  `dashmap` (already a dependency, `registry/handle.rs:152`) rather than `std::sync::Mutex`
  precisely so there is no poison case to map to a typed error on a production path.
- Errors are `thiserror` domain types; the refusal stays `EngineError::InvalidState`
  (`error.rs:339`), the append loser's surfaces as `EngineError::Durability` (`error.rs:119`).
- No `#[allow]` / `#[expect]` / `#[deny]` / `#[ignore]` / `_var` / `#[cfg(any())]` bypasses. Any
  runtime gating in a test is a runtime check that logs and returns `Ok(())`.
- No arbitrary limits, no assumed defaults — the gate holds until released; it has no timeout.
- No backwards-compatibility shims: the old `cached_status().is_terminal()` branch is **replaced**,
  not kept alongside.
- Files stay under 500 code lines. Measured non-test code lines at HEAD:
  `registry/table.rs` 187, `lifecycle/reopen.rs` 312, `lifecycle/completion.rs` 269. All three have
  ample headroom for their edits; no split is required. The gate module is new and small.
- `mod.rs` files carry only `pub mod` and re-exports — `lifecycle/mod.rs` gains one `mod` line.
- Clippy `-D warnings` and `cargo fmt --check` clean before commit.
