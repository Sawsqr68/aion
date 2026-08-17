//! Process materialization and recovered-await readiness gates.

use crate::EngineError;

use super::delivery::{
    next_signal_delivery_backoff, sleep_signal_delivery_backoff, yield_signal_delivery_backoff,
};
use super::{Pid, RuntimeHandle, runtime_error};

impl RuntimeHandle {
    pub(crate) fn wait_for_process_ready(&self, pid: Pid) -> Result<(), EngineError> {
        let deadline = std::time::Instant::now() + self.signal_delivery.ready_timeout;
        while std::time::Instant::now() < deadline {
            if self.scheduler.trap_exit(pid).is_some() {
                return Ok(());
            }
            sleep_signal_delivery_backoff(self.signal_delivery.initial_backoff);
        }
        self.scheduler
            .trap_exit(pid)
            .map(|_| ())
            .ok_or_else(|| runtime_error(format!("process {pid} is not ready")))
    }

    /// Async twin of [`Self::wait_for_process_ready`]: identical readiness
    /// semantics, but waits yield to the executor so unrelated deliveries run.
    pub(crate) async fn wait_for_process_ready_async(&self, pid: Pid) -> Result<(), EngineError> {
        let deadline = std::time::Instant::now() + self.signal_delivery.ready_timeout;
        while std::time::Instant::now() < deadline {
            if self.scheduler.trap_exit(pid).is_some() {
                return Ok(());
            }
            yield_signal_delivery_backoff(self.signal_delivery.initial_backoff).await;
        }
        self.scheduler
            .trap_exit(pid)
            .map(|_| ())
            .ok_or_else(|| runtime_error(format!("process {pid} is not ready")))
    }

    /// Wait until a recovered workflow has committed a suspending-await park.
    pub(crate) async fn wait_for_pending_await(&self, pid: Pid) -> Result<(), EngineError> {
        let budget = self
            .signal_delivery
            .ready_timeout
            .saturating_mul(self.signal_delivery.max_enqueue_attempts.max(1));
        let deadline = std::time::Instant::now() + budget;
        let mut backoff = self.signal_delivery.initial_backoff;
        while std::time::Instant::now() < deadline {
            if self.nif_state.has_pending_await(pid) {
                return Ok(());
            }
            if !self.is_live(pid) {
                return Err(runtime_error(format!(
                    "recovered workflow process {pid} exited before reaching its pending await"
                )));
            }
            yield_signal_delivery_backoff(backoff).await;
            backoff = next_signal_delivery_backoff(backoff, self.signal_delivery.max_backoff);
        }
        if self.nif_state.has_pending_await(pid) {
            Ok(())
        } else {
            Err(runtime_error(format!(
                "recovered workflow process {pid} did not reach its pending await within {}ms",
                budget.as_millis()
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::runtime::RuntimeConfig;
    use crate::runtime::nif_state::PendingAwait;

    use super::RuntimeHandle;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_live_recovered_process_may_reach_its_await_after_the_old_deadline()
    -> Result<(), Box<dyn std::error::Error>> {
        let runtime = Arc::new(RuntimeHandle::new(RuntimeConfig::default())?);
        let pid = runtime.spawn_test_process()?;
        let old_budget = runtime
            .signal_delivery
            .ready_timeout
            .saturating_mul(runtime.signal_delivery.max_enqueue_attempts.max(1));

        let waiting_runtime = Arc::clone(&runtime);
        let waiter = tokio::spawn(async move { waiting_runtime.wait_for_pending_await(pid).await });
        tokio::time::sleep(old_budget + std::time::Duration::from_millis(50)).await;
        assert!(
            runtime.is_live(pid),
            "fixture control: the recovered process must still be alive after the prior deadline"
        );
        runtime
            .nif_state
            .pending_awaits
            .insert(pid, PendingAwait::Signal { index: 0 });

        let readiness = waiter.await?;
        runtime.terminate_test_process_with_error(pid)?;
        runtime.shutdown()?;
        readiness.map_err(Into::into)
    }
}
