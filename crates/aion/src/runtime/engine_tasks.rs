//! Dedicated runtime and lifecycle registry for engine-side background tasks:
//! child-terminal watchers, post-record spawn recovery, and process-exit
//! completion retries.
//!
//! These tasks must not outlive the engine epoch: one still running after
//! shutdown could double-write a history that a successor engine instance over
//! the same store also records into — a second writer, against the single-writer
//! invariant, whose symptom is `SequenceConflict`. Tokio's `abort` alone does not
//! guarantee that — an aborted task finishes its in-flight poll (which can be a
//! recorder append) — so the epoch close must *await* every aborted task.
//! Awaiting a task parked on the host's runtime from a synchronous
//! `Engine::shutdown` would deadlock a current-thread host runtime (the blocked
//! thread is the one that drives the tasks), so the tasks run on an engine-owned
//! runtime with its own worker thread: shutdown can block on a channel while that
//! worker drives every abort to completion, regardless of the host runtime
//! flavor.
//!
//! # Why this is owned by the runtime handle rather than by a bridge
//!
//! It began as a child-workflow component, constructed by and reachable only
//! through `ChildNifBridge` — which `EngineNifState` holds as an OPTIONAL,
//! conditionally-installed bridge.
//!
//! The process-exit completion retry is a core lifecycle path: it runs for every
//! workflow on every node, including nodes that never spawn a child workflow.
//! Making the durability of a terminal event depend on whether an unrelated
//! bridge happened to be installed would be a silent failure — on a node with no
//! child bridge the retry would have nowhere to run and the guarantee would
//! quietly not apply.
//!
//! ⇒ [`RuntimeHandle`](super::RuntimeHandle) owns it, as it owns every other
//! engine-epoch-scoped object, and the child bridge borrows the same `Arc`.
//! **One epoch-closed executor per node, not two** — two would be the same
//! shutdown discipline maintained in two places, which is the shape that has
//! already drifted every time this repository has allowed it.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use aion_core::{RunId, WorkflowId};
use dashmap::DashMap;
use dashmap::mapref::entry::Entry;
use tokio::task::JoinHandle;

use crate::EngineError;

/// Identity of one completion-retry registration.
///
/// 🔴 THE `monitor_pid` COMPONENT IS WHAT MAKES THIS MAP CORRECT, AND IT WAS
/// ADDED TO FIX A SILENT PERMANENT ZOMBIE.
///
/// The key was `(WorkflowId, RunId)`. **A reopen reuses the run id**
/// (`lifecycle/reopen.rs` re-registers under the same `run_id` and installs a
/// fresh monitor), so a superseded lease's still-armed retry and the successor
/// lease's exit collided on one key. The successor's arm was then refused as
/// [`ArmOutcome::AlreadyArmed`] — "somebody already owns this terminal" — while
/// the incumbent was a retry that goes on to *stand down without writing*,
/// because `monitor_stands_down` sees its pid superseded. The successor's
/// terminal was never recorded, the run projected `Running` for the life of the
/// epoch, and nothing was logged above `debug!`.
///
/// What this map exists to bound is **one retry per WRITER**, and the writer is
/// the monitor lease, not the run. Two armed retries under different pids are
/// safe: the older stands down by the identity check `monitor_stands_down`
/// already performs, so exactly one writes. The same lease arming twice still
/// collides on the same key, so the single-writer guarantee this map was built
/// for is unchanged.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct CompletionRetryKey {
    /// The workflow whose terminal is owed.
    pub(crate) workflow_id: WorkflowId,
    /// The run whose terminal is owed. Reused across a reopen — which is
    /// precisely why it is not sufficient on its own.
    pub(crate) run_id: RunId,
    /// The pid of the monitor lease that owes the write. Distinguishes a
    /// superseded lease's retry from its successor's.
    pub(crate) monitor_pid: super::Pid,
}

/// What happened to an arm request.
///
/// # Why this is not a `bool`
///
/// 🔴 The two ways an arm can be refused mean **opposite things about whether
/// the work has an owner**, and a `bool` cannot carry that difference:
///
/// - [`Self::AlreadyArmed`] — a live task for this exact key is running. The
///   work IS owned by that task; the second arm is the redundant one.
///
///   🔴 "Owned" is a claim about the KEY, and it is only as strong as the key.
///   It said "Nothing is lost", which was false for the completion-retry map
///   while that map was keyed `(WorkflowId, RunId)`: a reopen reuses the run id,
///   so the incumbent could be a *superseded* lease's retry that stands down
///   without writing, and the refused arm's terminal was then lost for the
///   epoch. See [`CompletionRetryKey`] — the key now carries the monitor pid, so
///   an incumbent under this key really is the same writer.
/// - [`Self::EpochClosed`] — the engine-task epoch is closing or closed.
///   Nothing was spawned and nothing in this process will spawn it; only a
///   successor engine's startup sweep re-installs the work.
///
/// While this was a `bool`, every call site collapsed the two into "not armed"
/// and logged the `EpochClosed` sentence for both — so an operator watching a
/// run whose retry was already in flight was told the run "stays Running until
/// a monitor is re-installed", which is a false durable fact about the exact
/// thing the retry exists to get right. The distinction is known here and only
/// here; returning it is the only way a caller can report what actually
/// happened rather than what it assumed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArmOutcome {
    /// A new task was spawned and this registry entry owns it.
    Armed,
    /// A live task for this key is already running and owns the work; nothing
    /// new was spawned and nothing needs to be.
    AlreadyArmed,
    /// The engine-task epoch is closing or already closed: no task was spawned,
    /// and none will be until a successor engine re-installs the work.
    EpochClosed,
}

/// Engine-owned background-task executor and task-handle registry.
///
/// Arming is gated: once [`EngineTaskRuntime::shutdown`] begins, no new task
/// can be armed, every retained handle is aborted *and awaited*, and the
/// owned runtime is released — only then is the epoch considered closed.
/// Dropping the registry without an explicit shutdown is backstopped by
/// [`Drop`], which aborts everything and releases the runtime without
/// blocking (safe in any context).
pub(crate) struct EngineTaskRuntime {
    /// Owned executor; `None` once shut down.
    ///
    /// One dedicated worker thread: the tasks are pure async (store reads,
    /// recorder appends, backoff sleeps, doorbell awaits), so a single
    /// worker drives any number of them; what matters is that it is *not* a
    /// host-runtime thread, so shutdown can block on it safely.
    runtime: Mutex<Option<tokio::runtime::Runtime>>,
    /// Armed child-terminal watcher tasks keyed by `(parent pid, child id)`.
    ///
    /// beamr never reuses pids within a scheduler, so a removed key can
    /// never collide with a later process.
    watches: DashMap<(u64, WorkflowId), JoinHandle<()>>,
    /// Spawn-recovery tasks keyed by the recorded child workflow id.
    spawn_retries: DashMap<WorkflowId, JoinHandle<()>>,
    /// Process-exit completion retries keyed by the monitor lease whose
    /// terminal has not landed yet — see [`CompletionRetryKey`] for why the pid
    /// is part of the key and not an implementation detail.
    ///
    /// Every key component is read off the `WorkflowHandle` the monitor was
    /// installed with, so one monitor lease can never hold more than one retry.
    ///
    /// That bounds the map by the leases this node is actually monitoring, which
    /// is the property that matters. An earlier revision claimed more — that
    /// both components are "server-derived in full, never off anything a caller
    /// supplies" — and the `RunId` half is, but the `WorkflowId` half is not:
    /// `StartWorkflowOptions::workflow_id` is a public caller-supplied field
    /// (`lifecycle/start.rs`) threaded through admission. A caller still cannot
    /// grow this map beyond its running workflows, because an entry exists only
    /// for a run this node monitored to exit; the bound just does not come from
    /// where that sentence said it did.
    completion_retries: DashMap<CompletionRetryKey, JoinHandle<()>>,
    /// Arm gate: set at the start of shutdown, never cleared.
    shutting_down: AtomicBool,
}

impl EngineTaskRuntime {
    /// Build the executor with its dedicated worker thread.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::Runtime`] when the OS refuses the worker
    /// thread.
    pub(crate) fn new() -> Result<Self, EngineError> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .thread_name("aion-engine-tasks")
            .enable_all()
            .build()
            .map_err(|error| EngineError::Runtime {
                reason: format!("failed to start the engine-task runtime: {error}"),
            })?;
        Ok(Self {
            runtime: Mutex::new(Some(runtime)),
            watches: DashMap::new(),
            spawn_retries: DashMap::new(),
            completion_retries: DashMap::new(),
            shutting_down: AtomicBool::new(false),
        })
    }

    /// Arm a child-terminal watcher task for one `(parent pid, child id)`.
    ///
    /// Idempotent per key; refused once shutdown began. See [`ArmOutcome`] for
    /// why the two refusals are distinguished.
    pub(crate) fn arm_watch<F>(&self, parent_pid: u64, child_id: WorkflowId, task: F) -> ArmOutcome
    where
        F: Future<Output = ()> + Send + 'static,
    {
        Self::arm(
            &self.shutting_down,
            &self.runtime,
            &self.watches,
            (parent_pid, child_id),
            task,
        )
    }

    /// Arm a spawn-recovery task for one recorded child workflow id.
    ///
    /// Idempotent per child id; refused once shutdown began. See [`ArmOutcome`]
    /// for why the two refusals are distinguished.
    pub(crate) fn arm_spawn_retry<F>(&self, child_id: WorkflowId, task: F) -> ArmOutcome
    where
        F: Future<Output = ()> + Send + 'static,
    {
        Self::arm(
            &self.shutting_down,
            &self.runtime,
            &self.spawn_retries,
            child_id,
            task,
        )
    }

    /// Arm a process-exit completion retry for one monitor lease.
    ///
    /// Idempotent per lease: a second arm while one is in flight is refused
    /// ([`ArmOutcome::AlreadyArmed`]), so a lease cannot accumulate racing
    /// writers of its own terminal. Refused with [`ArmOutcome::EpochClosed`]
    /// once shutdown began — a different fact, and the caller reports it
    /// differently.
    ///
    /// 🔴 Per-LEASE, not per-run, and the difference is a silent zombie: see
    /// [`CompletionRetryKey`].
    pub(crate) fn arm_completion_retry<F>(&self, lease: CompletionRetryKey, task: F) -> ArmOutcome
    where
        F: Future<Output = ()> + Send + 'static,
    {
        Self::arm(
            &self.shutting_down,
            &self.runtime,
            &self.completion_retries,
            lease,
            task,
        )
    }

    fn arm<K, F>(
        shutting_down: &AtomicBool,
        runtime: &Mutex<Option<tokio::runtime::Runtime>>,
        registry: &DashMap<K, JoinHandle<()>>,
        key: K,
        task: F,
    ) -> ArmOutcome
    where
        K: std::hash::Hash + Eq + Clone,
        F: Future<Output = ()> + Send + 'static,
    {
        if shutting_down.load(Ordering::Acquire) {
            return ArmOutcome::EpochClosed;
        }
        let handle = {
            let guard = match runtime.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            let Some(owned) = guard.as_ref() else {
                // The runtime is released only by `shutdown` and by `Drop`;
                // either way the epoch is over. Same fact as the gate above.
                return ArmOutcome::EpochClosed;
            };
            owned.handle().clone()
        };
        // Kept for the undo path below: `entry` consumes the key, and the
        // window that undo closes only exists AFTER the key is in the map.
        let undo_key = key.clone();
        // The id of the task THIS call spawned, so the undo below can retract
        // exactly its own arm. See the undo comment for the interleaving that
        // makes removing by key alone wrong.
        let spawned_id;
        match registry.entry(key) {
            Entry::Occupied(slot) => {
                if slot.get().is_finished() {
                    // A finished task's self-removal can race a re-arm for
                    // the same key; replace the dead handle.
                    let spawned = handle.spawn(task);
                    spawned_id = spawned.id();
                    let (key, _finished) = slot.replace_entry(spawned);
                    let _ = key;
                } else {
                    // 🔴 NOT a failure and NOT the epoch closing: a live task
                    // for this exact key owns the work already. Reported as its
                    // own variant so the caller does not tell an operator the
                    // work is unowned when it is in flight.
                    return ArmOutcome::AlreadyArmed;
                }
            }
            Entry::Vacant(slot) => {
                // The entry guard holds the shard lock, so the task's own
                // completion-time removal blocks until this insert finishes.
                let spawned = handle.spawn(task);
                spawned_id = spawned.id();
                slot.insert(spawned);
            }
        }
        // Re-read the gate after inserting. `shutdown` can run its abort sweep
        // between the check above and this insert, in which case our task was
        // never in the map it emptied — it would be cancelled by the runtime
        // drop, but `arm` would already have reported `Armed` and the caller
        // would log that the work is owned when nothing owns it. Undo the arm
        // so the refusal is reported honestly.
        //
        // 🔴 Retract by HANDLE IDENTITY, not by key. Removing by key alone
        // retracts whatever occupies the key now, which need not be this arm:
        // thread A arms key K and is descheduled here; A's task completes and
        // its own release clears the entry; thread B arms K, gets a live
        // registration and tells the operator the work is owned; A resumes,
        // sees the shutdown flag and aborts B's task under B's honest log. The
        // epoch is closing so nothing durable is lost either way — but the log
        // would be false, and this arm exists precisely so refusals are
        // reported honestly.
        if shutting_down.load(Ordering::Acquire) {
            if let Some((_, handle)) = registry.remove_if(&undo_key, |_, h| h.id() == spawned_id) {
                handle.abort();
            }
            return ArmOutcome::EpochClosed;
        }
        ArmOutcome::Armed
    }

    /// Drop the registry entry for a finished watcher task.
    pub(crate) fn remove_watch(&self, parent_pid: u64, child_id: &WorkflowId) {
        self.watches.remove(&(parent_pid, child_id.clone()));
    }

    /// Drop the registry entry for a finished spawn-recovery task.
    pub(crate) fn remove_spawn_retry(&self, child_id: &WorkflowId) {
        self.spawn_retries.remove(child_id);
    }

    /// Drop the registry entry for a finished completion retry — but only if
    /// the entry still belongs to `task`.
    ///
    /// 🔴 THE IDENTITY CHECK IS DEFENCE IN DEPTH, NOT A GUARD ON A REACHABLE
    /// RACE — and an earlier revision of this comment claimed the opposite.
    ///
    /// It said the `Occupied` + `is_finished` arm in [`Self::arm`] replaces a
    /// dead handle "and the outgoing task's release can land after that
    /// replacement". It cannot, on the production call path.
    ///
    /// The property relied on: the task's future — and with it
    /// [`CompletionRetrySlot`], whose `Drop` is the only caller — is dropped
    /// strictly BEFORE the COMPLETE bit that [`JoinHandle::is_finished`] reads,
    /// on every path. So `arm` can never observe `Occupied` +
    /// `is_finished() == true` for an entry whose release has not already run.
    ///
    /// Checked by reading the tokio this workspace actually links, which
    /// `Cargo.lock` pins at **1.52.3** — `tokio-1.52.3/src/runtime/task/
    /// harness.rs`, every line number below from that file:
    ///
    /// - **Normal completion.** `poll_future` hands the output to
    ///   `core.store_output` (549), which sets `Stage::Finished` over
    ///   `Stage::Future` and so drops the future there. `complete()` (331) only
    ///   then reaches `transition_to_complete()` (334), which sets COMPLETE.
    /// - **Cancellation.** `shutdown()` (240) calls `cancel_task` (500), whose
    ///   first act is `drop_future_or_output()` (503), and only afterwards
    ///   `complete()`.
    /// - **Join-handle drop.** `drop_join_handle_slow` (287) drops at 303.
    /// - **Panic.** `poll_future` routes the panic into that same
    ///   `store_output` guard (547-550), so it takes the first path above.
    ///
    /// 🔴 Those line numbers are pinned to 1.52.3 and a version bump will
    /// silently invalidate them. The claim above is the PROPERTY, not the
    /// citation: if the pin moves, re-read the four paths rather than trusting
    /// this list. Nothing in the build fails when the pin moves, which is
    /// exactly why the version is named here instead of left implicit — an
    /// earlier revision of this comment cited `1.53.1`, a version this
    /// workspace has never linked.
    ///
    /// The only ways an entry outlives its task are [`CompletionRetrySlot`]
    /// declining to claim, and the `Weak` failing to upgrade — and in both of
    /// those no later release exists to be foreign, so nothing can be evicted.
    ///
    /// It is kept because it is free and because this is a `pub(crate)` surface
    /// that could acquire a second caller, at which point the property stops
    /// being a consequence of tokio's ordering and starts needing its own guard.
    /// [`Self::remove_watch`] and [`Self::remove_spawn_retry`] remove by key
    /// alone; the difference is that neither of their keys is reused across
    /// leases the way [`CompletionRetryKey`] documents.
    ///
    /// Comparing [`JoinHandle::id`] against the caller's own task id makes the
    /// release affect exactly the registration it was issued for.
    pub(crate) fn remove_completion_retry(
        &self,
        lease: &CompletionRetryKey,
        task: tokio::task::Id,
    ) {
        self.completion_retries
            .remove_if(lease, |_, handle| handle.id() == task);
    }

    /// Abort and drop the watcher armed for one `(parent pid, child id)`.
    ///
    /// Used when a `with_timeout` scope expires for an `await_child`: the
    /// aborted await must not let the watcher record the child terminal
    /// into the parent later, or replay would resolve the await against an
    /// arrival the live run never observed (F1).
    pub(crate) fn abort_watch(&self, parent_pid: u64, child_id: &WorkflowId) {
        if let Some((_, handle)) = self.watches.remove(&(parent_pid, child_id.clone())) {
            handle.abort();
        }
    }

    /// Abort and drop every watcher armed by `parent_pid` (process exit).
    pub(crate) fn abort_watches_for_parent(&self, parent_pid: u64) {
        self.watches.retain(|(pid, _), handle| {
            if *pid == parent_pid {
                handle.abort();
                false
            } else {
                true
            }
        });
    }

    /// Number of currently armed watcher tasks.
    #[cfg(test)]
    pub(crate) fn armed_watch_count(&self) -> usize {
        self.watches.len()
    }

    /// Number of currently armed spawn-recovery tasks.
    #[cfg(test)]
    pub(crate) fn armed_spawn_retry_count(&self) -> usize {
        self.spawn_retries.len()
    }

    /// Number of currently armed process-exit completion retries.
    #[cfg(test)]
    pub(crate) fn armed_completion_retry_count(&self) -> usize {
        self.completion_retries.len()
    }

    /// Close the epoch: gate new arms, abort every task, await each aborted
    /// handle to quiescence, then release the owned runtime.
    ///
    /// Blocking is safe in any context: the owned runtime is dropped on a
    /// dedicated joiner thread (see [`shutdown_runtime_and_join`]), so the
    /// caller never drops a runtime from inside an async context, and the drop
    /// itself is what waits for every aborted task's in-flight poll to finish.
    pub(crate) fn shutdown(&self) {
        self.gate_and_abort();
        let runtime = {
            let mut guard = match self.runtime.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            guard.take()
        };
        let Some(runtime) = runtime else {
            return;
        };
        // Nothing new can be spawned (gate above, runtime slot emptied).
        // Quiescence of every aborted task comes from the blocking runtime
        // drop below: it cancels all remaining tasks and waits for in-flight
        // polls to finish before returning, which is the abort-AND-await
        // contract the epoch close requires.
        shutdown_runtime_and_join(runtime);
    }

    /// Whether the epoch is still open — i.e. whether a task armed on this
    /// executor may still perform a durable write.
    ///
    /// Read at the terminal-append boundary in
    /// [`crate::lifecycle::completion`], because that is the only instant at
    /// which the second-writer hazard is actually realised. A check anywhere
    /// earlier can be walked past: the attempt that passed it goes on to hold
    /// the engine open across a history read, a lock acquisition and a store
    /// round-trip before it writes anything.
    pub(crate) fn is_epoch_open(&self) -> bool {
        !self.shutting_down.load(Ordering::Acquire)
    }

    /// Gate new arms and abort every armed task, without awaiting quiescence
    /// and without releasing the owned runtime.
    ///
    /// The prologue shared by [`Self::shutdown`], [`Self::begin_close`] and
    /// `Drop`. It lives in one place because three callers that each spelled
    /// out the same three `retain`s is one rule known in three places, and a
    /// rule held equal only by diligence has already drifted or will: a fourth
    /// task kind added to one copy and not the others would leave an epoch that
    /// reports itself closed while a task of that kind is still running.
    fn gate_and_abort(&self) {
        self.shutting_down.store(true, Ordering::Release);
        self.watches.retain(|_, handle| {
            handle.abort();
            false
        });
        self.spawn_retries.retain(|_, handle| {
            handle.abort();
            false
        });
        self.completion_retries.retain(|_, handle| {
            handle.abort();
            false
        });
    }

    /// Close the epoch to new and in-flight durable writes without blocking.
    ///
    /// This is the non-blocking half of [`Self::shutdown`], for `Drop` paths
    /// that may run inside a host async context where a blocking join would
    /// panic. It states its own limit: it **gates and aborts, it does not
    /// await**. A task already past the append boundary is cancelled at its
    /// next await point rather than before its current store call returns.
    /// Callers needing abort-AND-await must use [`Self::shutdown`].
    pub(crate) fn begin_close(&self) {
        self.gate_and_abort();
    }
}

/// Sleep the current backoff, then advance it up the ladder toward `ceiling`.
///
/// Shared by every engine background task that retries a durable write, so the
/// cadence is one rule in one place rather than a constant re-chosen per call
/// site. It takes the ceiling rather than a whole policy struct on purpose: the
/// callers no longer agree on WHICH policy governs them — signal delivery bounds
/// an enqueue wait, completion retry bounds a durable store round-trip — and a
/// helper that named one of those types would quietly re-couple them.
pub(crate) async fn sleep_backoff(current: &mut std::time::Duration, ceiling: std::time::Duration) {
    tokio::time::sleep(*current).await;
    let doubled = current.saturating_mul(2);
    *current = if doubled > ceiling { ceiling } else { doubled };
}

/// Shut the owned runtime down and wait for its worker to finish in-flight
/// polls, from any calling context.
fn shutdown_runtime_and_join(runtime: tokio::runtime::Runtime) {
    // Dropping a `Runtime` inside an async context panics; spawn a plain
    // thread to perform the blocking drop and join it. The drop cancels all
    // remaining tasks and waits for in-flight polls to complete, which is
    // exactly the quiescence guarantee the epoch close needs.
    match std::thread::Builder::new()
        .name("aion-engine-tasks-shutdown".to_owned())
        .spawn(move || drop(runtime))
    {
        Ok(joiner) => {
            if joiner.join().is_err() {
                tracing::error!("engine-task runtime shutdown thread panicked");
            }
        }
        Err(error) => {
            // The runtime moved into the closure and is dropped by the failed
            // spawn itself, on THIS thread. That is the one path where the drop
            // is not isolated, so it is reported rather than described as a
            // graceful fallback: from an async context it can panic, and an
            // operator seeing this line needs to know the epoch close did not
            // get the thread it asked for.
            tracing::error!(
                error = %error,
                "could not spawn the engine-task runtime shutdown thread; the \
                 runtime was dropped on the calling thread instead"
            );
        }
    }
}

impl Drop for EngineTaskRuntime {
    fn drop(&mut self) {
        // Backstop for an engine dropped without an explicit shutdown: gate,
        // abort everything, and release the runtime without blocking (this
        // can run inside a host async context, where a blocking drop would
        // panic).
        // 🔴 AN EARLIER REVISION CLAIMED "measured, not assumed: deleting the
        // `completion_retries` sweep from `gate_and_abort` leaves every test in
        // the crate green", and concluded the sweep was not load-bearing on
        // this path. **BOTH HALVES ARE RETRACTED.**
        //
        // The result is false. Re-run at this tree, that deletion turns
        // `shutdown_gates_new_arms_and_awaits_aborted_tasks` RED — it asserts
        // `armed_completion_retry_count() == 0` after `shutdown()`, and its
        // fixture's `park_forever` task carries no `CompletionRetrySlot`, so
        // nothing else empties the map.
        //
        // The METHOD was wrong too, and that is the more useful half. The
        // mutation deletes a line from a helper THREE callers share
        // (`shutdown`, `begin_close`, `Drop`), so whatever it turns red is a
        // verdict about all three — it cannot say anything about this path
        // alone. A shared gauge cannot attribute to one consumer. Isolating the
        // drop path would need the sweep removed for `Drop` only, which is
        // exactly the per-caller duplication `gate_and_abort` exists to prevent;
        // the honest answer is that this path's share is not separately
        // measurable here, not that it is zero.
        //
        // What IS true without a measurement, by reading
        // `shutdown_runtime_and_join`: the runtime drop below cancels every
        // remaining task whichever map its handle sat in, so on this path the
        // tasks stop either way. That makes the drop SUFFICIENT. It does not
        // make the sweep unnecessary — the map is also what
        // `armed_completion_retry_count` reports and what the next arm consults,
        // and a released engine that left stale entries behind would report a
        // lease as owned by a task that no longer exists.
        //
        // What the gate half genuinely buys here, and what the runtime drop
        // alone could never buy, is the flag: `is_epoch_open` is what the
        // terminal-append boundary reads, and this drop is exactly the case
        // where an attempt holding its own strong handle has kept that boundary
        // reachable.
        self.gate_and_abort();
        let runtime = {
            let mut guard = match self.runtime.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            guard.take()
        };
        if let Some(runtime) = runtime {
            runtime.shutdown_background();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    use aion_core::{RunId, WorkflowId};

    use super::{ArmOutcome, CompletionRetryKey, EngineTaskRuntime};

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    /// Sets a flag when the future is dropped (completion or abort).
    struct DropFlag(Arc<AtomicBool>);

    impl Drop for DropFlag {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Release);
        }
    }

    fn park_forever(flag: Arc<AtomicBool>) -> impl Future<Output = ()> + Send + 'static {
        // The guard is captured at construction, not at first poll: a task
        // aborted before it ever runs still drops its future, and the flag
        // must observe that.
        let guard = DropFlag(flag);
        async move {
            let _guard = guard;
            loop {
                tokio::time::sleep(Duration::from_secs(3600)).await;
            }
        }
    }

    #[test]
    fn arming_is_idempotent_per_key() -> TestResult {
        let tasks = EngineTaskRuntime::new()?;
        let parent = 7;
        let child = WorkflowId::new_v4();
        let flag = Arc::new(AtomicBool::new(false));

        assert_eq!(
            tasks.arm_watch(parent, child.clone(), park_forever(Arc::clone(&flag))),
            ArmOutcome::Armed
        );
        // 🔴 `AlreadyArmed`, NOT merely "refused". The two refusals mean
        // opposite things about whether the work has an owner, and while this
        // returned a `bool` this line passed for either — a shutdown-gated
        // refusal here would have read as idempotence.
        assert_eq!(
            tasks.arm_watch(parent, child.clone(), park_forever(Arc::clone(&flag))),
            ArmOutcome::AlreadyArmed
        );
        assert_eq!(tasks.armed_watch_count(), 1);

        // A different child under the same parent is its own watcher.
        assert_eq!(
            tasks.arm_watch(
                parent,
                WorkflowId::new_v4(),
                park_forever(Arc::clone(&flag))
            ),
            ArmOutcome::Armed
        );
        assert_eq!(tasks.armed_watch_count(), 2);
        tasks.shutdown();
        Ok(())
    }

    #[test]
    fn abort_watch_disarms_a_single_key() -> TestResult {
        let tasks = EngineTaskRuntime::new()?;
        let child = WorkflowId::new_v4();
        let other = WorkflowId::new_v4();
        let flag = Arc::new(AtomicBool::new(false));
        assert_eq!(
            tasks.arm_watch(3, child.clone(), park_forever(Arc::clone(&flag))),
            ArmOutcome::Armed
        );
        assert_eq!(
            tasks.arm_watch(3, other.clone(), park_forever(Arc::clone(&flag))),
            ArmOutcome::Armed
        );

        tasks.abort_watch(3, &child);

        assert_eq!(tasks.armed_watch_count(), 1);
        // The remaining key is the other child: re-arming it is still a
        // no-op, re-arming the aborted one is accepted.
        assert_eq!(
            tasks.arm_watch(3, other, park_forever(Arc::clone(&flag))),
            ArmOutcome::AlreadyArmed
        );
        assert_eq!(
            tasks.arm_watch(3, child, park_forever(Arc::clone(&flag))),
            ArmOutcome::Armed
        );
        tasks.shutdown();
        Ok(())
    }

    #[test]
    fn abort_for_parent_leaves_other_parents_armed() -> TestResult {
        let tasks = EngineTaskRuntime::new()?;
        let flag = Arc::new(AtomicBool::new(false));
        for parent in [31, 31, 32] {
            assert_eq!(
                tasks.arm_watch(
                    parent,
                    WorkflowId::new_v4(),
                    park_forever(Arc::clone(&flag))
                ),
                ArmOutcome::Armed
            );
        }

        tasks.abort_watches_for_parent(31);

        assert_eq!(tasks.armed_watch_count(), 1);
        tasks.shutdown();
        Ok(())
    }

    /// A release issued for one task cannot evict a different task's entry.
    ///
    /// 🔴 READ THE SCOPE NARROWLY — AN EARLIER REVISION OF THIS COMMENT DID NOT.
    /// It claimed the race "is not hypothetical and has no other guard", and
    /// justified that with a reopen reusing the run id so successive tasks claim
    /// one key. **That justification is retracted.** [`CompletionRetryKey`] now
    /// carries the monitor pid, and a reopened run's successor lease is a
    /// different pid — so the reopen story produces two DIFFERENT keys and
    /// cannot produce this collision at all.
    ///
    /// What remains is real but smaller, and worth stating exactly. Two tasks
    /// can still hold one key in succession for a single lease: a retry that
    /// ends without recording (an unretryable failure, an epoch close) leaves a
    /// finished handle behind until its drop guard runs, and a later process
    /// exit for that SAME lease arms again — [`EngineTaskRuntime::arm`]'s
    /// `Occupied` + `is_finished` arm deliberately REPLACES the dead handle with
    /// a fresh one. The outgoing task's release runs in its own drop, which can
    /// land after that replacement. Removing by key alone would then delete the
    /// live successor's registration, the map would report the lease as unowned
    /// while a retry for it was still running, and the next arm would spawn a
    /// SECOND writer of that run's terminal. Invariant 3 lost to bookkeeping,
    /// with no bad append anywhere to point at.
    ///
    /// Arming-order alone cannot pin this: the claim-on-first-poll in
    /// `lifecycle::completion_retry` stops a REFUSED arm from releasing, but a
    /// release from a task that genuinely ran and genuinely held the key is not
    /// a refusal, and nothing upstream of this function distinguishes it. So
    /// the check is measured here, directly, against both a foreign id and the
    /// real one.
    #[test]
    fn a_release_from_a_foreign_task_cannot_evict_a_live_completion_retry() -> TestResult {
        let tasks = EngineTaskRuntime::new()?;
        let run = CompletionRetryKey {
            workflow_id: WorkflowId::new_v4(),
            run_id: RunId::new_v4(),
            monitor_pid: 1,
        };
        let flag = Arc::new(AtomicBool::new(false));

        // The foreign id is a REAL task id from this same runtime, taken from a
        // task that has already finished — which is exactly the shape a
        // superseded predecessor's release carries. A fabricated id would prove
        // less: it could be rejected by something other than the comparison.
        let (foreign_tx, foreign_rx) = std::sync::mpsc::channel();
        assert_eq!(
            tasks.arm_spawn_retry(WorkflowId::new_v4(), async move {
                let _ = foreign_tx.send(tokio::task::id());
            }),
            ArmOutcome::Armed
        );
        let foreign = foreign_rx.recv_timeout(Duration::from_secs(10))?;

        let (live_tx, live_rx) = std::sync::mpsc::channel();
        assert_eq!(
            tasks.arm_completion_retry(run.clone(), {
                let flag = Arc::clone(&flag);
                async move {
                    let _ = live_tx.send(tokio::task::id());
                    park_forever(flag).await;
                }
            }),
            ArmOutcome::Armed
        );
        let live = live_rx.recv_timeout(Duration::from_secs(10))?;
        assert_ne!(
            foreign, live,
            "control: the two ids must differ, or the assertions below cannot tell the identity \
             check from an unconditional removal"
        );
        assert_eq!(tasks.armed_completion_retry_count(), 1);

        tasks.remove_completion_retry(&run, foreign);

        assert_eq!(
            tasks.armed_completion_retry_count(),
            1,
            "a release issued for a different task evicted the live retry's registration; the run \
             now reads as unowned while a retry for it is still running, and the next arm for it \
             would spawn a second writer of the same terminal"
        );

        // Positive control: the check is an identity comparison, not a refusal
        // to remove anything. The real owner's release DOES clear the entry.
        tasks.remove_completion_retry(&run, live);
        assert_eq!(
            tasks.armed_completion_retry_count(),
            0,
            "the owning task's own release must clear its entry, or a finished retry would pin \
             its run's key forever and no later exit for that run could ever arm"
        );

        tasks.shutdown();
        Ok(())
    }

    /// 🔴 A REOPENED RUN'S SUCCESSOR LEASE MUST BE ABLE TO ARM ITS OWN RETRY.
    ///
    /// Review 8 found this as a silent permanent zombie, and it is the reason
    /// [`CompletionRetryKey`] carries a pid. A reopen REUSES the run id, so with
    /// the old `(WorkflowId, RunId)` key the successor lease's arm collided with
    /// its own superseded predecessor's still-sleeping retry and came back
    /// `AlreadyArmed` — "somebody owns this terminal". The incumbent then stood
    /// down without writing, because `monitor_stands_down` sees its pid
    /// superseded. The successor's terminal was never recorded, the run
    /// projected `Running` for the life of the epoch, and the only trace was a
    /// `debug!` line asserting the opposite.
    ///
    /// The three assertions are one property split by what each can catch:
    ///
    /// - **The control.** Same workflow, same run, SAME pid arms once and is
    ///   refused the second time. Without it, a key that ignored the run
    ///   entirely would also pass the decisive assertion below while destroying
    ///   the single-writer guarantee this map exists for.
    /// - **The decisive one.** Same workflow, same run, DIFFERENT pid is
    ///   `Armed`, not `AlreadyArmed`.
    /// - **The count.** Both leases hold a registration simultaneously. Two
    ///   armed retries under different pids is the correct state, not a leak:
    ///   the superseded one stands down by the identity check it already
    ///   performs, so exactly one writes.
    ///
    /// The defect this reconstructs is a key that does not distinguish two
    /// leases over the same run. A mutation built out of the new field (for
    /// example always passing the same pid) would reconstruct nothing and pass.
    ///
    /// 🔴 The literal prior key — deleting the `monitor_pid` FIELD — cannot be
    /// run against this test. The test constructs the key with that field and
    /// the control reads it, so deleting it does not compile, and **a mutation
    /// that fails to compile is a NON-RUN: neither a kill nor a survivor.** The
    /// executable equivalent, and the one actually run, keeps the field and
    /// hand-writes `PartialEq`/`Hash` to ignore it — same observable key
    /// collapse, and it compiles because every struct literal still type-checks.
    /// Under it this test fails at "the decisive one"
    /// (`left: AlreadyArmed, right: Armed`) and NOT at the control, which is the
    /// point of the control being a statement about the fixture rather than a
    /// second copy of the subject.
    #[test]
    fn a_reopened_runs_successor_lease_can_arm_its_own_completion_retry() -> TestResult {
        let tasks = EngineTaskRuntime::new()?;
        let workflow_id = WorkflowId::new_v4();
        let run_id = RunId::new_v4();
        let predecessor = CompletionRetryKey {
            workflow_id: workflow_id.clone(),
            run_id: run_id.clone(),
            monitor_pid: 11,
        };
        // Same run, new lease — what `respawn_and_register` installs after a
        // reopen. Only the pid differs, which is the whole point.
        let successor = CompletionRetryKey {
            workflow_id,
            run_id,
            monitor_pid: 12,
        };
        // CONTROL — and it is deliberately a statement about the FIXTURE, not
        // about the key's equality.
        //
        // 🔴 Its first form was `assert_ne!(predecessor, successor)`, which is
        // the same claim the decisive assertion below tests. That made the test
        // useless against the mutation it exists to catch: a key that stopped
        // distinguishing leases collapses `predecessor == successor`, the
        // control fires FIRST, and the decisive assertion never executes — so
        // it measured nothing, and the reader is handed a message about test
        // setup instead of about the defect. A control must survive the
        // mutation the test is aimed at; if it cannot, it is not a control, it
        // is a second copy of the subject.
        assert_ne!(
            predecessor.monitor_pid, successor.monitor_pid,
            "control: the fixture must have built two DIFFERENT leases — same workflow, same \
             run, different monitor pid — or there is no reopen here to test"
        );
        assert_eq!(
            (&predecessor.workflow_id, &predecessor.run_id),
            (&successor.workflow_id, &successor.run_id),
            "control: and they must be the SAME run, or this is two unrelated workflows and the \
             collision the reopen causes never arises"
        );

        let predecessor_flag = Arc::new(AtomicBool::new(false));
        assert_eq!(
            tasks.arm_completion_retry(
                predecessor.clone(),
                park_forever(Arc::clone(&predecessor_flag))
            ),
            ArmOutcome::Armed,
            "the superseded lease's retry is armed and still sleeping on its backoff"
        );

        // CONTROL: the same lease arming twice is still refused. This is the
        // guarantee the pid must not have weakened.
        let duplicate_flag = Arc::new(AtomicBool::new(false));
        assert_eq!(
            tasks.arm_completion_retry(predecessor, park_forever(Arc::clone(&duplicate_flag))),
            ArmOutcome::AlreadyArmed,
            "control: one LEASE must still never hold two retries — if this is `Armed` the pid \
             did not narrow the key, it replaced it, and one run's terminal has two writers"
        );

        // DECISIVE: the successor lease is a different writer and owes its own
        // terminal, so it must get its own retry.
        let successor_flag = Arc::new(AtomicBool::new(false));
        assert_eq!(
            tasks.arm_completion_retry(successor, park_forever(Arc::clone(&successor_flag))),
            ArmOutcome::Armed,
            "THE DECISIVE ONE: a reopened run's successor lease was refused as `AlreadyArmed` by \
             its own superseded predecessor, whose retry then stood down without writing — so \
             the successor's terminal was never recorded and the run projected Running forever"
        );

        assert_eq!(
            tasks.armed_completion_retry_count(),
            2,
            "both leases hold a registration: the superseded one until it stands down, the \
             successor until it writes"
        );

        tasks.shutdown();
        Ok(())
    }

    #[test]
    fn shutdown_gates_new_arms_and_awaits_aborted_tasks() -> TestResult {
        let tasks = EngineTaskRuntime::new()?;
        let watch_flag = Arc::new(AtomicBool::new(false));
        let retry_flag = Arc::new(AtomicBool::new(false));
        let completion_flag = Arc::new(AtomicBool::new(false));
        let child = WorkflowId::new_v4();
        let run = CompletionRetryKey {
            workflow_id: WorkflowId::new_v4(),
            run_id: RunId::new_v4(),
            monitor_pid: 1,
        };
        assert_eq!(
            tasks.arm_watch(9, child.clone(), park_forever(Arc::clone(&watch_flag))),
            ArmOutcome::Armed
        );
        assert_eq!(
            tasks.arm_spawn_retry(child.clone(), park_forever(Arc::clone(&retry_flag))),
            ArmOutcome::Armed
        );
        assert_eq!(
            tasks.arm_completion_retry(run.clone(), park_forever(Arc::clone(&completion_flag))),
            ArmOutcome::Armed
        );
        assert_eq!(tasks.armed_spawn_retry_count(), 1);
        assert_eq!(tasks.armed_completion_retry_count(), 1);

        tasks.shutdown();

        // Awaited, not just aborted: by the time shutdown returns, both task
        // futures have been dropped to quiescence.
        assert!(
            watch_flag.load(Ordering::Acquire),
            "watcher task must be fully dropped before the epoch closes"
        );
        assert!(
            retry_flag.load(Ordering::Acquire),
            "spawn-retry task must be fully dropped before the epoch closes"
        );
        // The completion retry is the ONLY task kind that appends terminal
        // events, so it is the one whose survival past the epoch would be a
        // second writer against a successor engine over the same store
        // (invariant 3). It was the kind `Drop` originally forgot.
        assert!(
            completion_flag.load(Ordering::Acquire),
            "completion-retry task must be fully dropped before the epoch closes"
        );
        assert_eq!(tasks.armed_watch_count(), 0);
        assert_eq!(tasks.armed_spawn_retry_count(), 0);
        assert_eq!(tasks.armed_completion_retry_count(), 0);
        // The gate holds: nothing can be armed after shutdown, and every
        // refusal names the epoch as the reason.
        //
        // 🔴 `EpochClosed`, not just "refused". This is the half of the
        // distinction that matters to an operator: nothing owns this work and
        // nothing in this process will, until a successor's startup sweep runs.
        // Paired with `arming_is_idempotent_per_key`'s `AlreadyArmed`
        // assertion, the two tests can no longer both pass with the two
        // refusals swapped — which is exactly what the `bool` allowed.
        assert_eq!(
            tasks.arm_watch(9, child.clone(), park_forever(Arc::clone(&watch_flag))),
            ArmOutcome::EpochClosed
        );
        assert_eq!(
            tasks.arm_spawn_retry(child, park_forever(Arc::clone(&retry_flag))),
            ArmOutcome::EpochClosed
        );
        assert_eq!(
            tasks.arm_completion_retry(run, park_forever(Arc::clone(&completion_flag))),
            ArmOutcome::EpochClosed
        );
        Ok(())
    }

    /// Dropping the registry cancels an armed completion retry.
    ///
    /// `shutdown` is the explicit close; `Drop` is what runs when an engine is
    /// released without one, and it must cover the terminal-appending task kind
    /// too — the one that can double-write a history past the epoch that owned
    /// it.
    ///
    /// 🔴 Read what this does and does not control. The property is real and
    /// worth pinning — an engine released without an explicit shutdown must not
    /// leave a task that appends terminal events running. But the mechanism that
    /// delivers it is the owned runtime's drop, not the `completion_retries`
    /// abort sweep in `Drop`: deleting that sweep leaves this test green.
    ///
    /// So this is a pin, not a control for that line, and it is not cited as
    /// one. A line with no independent observable has no independent control,
    /// and saying so is cheaper than a test that passes for a reason other than
    /// the one it names.
    #[test]
    fn dropping_the_registry_aborts_completion_retries_too() -> TestResult {
        let flag = Arc::new(AtomicBool::new(false));
        {
            let tasks = EngineTaskRuntime::new()?;
            assert_eq!(
                tasks.arm_completion_retry(
                    CompletionRetryKey {
                        workflow_id: WorkflowId::new_v4(),
                        run_id: RunId::new_v4(),
                        monitor_pid: 1,
                    },
                    park_forever(Arc::clone(&flag)),
                ),
                ArmOutcome::Armed
            );
            assert_eq!(tasks.armed_completion_retry_count(), 1);
        }
        // `Drop` releases the runtime without blocking, so the abort is
        // observed rather than awaited. Poll for it instead of asserting on a
        // race we deliberately did not synchronize.
        let aborted = (0..500).any(|_| {
            if flag.load(Ordering::Acquire) {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
            false
        });
        assert!(
            aborted,
            "dropping the registry must abort the armed completion retry"
        );
        Ok(())
    }

    #[tokio::test]
    async fn shutdown_is_safe_from_inside_a_host_async_context() -> TestResult {
        let tasks = EngineTaskRuntime::new()?;
        let flag = Arc::new(AtomicBool::new(false));
        assert_eq!(
            tasks.arm_watch(11, WorkflowId::new_v4(), park_forever(Arc::clone(&flag))),
            ArmOutcome::Armed
        );

        // Engine::shutdown runs in whatever context the embedder calls it
        // from — including a current-thread tokio test like this one.
        tasks.shutdown();

        assert!(flag.load(Ordering::Acquire));
        Ok(())
    }

    #[tokio::test]
    async fn drop_backstop_aborts_without_blocking() -> TestResult {
        let flag = Arc::new(AtomicBool::new(false));
        {
            let tasks = EngineTaskRuntime::new()?;
            assert_eq!(
                tasks.arm_watch(12, WorkflowId::new_v4(), park_forever(Arc::clone(&flag))),
                ArmOutcome::Armed
            );
            // Dropped without shutdown: the backstop must abort the task and
            // release the runtime without panicking in this async context.
        }
        // Background shutdown is asynchronous; the abort lands promptly.
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while !flag.load(Ordering::Acquire) {
            if std::time::Instant::now() > deadline {
                return Err("drop backstop never aborted the armed task".into());
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        Ok(())
    }
}
