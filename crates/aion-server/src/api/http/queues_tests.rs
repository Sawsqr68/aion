//! Tests for the fleet-wide unserved-queue read.

use std::sync::Arc;

use aion_core::{ActivityId, WorkflowId};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use super::super::router::workflow_router;
use super::super::test_support::{runtime_config, server_state, shared_engine};
use crate::worker::{
    PoolCensus, QueueServicePolicy, QueueServiceReason, ServiceAddress, queue_service,
};
use crate::{
    NamespaceResolver, ServerState, StaticScheduleNamespaces, StaticWorkflowNamespaces,
    config::NamespaceMode,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const NAMESPACE: &str = "default";

async fn state() -> Result<ServerState, Box<dyn std::error::Error>> {
    let (engine, store, visibility) = shared_engine().await?;
    std::hint::black_box((store, visibility));
    let resolver = NamespaceResolver::from_parts(
        NamespaceMode::SharedEngine,
        Some(engine),
        Arc::new(StaticWorkflowNamespaces::default()),
        Arc::new(StaticScheduleNamespaces::default()),
    );
    let mut runtime = runtime_config();
    runtime.auth.enabled = false;
    server_state(resolver, runtime).await
}

async fn read(
    state: ServerState,
) -> Result<(StatusCode, serde_json::Value), Box<dyn std::error::Error>> {
    let response = workflow_router(state)
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/queues/unserved")
                .body(Body::empty())?,
        )
        .await?;
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await?;
    Ok((status, serde_json::from_slice(&bytes)?))
}

/// The control, and it runs first: a server with nothing parked answers with an
/// empty list, not an error and not a phantom entry.
#[tokio::test]
async fn a_calm_server_reports_no_unserved_queues() -> TestResult {
    let (status, body) = read(state().await?).await?;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, serde_json::json!([]));
    Ok(())
}

/// A parked dispatch is readable, with the reason, the sentence, the census, and
/// the run waiting on it — the answer to "which runs are stuck, on what" that
/// until now existed in the server and reached no surface at all.
#[tokio::test]
async fn a_parked_dispatch_is_readable_with_its_reason_and_its_waiters() -> TestResult {
    let state = state().await?;
    let workflow_id = WorkflowId::new_v4();
    let activity_id = ActivityId::from_sequence_position(3);
    let address = ServiceAddress {
        namespace: String::from(NAMESPACE),
        task_queue: String::from("nobody-serves-this"),
        activity_type: String::from("greet"),
        node: None,
    };
    state
        .queue_service_state()
        .mark(queue_service::state::Parked {
            address: &address,
            reason: QueueServiceReason::NoLivePollers,
            policy: QueueServicePolicy::Strict,
            census: PoolCensus::default(),
            workflow_id: &workflow_id,
            activity_id: &activity_id,
        })?;

    let (status, body) = read(state).await?;

    assert_eq!(status, StatusCode::OK);
    let rows = body.as_array().ok_or("response is not an array")?;
    assert_eq!(rows.len(), 1, "{body}");
    let row = &rows[0];
    assert_eq!(row["namespace"], NAMESPACE);
    assert_eq!(row["task_queue"], "nobody-serves-this");
    assert_eq!(row["activity_type"], "greet");
    // The vocabulary is the taxonomy's own, computed from the same source the
    // handler reads rather than pinned as a literal.
    assert_eq!(row["reason"], QueueServiceReason::NoLivePollers.as_str());
    assert_eq!(row["policy"], QueueServicePolicy::Strict.as_str());
    assert_eq!(
        row["detail"],
        QueueServiceReason::NoLivePollers.explain(&address)
    );
    assert_eq!(row["workers_in_pool"], 0);
    let waiting = row["waiting"].as_array().ok_or("waiting is not an array")?;
    assert_eq!(waiting.len(), 1, "{row}");
    assert_eq!(waiting[0]["workflow_id"], workflow_id.to_string());
    assert_eq!(waiting[0]["activity_id"], activity_id.to_string());
    Ok(())
}

/// Clearing the last waiter clears the address, and the read agrees — so the
/// surface reports a live condition rather than an ever-growing scar list.
#[tokio::test]
async fn a_resolved_dispatch_leaves_the_read() -> TestResult {
    let state = state().await?;
    let workflow_id = WorkflowId::new_v4();
    let activity_id = ActivityId::from_sequence_position(0);
    let address = ServiceAddress {
        namespace: String::from(NAMESPACE),
        task_queue: String::from("nobody-serves-this"),
        activity_type: String::from("greet"),
        node: None,
    };
    let queue_state = state.queue_service_state().clone();
    queue_state.mark(queue_service::state::Parked {
        address: &address,
        reason: QueueServiceReason::NoLivePollers,
        policy: QueueServicePolicy::Strict,
        census: PoolCensus::default(),
        workflow_id: &workflow_id,
        activity_id: &activity_id,
    })?;
    // Non-vacuity: it really was reported before it was cleared.
    let (_, before) = read(state.clone()).await?;
    assert_eq!(before.as_array().map(Vec::len), Some(1), "{before}");

    queue_state.clear(&address, &workflow_id, &activity_id)?;
    let (status, after) = read(state).await?;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(after, serde_json::json!([]));
    Ok(())
}
