use super::*;

/// The live expiry decision must be a pure function of the RESOLUTION
/// snapshot. Race modeled: the await's snapshot (a stale read) lacks
/// the scope deadline's `TimerFired`, which is recorded by the time any
/// later read runs. Before the fix `expired_scope_message` re-read the
/// store, saw the fired deadline, and recorded the durable timeout
/// failure on the spot — a branch decided from events the resolution
/// never observed. After the fix the stale-snapshot pass suspends; the
/// deadline's wake re-enters with a fresh snapshot, records the timeout
/// failure durably, and a fresh engine epoch returns it verbatim while
/// appending nothing.
#[tokio::test(flavor = "multi_thread")]
async fn stale_snapshot_expiry_suspends_then_converges_with_replay() -> TestResult {
    // Stale snapshot = WorkflowStarted + Scheduled + Started: the
    // deadline `TimerFired` (seq 4) is the one event past the window.
    let backing = Arc::new(StaleReadStore::new(3));
    let store: Arc<dyn EventStore> = Arc::clone(&backing) as Arc<dyn EventStore>;
    let (workflow_id, run_id) = seed_pending_activity_then_deadline(&store, 7).await?;
    let harness =
        AwaitHarness::over_store(Arc::clone(&store), workflow_id.clone(), run_id.clone()).await?;
    harness.install_fresh_read_bridge();
    backing.set_stale_target(&workflow_id, 1);
    harness.arm_live_scope(7);

    // Pass 1 — stale resolution snapshot (no terminal, no TimerFired):
    // must suspend, never decide the branch from a fresh read; nothing
    // is recorded.
    assert_eq!(
        harness.step(),
        Ok(ActivityAwaitStep::Suspend),
        "a snapshot lacking both events must park, not branch"
    );
    assert_eq!(harness.history_len().await?, 4);

    // Pass 2 — fresh snapshot: the deadline is in the resolution read;
    // the timeout failure is recorded durably and returned.
    assert_eq!(
        harness.step(),
        Ok(ActivityAwaitStep::Failed(
            "timeout:deadline expired".to_owned()
        ))
    );
    let history = harness.store.read_history(&workflow_id).await?;
    assert!(
        matches!(history.last(), Some(Event::ActivityFailed { .. })),
        "the timeout branch must be recorded durably: {history:#?}"
    );
    let history_len = history.len();
    harness.shutdown()?;

    // Fresh engine epoch over the final store (the restart analogue),
    // scope replay-derived expired exactly as `arm_scope` derives it:
    // the recorded failure resolves verbatim, appending nothing.
    let replay = AwaitHarness::over_store(store, workflow_id, run_id).await?;
    replay.arm_replayed_expired_scope(7);
    assert_eq!(
        replay.step(),
        Ok(ActivityAwaitStep::Failed(
            "timeout:deadline expired".to_owned()
        )),
        "replay must take the same branch as the converged live run"
    );
    assert_eq!(replay.history_len().await?, history_len);
    replay.shutdown()
}

/// A completion sitting in the runtime maps settles the await — and is
/// recorded durably — ahead of the scope-expiry branch, even when the
/// resolution snapshot already contains the fired deadline. The
/// recorded terminal IS the decision, so a fresh engine epoch resolves
/// the completion identically (no deadline-vs-terminal seq ordering is
/// needed for activities: this await records its own terminals, no
/// third party races them into history).
#[tokio::test(flavor = "multi_thread")]
async fn delivered_completion_settles_durably_ahead_of_snapshot_expiry() -> TestResult {
    let backing = Arc::new(StaleReadStore::new(0));
    let store: Arc<dyn EventStore> = Arc::clone(&backing) as Arc<dyn EventStore>;
    let (workflow_id, run_id) = seed_pending_activity_then_deadline(&store, 7).await?;
    let harness =
        AwaitHarness::over_store(Arc::clone(&store), workflow_id.clone(), run_id.clone()).await?;
    harness.arm_live_scope(7);
    harness.runtime.deliver_activity_completion_message(
        harness.pid,
        "activity:0",
        r#""r0""#.to_owned(),
    )?;

    assert_eq!(
        harness.step(),
        Ok(ActivityAwaitStep::Completed(br#""r0""#.to_vec()))
    );
    let history = harness.store.read_history(&workflow_id).await?;
    assert!(
        matches!(history.last(), Some(Event::ActivityCompleted { .. })),
        "the completion must be recorded durably: {history:#?}"
    );
    let history_len = history.len();
    harness.shutdown()?;

    let replay = AwaitHarness::over_store(store, workflow_id, run_id).await?;
    replay.arm_replayed_expired_scope(7);
    assert_eq!(
        replay.step(),
        Ok(ActivityAwaitStep::Completed(br#""r0""#.to_vec())),
        "replay must resolve the recorded completion, not re-derive the race"
    );
    assert_eq!(replay.history_len().await?, history_len);
    replay.shutdown()
}

#[tokio::test(flavor = "multi_thread")]
async fn poisoned_take_fails_typed_and_monitor_drains_retained_state() -> TestResult {
    let backing = Arc::new(StaleReadStore::new(0));
    let store: Arc<dyn EventStore> = Arc::clone(&backing) as Arc<dyn EventStore>;
    let (workflow_id, run_id) = seed_pending_activity_then_deadline(&store, 7).await?;
    let harness = AwaitHarness::over_store(store, workflow_id, run_id).await?;
    let baseline = harness.runtime.activity_delivery_gate_count();
    harness
        .runtime
        .deliver_activity_completion_message_with_attempt(
            harness.pid,
            "activity:0",
            r#""retained""#.to_owned(),
            Some(3),
        )?;
    assert_eq!(harness.runtime.retained_activity_completions(), 1);
    assert_eq!(
        harness.runtime.retained_activity_attempt_count_for_test(),
        1
    );
    harness
        .runtime
        .force_activity_delivery_poison_for_test(harness.pid)?;

    let step = harness.step_typed();
    assert!(matches!(
        step,
        Err(EngineError::ActivityDeliveryPoisoned { process_id })
            if process_id == harness.pid
    ));
    assert_eq!(
        harness.history_len().await?,
        4,
        "poison must neither suspend nor record a fabricated attempt-one completion"
    );

    let (monitor_sender, monitor_receiver) = std::sync::mpsc::channel();
    harness
        .runtime
        .monitor_process_for_test(harness.pid, move |outcome| {
            if monitor_sender.send(outcome).is_err() {
                tracing::error!("poisoned-take monitor receiver dropped");
            }
        })?;
    harness.runtime.cancel_pid(harness.pid)?;
    let monitored = monitor_receiver.recv_timeout(std::time::Duration::from_secs(10))?;
    assert!(matches!(
        monitored,
        Err(EngineError::ActivityDeliveryPoisoned { process_id })
            if process_id == harness.pid
    ));
    assert_eq!(harness.runtime.retained_activity_completions(), 0);
    assert_eq!(
        harness.runtime.retained_activity_attempt_count_for_test(),
        0
    );
    assert_eq!(harness.runtime.activity_delivery_gate_count(), baseline);
    harness.shutdown()
}
/// The completion task notes the final attempt where the awaiting NIF
/// takes it, and the recorded terminal carries it (NOI-0 fidelity across
/// the retry loop).
///
/// 🔴 THIS TEST PREVIOUSLY PINNED THE DEFECT. Before #90 it asserted the
/// terminal message verbatim as `"retryable:reset three"` — a Terminal
/// record whose text claims the failure was retryable — so the durable
/// disagreement had a green test standing behind it and review read the
/// wrong classification as intended behaviour. The message is now
/// asserted as an INVARIANT (terminal class, retryable class absent,
/// diagnostic tail preserved), which no rewording can silently satisfy.
#[tokio::test(flavor = "multi_thread")]
async fn awaited_terminal_records_the_noted_final_attempt() -> TestResult {
    let backing = Arc::new(StaleReadStore::new(0));
    let store: Arc<dyn EventStore> = Arc::clone(&backing) as Arc<dyn EventStore>;
    let (workflow_id, run_id) = seed_pending_activity_then_deadline(&store, 7).await?;
    let harness = AwaitHarness::over_store(Arc::clone(&store), workflow_id.clone(), run_id).await?;
    harness
        .runtime
        .deliver_activity_failure_message_with_attempt(
            harness.pid,
            "activity:0",
            "retryable:reset three".to_owned(),
            Some(3),
        )?;

    let step = harness.step()?;
    let ActivityAwaitStep::Failed(live_message) = step else {
        return Err(format!("the delivered failure must settle the await: {step:?}").into());
    };
    assert!(
        !crate::runtime::nif_activity_retry::is_retryable_reason(&live_message),
        "a terminal await result still classified as retryable: {live_message}"
    );
    assert!(
        live_message.starts_with("terminal:"),
        "the terminal result must carry the terminal class: {live_message}"
    );
    assert!(
        live_message.ends_with("reset three"),
        "the last attempt's diagnostic tail must survive verbatim: {live_message}"
    );

    let history = store.read_history(&workflow_id).await?;
    let Some(Event::ActivityFailed { attempt, error, .. }) = history.last() else {
        return Err(format!("the terminal must be recorded durably: {history:#?}").into());
    };
    assert_eq!(
        *attempt, 3,
        "the recorded terminal must carry the noted final attempt: {history:#?}"
    );
    // Replay re-derives the failure by READING this recorded event, so a
    // recorded message that differed from the one the live run returned
    // would be a replay divergence, not a cosmetic mismatch. Asserted as
    // an equality between the two values rather than as the same literal
    // written twice — a literal cannot catch them drifting apart.
    assert_eq!(
        error.message, live_message,
        "the recorded terminal must be the string the live await returned: {history:#?}"
    );
    harness.shutdown()
}
