use super::*;

const FALLBACK_CONFIG: &str = r#"{"retry":{"max_attempts":2,"backoff":{"kind":"fixed","delay_ms":1}},"fallback":["default","claude"]}"#;

fn refused(workflow_id: &WorkflowId, sequence: u64, attempt: u32) -> Event {
    Event::ActivityFailed {
        envelope: envelope(workflow_id, sequence),
        activity_id: ActivityId::from_sequence_position(0),
        error: aion_core::ActivityError {
            kind: aion_core::ActivityErrorKind::PolicyRefused,
            message: "policy_refused:provider policy".to_owned(),
            details: None,
        },
        attempt,
    }
}

fn hop(
    workflow_id: &WorkflowId,
    sequence: u64,
    attempt: u32,
    to_task_queue: &str,
    fallback_index: u32,
) -> Event {
    Event::ActivityFallbackRouted {
        envelope: envelope(workflow_id, sequence),
        activity_id: ActivityId::from_sequence_position(0),
        attempt,
        from_task_queue: "default".to_owned(),
        to_task_queue: to_task_queue.to_owned(),
        fallback_index,
    }
}

fn started(workflow_id: &WorkflowId, sequence: u64, attempt: u32) -> Event {
    Event::ActivityStarted {
        envelope: envelope(workflow_id, sequence),
        activity_id: ActivityId::from_sequence_position(0),
        attempt,
    }
}

#[tokio::test]
async fn replay_before_refusal_record_chooses_the_same_authored_hop() -> TestResult {
    let harness = RetryLoopHarness::seeded(FALLBACK_CONFIG).await?;
    let dispatcher = ScriptedRetryDispatcher::new(vec![
        Err("policy_refused:provider policy".to_owned()),
        Ok(r#""done""#.to_owned()),
    ]);

    let outcome = dispatch_with_retries(
        &(Arc::clone(&dispatcher) as Arc<dyn ActivityDispatcher>),
        &harness.seam,
        &harness.request,
    )
    .await;

    assert!(matches!(outcome.terminal, RetryLoopTerminal::Completed(_)));
    assert_eq!(dispatcher.seen_task_queues(), vec!["default", "claude"]);
    assert_eq!(
        harness
            .history()
            .await?
            .iter()
            .filter(|event| matches!(event, Event::ActivityFallbackRouted { .. }))
            .count(),
        1
    );
    Ok(())
}

#[tokio::test]
async fn replay_between_refusal_and_hop_records_the_same_hop_before_dispatch() -> TestResult {
    let harness = RetryLoopHarness::seeded_with_tail(FALLBACK_CONFIG, 2, |workflow_id| {
        vec![refused(workflow_id, 4, 1)]
    })
    .await?;
    let dispatcher = ScriptedRetryDispatcher::new(vec![Ok(r#""done""#.to_owned())]);

    let outcome = dispatch_with_retries(
        &(Arc::clone(&dispatcher) as Arc<dyn ActivityDispatcher>),
        &harness.seam,
        &harness.request,
    )
    .await;

    assert!(matches!(outcome.terminal, RetryLoopTerminal::Completed(_)));
    assert_eq!(dispatcher.seen_task_queues(), vec!["claude"]);
    let history = harness.history().await?;
    assert!(matches!(
        history.get(4),
        Some(Event::ActivityFallbackRouted { to_task_queue, .. }) if to_task_queue == "claude"
    ));
    assert!(matches!(
        history.get(5),
        Some(Event::ActivityStarted { attempt: 2, .. })
    ));
    assert_eq!(
        history
            .iter()
            .filter(|event| matches!(event, Event::ActivityFallbackRouted { .. }))
            .count(),
        1
    );
    Ok(())
}

#[tokio::test]
async fn replay_between_hop_and_start_reuses_the_event_queue_not_fresh_derivation() -> TestResult {
    let config = r#"{"fallback":["claude"]}"#;
    let harness = RetryLoopHarness::seeded_with_tail(config, 2, |workflow_id| {
        vec![
            refused(workflow_id, 4, 1),
            hop(workflow_id, 5, 1, "gemini", 0),
            started(workflow_id, 6, 2),
        ]
    })
    .await?;
    let dispatcher = ScriptedRetryDispatcher::new(vec![Ok(r#""done""#.to_owned())]);

    let outcome = dispatch_with_retries(
        &(Arc::clone(&dispatcher) as Arc<dyn ActivityDispatcher>),
        &harness.seam,
        &harness.request,
    )
    .await;

    assert!(matches!(outcome.terminal, RetryLoopTerminal::Completed(_)));
    assert_eq!(dispatcher.seen_task_queues(), vec!["gemini"]);
    assert_eq!(
        harness
            .history()
            .await?
            .iter()
            .filter(|event| matches!(event, Event::ActivityFallbackRouted { .. }))
            .count(),
        1
    );
    Ok(())
}

#[tokio::test]
async fn replay_between_start_and_delivery_uses_the_recorded_hop_queue() -> TestResult {
    let harness = RetryLoopHarness::seeded_with_tail(FALLBACK_CONFIG, 2, |workflow_id| {
        vec![
            refused(workflow_id, 4, 1),
            hop(workflow_id, 5, 1, "claude", 1),
            started(workflow_id, 6, 2),
        ]
    })
    .await?;
    let dispatcher = ScriptedRetryDispatcher::new(vec![Ok(r#""done""#.to_owned())]);

    let outcome = dispatch_with_retries(
        &(Arc::clone(&dispatcher) as Arc<dyn ActivityDispatcher>),
        &harness.seam,
        &harness.request,
    )
    .await;

    assert!(matches!(outcome.terminal, RetryLoopTerminal::Completed(_)));
    assert_eq!(dispatcher.seen_attempts(), vec![2]);
    assert_eq!(dispatcher.seen_task_queues(), vec!["claude"]);
    Ok(())
}

#[tokio::test]
async fn replay_after_last_refusal_finishes_without_inventing_an_execution() -> TestResult {
    let harness = RetryLoopHarness::seeded_with_tail(FALLBACK_CONFIG, 3, |workflow_id| {
        vec![
            refused(workflow_id, 4, 1),
            hop(workflow_id, 5, 1, "claude", 1),
            started(workflow_id, 6, 2),
            refused(workflow_id, 7, 2),
        ]
    })
    .await?;
    let dispatcher = ScriptedRetryDispatcher::new(Vec::new());

    let outcome = dispatch_with_retries(
        &(Arc::clone(&dispatcher) as Arc<dyn ActivityDispatcher>),
        &harness.seam,
        &harness.request,
    )
    .await;

    assert!(matches!(
        outcome.terminal,
        RetryLoopTerminal::Failed(reason)
            if reason.starts_with("policy_refused:")
                && reason.contains("default, claude")
    ));
    assert!(dispatcher.seen_attempts().is_empty());
    assert_eq!(
        harness
            .history()
            .await?
            .iter()
            .filter(|event| matches!(event, Event::ActivityFallbackRouted { .. }))
            .count(),
        1
    );
    Ok(())
}
