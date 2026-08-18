# Leg brief — reopen: identity-checked admission instead of a lagging cached status

**Repository:** aion (this tree). **Crate:** `aion-rs` at `crates/aion/`.
**Issue:** ablative-io/aion #51. **Verdict:** Reproduced, high confidence.
**Baseline HEAD:** `bb3b770538d5a117a2ff63119fcdfc70d94bc368` — identical to the tree the issue was
measured in. Every line reference below was re-read by hand in this tree and is accurate as written.

This brief is self-contained. You do not need to read the issue, the investigation, or `.fleet/plan.md`.

> **Order of work is mandatory: the two red tests in R4 are written and observed FAILING before any
> line of R1/R2 is written.** R3 (the gate) is infrastructure the red tests need, so it lands first,
> but the tests must go red on the *unfixed* `reopen.rs` before you touch it. A pin that was never
> observed red is not a pin.

---

## 1. What is broken

A workflow run that fails while resident terminates through
`crate::lifecycle::completion::handle_process_exit_attempt`
(`crates/aion/src/lifecycle/completion.rs:321`). It does these things **in this order**:

1. `crates/aion/src/lifecycle/completion.rs:418-424` — appends `WorkflowFailed` through the handle's
   recorder. The comment at 425-429 calls this "DURABLE POINT 2": the terminal is now in the store
   and everything below is bookkeeping.
2. `crates/aion/src/lifecycle/completion.rs:519` — `handle.completion().notify(terminal);`
3. `crates/aion/src/lifecycle/completion.rs:530-536` — `upsert_workflow_visibility(...)`
4. `crates/aion/src/lifecycle/completion.rs:537` — `reconcile_terminal_registry(...)`, defined at
   `completion.rs:541-552`:
   ```rust
   async fn reconcile_terminal_registry(
       context: &ProcessExitContext, id: &WorkflowId, run: &RunId,
   ) -> Result<(), EngineError> {
       let history = context.store.read_history(id).await?;
       context.registry.reconcile(id, run, &history)?;
       context.registry.replace_residency(id, run, Residency::Suspended)?;
       Ok(())
   }
   ```
   Note it **reconciles and suspends; it never removes**. A failed resident run's handle stays in the
   registry by design.

`Engine::result` (`crates/aion/src/engine/api_workflow_ops.rs:500-538`) returns as soon as step 2 is
observed: it subscribes at line 520 and returns on the first `Some` at 522-523.
`CompletionNotifier::notify` (`crates/aion/src/registry/handle.rs:72-74`) is a
`watch::Sender::send_replace`, so the waiter wakes immediately.

**Therefore there is a real interval between step 2 and step 4** in which durable history projects
`Failed` while the still-registered handle caches `Running`. (Nothing rewrites `cached_status`
before step 4; the only production `Registry::reconcile` callers are `lifecycle/transition.rs:193`,
`lifecycle/completion.rs:547`, `engine/startup.rs:321`, `engine/startup.rs:396` and
`engine/api_workflow_ops.rs:563`, none on this path.)

`crate::lifecycle::reopen::reopen` (`crates/aion/src/lifecycle/reopen.rs:83-151`) validates history
**first** and correctly concludes reopenable-terminal at `reopen.rs:97-98`. Then, at
`reopen.rs:109-119`, it independently treats the handle's cached status as proof of a concurrent
reopen — **this block is the defect**:

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

In the interval above `cached_status()` is `Running`, so one prompt, legitimate, **single** reopen
takes the `else` arm and is refused as concurrent. That is issue #51.

### Why the obvious repair is wrong

Do **not** "just call `Registry::reconcile` then `Registry::remove` by key". Between this caller's
`read_history` (`reopen.rs:88`) and any such mutation, a genuinely concurrent reopen can append
`WorkflowReopened` and register a **new** handle. An unconditional remove-by-key would evict that
winner. The removal must be checked against the **exact handle instance this caller observed**.

### What actually serializes two reopen writers

`Recorder::record_workflow_reopened` (`crates/aion/src/durability/recorder.rs:501-515`) is the sole
append path for `Event::WorkflowReopened`. It routes through `Recorder::append_with`
(`recorder.rs:1023-1030`) into `Recorder::append_one` (`recorder.rs:1032-1043`):

```rust
    async fn append_one(&mut self, event: Event) -> Result<(), DurabilityError> {
        let expected_seq = self.sequence.current();
        self.store
            .append(self.write_token, &self.workflow_id, std::slice::from_ref(&event), expected_seq)
            .await?;
        self.sequence.mark_append_success(1)
    }
```

> ℹ️ The investigation cited `recorder.rs:207` for this. That line does carry the same
> `expected_seq` discipline, but it lives in `record_workflow_started_with_attributes`, a two-event
> batch path **reopen never takes**. `append_one` above is the function reopen actually goes
> through. Same rule, different function — use `append_one`.

Two reopens whose `Recorder::resume_at(id, store, history_head)` (`recorder.rs:67-76`) was built at
the same head compute the same `expected_seq`. The store admits exactly one and refuses the other
with `StoreError::SequenceConflict` (`crates/aion-store/src/memory.rs:414`), which surfaces as
`EngineError::Durability(DurabilityError::Store(..))` (`crates/aion/src/error.rs:119`).

**So the registry check is an early, courteous rejection — the recorder is the real serializer.**
That is precisely what makes an identity-checked removal safe: a caller that passes admission on a
stale snapshot still cannot double-append.

---

## 2. Requirements

### R1 — `Registry::remove_if_pid`: an identity-checked removal trichotomy

**File:** `crates/aion/src/registry/table.rs` (187 non-test code lines at HEAD — ample headroom
under the 500-line law; do not split it).

Add a public result type and method beside the existing `Registry::remove`
(`table.rs:169-187`). The whole decision happens under the **single `slots` lock**, so there is no
window between observing the occupant and acting on it.

```rust
/// The outcome of an identity-checked handle removal.
///
/// Reopen admission needs three answers, not two. A caller that read history,
/// found a reopenable terminal, and then observed a registered handle must know
/// whether the entry it is about to clear is still the one it looked at — a
/// concurrent reopen can append `WorkflowReopened` and register a fresh handle
/// in that window, and clearing by key would evict the winner.
#[derive(Debug)]
pub enum HandleRemoval {
    /// The observed handle was still registered and has been removed.
    Removed(WorkflowHandle),
    /// No handle is registered for the pair. Another caller in the same
    /// pre-append window already cleared it; nothing has been disturbed.
    Absent,
    /// A DIFFERENT handle occupies the slot. It is left untouched, and returned
    /// so the caller can name it in its refusal.
    Replaced(WorkflowHandle),
}
```

```rust
    /// Removes the handle for `(id, run)` only if it is still the instance the
    /// caller observed, identified by `expected_pid`.
    ///
    /// The compare and the delete happen under one `slots` lock, which is what
    /// distinguishes this from a `get` followed by a `remove`: between those two
    /// calls a concurrent reopen can register its own handle, and removing by
    /// key would evict it. See [`HandleRemoval`].
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::RegistryPoisoned`] if the registry lock was poisoned.
    pub fn remove_if_pid(
        &self,
        id: &WorkflowId,
        run: &RunId,
        expected_pid: u64,
    ) -> Result<HandleRemoval, EngineError> {
        // Lock ordering: slots first, then index. Never the reverse.
        let mut slots = self.slots()?;
        let key = (id.clone(), run.clone());
        let Some(occupant) = slots.handles.get(&key) else {
            return Ok(HandleRemoval::Absent);
        };
        if occupant.pid() != expected_pid {
            return Ok(HandleRemoval::Replaced(occupant.clone()));
        }
        let removed = slots.handles.remove(&key);
        drop(slots);
        self.forget_live_index_entry(id, run)?;
        match removed {
            Some(handle) => Ok(HandleRemoval::Removed(handle)),
            // Unreachable: the occupant was observed under the lock just held.
            // Reported rather than asserted — no `unwrap`/`expect` in library code.
            None => Ok(HandleRemoval::Absent),
        }
    }
```

**Do not duplicate the index rule.** `Registry::remove` already carries a compare-and-delete on the
secondary index (`table.rs:180-185`) whose comment explains it: during continue-as-new a newer run
overwrote the entry and must survive a stale run's removal. Extract that into one private helper and
call it from **both** `remove` and `remove_if_pid`:

```rust
    /// Drops the live-pid index entry for `id`, but only while it still points at
    /// `run`. During continue-as-new a newer run has already overwritten the
    /// entry, and that newer run must survive this removal.
    fn forget_live_index_entry(&self, id: &WorkflowId, run: &RunId) -> Result<(), EngineError> {
        let mut index = self.index()?;
        if let std::collections::hash_map::Entry::Occupied(entry) = index.entry(id.clone()) {
            if entry.get().0 == *run {
                entry.remove();
            }
        }
        Ok(())
    }
```

Rewrite `Registry::remove`'s body to use it. Preserve the lock ordering documented at
`table.rs:72-78` (`slots` before `index`, never the reverse) — note the `drop(slots)` above releases
`slots` before `index` is taken, which is compatible with that order.

Export `HandleRemoval` from `crates/aion/src/registry/mod.rs` alongside the existing
`pub use table::Registry;` (`registry/mod.rs`, the `pub use table::…` line).

**Unit tests** (in `table.rs`'s existing `#[cfg(test)] mod tests`, which already has a `handle(pid,
version_byte, status)` helper at `table.rs:456-473`): each arm of the trichotomy — `Removed` on a
matching pid; `Absent` on an empty slot; `Replaced` returning the *other* handle and leaving it
registered; and that `Removed` clears the live-pid index while a `Replaced` refusal does not.

---

### R2 — `reopen` stops trusting `cached_status`

**File:** `crates/aion/src/lifecycle/reopen.rs`. **Replace** lines 100-119 (the comment block and the
`if let Some(existing)` block quoted in §1). Do not keep the old branch alongside — the repository
forbids compat shims and zombie code.

```rust
    // History has already said reopenable-terminal, and history is the authority.
    // The handle is NOT: a run that fails while resident keeps its registered
    // handle, and `reconcile_terminal_registry` reconciles that handle's cached
    // status only AFTER the completion doorbell has already released
    // `Engine::result`. In that window a prompt, single, legitimate reopen sees a
    // handle still caching `Running` over a durably `Failed` history — and a
    // check keyed on the cached status refuses it as a concurrent reopen (#51).
    //
    // What matters is IDENTITY, not status: is the entry still the one this
    // caller observed? Clearing by key instead would evict a winner that appended
    // `WorkflowReopened` and registered a fresh handle between the history read
    // above and this line.
    //
    // Two callers that both pass here are still serialized by the recorder: both
    // built at the same head, so both compute the same `expected_seq`, and
    // `Recorder::append_one` lets exactly one `WorkflowReopened` land — the loser
    // gets `SequenceConflict`. And a caller whose own history read already
    // contained `WorkflowReopened` never reaches this line at all:
    // `validate_and_compute_reopened` rejected it as Running.
    if let Some(existing) = context.registry.get(id, run)? {
        match context.registry.remove_if_pid(id, run, existing.pid())? {
            // The leftover lease this caller observed, cleared by identity — or
            // already cleared by an overlapping caller in the same pre-append
            // window. Either way nothing live was disturbed: proceed.
            HandleRemoval::Removed(_) | HandleRemoval::Absent => {}
            // A DIFFERENT lease now holds the slot: another reopen won and is
            // driving the run. Refuse without touching it.
            HandleRemoval::Replaced(winner) => {
                return Err(EngineError::InvalidState {
                    reason: format!(
                        "workflow {id} run {run} was already reopened by another caller \
                         (concurrent reopen): run is now driven by pid {}",
                        winner.pid()
                    ),
                });
            }
        }
    }
```

Add `HandleRemoval` to the existing `use crate::registry::{Registry, WorkflowHandle};` import at
`reopen.rs:47`.

Update the `# Errors` doc on `reopen` (`reopen.rs:76-82`) so `InvalidState` is described as covering
*a different lease already driving the run*, rather than "already Running".

> ⚠️ **Keep the phrase "concurrent reopen" and keep "already Running" OUT of it.**
> `crates/aion/tests/reopen_e2e.rs:212-215` asserts that a `Completed` run's rejection does not
> contain "already Running". The message above satisfies both constraints.

---

### R3 — the runtime-armed terminal-reconcile gate

**New file:** `crates/aion/src/lifecycle/terminal_reconcile_gate.rs`.

**Constraints, all binding:**
- The **call site** in `handle_process_exit_attempt` is unconditional production code — no
  `#[cfg(test)]` around it, no `#[cfg]` fragmentation of the function.
- It is **inert in production**: the gate map is empty, so the call is one `DashMap` miss.
- The **arming API is `#[cfg(test)]`**, so no production caller can arm it. This follows the crate's
  existing idiom of a test-only surface on a production type — see
  `crates/aion/src/runtime/handle.rs:912` (`#[cfg(test)] #[path = "handle/test_support.rs"] mod
  test_support;`) and `crates/aion/src/loader/catalog.rs:729`.
- Keyed by `(WorkflowId, RunId)`, so a sibling test's run running in parallel in the same process is
  never held. (This is the lesson `crates/aion/src/log_capture.rs` records at length about
  process-global test state.)
- **No `std::sync::Mutex`.** Use `dashmap` — already a direct dependency
  (`crates/aion/Cargo.toml:35`, used at `crates/aion/src/registry/handle.rs:152`). This removes the
  lock-poison case entirely, so the production call site needs no error return and no
  `unwrap`/`expect`.
- No timeout on the hold — repository law forbids invented limits. The guard's `Drop` is what
  guarantees release.

```rust
//! A runtime-armed hold point between the completion doorbell and terminal
//! registry reconciliation.
//!
//! `handle_process_exit_attempt` appends the terminal, rings the completion
//! doorbell — which releases `Engine::result` — and only then reconciles the
//! registry. Issue #51 lives in that interval: history is terminal while the
//! still-registered handle caches `Running`. The interval is real but narrow,
//! and a test that tried to hit it by racing would be a flake generator, so this
//! module lets a test STOP the completion path inside it and hold it open.
//!
//! 🔴 INERT IN PRODUCTION, AND NOT BY CONVENTION. The gate map is empty unless a
//! test arms it, so [`hold`] is one `DashMap` miss on every production exit. The
//! arming API is `#[cfg(test)]`, so no production caller can arm it even by
//! mistake — the call site stays unconditional and the production function is
//! not fragmented by `#[cfg]`.
//!
//! 🔴 KEYED BY `(WorkflowId, RunId)`, NOT GLOBAL. Unit tests run in parallel in
//! one process. A global hold would park a sibling test's completion and, worse,
//! let this test's release satisfy a run it never arranged.
//!
//! 🔴 THE HOLD IS RELEASED BY `Drop`, NOT BY A CALL. The held path is
//! `block_on`-ed on the single process-exit callback thread in production, so a
//! hold that outlived its test would wedge every later exit in the process. An
//! assertion panicking mid-test must therefore still release it, which only an
//! RAII guard guarantees.

use std::sync::Arc;
use std::sync::OnceLock;

use aion_core::{RunId, WorkflowId};
use dashmap::DashMap;
use tokio::sync::watch;

/// One armed hold. Both signals are `watch` channels — the same primitive
/// [`crate::registry::CompletionNotifier`] uses — because a `watch` receiver
/// checks the CURRENT value before parking, so neither side can miss an edge
/// that fired before it started waiting.
struct Gate {
    reached: watch::Sender<bool>,
    release: watch::Sender<bool>,
}

type GateKey = (WorkflowId, RunId);

fn gates() -> &'static DashMap<GateKey, Arc<Gate>> {
    static GATES: OnceLock<DashMap<GateKey, Arc<Gate>>> = OnceLock::new();
    GATES.get_or_init(DashMap::new)
}

/// Hold the completion path here if a test has armed this run, otherwise return
/// immediately.
///
/// Production never arms a gate, so this is a single map miss.
pub(crate) async fn hold(id: &WorkflowId, run: &RunId) {
    let Some(gate) = gates().get(&(id.clone(), run.clone())).map(|entry| Arc::clone(&entry)) else {
        return;
    };
    gate.reached.send_replace(true);
    let mut release = gate.release.subscribe();
    // `wait_for` inspects the current value first, so a release that already
    // happened is observed rather than waited for. An `Err` means every sender
    // is gone — the guard was dropped — which is also a release.
    drop(release.wait_for(|released| *released).await);
}
```

The `#[cfg(test)]` arming half — put it in the same file behind one `#[cfg(test)]` block, or in a
`#[cfg(test)] #[path = "…"] mod test_support;` sibling if you prefer the `handle.rs:912` shape:

```rust
/// An armed gate. Releases on `Drop`, so a panicking assertion cannot wedge the
/// process-exit path for the rest of the process.
#[cfg(test)]
pub(crate) struct ArmedGate {
    key: GateKey,
    gate: Arc<Gate>,
}

#[cfg(test)]
impl ArmedGate {
    /// Resolves once the completion path has reached the hold — i.e. the terminal
    /// is durable and the doorbell has rung, but reconciliation has not run.
    pub(crate) async fn wait_until_reached(&self) {
        let mut reached = self.gate.reached.subscribe();
        drop(reached.wait_for(|reached| *reached).await);
    }

    /// Whether the completion path has arrived at the hold yet.
    pub(crate) fn was_reached(&self) -> bool {
        *self.gate.reached.borrow()
    }
}

#[cfg(test)]
impl Drop for ArmedGate {
    fn drop(&mut self) {
        self.gate.release.send_replace(true);
        drop(gates().remove(&self.key));
    }
}

/// Arm a hold for one run. The gate is disarmed and released when the returned
/// guard is dropped.
#[cfg(test)]
pub(crate) fn arm(id: &WorkflowId, run: &RunId) -> ArmedGate {
    let key = (id.clone(), run.clone());
    let gate = Arc::new(Gate {
        reached: watch::channel(false).0,
        release: watch::channel(false).0,
    });
    gates().insert(key.clone(), Arc::clone(&gate));
    ArmedGate { key, gate }
}
```

> Note `watch::channel(false).0` keeps only the sender; `Sender::subscribe()` is what both sides use,
> and the sender lives as long as the `Arc<Gate>`, so `wait_for` in `hold` cannot see a closed
> channel while the guard is alive.

**Declare the module** in `crates/aion/src/lifecycle/mod.rs` (which currently holds only `pub mod` /
`mod` declarations plus re-exports — keep it that way). Place it alphabetically among the existing
declarations with a doc comment matching the file's style, e.g.:

```rust
/// A runtime-armed hold between the completion doorbell and terminal registry
/// reconciliation, used to pin issue #51 deterministically.
mod terminal_reconcile_gate;
```

`mod` (not `pub mod`): nothing outside the crate needs it.

**The call site.** In `crates/aion/src/lifecycle/completion.rs`, immediately after line 519
(`handle.completion().notify(terminal);`) and **before** the `upsert_workflow_visibility` call that
begins at line 530 — i.e. between line 519 and the `// 🔴 Below this line the recorder lock has
dropped…` comment at 521:

```rust
    handle.completion().notify(terminal);

    // A test-armed hold, and nothing at all in production: the gate map is empty
    // unless a test armed this exact run, so this is one map miss. It exists
    // because the interval that follows — doorbell rung, `Engine::result`
    // already released, registry not yet reconciled — is where #51 lives, and
    // holding it open is the only way to pin that deterministically rather than
    // by racing.
    crate::lifecycle::terminal_reconcile_gate::hold(handle.workflow_id(), handle.run_id()).await;
```

Do **not** place it in the already-terminal (`Err(existing)`) branch at `completion.rs:449-516`; the
issue is about the branch that records the terminal itself.

---

### R4 — the red-first pins

**File:** `crates/aion/src/lifecycle/reopen_tests.rs` (360 lines at HEAD; it is included from
`reopen.rs:471-473` via `#[cfg(test)] #[path = "reopen_tests.rs"] mod tests;`).

**Everything you need is reachable and Gleam-free.** Read this before you start:

- `handle_process_exit_attempt` is **`pub(super)`** (`completion.rs:321`), i.e. visible throughout
  `crate::lifecycle`. `reopen_tests.rs` is `mod tests` inside `crate::lifecycle::reopen`, so it can
  call `crate::lifecycle::completion::handle_process_exit_attempt` directly. So are `TerminalIntent`
  (`completion_retry.rs:39`), `TerminalIntentOutcome` (`completion_retry.rs:121`) and
  `TerminalProgress` (`completion_retry.rs:183`). **No visibility changes are needed.**
- **Copy the fixture shape from `completion.rs:758-807`** (`active_workflow_over`). It builds a
  complete `ProcessExitContext` + `Registry` + a registered `WorkflowHandle` with
  `cached_status: WorkflowStatus::Running` over an `InMemoryStore`, seeding history with
  `Recorder::record_workflow_started`, all with **no BEAM workflow process and no Gleam**.
- A **real** reopen — `respawn_and_register` → `recover_active_workflow` → a genuinely spawned BEAM
  process — is drivable in-lib with the crate's existing pair, exactly as
  `crates/aion/src/engine/startup_tests.rs:705-712` and
  `crates/aion/src/lifecycle/continue_as_new.rs:318-320` already do:
  ```rust
  runtime.register_waiting_test_module("checkout_deployed", "run"); // handle/test_support.rs:105
  catalog.note_loaded_workflow_for_test(                            // catalog_test_support.rs:21
      "checkout", "checkout_deployed", "run", package_hash,
  );
  ```
  The registered module's exported function is a single `Wait` instruction, so the spawned process
  parks. This is why the test can **require reopen to succeed** rather than merely dodge one error.
- ⚠️ **The catalog hash must equal the recorded `package_version`.** The fixture at
  `completion.rs:775` records `PackageVersion::new("a".repeat(64))`. `ContentHash::from_bytes([0xaa;
  32])` renders as 64 `a` characters, which matches. Confirm the rendering when you write it; if the
  pin and the catalogued version disagree, `recover_active_workflow` treats the run as degraded and
  reopen fails for the wrong reason.
- `rearm_reopened_timers` (`reopen.rs:250-299`) returns early when there are no timers
  (`reopen.rs:256-258`), and these fixtures record none — so no `wait_for_pending_await` is involved.
- Build the `ReopenWorkflowContext` (`reopen.rs:52-67`) from the **same** `Arc<RuntimeHandle>`,
  `Arc<Registry>` and store as the `ProcessExitContext`, or the two halves will not see each other.
- Use `#[tokio::test(flavor = "multi_thread")]`: the completion path is parked at the gate on one
  task while the test drives the reopen on another. Call `runtime.shutdown()?` at the end, as the
  neighbouring tests do.

#### Test 1 — `reopen_after_result_before_terminal_registry_reconcile_succeeds`

This is the issue, verbatim. Sequence:

1. Build the fixture: store, registry, `ProcessExitContext`, a registered handle with
   `cached_status: Running`, `WorkflowStarted` in history. Register the waiting test module and note
   the catalogued workflow.
2. `let gate = terminal_reconcile_gate::arm(&workflow_id, &run_id);` — **before** the terminal is
   driven.
3. Spawn a task that drives the run terminal:
   `handle_process_exit_attempt(&context, &handle, &intent, &mut progress)` with
   `TerminalIntentOutcome::Failed(..)`. (Or subscribe to `handle.completion()` first and drive it —
   either way the terminal must be recorded by the production path, not seeded by hand, or the test
   is not measuring the ordering that causes the bug.)
4. `gate.wait_until_reached().await` — the hold has been entered, so: the terminal is durable, the
   doorbell has rung (`Engine::result` would already have returned), and reconciliation has **not**
   run.
5. **Assert both halves of the disagreement**, which is what makes the test a pin rather than a
   smoke test:
   - `aion_core::status_from_events(&store.read_history(&workflow_id).await?) == WorkflowStatus::Failed`
   - `registry.get(&workflow_id, &run_id)?.ok_or(..)?.cached_status() == WorkflowStatus::Running`
   If the second assertion ever stops holding, the test has stopped reproducing the window and must
   be fixed, not deleted.
6. Invoke reopen **exactly once**: `reopen::reopen(context, &workflow_id, &run_id).await` and
   **require `Ok`**. Assert the returned handle's `cached_status()` is `Running`, that history now
   projects `Running`, and that history contains exactly one `Event::WorkflowReopened`.
7. `drop(gate);` to release the held completion path, then join the spawned task and assert it
   returned `Ok`.

**Observe this test FAIL before writing R1/R2.** Expected red: step 6 returns
`Err(EngineError::InvalidState { reason })` with `reason` containing `"concurrent reopen"`. If it
fails any other way — especially at step 5 — your fixture is not reproducing the window, and fixing
`reopen.rs` will not make it green for the right reason.

#### Test 2 — `two_reopens_from_one_terminal_head_admit_exactly_one_writer`

The true-concurrency twin. Same fixture, but the run is driven to `WorkflowFailed` and fully
reconciled first (no gate needed — release it, or never arm one). Then release **two** reopen callers
against the same terminal history head, each with its own `ReopenWorkflowContext` over the shared
store/registry/runtime, spawned as two tasks and joined.

Assert, in this order:

1. **Exactly one** `Event::WorkflowReopened` in `store.read_history(&workflow_id)`. This is the
   load-bearing assertion — invariant #3, single writer per workflow.
2. **Exactly one** caller returned `Ok`.
3. The loser returned `Err`, and its error is one of the two legitimate refusals:
   - `EngineError::InvalidState { reason }` with `reason` containing `"concurrent reopen"` — it
     observed the winner's freshly registered handle, so `remove_if_pid` returned `Replaced`; or
   - a `SequenceConflict`-bearing durability error — both callers passed admission and the
     recorder's `expected_seq` refused the second append
     (`EngineError::Durability(DurabilityError::Store(StoreError::SequenceConflict { .. }))`).

   Assert the **disjunction**, and fail with the actual error rendered in the message if it is
   neither. Do not assert one specific arm: which one occurs is a genuine scheduling outcome, and
   pinning one arm would make the test a flake.
4. **The winner's handle is still registered**: `registry.get(&workflow_id, &run_id)?` is `Some`, and
   its `pid()` equals the pid of the handle the winning `reopen` returned. This is the assertion that
   catches a regression where the loser's cleanup evicts the winner — the exact hazard `remove_if_pid`
   exists to prevent.

---

### R5 — realign the two e2e tests whose premise this change deliberately inverts

**File:** `crates/aion/tests/reopen_e2e.rs` (517 lines, **green at HEAD on this box**: 9 passed,
0.54s — it is Gleam-free, building its package from the checked-in Erlang fixture
`crates/aion/tests/fixtures/aion_fixture_workflow.beam`).

This is **not optional cleanup**; without it the fix cannot land green. Its helper
`lingering_terminal_handle` (`reopen_e2e.rs:356-381`) registers handles with **`pid: 0`**.

- ✅ `reopen_clears_the_lingering_handle_of_a_run_that_failed_while_resident` (384-420) — inserts a
  leftover with `cached_status: Failed`, pid 0, over `Failed` history, requires reopen to succeed.
  Under R2: `remove_if_pid(id, run, 0) → Removed → proceed`. **Passes unchanged. Do not touch it.**

- ❌ `reopen_still_rejects_a_genuinely_running_concurrent_reopen` (423-468) — inserts a handle with
  `cached_status: Running`, pid 0, over history that is still `Failed`, and asserts `InvalidState`
  plus zero `WorkflowReopened`. **This test encodes the bug.** Its premise — that a non-terminal
  cached status over reopenable history proves a concurrent reopen — is exactly the inference R2
  removes. Under the fix it becomes `Removed → proceed → append`, and **both** assertions fail.

  **Rewrite it, do not delete it.** Replace it with a test of the rejection that is genuinely real
  under the new rule: the observed handle is **replaced by a different pid** between `get` and the
  removal, so `remove_if_pid` returns `Replaced`. Concretely: register a handle over `Failed`
  history whose pid differs from the one reopen will observe — the cleanest construction is to
  register handle A, then have the test's own code swap in handle B with a **different pid** before
  reopen's `remove_if_pid` runs. If that interleaving cannot be arranged from an integration test
  without a hook, keep the `Replaced` coverage as a `Registry`-level unit test in R1 (which already
  covers it directly) and rewrite this e2e test onto the other real rejection: the winner has
  already appended `WorkflowReopened`, so history projects `Running` and
  `validate_and_compute_reopened` refuses. Rename it to say what it now proves, e.g.
  `reopen_rejects_a_second_caller_once_the_winner_has_appended`. **State in the test's doc comment
  why the old premise was wrong**, citing #51 — a rewritten test with no explanation reads as a
  weakened test.

- ✅ `double_reopen_of_a_running_run_is_rejected` (225-257) — the second reopen sees history that
  already contains `WorkflowReopened`, so `validate_and_compute_reopened` refuses it before the
  registry is consulted at all. **Unaffected. Do not touch it.**

- ⚠️ `reopen_rejects_non_reopenable_states` (174-222) asserts at 212-215 that a `Completed`
  rejection does **not** contain `"already Running"`. The R2 message satisfies this. Re-run it and
  confirm.

---

## 3. Repository laws that bind this change

Non-negotiable; a violation is a correctness bug, not a style nit.

- **No `unwrap` / `expect` / `panic` / `todo` in library code**, including the new gate module. Using
  `dashmap` instead of `std::sync::Mutex` is deliberate: there is no poison case to map, so the
  production call site needs no error return.
- **Errors are `thiserror` domain types.** The refusal stays `EngineError::InvalidState`
  (`crates/aion/src/error.rs:339`); the append loser surfaces as `EngineError::Durability`
  (`error.rs:119`). Do not invent a new error variant for this.
- **No bypasses.** `#[allow]`, `#[expect]`, `#[deny]`, `#[ignore]` on tests, `_var` renames, and
  `#[cfg(any())]` dead-code hiding are all bypasses, not fixes. If clippy fires, fix the code. A
  test that needs a runtime gate does the check at runtime, emits a `tracing::info!` skip line, and
  returns `Ok(())`.
- **No arbitrary limits or assumed defaults.** The gate has no timeout; its `Drop` is the guarantee.
- **No backwards compatibility.** Replace the old `cached_status().is_terminal()` branch; do not keep
  it alongside, do not `#[deprecated]` anything.
- **Files under 500 code lines** (excluding tests, comments, whitespace). Measured at HEAD:
  `registry/table.rs` 187, `lifecycle/reopen.rs` 312, `lifecycle/completion.rs` 269. All have ample
  headroom; **no split is required**. If your edits somehow push one past the ceiling, split it into
  a named module — never a `mod.rs`, which carries only `pub mod` declarations and re-exports.
- **Invariant #3 (single writer per workflow) is what this change is protecting.** Never call
  `EventStore::append` directly; every append goes through the one `Recorder`. A `SequenceConflict`
  is the double-writer indicator — in Test 2 it is an *expected, legitimate* refusal for the losing
  caller, and nowhere else.
- **Invariant #4 (status is a projection).** The whole fix is an instance of this: history is the
  authority, `cached_status` is a lagging cache, and residency (`Resident`/`Suspended`) is separate
  and orthogonal.

---

## 4. Gate — how you know you are done

Run from the repository root:

```sh
sh ./.fleet-51-gate.sh
```

It binds `PATH="$HOME/.cargo/bin:…"` explicitly (cargo lives under `$HOME/.cargo/bin` on this box;
an earlier gate went red purely because PATH was unbound), probes for `cargo`, and exits nonzero
naming the failing command. Steps, with baselines measured at HEAD before any change:

| Step | Command | Baseline at HEAD |
|---|---|---|
| 1 | `cargo test -p aion-rs --lib` | 706 passed, 0 failed |
| 2 | `cargo test -p aion-rs --test reopen_e2e` | 9 passed, 0 failed |
| 3 | `cargo clippy -p aion-rs --all-targets -- -D warnings` | clean, exit 0 |

Also run `cargo fmt --check` before committing.

**Declared exclusion.** The Gleam-dependent e2e targets are excluded by name (listed in the script's
header) because the box toolchain — gleam 1.18.1 / OTP 27 — emits a `gleam_stdlib` beam that beamr's
validator refuses: *"BEAM module validation failed: instruction 392: Y register index 5 is outside
frame size 1"*. That is issue **#64**, owned upstream in beamr, unrelated to #51. Those targets are
covered by the Mac landing battery.

**`activity_retry_e2e` is one of them and is excluded**, even though the investigation cites
`crates/aion/tests/activity_retry_e2e.rs:224`. Measured at HEAD on this box on an unmodified tree it
is already **red**: 8 passed, 3 failed —
`exhausted_retries_fail_the_run_and_reopen_continues_the_trail`,
`in_process_dispatch_miss_is_typed_recorded_and_fails_the_run`, and
`retryable_flakes_cost_retries_not_the_run` — all three dying on the #64 error above.
`gleam_test_support::skip_if_unavailable()` does not rescue it: that helper only probes `gleam
--version` (`crates/aion/tests/test_support/gleam.rs:171-188`), gleam *is* installed here, so the
skip never fires and the run proceeds into beamr's refusal. **If you see it fail, that is #64, not
your change** — do not attempt to fix it here.

---

## 5. Definition of done

1. Both R4 tests were **observed red** against unmodified `reopen.rs`, and the red was the expected
   `InvalidState { reason: … "concurrent reopen" … }` — not an unrelated failure.
2. R1, R2, R3, R5 implemented as specified.
3. `sh ./.fleet-51-gate.sh` exits 0, with step 1 showing **at least 706 + your new tests** passing
   and step 2 showing 9 passing.
4. `cargo fmt --check` clean.
5. No `#[allow]`/`#[expect]`/`#[ignore]`/`_var` bypasses introduced anywhere in the diff.
6. The rewritten `reopen_e2e` test states in its doc comment why its old premise was wrong.
