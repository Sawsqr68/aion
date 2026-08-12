//! #223: the authored per-attempt activity timeout, at the dispatch seam.
//!
//! Every test here gates its first attempt on a condition variable the TEST
//! releases, so "the dispatch has not returned" is a fact rather than a race
//! against a sleep. The only elapsed-time assumption left is that a bound of
//! 20ms expires while a dispatch is parked on a gate that nothing has opened
//! yet — which no amount of load can falsify, because the gate cannot open
//! until the assertions are done.

use super::*;

/// A dispatcher whose FIRST attempt parks until the test opens the gate, and
/// whose later attempts return immediately.
///
/// Parking (rather than sleeping) is what makes the expiry deterministic: an
/// attempt that has not returned cannot accidentally beat the deadline
/// because the box was fast.
pub(super) struct GatedRetryDispatcher {
    gate: std::sync::Mutex<bool>,
    opened: std::sync::Condvar,
    attempts: std::sync::Mutex<Vec<u32>>,
    later: std::sync::Mutex<Result<String, String>>,
}

impl GatedRetryDispatcher {
    fn new(later: Result<String, String>) -> Arc<Self> {
        Arc::new(Self {
            gate: std::sync::Mutex::new(false),
            opened: std::sync::Condvar::new(),
            attempts: std::sync::Mutex::new(Vec::new()),
            later: std::sync::Mutex::new(later),
        })
    }

    /// Release the parked first attempt so its blocking task can finish
    /// before the test's runtime is torn down. A `spawn_blocking` task is not
    /// cancellable, so leaving it parked would wedge teardown.
    fn open(&self) -> Result<(), String> {
        let mut gate = self
            .gate
            .lock()
            .map_err(|_| "gate lock poisoned".to_owned())?;
        *gate = true;
        self.opened.notify_all();
        Ok(())
    }

    fn seen_attempts(&self) -> Vec<u32> {
        self.attempts
            .lock()
            .map(|attempts| attempts.clone())
            .unwrap_or_default()
    }
}

impl ActivityDispatcher for GatedRetryDispatcher {
    fn dispatch(&self, request: ActivityDispatch) -> Result<String, String> {
        let first = {
            let mut attempts = self
                .attempts
                .lock()
                .map_err(|_| "attempts lock poisoned".to_owned())?;
            attempts.push(request.attempt);
            attempts.len() == 1
        };
        if first {
            let mut gate = self
                .gate
                .lock()
                .map_err(|_| "gate lock poisoned".to_owned())?;
            while !*gate {
                gate = self
                    .opened
                    .wait(gate)
                    .map_err(|_| "gate wait poisoned".to_owned())?;
            }
            return Ok(r#""released too late to matter""#.to_owned());
        }
        self.later
            .lock()
            .map_err(|_| "later lock poisoned".to_owned())?
            .clone()
    }
}

/// A 20ms bound with two attempts of budget.
const BOUNDED_RETRY_CONFIG: &str =
    r#"{"retry":{"max_attempts":2,"backoff":{"kind":"fixed","delay_ms":2}},"timeout_ms":20}"#;
/// A 20ms bound and the SDK's run-exactly-once default.
const BOUNDED_NO_RETRY_CONFIG: &str = r#"{"retry":null,"timeout_ms":20}"#;
/// No bound at all — the control for every assertion above.
const UNBOUNDED_NO_RETRY_CONFIG: &str = r#"{"retry":null,"timeout_ms":null}"#;

/// The authored bound ends the attempt, and the retry schedule TAKES the
/// expiry — `timeout_inside_retry`'s "per-attempt timeout inside a bounded
/// retry schedule", executed.
///
/// This is also the mutation killer for the loop's `expired ||` term: drop it
/// and the expiry falls to the non-retryable `timeout:` class, the second
/// attempt never runs, and this test goes red.
#[tokio::test]
async fn an_expired_attempt_is_retried_while_budget_remains() -> TestResult {
    let harness = RetryLoopHarness::seeded(BOUNDED_RETRY_CONFIG).await?;
    let dispatcher = GatedRetryDispatcher::new(Ok(r#""done""#.to_owned()));
    let outcome = super::dispatch_with_retries(
        &(Arc::clone(&dispatcher) as Arc<dyn ActivityDispatcher>),
        &harness.seam,
        &harness.request,
    )
    .await;
    dispatcher.open()?;

    assert!(
        matches!(
            &outcome.terminal,
            RetryLoopTerminal::Completed(payload) if payload == r#""done""#
        ),
        "the retry after the expiry must be the delivered outcome: {:?}",
        outcome.terminal
    );
    assert_eq!(outcome.attempt, 2, "the completing attempt is attempt 2");
    assert_eq!(
        dispatcher.seen_attempts(),
        vec![1, 2],
        "the expired attempt must be followed by an incremented re-delivery"
    );
    let history = harness.history().await?;
    assert!(
        matches!(
            history.get(3),
            Some(Event::ActivityFailed { error, attempt: 1, .. })
                if error.kind == aion_core::ActivityErrorKind::Retryable
                    && error.message.starts_with("timeout:")
                    && error.message.contains("20ms")
        ),
        "the expiry must be recorded as a non-terminal failure naming the bound: {history:#?}"
    );
    Ok(())
}

/// With no retry policy — the SDK's run-exactly-once default — an expiry is
/// the activity's terminal failure, and it says which bound fired.
#[tokio::test]
async fn an_expired_attempt_without_a_policy_fails_naming_the_bound() -> TestResult {
    let harness = RetryLoopHarness::seeded(BOUNDED_NO_RETRY_CONFIG).await?;
    let dispatcher = GatedRetryDispatcher::new(Ok(r#""never reached""#.to_owned()));
    let outcome = super::dispatch_with_retries(
        &(Arc::clone(&dispatcher) as Arc<dyn ActivityDispatcher>),
        &harness.seam,
        &harness.request,
    )
    .await;
    dispatcher.open()?;

    assert!(
        matches!(
            &outcome.terminal,
            RetryLoopTerminal::Failed(reason)
                if reason.starts_with("timeout:") && reason.contains("20ms")
        ),
        "an expiry with no retry budget must fail the activity, and the reason must carry the \
         `timeout:` prefix the SDK types `error.ActivityTimedOut` from plus the bound that \
         fired: {:?}",
        outcome.terminal
    );
    assert_eq!(
        dispatcher.seen_attempts(),
        vec![1],
        "no policy means no re-delivery"
    );
    Ok(())
}

/// 🔴 THE CONTROL. The same parked dispatch, with NO authored bound, is not
/// ended by anything: the loop is still waiting when a bound ten times the
/// other tests' would long since have fired.
///
/// Without this, every assertion above is equally satisfied by an engine that
/// invented a deadline of its own — which is the one thing this reader must
/// never do.
#[tokio::test]
async fn an_unbounded_dispatch_is_never_ended_by_the_engine() -> TestResult {
    let harness = RetryLoopHarness::seeded(UNBOUNDED_NO_RETRY_CONFIG).await?;
    let dispatcher = GatedRetryDispatcher::new(Ok(r#""never reached""#.to_owned()));
    let erased = Arc::clone(&dispatcher) as Arc<dyn ActivityDispatcher>;
    let loop_future = super::dispatch_with_retries(&erased, &harness.seam, &harness.request);
    let still_waiting =
        tokio::time::timeout(std::time::Duration::from_millis(200), loop_future).await;
    // Open BEFORE asserting, always. A `spawn_blocking` task cannot be
    // cancelled, so a panic between the await and the release would leave the
    // parked attempt holding a blocking thread and wedge the runtime's
    // teardown — a FAILING test would hang instead of reporting, which is the
    // one failure mode a control must never have. Found by running this file
    // under a mutation that made the control fail.
    dispatcher.open()?;
    assert!(
        still_waiting.is_err(),
        "an activity whose document declares no timeout must still be waiting: {still_waiting:?}"
    );
    assert_eq!(
        dispatcher.seen_attempts(),
        vec![1],
        "the single parked attempt was never abandoned"
    );
    Ok(())
}
