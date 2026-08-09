//! Graceful shutdown and single-node activity drain coordination.

use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Notify, watch};
use tracing::{error, info, warn};

use crate::ServerState;
use crate::error::ServerError;
use crate::worker::LostWorkerReport;

/// Process exit selected by the shutdown coordinator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShutdownOutcome {
    /// Drain completed before the configured timeout.
    Clean,
    /// In-flight activities outlived the drain timeout and were parked for
    /// restart recovery (#207): nothing recorded, nothing delivered — the
    /// recoverable-by-design state, so a fully-parked drain is a SUCCESS. A
    /// long-running activity (an agent round runs hours) outliving any sane
    /// drain window is the expected case, and a non-zero exit on every routine
    /// deploy would train operators to ignore failures.
    Parked,
    /// The drain timed out AND the park itself failed (lock poison, sink
    /// error): in-flight state could not be handed to restart recovery.
    TimedOut,
    /// A second termination signal requested immediate process exit.
    Forced,
}

impl ShutdownOutcome {
    /// Convert the outcome to the process exit code required by the operations contract.
    #[must_use]
    pub fn exit_code(self) -> ExitCode {
        match self {
            Self::Clean | Self::Parked => ExitCode::SUCCESS,
            Self::TimedOut => ExitCode::FAILURE,
            Self::Forced => ExitCode::from(130),
        }
    }
}

/// Cloneable gate shared by transports, dispatchers, worker streams, and the
/// shutdown coordinator.
#[derive(Clone, Debug, Default)]
pub struct DrainState {
    inner: Arc<DrainStateInner>,
}

#[derive(Debug)]
struct DrainStateInner {
    /// The drain latch, held in a `watch` rather than an `AtomicBool` because
    /// one gated seam cannot poll a flag: the bridge's park for an arriving
    /// worker BLOCKS until a worker appears, so it needs this same latch in
    /// awaitable form to stop blocking when the server starts draining.
    ///
    /// One latch answering both questions is the point. Two independent notions
    /// of "we are shutting down" inside the dispatch path is exactly how a
    /// drain gate and a dispatch parked behind it come to disagree — and the
    /// disagreement is unobservable until a process refuses to exit.
    draining: watch::Sender<bool>,
    empty: Notify,
}

impl Default for DrainStateInner {
    fn default() -> Self {
        Self {
            draining: watch::Sender::new(false),
            empty: Notify::default(),
        }
    }
}

impl DrainState {
    /// Return whether drain has begun and new workflow/activity starts must be rejected.
    #[must_use]
    pub fn is_draining(&self) -> bool {
        *self.inner.draining.borrow()
    }

    /// Mark the server draining. Returns true for the first caller that changed the state.
    #[must_use]
    pub fn begin(&self) -> bool {
        // Sets the latch AND wakes every awaiting seam in one write, so a park
        // released by drain can never observe a latch that has not been set
        // yet.
        !self.inner.draining.send_replace(true)
    }

    /// Resolve as soon as drain has begun — [`Self::is_draining`] in awaitable
    /// form, over the same latch.
    ///
    /// For the seam that must block on an external arrival (a worker
    /// registering) and therefore cannot re-read a flag between iterations.
    /// Waking here decides nothing on its own: the woken caller re-runs its
    /// normal loop and meets [`Self::ensure_accepting`], which is still the only
    /// place a drain refusal is produced.
    pub async fn wait_for_drain(&self) {
        let mut draining = self.inner.draining.subscribe();
        while !*draining.borrow_and_update() {
            if draining.changed().await.is_err() {
                // Unreachable while this handle lives — it owns the `Arc` the
                // sender sits in — but a closed latch must read as "drained"
                // rather than block forever on a state that cannot recover.
                break;
            }
        }
    }

    /// Reject a new unit of work if drain has already begun.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::WorkerDispatch`] with a stable drain message when work is closed.
    pub fn ensure_accepting(
        &self,
        namespace: &str,
        activity_type: &str,
    ) -> Result<(), ServerError> {
        if self.is_draining() {
            Err(ServerError::worker_dispatch(
                namespace.to_owned(),
                activity_type.to_owned(),
                "server is draining and not accepting new activity tasks",
            ))
        } else {
            Ok(())
        }
    }

    /// Wake waiters after in-flight accounting may have reached zero.
    pub fn notify_activity_drained(&self) {
        self.inner.empty.notify_waiters();
    }

    async fn wait_for_empty(&self, state: &ServerState) -> Result<(), ServerError> {
        loop {
            let in_flight = state.heartbeat_tracker().in_flight_count()?;
            if in_flight == 0 {
                return Ok(());
            }
            let notified = self.inner.empty.notified();
            if state.heartbeat_tracker().in_flight_count()? == 0 {
                return Ok(());
            }
            notified.await;
        }
    }
}

/// Run the graceful drain after the first termination signal.
///
/// The caller is responsible for stopping transports as soon as drain begins.
///
/// # Errors
///
/// Returns [`ServerError`] if worker-drain broadcast, in-flight accounting, timeout failure
/// surfacing, or engine shutdown fails.
pub async fn drain_after_first_signal(
    state: ServerState,
    second_signal: impl std::future::Future<Output = ()>,
) -> Result<ShutdownOutcome, ServerError> {
    let drain = state.drain_state().clone();
    let first = drain.begin();
    if first {
        info!("shutdown signal received; beginning graceful drain");
    }

    let delivered_workers = state.worker_registry().broadcast_drain()?;
    info!(delivered_workers, "sent drain request to connected workers");

    let timeout = state.runtime_config().drain_timeout;
    tokio::pin!(second_signal);

    let outcome = tokio::select! {
        () = &mut second_signal => {
            warn!("second shutdown signal received; forcing immediate exit");
            ShutdownOutcome::Forced
        }
        result = wait_for_drain_or_timeout(&state, &drain, timeout) => result?,
    };

    // W-4 containment: managed worker PROCESSES stop with the server — AFTER
    // the drain window, never before it. Draining exists to let in-flight work
    // finish, and killing the workers doing that work would invert it; by the
    // time this runs the activities have either completed or been parked for
    // restart recovery (#207), and the next boot reconciles the fleet back up.
    //
    // It runs on the FORCED path too. A second signal asks for an immediate
    // exit, and this does bound that by the operator's own `stop_grace` — but a
    // worker outliving the server that owns it is worse than a bounded moment,
    // and the alternative (relying on the drop guard as the process unwinds)
    // reaps without ever verifying that it did.
    //
    // Deliberately NOT allowed to change the process exit contract (#72/#207):
    // a failure here is reported in full, loudly, and shutdown proceeds.
    // Failing the exit on a worker that would not die would turn a routine
    // deploy red, which is how operators learn to ignore failures.
    stop_managed_workers(&state).await;

    if matches!(outcome, ShutdownOutcome::Forced) {
        return Ok(outcome);
    }

    state.shutdown()?;
    Ok(outcome)
}

async fn wait_for_drain_or_timeout(
    state: &ServerState,
    drain: &DrainState,
    timeout: Duration,
) -> Result<ShutdownOutcome, ServerError> {
    match tokio::time::timeout(timeout, drain.wait_for_empty(state)).await {
        Ok(result) => {
            result?;
            info!("activity drain completed cleanly");
            Ok(ShutdownOutcome::Clean)
        }
        Err(_elapsed) => {
            // #207 drain-timeout backstop: PARK the remaining in-flight
            // dispatches for restart recovery instead of synthesizing
            // transport-loss failures. Nothing is recorded, so the
            // durable log converges on the kill -9 shape and post-restart
            // replay re-dispatches every parked ordinal. A park that itself
            // fails leaves in-flight state unhanded — the one remaining
            // FAILURE-worthy drain outcome.
            match state
                .heartbeat_tracker()
                .park_all_in_flight_workers(state.worker_registry(), state.pending_activities())
            {
                Ok(reports) => {
                    log_parked_workers(&reports);
                    Ok(ShutdownOutcome::Parked)
                }
                Err(park_error) => {
                    error!(
                        %park_error,
                        "activity drain timed out and parking the remaining in-flight \
                         activities failed; exiting with the failure drain outcome"
                    );
                    Ok(ShutdownOutcome::TimedOut)
                }
            }
        }
    }
}

/// Stop every supervised managed worker, and say exactly what happened.
///
/// An empty failure list is the no-orphan claim: each stop returned only after
/// a signal-zero probe found the worker's process group empty. Anything else is
/// logged per worker with the observation that contradicted it — never summed
/// into a single count that could read as calm.
async fn stop_managed_workers(state: &ServerState) {
    let failures = state.worker_supervisor().shutdown().await;
    if failures.is_empty() {
        info!("managed workers stopped; every process group confirmed empty");
        return;
    }
    for failure in &failures {
        error!(%failure, "a managed worker could not be confirmed stopped at shutdown");
    }
    error!(
        unstopped = failures.len(),
        "shutdown could not prove every managed worker stopped; check for orphaned processes"
    );
}

fn log_parked_workers(reports: &[LostWorkerReport]) {
    let parked_tasks: usize = reports.iter().map(|report| report.tasks.len()).sum();
    if parked_tasks == 0 {
        info!("activity drain timed out with no tracked in-flight activities to park");
    } else {
        info!(
            parked_workers = reports.len(),
            parked_tasks,
            "activity drain timed out; remaining activities parked for restart recovery"
        );
    }
}

#[cfg(test)]
mod tests {
    use std::process::ExitCode;
    use std::time::Duration;

    use super::{DrainState, ShutdownOutcome};

    /// How long a woken waiter is allowed to take. Generous, and not a
    /// behavioural bound: a correct latch resolves in microseconds and a broken
    /// one never resolves, so this only decides how long a failure takes to
    /// report.
    const WAKE_BUDGET: Duration = Duration::from_secs(5);

    #[test]
    fn begin_is_idempotent_and_sets_draining() {
        let drain = DrainState::default();

        assert!(!drain.is_draining());
        assert!(drain.begin());
        assert!(drain.is_draining());
        assert!(!drain.begin());
    }

    /// #72: a waiter already parked on the latch is woken by `begin`.
    ///
    /// This is the ordering the bridge's park depends on and the one a
    /// notification-only signal gets wrong: the waiter registers first and the
    /// latch flips afterwards, so nothing it could poll has changed yet. If the
    /// wake is ever lost here, a dispatch parked for a worker becomes
    /// unwakeable and the process cannot exit.
    #[tokio::test]
    async fn begin_wakes_a_waiter_that_registered_before_the_latch_flipped() {
        let drain = DrainState::default();
        let waiting = drain.clone();
        let waiter = tokio::spawn(async move { waiting.wait_for_drain().await });
        // Let the waiter reach its await before the latch is touched.
        tokio::task::yield_now().await;
        assert!(!drain.is_draining());
        assert!(drain.begin());

        let woken = tokio::time::timeout(WAKE_BUDGET, waiter).await;
        assert!(
            matches!(woken, Ok(Ok(()))),
            "a waiter registered before `begin` was not woken: {woken:?}"
        );
    }

    /// The other half of the same race: a waiter arriving AFTER the latch
    /// flipped must not wait for a notification that has already been sent.
    #[tokio::test]
    async fn wait_for_drain_resolves_at_once_once_drain_has_begun() {
        let drain = DrainState::default();
        assert!(drain.begin());

        let resolved = tokio::time::timeout(WAKE_BUDGET, drain.wait_for_drain()).await;
        assert!(
            resolved.is_ok(),
            "a waiter arriving after `begin` blocked instead of resolving"
        );
    }

    /// #207 exit contract: a fully-parked drain is a SUCCESS (parked state is
    /// recoverable by design); FAILURE is reserved for a park that itself
    /// failed; a forced exit keeps 130. `ExitCode` carries no `PartialEq`, so
    /// the mapping is asserted through its debug representation.
    #[test]
    fn exit_codes_map_parked_to_success_and_timed_out_to_failure() {
        let debug = |code: ExitCode| format!("{code:?}");
        assert_eq!(
            debug(ShutdownOutcome::Clean.exit_code()),
            debug(ExitCode::SUCCESS)
        );
        assert_eq!(
            debug(ShutdownOutcome::Parked.exit_code()),
            debug(ExitCode::SUCCESS)
        );
        assert_eq!(
            debug(ShutdownOutcome::TimedOut.exit_code()),
            debug(ExitCode::FAILURE)
        );
        assert_eq!(
            debug(ShutdownOutcome::Forced.exit_code()),
            debug(ExitCode::from(130))
        );
    }
}
