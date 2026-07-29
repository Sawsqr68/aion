//! Transport-domain failure classification for activities whose worker died.
//!
//! # The failure this exists to stop lying about
//!
//! When a worker is lost before reporting an activity result, the server
//! synthesized a failure that CALLED itself retryable
//! (`retryable:worker WorkerId(2) lost before reporting activity result`) and
//! then was never retried: the engine's retry loop only retries under an
//! AUTHORED retry policy, and an activity with no `retry` decorator has none, so
//! the reason was delivered verbatim as a TERMINAL `ActivityFailed` and the
//! workflow took its fault arm and died. Live evidence: gate runs
//! `dca75b24-bd33-414d-9841-4dc80e6a1f43` and
//! `e6fc9d83-d455-4c29-97a4-cdc8b59f8337`. Consequence: every infrastructure
//! death read to the operator as a red gate.
//!
//! # The two domains
//!
//! An authored `retry N` prices the probability that the ACTION fails — a flaky
//! test, a racy build, a real red. A transport death is a different failure
//! domain: the action never produced a result at all. Charging a transport death
//! against the action's budget makes authored semantics weather-dependent —
//! `retry 2` meaning less on a bad network day — and that is precisely the
//! property a gate runner cannot have. So worker loss is ATTEMPT-NEUTRAL: it
//! consumes no authored budget and re-dispatches the same attempt.
//!
//! # Why attempt-neutral still terminates
//!
//! Attempt-neutral must not mean unbounded, or a permanently flapping link
//! (worker registers, takes the dispatch, dies, forever) would re-dispatch one
//! activity for ever while the action's budget is never touched. So the
//! transport domain carries its OWN ceiling, here, where the failure lives:
//! [`TransportLossLedger`] gives each execution site a wall-clock budget from
//! its FIRST transport loss, and once that budget is spent the failure that
//! surfaces names itself as transport-domain exhaustion
//! ([`TRANSPORT_EXHAUSTED_REASON_PREFIX`]) rather than wearing the action's
//! name. The operator can always tell "your action is red" from "your
//! infrastructure is flapping" by reading the terminal event.
//!
//! The budget is DERIVED, not invented: it is
//! [`TRANSPORT_LOSS_BUDGET_WINDOWS`] × the operator's own
//! `worker.heartbeat_window` — the single value they already wrote to declare
//! what silence means. The transport's whole detect-and-recover cycle (declare
//! the link dead within one window, redial, re-register, re-dispatch) completes
//! inside one window, so four consecutive windows without a single successful
//! delivery is not a transient. A quarter/quadruple relation to that window is
//! the established derivation in this crate: the expiry sweep's cadence is
//! `window / 4` for the same reason, and there is no second knob for either.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use aion_core::{ActivityId, WorkflowId};

use super::registry::WorkerId;
use crate::error::ServerError;

/// Reason prefix for a transport-domain loss that is still inside its budget:
/// the activity never executed to a result and MUST be re-dispatched
/// attempt-neutrally when a worker is available again.
///
/// Consumed by the engine's retry loop
/// (`aion::runtime::nif_activity_retry::is_worker_lost_reason`); the two sides
/// pin this exact string.
pub const WORKER_LOST_REASON_PREFIX: &str = "lost:";

/// Reason prefix for a transport-domain loss whose budget is SPENT: the
/// infrastructure has failed to deliver this activity for longer than the
/// transport is allowed, so the run terminates naming the transport — never the
/// action — as the thing that failed.
pub const TRANSPORT_EXHAUSTED_REASON_PREFIX: &str = "transport-exhausted:";

/// Heartbeat windows of transport-loss budget one execution site gets before the
/// link is declared flapping. See the module docs for the derivation.
pub const TRANSPORT_LOSS_BUDGET_WINDOWS: u32 = 4;

/// Whether `reason` is a transport-domain class (either half of the pair).
///
/// Used to keep the ledger honest: a transport-domain resolution must NOT clear
/// the running budget (that is the whole point of the ceiling), while any other
/// resolution — a real result, a real action failure — must.
#[must_use]
pub fn is_transport_domain_reason(reason: &str) -> bool {
    reason.starts_with(WORKER_LOST_REASON_PREFIX)
        || reason.starts_with(TRANSPORT_EXHAUSTED_REASON_PREFIX)
}

/// The canonical detail text for one worker loss, shared by every site that
/// synthesizes one so the operator reads identical wording wherever it surfaces.
#[must_use]
pub fn worker_lost_detail(worker_id: WorkerId) -> String {
    format!("worker {worker_id:?} lost before reporting activity result")
}

/// Per-execution transport-loss accounting behind the attempt-neutral
/// re-dispatch.
///
/// Cloneable handle over shared state (the dispatcher is cloned per dispatch),
/// so every loss for one `(workflow, activity)` lands in the same budget.
#[derive(Clone, Debug)]
pub struct TransportLossLedger {
    inner: Arc<Mutex<HashMap<ExecutionKey, LossRecord>>>,
    budget: Duration,
}

impl Default for TransportLossLedger {
    /// A ledger with a ZERO budget: an execution site gets exactly ONE
    /// re-dispatchable loss (its first, at elapsed zero) and the next one is
    /// exhaustion.
    ///
    /// Only reachable in wirings that never supply a heartbeat window (isolated
    /// unit tests). Defaulting to zero rather than to some invented span is the
    /// honest choice: a ledger that was never told the operator's window must
    /// not silently grant a budget nobody declared.
    fn default() -> Self {
        Self::new(Duration::ZERO)
    }
}

/// Identity of one activity execution site across its transport losses.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ExecutionKey(WorkflowId, ActivityId);

/// One execution site's running transport-loss budget.
#[derive(Clone, Copy, Debug)]
struct LossRecord {
    first_loss_at: Instant,
    losses: u32,
}

/// How one transport loss was classified, plus the accounting behind it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransportLossVerdict {
    /// The prefixed reason string handed to the engine seam.
    pub reason: String,
    /// Consecutive transport losses this execution site has now taken.
    pub losses: u32,
    /// Whether the budget is spent (the reason is transport-domain exhaustion).
    pub exhausted: bool,
}

impl TransportLossLedger {
    /// Build a ledger whose per-execution budget is
    /// [`TRANSPORT_LOSS_BUDGET_WINDOWS`] × `heartbeat_window`.
    #[must_use]
    pub fn new(heartbeat_window: Duration) -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
            budget: heartbeat_window.saturating_mul(TRANSPORT_LOSS_BUDGET_WINDOWS),
        }
    }

    /// The wall-clock transport-loss budget one execution site gets from its
    /// first loss.
    #[must_use]
    pub const fn budget(&self) -> Duration {
        self.budget
    }

    /// Classify one transport loss for `(workflow_id, activity_id)`.
    ///
    /// Inside the budget this returns a [`WORKER_LOST_REASON_PREFIX`] reason,
    /// which the engine re-dispatches attempt-neutrally. Once the budget is
    /// spent it returns a [`TRANSPORT_EXHAUSTED_REASON_PREFIX`] reason — which
    /// no retry path recognises, so the run terminates — and RETIRES the record,
    /// so a later, genuinely new execution of the same ordinal starts from a
    /// clean budget rather than inheriting a spent one.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] when the ledger cannot be trusted.
    pub fn record_loss(
        &self,
        workflow_id: &WorkflowId,
        activity_id: &ActivityId,
        detail: &str,
    ) -> Result<TransportLossVerdict, ServerError> {
        self.record_loss_at(workflow_id, activity_id, detail, Instant::now())
    }

    /// [`Self::record_loss`] against an explicit clock, so the ceiling is
    /// testable without sleeping out a real budget.
    fn record_loss_at(
        &self,
        workflow_id: &WorkflowId,
        activity_id: &ActivityId,
        detail: &str,
        now: Instant,
    ) -> Result<TransportLossVerdict, ServerError> {
        let key = ExecutionKey(workflow_id.clone(), activity_id.clone());
        let mut ledger = self.state()?;
        let record = ledger.entry(key.clone()).or_insert(LossRecord {
            first_loss_at: now,
            losses: 0,
        });
        record.losses = record.losses.saturating_add(1);
        let losses = record.losses;
        let elapsed = now.saturating_duration_since(record.first_loss_at);
        if elapsed > self.budget {
            ledger.remove(&key);
            return Ok(TransportLossVerdict {
                reason: format!(
                    "{TRANSPORT_EXHAUSTED_REASON_PREFIX}the transport failed to deliver this \
                     activity for {}ms across {losses} worker losses, past its {}ms budget \
                     ({TRANSPORT_LOSS_BUDGET_WINDOWS} heartbeat windows); the infrastructure is \
                     flapping, the activity never ran. Last loss: {detail}",
                    elapsed.as_millis(),
                    self.budget.as_millis()
                ),
                losses,
                exhausted: true,
            });
        }
        Ok(TransportLossVerdict {
            reason: format!("{WORKER_LOST_REASON_PREFIX}{detail}"),
            losses,
            exhausted: false,
        })
    }

    /// Retire an execution site's transport-loss budget because it resolved
    /// OUTSIDE the transport domain — a real result, or a real action failure.
    ///
    /// A transport-domain resolution deliberately does NOT clear: the running
    /// budget is exactly what bounds a flapping link.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] when the ledger cannot be trusted.
    pub fn clear(
        &self,
        workflow_id: &WorkflowId,
        activity_id: &ActivityId,
    ) -> Result<(), ServerError> {
        self.state()?
            .remove(&ExecutionKey(workflow_id.clone(), activity_id.clone()));
        Ok(())
    }

    fn state(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, HashMap<ExecutionKey, LossRecord>>, ServerError> {
        self.inner
            .lock()
            .map_err(|_| ServerError::lock_poisoned("transport loss ledger"))
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use aion_core::{ActivityId, WorkflowId};

    use super::{
        TRANSPORT_EXHAUSTED_REASON_PREFIX, TransportLossLedger, WORKER_LOST_REASON_PREFIX,
        is_transport_domain_reason,
    };

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn site() -> (WorkflowId, ActivityId) {
        (WorkflowId::new_v4(), ActivityId::from_sequence_position(3))
    }

    /// A loss inside the budget is the re-dispatchable class, and it does NOT
    /// wear the action's retry vocabulary.
    #[test]
    fn a_loss_inside_budget_is_the_redispatchable_class() -> TestResult {
        let ledger = TransportLossLedger::new(Duration::from_secs(30));
        let (workflow_id, activity_id) = site();
        let verdict = ledger.record_loss(&workflow_id, &activity_id, "worker gone")?;
        assert!(
            verdict.reason.starts_with(WORKER_LOST_REASON_PREFIX),
            "{verdict:?}"
        );
        assert!(!verdict.exhausted);
        assert_eq!(verdict.losses, 1);
        assert!(
            !verdict.reason.starts_with("retryable:"),
            "a transport death must never borrow the action's retry vocabulary: {verdict:?}"
        );
        Ok(())
    }

    /// THE CEILING: a link that keeps flapping past the budget stops being
    /// re-dispatched and terminates naming the TRANSPORT, not the action.
    #[test]
    fn a_flapping_link_exhausts_its_budget_and_names_the_transport() -> TestResult {
        let window = Duration::from_secs(30);
        let ledger = TransportLossLedger::new(window);
        let (workflow_id, activity_id) = site();
        let start = Instant::now();

        // Losses across the whole budget stay re-dispatchable.
        for step in 0..=4 {
            let verdict = ledger.record_loss_at(
                &workflow_id,
                &activity_id,
                "worker gone",
                start + window * step,
            )?;
            assert!(!verdict.exhausted, "step {step}: {verdict:?}");
        }
        // One tick past the budget is exhaustion.
        let verdict = ledger.record_loss_at(
            &workflow_id,
            &activity_id,
            "worker gone",
            start + window * 4 + Duration::from_millis(1),
        )?;
        assert!(verdict.exhausted, "{verdict:?}");
        assert!(
            verdict
                .reason
                .starts_with(TRANSPORT_EXHAUSTED_REASON_PREFIX),
            "{verdict:?}"
        );
        assert!(
            verdict.reason.contains("the infrastructure is flapping"),
            "the terminal must let an operator tell infra from a red action: {verdict:?}"
        );
        assert_eq!(verdict.losses, 6);

        // The record is retired, so a genuinely new execution starts clean.
        let fresh = ledger.record_loss_at(
            &workflow_id,
            &activity_id,
            "worker gone",
            start + window * 4 + Duration::from_millis(2),
        )?;
        assert!(
            !fresh.exhausted,
            "a retired record starts a fresh budget: {fresh:?}"
        );
        Ok(())
    }

    /// A non-transport resolution retires the budget, so an activity that
    /// survived one blip does not carry it into the next.
    #[test]
    fn a_real_resolution_retires_the_budget() -> TestResult {
        let window = Duration::from_secs(30);
        let ledger = TransportLossLedger::new(window);
        let (workflow_id, activity_id) = site();
        let start = Instant::now();
        ledger.record_loss_at(&workflow_id, &activity_id, "worker gone", start)?;
        ledger.clear(&workflow_id, &activity_id)?;
        let verdict = ledger.record_loss_at(
            &workflow_id,
            &activity_id,
            "worker gone",
            start + window * 10,
        )?;
        assert!(
            !verdict.exhausted,
            "a cleared site must not inherit a spent budget: {verdict:?}"
        );
        assert_eq!(verdict.losses, 1);
        Ok(())
    }

    /// Two execution sites keep independent budgets.
    #[test]
    fn budgets_are_per_execution_site() -> TestResult {
        let ledger = TransportLossLedger::new(Duration::from_secs(30));
        let (workflow_id, activity_id) = site();
        let other = ActivityId::from_sequence_position(9);
        ledger.record_loss(&workflow_id, &activity_id, "worker gone")?;
        let verdict = ledger.record_loss(&workflow_id, &other, "worker gone")?;
        assert_eq!(verdict.losses, 1, "a sibling ordinal has its own budget");
        Ok(())
    }

    /// Both halves of the pair are recognised as transport-domain, and nothing
    /// else is.
    #[test]
    fn transport_domain_recognition_covers_exactly_the_pair() {
        assert!(is_transport_domain_reason("lost:worker WorkerId(2) lost"));
        assert!(is_transport_domain_reason("transport-exhausted:flapping"));
        assert!(!is_transport_domain_reason("retryable:upstream refused"));
        assert!(!is_transport_domain_reason("terminal:boom"));
        assert!(!is_transport_domain_reason("parked:server-draining"));
    }
}
