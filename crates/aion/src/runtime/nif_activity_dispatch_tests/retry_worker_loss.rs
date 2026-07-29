//! Hit D pins: an activity whose WORKER died must survive the death.
//!
//! # The defect these pin
//!
//! The server synthesized a failure that CALLED itself retryable
//! (`retryable:worker WorkerId(2) lost before reporting activity result`) and
//! then was never retried: the loop only retries under an AUTHORED retry policy,
//! and the SDK's run-exactly-once default (`"retry": null`) has none — so the
//! reason was delivered verbatim as a TERMINAL failure and the workflow took its
//! fault arm and died. Live evidence: gate runs
//! `dca75b24-bd33-414d-9841-4dc80e6a1f43` and
//! `e6fc9d83-d455-4c29-97a4-cdc8b59f8337`. Every infrastructure death read to
//! the operator as a red gate.
//!
//! # What the fix asserts
//!
//! Worker loss is a TRANSPORT-domain class (`lost:`), re-dispatched at the SAME
//! attempt with nothing recorded — attempt-neutral, because the action never
//! ran and an authored `retry N` prices the ACTION's flakiness, not the
//! network's. The transport carries its own ceiling on that neutrality: once the
//! server's transport-loss budget is spent it sends `transport-exhausted:`
//! instead, which no arm recognises, so a permanently flapping link still
//! terminates — naming the transport, never the action.

use super::{support::*, *};

/// The reason class `aion-server`'s transport-loss ledger emits while an
/// execution site still has budget. Pinned verbatim here: the prefix is a
/// cross-crate contract, and a drift would silently restore the old defect.
const WORKER_LOST: &str = "lost:worker WorkerId(2) lost before reporting activity result";

/// The reason class it emits once the budget is spent.
const TRANSPORT_EXHAUSTED: &str = "transport-exhausted:the transport failed to deliver this activity for 121000ms across 5 \
     worker losses, past its 120000ms budget (4 heartbeat windows); the infrastructure is \
     flapping, the activity never ran.";

/// THE HIT D GATE. An activity with the SDK's run-exactly-once contract
/// (`"retry": null` — no authored retry budget at all) whose worker dies before
/// reporting must be RE-DISPATCHED, not failed. Before the transport-domain
/// split this delivered a terminal failure on the first loss, killing the
/// workflow on every infrastructure death.
#[tokio::test]
async fn a_worker_loss_redispatches_even_with_no_authored_retry_policy() -> TestResult {
    let harness = RetryLoopHarness::seeded(r#"{"retry":null}"#).await?;
    let dispatcher = ScriptedRetryDispatcher::new(vec![
        Err(WORKER_LOST.to_owned()),
        Ok(r#""served-by-the-replacement-worker""#.to_owned()),
    ]);

    let outcome = super::dispatch_with_retries(
        &(Arc::clone(&dispatcher) as Arc<dyn ActivityDispatcher>),
        &harness.seam,
        &harness.request,
    )
    .await;

    match outcome.terminal {
        RetryLoopTerminal::Completed(payload) => {
            assert_eq!(payload, r#""served-by-the-replacement-worker""#);
        }
        other => {
            return Err(format!(
                "a lost worker must never terminate the activity; got {}",
                terminal_name(&other)
            )
            .into());
        }
    }
    Ok(())
}

/// ATTEMPT-NEUTRAL: the re-dispatch carries the SAME attempt number, and the
/// durable log grows by nothing. A transport death that consumed the action's
/// budget would make `retry 2` mean less on a bad network day.
#[tokio::test]
async fn a_worker_loss_consumes_no_authored_budget_and_records_nothing() -> TestResult {
    let harness = RetryLoopHarness::seeded(FIXED_RETRY_CONFIG).await?;
    let seeded = harness.history().await?.len();
    let dispatcher = ScriptedRetryDispatcher::new(vec![
        Err(WORKER_LOST.to_owned()),
        Err(WORKER_LOST.to_owned()),
        Ok(r#""finally""#.to_owned()),
    ]);

    let outcome = super::dispatch_with_retries(
        &(Arc::clone(&dispatcher) as Arc<dyn ActivityDispatcher>),
        &harness.seam,
        &harness.request,
    )
    .await;

    assert!(
        matches!(outcome.terminal, RetryLoopTerminal::Completed(_)),
        "two transport deaths under a 3-attempt policy must still complete"
    );
    assert_eq!(
        dispatcher.seen_attempts(),
        vec![1, 1, 1],
        "every re-dispatch after a worker loss must carry the SAME attempt: the \
         action never ran, so it consumed no authored budget"
    );
    assert_eq!(
        outcome.attempt, 1,
        "the surviving attempt is still the first"
    );
    assert_eq!(
        harness.history().await?.len(),
        seeded,
        "an attempt-neutral re-dispatch records NOTHING — no ActivityFailed, no \
         second ActivityStarted"
    );
    Ok(())
}

/// The AUTHORED budget still works, unchanged, for real action failures: the
/// transport-domain class must not have widened what counts as retryable.
#[tokio::test]
async fn an_action_failure_still_consumes_its_authored_budget() -> TestResult {
    let harness = RetryLoopHarness::seeded(FIXED_RETRY_CONFIG).await?;
    let dispatcher = ScriptedRetryDispatcher::new(vec![
        Err("retryable:the test was red".to_owned()),
        Ok(r#""green-on-retry""#.to_owned()),
    ]);

    let outcome = super::dispatch_with_retries(
        &(Arc::clone(&dispatcher) as Arc<dyn ActivityDispatcher>),
        &harness.seam,
        &harness.request,
    )
    .await;

    assert!(matches!(outcome.terminal, RetryLoopTerminal::Completed(_)));
    assert_eq!(
        dispatcher.seen_attempts(),
        vec![1, 2],
        "a real action failure still increments the attempt and spends budget"
    );
    Ok(())
}

/// THE CEILING (adversarial condition): a permanently flapping link — worker
/// registers, takes the dispatch, dies, forever — must NOT re-dispatch for ever.
/// The transport stops sending the re-dispatchable class once its own budget is
/// spent, and the terminal that surfaces names the TRANSPORT, so an operator can
/// tell "your action is red" from "your infrastructure is flapping".
#[tokio::test]
async fn a_flapping_link_terminates_naming_the_transport_not_the_action() -> TestResult {
    let harness = RetryLoopHarness::seeded(r#"{"retry":null}"#).await?;
    let dispatcher = ScriptedRetryDispatcher::new(vec![
        Err(WORKER_LOST.to_owned()),
        Err(WORKER_LOST.to_owned()),
        Err(WORKER_LOST.to_owned()),
        Err(WORKER_LOST.to_owned()),
        Err(TRANSPORT_EXHAUSTED.to_owned()),
    ]);

    let outcome = super::dispatch_with_retries(
        &(Arc::clone(&dispatcher) as Arc<dyn ActivityDispatcher>),
        &harness.seam,
        &harness.request,
    )
    .await;

    match outcome.terminal {
        RetryLoopTerminal::Failed(reason) => {
            assert!(
                reason.starts_with("transport-exhausted:"),
                "the terminal must name the transport domain: {reason}"
            );
            assert!(
                reason.contains("the infrastructure is flapping"),
                "the operator must be able to tell infra from a red action: {reason}"
            );
        }
        other => {
            return Err(format!(
                "a flapping link must terminate, not loop for ever; got {}",
                terminal_name(&other)
            )
            .into());
        }
    }
    assert_eq!(
        dispatcher.seen_attempts().len(),
        5,
        "the loop stops the moment the transport says its budget is spent"
    );
    Ok(())
}

/// A terminal recorded ELSEWHERE mid-loop (a `with_timeout` expiry, a workflow
/// terminal) still wins over the attempt-neutral re-dispatch. This arm records
/// nothing, so without an explicit settlement read it would never notice.
#[tokio::test]
async fn a_settled_ordinal_stops_the_worker_loss_redispatch() -> TestResult {
    let harness = RetryLoopHarness::seeded(r#"{"retry":null}"#).await?;
    // Record the ordinal's terminal BEFORE the loop runs, exactly as a
    // `with_timeout` expiry on the workflow thread would.
    {
        let mut recorder = harness.seam.recorder.lock().await;
        recorder
            .record_activity_failed(
                chrono::Utc::now(),
                ActivityId::from_sequence_position(0),
                aion_core::ActivityError {
                    kind: aion_core::ActivityErrorKind::Terminal,
                    message: "timeout:the workflow gave up on this ordinal".to_owned(),
                    details: None,
                },
                1,
            )
            .await?;
    }
    let settled = harness.history().await?.len();
    let dispatcher = ScriptedRetryDispatcher::new(vec![Err(WORKER_LOST.to_owned())]);

    let outcome = super::dispatch_with_retries(
        &(Arc::clone(&dispatcher) as Arc<dyn ActivityDispatcher>),
        &harness.seam,
        &harness.request,
    )
    .await;

    assert!(
        matches!(outcome.terminal, RetryLoopTerminal::SettledElsewhere),
        "an ordinal already settled elsewhere must stand the loop down"
    );
    assert_eq!(
        dispatcher.seen_attempts(),
        vec![1],
        "the settled ordinal is never re-dispatched"
    );
    assert_eq!(
        harness.history().await?.len(),
        settled,
        "the stand-down records nothing"
    );
    Ok(())
}

/// Names a retry-loop terminal for assertion messages, so a failure says WHICH
/// wrong outcome it got rather than just "not what we wanted".
fn terminal_name(terminal: &RetryLoopTerminal) -> &'static str {
    match terminal {
        RetryLoopTerminal::Completed(_) => "Completed",
        RetryLoopTerminal::Failed(_) => "Failed",
        RetryLoopTerminal::SettledElsewhere => "SettledElsewhere",
        RetryLoopTerminal::Parked => "Parked",
    }
}
