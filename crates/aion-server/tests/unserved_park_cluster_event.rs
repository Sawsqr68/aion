//! #266 T4 exhibit — an unbounded park is pushed to the operator, not only logged.
//!
//! The defect class this pins: a dispatch parked with **no availability
//! deadline** is the one selection outcome with no terminal echo. A served
//! dispatch completes; a deadline-bounded park expires into a typed
//! `WorkerUnavailable` refusal that reaches the workflow's history. The
//! unbounded park had only a WARN line and the queryable unserved state —
//! nothing pushed, so the #266 boot-ordering defect stranded recovered runs
//! for days with no operator-visible sign.
//!
//! The desired behaviour: while a dispatch parks unbounded, the wait path
//! emits `ClusterEvent::DispatchParked` on the deployment's cluster channel —
//! once per park episode, again only when the classified reason changes (the
//! same transition rule as the WARN), and never for a deadline-bounded park
//! (whose refusal already reaches history).
//!
//! Both dispatch loops are exercised: the blocking bridge selection wait
//! (`WorkerActivityDispatcher`) and the async gRPC outbox leg
//! (`worker::ActivityDispatcher`) — the leg #87 records as historically
//! invisible. Both share the single `observe_selection_miss` implementation,
//! and both dispatcher structs must thread the publisher for the event to
//! reach the wire, so each is driven through its own public dispatch path.

use std::collections::BTreeMap;
use std::num::NonZeroUsize;
use std::time::Duration;

use aion::{ActivityDispatch, ActivityDispatcher as _};
use aion_core::{ActivityId, ClusterEvent, ContentType, Payload, RunId, WorkflowId};
use aion_server::cluster_publisher::{ClusterEventPublisher, ClusterStreamLagged};
use aion_server::worker::{
    ConnectedWorkerRegistry, HeartbeatTracker, ScheduledActivity, WorkerActivityDispatcher,
};
use futures::StreamExt as _;
use futures::stream::BoxStream;

type TestError = Box<dyn std::error::Error>;

/// The queue nobody serves in these exhibits.
const QUEUE: &str = "nobody-serves-this";
/// The activity the parked dispatch needs served.
const ACTIVITY: &str = "greet";
/// How long to wait for an expected cluster event. The no-runtime park path
/// re-selects every 500ms (`PARK_POLL_INTERVAL`), so this covers many
/// iterations without racing a loaded box.
const EVENT_DEADLINE: Duration = Duration::from_secs(30);
/// How long to listen for events that must NOT arrive. Covers several park
/// iterations (500ms each), so a per-iteration re-emit would land inside it.
const SILENCE_WINDOW: Duration = Duration::from_millis(2_500);

type Subscription = BoxStream<'static, Result<ClusterEvent, ClusterStreamLagged>>;

fn publisher() -> Result<ClusterEventPublisher, TestError> {
    let capacity = NonZeroUsize::new(64).ok_or("capacity must be non-zero")?;
    Ok(ClusterEventPublisher::new(capacity))
}

fn dispatch_to_unserved_queue() -> ActivityDispatch {
    ActivityDispatch {
        namespace: "default".to_owned(),
        task_queue: QUEUE.to_owned(),
        node: None,
        workflow_id: WorkflowId::new_v4(),
        run_id: RunId::new_v4(),
        activity_id: ActivityId::from_sequence_position(0),
        name: ACTIVITY.to_owned(),
        input: "{}".to_owned(),
        config: "{}".to_owned(),
        attempt: 1,
        advisory: false,
        labels: BTreeMap::new(),
    }
}

/// Await the next cluster event within [`EVENT_DEADLINE`].
async fn next_event(subscription: &mut Subscription) -> Result<ClusterEvent, TestError> {
    let event = tokio::time::timeout(EVENT_DEADLINE, subscription.next())
        .await
        .map_err(|_| "no cluster event arrived within the deadline")?
        .ok_or("cluster event stream closed")?
        .map_err(|lag| format!("unexpected lag: {lag:?}"))?;
    Ok(event)
}

/// Assert no further cluster event arrives within [`SILENCE_WINDOW`].
async fn expect_silence(subscription: &mut Subscription) -> Result<(), TestError> {
    match tokio::time::timeout(SILENCE_WINDOW, subscription.next()).await {
        Err(_) => Ok(()),
        Ok(item) => Err(format!("expected no further cluster event, got: {item:?}").into()),
    }
}

/// Release a park by registering a compatible worker whose delivery channel is
/// already closed: the next 500ms selection poll resolves onto it, the hand-off
/// fails on the closed transport, and the dispatch returns instead of waiting
/// forever. The registration is RETURNED, not deregistered here — the parked
/// thread re-selects on a poll interval, so the worker must stay in the pool
/// until the dispatch has actually resolved (join first, then deregister).
fn release_park(
    registry: &ConnectedWorkerRegistry,
) -> Result<aion_server::worker::WorkerRegistration, TestError> {
    let (worker_tx, worker_rx) = tokio::sync::mpsc::channel(1);
    drop(worker_rx);
    let registration = registry.register_namespaces(
        [String::from("default")],
        QUEUE,
        None,
        [String::from(ACTIVITY)].iter(),
        worker_tx,
    )?;
    Ok(registration)
}

/// A dispatch parked with no availability deadline announces itself on the
/// cluster channel exactly once — re-observations of the same reason are
/// silent, matching the WARN's transition rule.
#[tokio::test(flavor = "multi_thread")]
async fn unbounded_park_emits_one_dispatch_parked_event() -> Result<(), TestError> {
    let publisher = publisher()?;
    let mut subscription = publisher.subscribe(0);
    let registry = ConnectedWorkerRegistry::default();
    let dispatcher = WorkerActivityDispatcher::new(
        registry.clone(),
        "default",
        HeartbeatTracker::new(Duration::from_secs(5)),
    )
    .with_cluster_publisher(publisher.clone());

    let dispatch = dispatch_to_unserved_queue();
    let workflow_id = dispatch.workflow_id.clone();
    // A plain OS thread: the bridge park takes its no-runtime path and
    // re-selects every 500ms, so the same NO_LIVE_POLLERS verdict is
    // re-observed many times while we listen.
    let parked = std::thread::spawn(move || dispatcher.dispatch(dispatch));

    let event = next_event(&mut subscription).await?;
    let ClusterEvent::DispatchParked {
        namespace,
        task_queue,
        activity_type,
        node,
        reason,
        policy,
        workflow_id: event_workflow,
        waited_ms: _,
        workers_in_pool,
        compatible_workers,
        ..
    } = event
    else {
        return Err(format!("expected DispatchParked, got: {event:?}").into());
    };
    assert_eq!(namespace, "default");
    assert_eq!(task_queue, QUEUE);
    assert_eq!(activity_type, ACTIVITY);
    assert_eq!(node, None);
    assert_eq!(reason, "NO_LIVE_POLLERS");
    assert_eq!(policy, "strict");
    assert_eq!(event_workflow, workflow_id);
    assert_eq!(workers_in_pool, 0);
    assert_eq!(compatible_workers, 0);

    // Several more park iterations re-observe the same reason: silence.
    expect_silence(&mut subscription).await?;

    let releaser = release_park(&registry)?;
    let outcome = parked
        .join()
        .map_err(|_| "parked dispatch thread panicked")?;
    assert!(
        outcome.is_err(),
        "the released dispatch must resolve via the closed transport: {outcome:?}"
    );
    releaser.deregister()?;
    Ok(())
}

/// A reason change mid-park re-emits with the new verdict — the transition
/// rule cuts both ways: repeats are silent, transitions are stated.
#[tokio::test(flavor = "multi_thread")]
async fn a_reason_change_mid_park_re_emits() -> Result<(), TestError> {
    let publisher = publisher()?;
    let mut subscription = publisher.subscribe(0);
    let registry = ConnectedWorkerRegistry::default();
    let dispatcher = WorkerActivityDispatcher::new(
        registry.clone(),
        "default",
        HeartbeatTracker::new(Duration::from_secs(5)),
    )
    .with_cluster_publisher(publisher.clone());

    let parked = std::thread::spawn(move || dispatcher.dispatch(dispatch_to_unserved_queue()));

    let first = next_event(&mut subscription).await?;
    let ClusterEvent::DispatchParked { reason, .. } = &first else {
        return Err(format!("expected DispatchParked, got: {first:?}").into());
    };
    assert_eq!(reason, "NO_LIVE_POLLERS");

    // A worker joins the pool serving a DIFFERENT activity: the address is
    // still unserved, but the verdict transitions to POLLERS_INCOMPATIBLE.
    // Its channel stays open so it remains registered.
    let (incompatible_tx, _incompatible_rx) = tokio::sync::mpsc::channel(1);
    let incompatible = registry.register_namespaces(
        [String::from("default")],
        QUEUE,
        None,
        [String::from("unrelated")].iter(),
        incompatible_tx,
    )?;

    let second = next_event(&mut subscription).await?;
    let ClusterEvent::DispatchParked {
        reason,
        workers_in_pool,
        compatible_workers,
        ..
    } = &second
    else {
        return Err(format!("expected DispatchParked, got: {second:?}").into());
    };
    assert_eq!(reason, "POLLERS_INCOMPATIBLE");
    assert_eq!(*workers_in_pool, 1);
    assert_eq!(*compatible_workers, 0);

    let releaser = release_park(&registry)?;
    let outcome = parked
        .join()
        .map_err(|_| "parked dispatch thread panicked")?;
    assert!(outcome.is_err(), "released dispatch must resolve: {outcome:?}");
    releaser.deregister()?;
    incompatible.deregister()?;
    Ok(())
}

/// A park bounded by a written service-availability deadline emits nothing:
/// its expiry already produces a typed refusal that reaches the workflow's
/// history, so a wire echo would double-report the same incident.
#[tokio::test(flavor = "multi_thread")]
async fn a_deadline_bounded_park_emits_no_event() -> Result<(), TestError> {
    let publisher = publisher()?;
    let mut subscription = publisher.subscribe(0);
    let registry = ConnectedWorkerRegistry::default();
    let bounded = aion_server::worker::QueueServiceConfig {
        service_availability_deadline: Some(Duration::from_millis(100)),
        ..aion_server::worker::QueueServiceConfig::default()
    };
    let dispatcher = WorkerActivityDispatcher::new(
        registry.clone(),
        "default",
        HeartbeatTracker::new(Duration::from_secs(5)),
    )
    .with_queue_service(bounded)
    .with_cluster_publisher(publisher.clone());

    let parked = std::thread::spawn(move || dispatcher.dispatch(dispatch_to_unserved_queue()));
    let outcome = parked
        .join()
        .map_err(|_| "bounded dispatch thread panicked")?;
    assert!(
        outcome.is_err(),
        "the bounded park must expire into a typed refusal: {outcome:?}"
    );

    // Any emission would have been sent synchronously before the refusal
    // returned, so the channel is already settled: it must be empty.
    expect_silence(&mut subscription).await?;
    Ok(())
}

/// The async gRPC outbox leg parks unbounded by construction (#87) and must
/// announce itself through the same seam — its dispatcher threads the
/// publisher independently of the bridge, so it is driven separately.
#[tokio::test(flavor = "multi_thread")]
async fn the_grpc_outbox_leg_announces_its_park() -> Result<(), TestError> {
    let publisher = publisher()?;
    let mut subscription = publisher.subscribe(0);
    let registry = ConnectedWorkerRegistry::default();
    let dispatcher = aion_server::worker::ActivityDispatcher::new(registry.clone())
        .with_cluster_publisher(publisher.clone());

    let scheduled = ScheduledActivity {
        namespace: "default".to_owned(),
        task_queue: QUEUE.to_owned(),
        activity_type: ACTIVITY.to_owned(),
        node: None,
        workflow_id: WorkflowId::new_v4(),
        activity_id: ActivityId::from_sequence_position(0),
        run_id: Some(RunId::new_v4()),
        input: Payload::new(ContentType::Json, b"{}".to_vec()),
        attempt: 1,
        labels: BTreeMap::new(),
    };
    let workflow_id = scheduled.workflow_id.clone();
    let parked = tokio::spawn(async move { dispatcher.dispatch(&scheduled).await });

    let event = next_event(&mut subscription).await?;
    let ClusterEvent::DispatchParked {
        reason,
        task_queue,
        workflow_id: event_workflow,
        ..
    } = event
    else {
        return Err(format!("expected DispatchParked, got: {event:?}").into());
    };
    assert_eq!(reason, "NO_LIVE_POLLERS");
    assert_eq!(task_queue, QUEUE);
    assert_eq!(event_workflow, workflow_id);

    // Wake the async wait with a compatible worker whose channel is closed:
    // the leg selects it, every candidate stream is closed, and the dispatch
    // resolves with its existing closed-transport failure.
    let (worker_tx, worker_rx) = tokio::sync::mpsc::channel(1);
    drop(worker_rx);
    let registration = registry.register_namespaces(
        [String::from("default")],
        QUEUE,
        None,
        [String::from(ACTIVITY)].iter(),
        worker_tx,
    )?;
    let outcome = tokio::time::timeout(EVENT_DEADLINE, parked)
        .await
        .map_err(|_| "released outbox-leg dispatch did not resolve")??;
    assert!(outcome.is_err(), "released dispatch must resolve: {outcome:?}");
    registration.deregister()?;
    Ok(())
}
