use super::*;

/// THE OPERATOR'S SCENARIO, FIXED: a plain activity dispatch through the
/// engine-seam bridge reaches the liminal-registered worker, executes there, and
/// the correlated reply resolves the dispatch — twice (no one-shot luck) — with
/// the bridge's in-flight liveness bookkeeping cleared afterwards.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn bridge_dispatch_reaches_liminal_worker_and_resolves() -> Result<(), TestError> {
    let server = RunningServer::start(DEFAULT_WINDOW)?;
    let executions = Arc::new(AtomicUsize::new(0));
    let worker = ServedWorker::spawn(
        server.address.to_string(),
        worker_activity_registry(Arc::clone(&executions))?,
    );
    server.wait_for_registered_worker(PROVISION)?;

    let dispatcher = Arc::new(server.bridge_dispatcher());
    let workflow_id = WorkflowId::new(Uuid::new_v4());

    // FIRST dispatch: before the fix this failed Terminal with "worker
    // WorkerId(_) has no gRPC stream sender (non-gRPC transport)".
    let first = dispatch_via_seam(
        &dispatcher,
        dispatch_request(
            &workflow_id,
            0,
            PROVISION,
            &serde_json::json!({ "resource": "database" }),
        ),
    )
    .await?;
    let first =
        first.map_err(|reason| test_error(format!("first bridge dispatch failed: {reason}")))?;
    let output: ProvisionOutput = serde_json::from_str(&first).map_err(test_error)?;
    assert!(output.provisioned, "the worker's handler genuinely ran");
    assert_eq!(
        output.resource, "database",
        "the handler saw the dispatched input"
    );
    assert_eq!(
        executions.load(Ordering::SeqCst),
        1,
        "the remote worker executed the first dispatch exactly once"
    );

    // SECOND dispatch on the same connection: the delivery is repeatable, not
    // one-shot luck (a fresh correlated push + reply per dispatch).
    let second = dispatch_via_seam(
        &dispatcher,
        dispatch_request(
            &workflow_id,
            1,
            PROVISION,
            &serde_json::json!({ "resource": "cache" }),
        ),
    )
    .await?;
    let second =
        second.map_err(|reason| test_error(format!("second bridge dispatch failed: {reason}")))?;
    let output: ProvisionOutput = serde_json::from_str(&second).map_err(test_error)?;
    assert_eq!(output.resource, "cache");
    assert_eq!(
        executions.load(Ordering::SeqCst),
        2,
        "the remote worker executed the second dispatch too"
    );

    // The bridge's liveness bookkeeping matches gRPC: both completions cleared
    // their in-flight entries (nothing left for a sweep to fail).
    assert_eq!(
        server
            .state
            .heartbeat_tracker()
            .in_flight_count()
            .map_err(test_error)?,
        0,
        "completed liminal dispatches must clear their in-flight tracking"
    );

    worker.stop()?;
    server.shutdown()?;
    Ok(())
}

/// THE SWEEPER SIDE (the false-lost-worker regression): with the PRODUCTION
/// heartbeat sweeper ticking against a deliberately short window, an activity
/// that genuinely runs FOUR windows long still completes — the worker's
/// automatic liveness beats over the liminal wire keep the tracked dispatch
/// alive, exactly as the gRPC runtime's quarter-window liveness pump does.
/// Without the liminal liveness path this fails in under a second: the sweeper
/// declares the healthy worker lost, the dispatch resolves as a transport
/// loss while the handler keeps running, and the worker is deregistered.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn dispatch_outliving_heartbeat_window_survives_the_sweeper() -> Result<(), TestError> {
    let server = RunningServer::start(SHORT_WINDOW)?;
    let executions = Arc::new(AtomicUsize::new(0));
    let worker = ServedWorker::spawn(
        server.address.to_string(),
        worker_activity_registry(Arc::clone(&executions))?,
    );
    server.wait_for_registered_worker(SLOW)?;

    let dispatcher = Arc::new(server.bridge_dispatcher());
    let workflow_id = WorkflowId::new(Uuid::new_v4());

    let result = dispatch_via_seam(
        &dispatcher,
        dispatch_request(
            &workflow_id,
            0,
            SLOW,
            &serde_json::json!({ "resource": "long-haul" }),
        ),
    )
    .await?;
    let result = result.map_err(|reason| {
        test_error(format!(
            "an over-window dispatch must not be failed by the sweeper: {reason}"
        ))
    })?;
    let output: ProvisionOutput = serde_json::from_str(&result).map_err(test_error)?;
    assert_eq!(output.resource, "long-haul");
    assert_eq!(
        executions.load(Ordering::SeqCst),
        1,
        "the activity executed exactly once — no duplicate from a false lost-worker retry"
    );
    // The healthy worker was NOT deregistered by the sweeper.
    assert!(
        server.worker_is_registered(SLOW)?,
        "a healthy worker running a long activity must stay registered"
    );
    // The completion cleared its in-flight entry (nothing left to sweep).
    assert_eq!(
        server
            .state
            .heartbeat_tracker()
            .in_flight_count()
            .map_err(test_error)?,
        0
    );

    worker.stop()?;
    server.shutdown()?;
    Ok(())
}

/// THE LOST-WORKER SIDE (the reply router's Disconnected arm): a worker whose
/// connection dies while its dispatch is in flight resolves the dispatch
/// promptly with the SAME TRANSPORT-domain lost-worker class the gRPC teardown
/// sweep reports — never a hang. The fake worker is a REAL liminal push client
/// that registers in-band, receives the pushed dispatch frame, and then drops
/// its connection without replying (a mid-activity `kill -9` at the wire).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn worker_lost_mid_dispatch_fails_with_the_transport_loss_class() -> Result<(), TestError> {
    let server = RunningServer::start(DEFAULT_WINDOW)?;

    let registration = liminal::protocol::WorkerRegistration {
        namespaces: vec![NAMESPACE.to_owned()],
        task_queue: TASK_QUEUE.to_owned(),
        node: None,
        activity_types: vec![DOOMED.to_owned()],
        identity: "doomed-worker".to_owned(),
        activities: Vec::new(),
    };
    let client = liminal_sdk::PushClient::connect_with_registration(
        &server.address.to_string(),
        registration,
    )
    .map_err(test_error)?;
    server.wait_for_registered_worker(DOOMED)?;

    // Receive the pushed dispatch, then die without replying.
    let doomed = std::thread::spawn(move || -> Result<(), TestError> {
        let _frame = client
            .recv_timeout(Duration::from_secs(10))
            .map_err(test_error)?;
        // Dropping the client joins its reader and closes the connection.
        drop(client);
        Ok(())
    });

    let dispatcher = Arc::new(server.bridge_dispatcher());
    let workflow_id = WorkflowId::new(Uuid::new_v4());
    let result = dispatch_via_seam(
        &dispatcher,
        dispatch_request(&workflow_id, 0, DOOMED, &serde_json::json!({})),
    )
    .await?;

    let reason = result.err().ok_or_else(|| {
        test_error("a dispatch whose worker died mid-flight must fail, not succeed")
    })?;
    // TRANSPORT domain, not the action's. The old contract here was
    // `retryable:`, and that framing WAS the Hit D defect: it named itself
    // retryable while the engine's retry loop, which only retries under an
    // AUTHORED policy, delivered it as a terminal failure whenever the activity
    // had none. The `lost:` class is re-dispatched attempt-neutrally instead.
    assert!(
        reason.starts_with(aion_server::worker::WORKER_LOST_REASON_PREFIX),
        "worker loss must surface in the transport-loss class, never the action's \
         retry vocabulary, got: {reason}"
    );
    assert!(
        reason.contains("lost before reporting activity result"),
        "the failure must be the lost-worker vocabulary, got: {reason}"
    );
    // The failed dispatch cleared its in-flight tracking (the router's
    // complete_task gate ran), leaving nothing for a later sweep.
    assert_eq!(
        server
            .state
            .heartbeat_tracker()
            .in_flight_count()
            .map_err(test_error)?,
        0
    );
    doomed
        .join()
        .map_err(|_| test_error("doomed worker thread panicked"))??;

    server.shutdown()?;
    Ok(())
}

/// CORRELATION: two dispatches in flight against ONE worker each resolve with
/// THEIR handler's result — the pending map plus the correlated push replies
/// never cross two outstanding ordinals.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_dispatches_correlate_replies_to_their_ordinals() -> Result<(), TestError> {
    let server = RunningServer::start(DEFAULT_WINDOW)?;
    let executions = Arc::new(AtomicUsize::new(0));
    let worker = ServedWorker::spawn(
        server.address.to_string(),
        worker_activity_registry(Arc::clone(&executions))?,
    );
    server.wait_for_registered_worker(PROVISION)?;

    let dispatcher = Arc::new(server.bridge_dispatcher());
    let workflow_id = WorkflowId::new(Uuid::new_v4());

    let first = dispatch_via_seam(
        &dispatcher,
        dispatch_request(
            &workflow_id,
            0,
            PROVISION,
            &serde_json::json!({ "resource": "alpha" }),
        ),
    );
    let second = dispatch_via_seam(
        &dispatcher,
        dispatch_request(
            &workflow_id,
            1,
            PROVISION,
            &serde_json::json!({ "resource": "beta" }),
        ),
    );
    let (first, second) = tokio::join!(first, second);

    let first: ProvisionOutput =
        serde_json::from_str(&first?.map_err(test_error)?).map_err(test_error)?;
    let second: ProvisionOutput =
        serde_json::from_str(&second?.map_err(test_error)?).map_err(test_error)?;
    assert_eq!(
        first.resource, "alpha",
        "ordinal 0 must resolve with ITS handler result"
    );
    assert_eq!(
        second.resource, "beta",
        "ordinal 1 must resolve with ITS handler result"
    );
    assert_eq!(executions.load(Ordering::SeqCst), 2);
    // The reply router resolves the waiter immediately before retiring the
    // heartbeat entry, so observe the eventual post-completion invariant rather
    // than racing that adjacent cleanup step.
    server.wait_for_no_in_flight_activities().await?;

    worker.stop()?;
    server.shutdown()?;
    Ok(())
}

/// ATTEMPT PARITY: the engine-provided `attempt` rides the liminal wire and
/// reaches the handler's `ActivityContext` exactly as over gRPC — a retry
/// dispatched over liminal executes as attempt N, never a re-stamped first
/// delivery.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn dispatch_attempt_reaches_the_handler() -> Result<(), TestError> {
    let server = RunningServer::start(DEFAULT_WINDOW)?;
    let executions = Arc::new(AtomicUsize::new(0));
    let worker = ServedWorker::spawn(
        server.address.to_string(),
        worker_activity_registry(executions)?,
    );
    server.wait_for_registered_worker(ATTEMPT_ECHO)?;

    let dispatcher = Arc::new(server.bridge_dispatcher());
    let workflow_id = WorkflowId::new(Uuid::new_v4());

    // A RETRY-shaped dispatch: the engine hands attempt 4 to the seam.
    let request = ActivityDispatch {
        attempt: 4,
        ..dispatch_request(&workflow_id, 0, ATTEMPT_ECHO, &serde_json::json!({}))
    };
    let result = dispatch_via_seam(&dispatcher, request).await?;
    let output: serde_json::Value =
        serde_json::from_str(&result.map_err(test_error)?).map_err(test_error)?;
    assert_eq!(
        output["attempt"], 4,
        "the handler must observe the engine's attempt, not a re-stamped 1"
    );

    worker.stop()?;
    server.shutdown()?;
    Ok(())
}

/// THE FAILURE SIDE: a worker handler returning a retryable `ActivityFailure`
/// surfaces through the bridge as a `retryable:`-prefixed error string — the
/// exact vocabulary the engine seam parses — so retryability classification over
/// the liminal wire is identical to a gRPC completion.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn bridge_dispatch_failure_surfaces_retryable_classification() -> Result<(), TestError> {
    let server = RunningServer::start(DEFAULT_WINDOW)?;
    let executions = Arc::new(AtomicUsize::new(0));
    let worker = ServedWorker::spawn(
        server.address.to_string(),
        worker_activity_registry(executions)?,
    );
    server.wait_for_registered_worker(FLAKY)?;

    let dispatcher = Arc::new(server.bridge_dispatcher());
    let workflow_id = WorkflowId::new(Uuid::new_v4());

    let result = dispatch_via_seam(
        &dispatcher,
        dispatch_request(&workflow_id, 0, FLAKY, &serde_json::json!({})),
    )
    .await?;
    let reason = result
        .err()
        .ok_or_else(|| test_error("a retryably-failing handler must fail the dispatch"))?;
    assert!(
        reason.starts_with("retryable:"),
        "the failure must carry the engine seam's retryable classification, got: {reason}"
    );
    assert!(
        reason.contains(FLAKY_REASON),
        "the handler's failure message must survive the wire, got: {reason}"
    );

    // The failed completion cleared its in-flight entry exactly like a success.
    assert_eq!(
        server
            .state
            .heartbeat_tracker()
            .in_flight_count()
            .map_err(test_error)?,
        0,
        "a failed liminal dispatch must clear its in-flight tracking"
    );

    worker.stop()?;
    server.shutdown()?;
    Ok(())
}

/// THE CONSOLE'S VIEW (NOI-6/NOI-7): a bridge-dispatched liminal activity is
/// bound into the attempt→owner back-index for exactly its in-flight window, so
/// the ops console's live-attempts enumeration (`intervenable_attempts`, the
/// read behind `POST /workflows/attempts`) sees it while it runs and stops
/// seeing it once it resolves. This is the operator-reported regression: the
/// bind existed only on the outbox row arm, so every bridge-dispatched agent
/// step left the console's attempt list empty — no transcript target, no
/// intervention controls.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn bridge_dispatch_is_enumerable_for_intervention_while_in_flight() -> Result<(), TestError> {
    let server = RunningServer::start(DEFAULT_WINDOW)?;
    let executions = Arc::new(AtomicUsize::new(0));
    let worker = ServedWorker::spawn(
        server.address.to_string(),
        worker_activity_registry(executions)?,
    );
    server.wait_for_registered_worker(SLOW)?;

    let dispatcher = Arc::new(server.bridge_dispatcher());
    let workflow_id = WorkflowId::new(Uuid::new_v4());

    // A RETRY-shaped dispatch: the enumeration must expose the engine's real
    // attempt (the key the worker stamps its intervention session with).
    let request = ActivityDispatch {
        attempt: 2,
        ..dispatch_request(
            &workflow_id,
            0,
            SLOW,
            &serde_json::json!({ "resource": "intervenable" }),
        )
    };
    let dispatch = {
        let dispatcher = Arc::clone(&dispatcher);
        tokio::spawn(futures::future::lazy(move |_| dispatcher.dispatch(request)))
    };

    // While the SLOW handler runs, the console's enumeration sees the attempt.
    let router = server.state.intervention_router();
    let deadline = Instant::now() + CONNECT_TIMEOUT;
    let attempts = loop {
        let attempts = router
            .intervenable_attempts(&workflow_id)
            .map_err(test_error)?;
        if !attempts.is_empty() {
            break attempts;
        }
        if Instant::now() > deadline {
            return Err(test_error(
                "the in-flight bridge dispatch never appeared in the live-attempts enumeration",
            ));
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    };
    let (key, _capabilities) = &attempts[0];
    assert_eq!(key.workflow_id, workflow_id);
    assert_eq!(key.activity_id, ActivityId::from_sequence_position(0));
    assert_eq!(
        key.attempt, 2,
        "the enumeration must carry the engine's real attempt, not a re-stamp"
    );

    // Once the dispatch resolves, the binding is released: a finished attempt
    // is never offered as an intervention target.
    let result = tokio::time::timeout(Duration::from_secs(20), dispatch)
        .await
        .map_err(|_| test_error("bridge dispatch did not resolve within the test deadline"))?
        .map_err(test_error)?;
    result.map_err(test_error)?;
    let deadline = Instant::now() + CONNECT_TIMEOUT;
    loop {
        if router
            .intervenable_attempts(&workflow_id)
            .map_err(test_error)?
            .is_empty()
        {
            break;
        }
        if Instant::now() > deadline {
            return Err(test_error(
                "the resolved dispatch was never released from the live-attempts enumeration",
            ));
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    worker.stop()?;
    server.shutdown()?;
    Ok(())
}
