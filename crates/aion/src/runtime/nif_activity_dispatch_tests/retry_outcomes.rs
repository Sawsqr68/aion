use super::*;

/// Retryable failure + budget left: the SAME ordinal re-dispatches at the
/// incremented attempt after the non-terminal failure and the retry start
/// are recorded — the observable per-attempt trail.
#[tokio::test]
async fn retryable_failure_redispatches_with_incremented_recorded_attempt() -> TestResult {
    let harness = RetryLoopHarness::seeded(FIXED_RETRY_CONFIG).await?;
    let dispatcher = ScriptedRetryDispatcher::new(vec![
        Err("retryable:stream reset".to_owned()),
        Ok(r#""done""#.to_owned()),
    ]);
    let outcome = super::dispatch_with_retries(
        &(Arc::clone(&dispatcher) as Arc<dyn ActivityDispatcher>),
        &harness.seam,
        &harness.request,
    )
    .await;

    assert!(
        matches!(
            &outcome.terminal,
            super::RetryLoopTerminal::Completed(payload) if payload == r#""done""#
        ),
        "the second attempt's success must be the delivered outcome"
    );
    assert_eq!(outcome.attempt, 2, "the completing attempt is attempt 2");
    assert_eq!(
        dispatcher.seen_attempts(),
        vec![1, 2],
        "the wire must carry the incremented attempt on the re-dispatch"
    );
    let history = harness.history().await?;
    assert!(
        matches!(
            history.get(3),
            Some(Event::ActivityFailed { error, attempt: 1, .. })
                if error.kind == aion_core::ActivityErrorKind::Retryable
                    && error.message == "retryable:stream reset"
        ),
        "the failed attempt must be recorded as a NON-terminal retryable failure: {history:#?}"
    );
    assert!(
        matches!(
            history.get(4),
            Some(Event::ActivityStarted { attempt: 2, .. })
        ),
        "the retry delivery must record its ActivityStarted: {history:#?}"
    );
    Ok(())
}

/// Exhausted budget: the loop stops at `max_attempts`, the LAST reason is
/// the delivered failure (verbatim), and the final attempt count rides
/// with it.
#[tokio::test]
async fn exhausted_retry_budget_fails_with_last_reason_and_attempt_count() -> TestResult {
    let harness = RetryLoopHarness::seeded(FIXED_RETRY_CONFIG).await?;
    let dispatcher = ScriptedRetryDispatcher::new(vec![
        Err("retryable:reset one".to_owned()),
        Err("retryable:reset two".to_owned()),
        Err("retryable:reset three".to_owned()),
        Ok(r#""never delivered""#.to_owned()),
    ]);
    let outcome = super::dispatch_with_retries(
        &(Arc::clone(&dispatcher) as Arc<dyn ActivityDispatcher>),
        &harness.seam,
        &harness.request,
    )
    .await;

    assert!(
        matches!(
            &outcome.terminal,
            super::RetryLoopTerminal::Failed(reason) if reason == "retryable:reset three"
        ),
        "the LAST reason must surface verbatim"
    );
    assert_eq!(outcome.attempt, 3, "the budget is total attempts");
    assert_eq!(
        dispatcher.seen_attempts(),
        vec![1, 2, 3],
        "exactly max_attempts deliveries, one per attempt"
    );
    let history = harness.history().await?;
    // Two recorded retryable failures (attempts 1 and 2) and two retry
    // starts (attempts 2 and 3); the THIRD failure is the delivered
    // terminal, recorded by the awaiting workflow, not the loop.
    let retryable_failures = history
        .iter()
        .filter(|event| {
            matches!(
                event,
                Event::ActivityFailed { error, .. }
                    if error.kind == aion_core::ActivityErrorKind::Retryable
            )
        })
        .count();
    assert_eq!(retryable_failures, 2, "{history:#?}");
    assert!(
        matches!(
            history.last(),
            Some(Event::ActivityStarted { attempt: 3, .. })
        ),
        "the final delivery's start must be recorded: {history:#?}"
    );
    Ok(())
}

/// Non-retryable failures behave exactly as before the retry loop:
/// one delivery, no recorded retry trail, the reason delivered verbatim.
#[tokio::test]
async fn non_retryable_failure_fails_immediately_without_a_retry_trail() -> TestResult {
    let harness = RetryLoopHarness::seeded(FIXED_RETRY_CONFIG).await?;
    let dispatcher = ScriptedRetryDispatcher::new(vec![Err("terminal:bad request".to_owned())]);
    let outcome = super::dispatch_with_retries(
        &(Arc::clone(&dispatcher) as Arc<dyn ActivityDispatcher>),
        &harness.seam,
        &harness.request,
    )
    .await;

    assert!(matches!(
        &outcome.terminal,
        super::RetryLoopTerminal::Failed(reason) if reason == "terminal:bad request"
    ));
    assert_eq!(outcome.attempt, 1);
    assert_eq!(dispatcher.seen_attempts(), vec![1]);
    assert_eq!(
        harness.history().await?.len(),
        3,
        "no retry events may be recorded for a non-retryable failure"
    );
    Ok(())
}

/// No declared policy (`"retry": null`) keeps the SDK's run-exactly-once
/// contract: a retryable-class failure is delivered after one attempt.
#[tokio::test]
async fn absent_policy_keeps_run_exactly_once_for_retryable_failures() -> TestResult {
    let harness = RetryLoopHarness::seeded(r#"{"retry":null}"#).await?;
    let dispatcher = ScriptedRetryDispatcher::new(vec![Err("retryable:stream reset".to_owned())]);
    let outcome = super::dispatch_with_retries(
        &(Arc::clone(&dispatcher) as Arc<dyn ActivityDispatcher>),
        &harness.seam,
        &harness.request,
    )
    .await;

    assert!(matches!(
        &outcome.terminal,
        super::RetryLoopTerminal::Failed(reason) if reason == "retryable:stream reset"
    ));
    assert_eq!(dispatcher.seen_attempts(), vec![1]);
    assert_eq!(harness.history().await?.len(), 3);
    Ok(())
}

/// F2: the completion task is the FOURTH durable writer this process can hold
/// open, and it survives both `Drop for Engine` and `Engine::shutdown` —
/// `spawn_completion_task` discards its `JoinHandle`, so nothing registers or
/// aborts it, and between attempts it sleeps an SDK-declared backoff with no
/// ceiling. Without an epoch check it could wake long after the engine was
/// released and append `ActivityStarted` into a history a successor has
/// already adopted: a second writer for one workflow (invariant 3).
///
/// Once the epoch closes, the loop must stand down and record NOTHING.
///
/// Killing mutation: delete the `is_epoch_open` guard at the top of
/// `record_retry_event`. The failure is then recorded, history grows, and both
/// the history-length assertion and the terminal assertion fail.
#[tokio::test]
async fn a_closed_engine_task_epoch_stops_the_retry_loop_writing() -> TestResult {
    let harness = RetryLoopHarness::seeded(FIXED_RETRY_CONFIG).await?;
    let before = harness.history().await?.len();

    // CONTROL: with the epoch OPEN this exact dispatcher records a retryable
    // failure and grows the history. Without this, "nothing was recorded" is
    // equally well explained by a fixture that never records anything.
    let control_dispatcher = ScriptedRetryDispatcher::new(vec![
        Err("retryable:stream reset".to_owned()),
        Ok(r#""done""#.to_owned()),
    ]);
    let control = super::dispatch_with_retries(
        &(Arc::clone(&control_dispatcher) as Arc<dyn ActivityDispatcher>),
        &harness.seam,
        &harness.request,
    )
    .await;
    assert!(
        matches!(control.terminal, super::RetryLoopTerminal::Completed(_)),
        "control: an open epoch must let the loop finish normally"
    );
    let after_control = harness.history().await?.len();
    assert!(
        after_control > before,
        "control: an open epoch must record the attempt trail ({before} -> {after_control})"
    );

    // Now close the epoch, exactly as `Engine::shutdown` and `Drop` both do.
    harness.seam.engine_tasks.begin_close();
    assert!(
        !harness.seam.engine_tasks.is_epoch_open(),
        "the epoch must actually be closed, or what follows measures nothing"
    );

    let dispatcher = ScriptedRetryDispatcher::new(vec![
        Err("retryable:stream reset".to_owned()),
        Ok(r#""done""#.to_owned()),
    ]);
    let outcome = super::dispatch_with_retries(
        &(Arc::clone(&dispatcher) as Arc<dyn ActivityDispatcher>),
        &harness.seam,
        &harness.request,
    )
    .await;

    assert!(
        matches!(outcome.terminal, super::RetryLoopTerminal::SettledElsewhere),
        "a closed epoch must stand the loop down rather than deliver an outcome for a run this \
         process is no longer the writer for"
    );
    assert_eq!(
        harness.history().await?.len(),
        after_control,
        "NOTHING may be appended after the epoch closes: an append here is the #119 \
         second-writer breach — the survivor either takes a SequenceConflict or replays a \
         command against an event the live run never issued"
    );
    Ok(())
}
