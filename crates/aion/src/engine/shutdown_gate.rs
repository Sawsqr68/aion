//! Engine shutdown gating: lifecycle operations are counted in, shutdown
//! closes the gate and waits for the count to drain before tearing the
//! runtime down.

use std::sync::{Arc, Condvar, Mutex};

use crate::EngineError;

#[derive(Clone, Default)]
pub(super) struct ShutdownGate {
    inner: Arc<ShutdownGateInner>,
}

#[derive(Default)]
struct ShutdownGateInner {
    /// Test-only injected `close_and_wait` failure.
    ///
    /// The real failure is mutex poison, which a test cannot arrange without
    /// poisoning a lock the rest of the fixture then trips over. This exists
    /// so `Engine::shutdown`'s FIRST fallible step can be made to fail — the
    /// only way to reach its second-error reporting branch, which is the fix
    /// for the swallowed-second-error defect and was executed by no test that
    /// asserted on it (measured: deleting the report left 659 tests green).
    #[cfg(test)]
    forced_close_failure: std::sync::atomic::AtomicBool,
    state: Mutex<ShutdownState>,
    idle: Condvar,
}

#[derive(Default)]
struct ShutdownState {
    shutting_down: bool,
    active_operations: usize,
}

impl ShutdownGate {
    pub(super) fn begin_start(&self) -> Result<LifecycleOperation, EngineError> {
        let mut state = self.state()?;
        if state.shutting_down {
            return Err(EngineError::ShuttingDown);
        }
        state.active_operations += 1;
        Ok(LifecycleOperation {
            inner: Arc::clone(&self.inner),
        })
    }

    pub(super) fn begin_operation(&self) -> Result<LifecycleOperation, EngineError> {
        let mut state = self.state()?;
        // Refuse like `begin_start`: an operation admitted while
        // `close_and_wait` is draining would prolong the drain arbitrarily,
        // and the engine it would act on is already tearing down.
        if state.shutting_down {
            return Err(EngineError::ShuttingDown);
        }
        state.active_operations += 1;
        Ok(LifecycleOperation {
            inner: Arc::clone(&self.inner),
        })
    }

    /// Arm the injected `close_and_wait` failure. See `forced_close_failure`.
    #[cfg(test)]
    pub(super) fn force_close_failure(&self) {
        self.inner
            .forced_close_failure
            .store(true, std::sync::atomic::Ordering::Release);
    }

    pub(super) fn close_and_wait(&self) -> Result<(), EngineError> {
        // The injected variant is the one THIS function can actually raise:
        // both of its real error paths map a poisoned lock to
        // `RegistryPoisoned`, so a fixture returning anything else would model
        // a machine that does not exist.
        #[cfg(test)]
        if self
            .inner
            .forced_close_failure
            .load(std::sync::atomic::Ordering::Acquire)
        {
            return Err(EngineError::RegistryPoisoned);
        }
        let mut state = self.state()?;
        state.shutting_down = true;
        while state.active_operations > 0 {
            state = self
                .inner
                .idle
                .wait(state)
                .map_err(|_| EngineError::RegistryPoisoned)?;
        }
        Ok(())
    }

    fn state(&self) -> Result<std::sync::MutexGuard<'_, ShutdownState>, EngineError> {
        self.inner
            .state
            .lock()
            .map_err(|_| EngineError::RegistryPoisoned)
    }
}

pub(super) struct LifecycleOperation {
    inner: Arc<ShutdownGateInner>,
}

impl Drop for LifecycleOperation {
    fn drop(&mut self) {
        if let Ok(mut state) = self.inner.state.lock() {
            state.active_operations = state.active_operations.saturating_sub(1);
            if state.active_operations == 0 {
                self.inner.idle.notify_all();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ShutdownGate;
    use crate::EngineError;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn operations_are_refused_once_shutdown_begins() -> TestResult {
        let gate = ShutdownGate::default();
        // Admitted while open; the gate drains to idle on drop.
        drop(gate.begin_operation()?);
        gate.close_and_wait()?;

        assert!(matches!(
            gate.begin_operation(),
            Err(EngineError::ShuttingDown)
        ));
        assert!(matches!(gate.begin_start(), Err(EngineError::ShuttingDown)));
        Ok(())
    }

    #[test]
    fn close_waits_for_admitted_operations_to_drain() -> TestResult {
        let gate = ShutdownGate::default();
        let operation = gate.begin_operation()?;
        let closer = {
            let gate = gate.clone();
            std::thread::spawn(move || gate.close_and_wait())
        };
        // The closer parks until the admitted operation drops.
        std::thread::sleep(std::time::Duration::from_millis(20));
        assert!(!closer.is_finished());
        drop(operation);
        closer
            .join()
            .map_err(|_| "close_and_wait thread panicked")??;
        Ok(())
    }
}
