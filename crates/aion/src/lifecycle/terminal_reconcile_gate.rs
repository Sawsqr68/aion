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

use std::sync::{Arc, OnceLock};

use aion_core::{RunId, WorkflowId};
use dashmap::DashMap;
use tokio::sync::watch;

/// One armed hold. Both signals are `watch` channels — the same primitive
/// [`crate::registry::CompletionNotifier`] uses — because a `watch` receiver
/// checks the current value before parking, so neither side can miss an edge
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
    let Some(gate) = gates()
        .get(&(id.clone(), run.clone()))
        .map(|entry| Arc::clone(&entry))
    else {
        return;
    };
    gate.reached.send_replace(true);
    let mut release = gate.release.subscribe();
    // `wait_for` inspects the current value first, so a release that already
    // happened is observed rather than waited for. An `Err` means every sender
    // is gone — the guard was dropped — which is also a release.
    drop(release.wait_for(|released| *released).await);
}

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
