use std::{collections::HashMap, sync::Arc};

use aion_core::{
    ActivityId, DISPLAY_NAME_ATTRIBUTE, Event, EventEnvelope, PackageVersion, Payload, RunId,
    SearchAttributeValue, WorkflowId,
};
use aion_store::{EventStore, WriteToken};
use axum::http::StatusCode;
use chrono::Utc;
use serde_json::{Value, json};
use tower::ServiceExt;

use super::router::workflow_router;
use super::test_support::{
    NAMESPACE, json_request, read_json, runtime_config, server_state, shared_engine,
};
use crate::{
    NamespaceResolver, StaticScheduleNamespaces, StaticWorkflowNamespaces, config::NamespaceMode,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[tokio::test]
async fn parent_run_derives_its_visible_leg_from_recorded_history() -> TestResult {
    let parent_id = WorkflowId::new(uuid::Uuid::from_u128(0x244));
    let parent_run = RunId::new(uuid::Uuid::from_u128(0x2440));
    let child_id = WorkflowId::new(uuid::Uuid::from_u128(0x2441));
    let child_run = RunId::new(uuid::Uuid::from_u128(0x2442));
    let (engine, store, _visibility) = shared_engine().await?;
    seed_parent(&store, &parent_id, &parent_run, &child_id).await?;
    seed_child(&store, &child_id, &child_run).await?;

    let ownership = StaticWorkflowNamespaces::default();
    ownership.record(parent_id.clone(), NAMESPACE)?;
    ownership.record(child_id.clone(), NAMESPACE)?;
    let resolver = NamespaceResolver::from_parts(
        NamespaceMode::SharedEngine,
        Some(engine),
        Arc::new(ownership),
        Arc::new(StaticScheduleNamespaces::default()),
    );
    let router = workflow_router(server_state(resolver, runtime_config()).await?);

    let response = router
        .clone()
        .oneshot(json_request(
            "/workflows/children",
            &json!({
                "namespace": NAMESPACE,
                "workflow_id": parent_id,
                "run_id": parent_run,
            }),
        )?)
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = read_json(response).await?;
    assert_eq!(
        body,
        json!({
            "children": [{
                "workflow_id": child_id,
                "run_id": child_run,
                "workflow_type": "builder_leg",
                "display_name": "Builder L1",
                "status": "Running",
                "current_activity_id": 7,
                "current_attempt": 2,
            }]
        })
    );

    let current_run_response = router
        .oneshot(json_request(
            "/workflows/children",
            &json!({
                "namespace": NAMESPACE,
                "workflow_id": parent_id,
            }),
        )?)
        .await?;
    assert_eq!(current_run_response.status(), StatusCode::OK);
    let current_run_body: Value = read_json(current_run_response).await?;
    assert_eq!(current_run_body, body);
    Ok(())
}

#[tokio::test]
async fn unknown_parent_run_is_not_reported_as_an_empty_child_list() -> TestResult {
    let parent_id = WorkflowId::new(uuid::Uuid::from_u128(0x247));
    let parent_run = RunId::new(uuid::Uuid::from_u128(0x2470));
    let unknown_run = RunId::new(uuid::Uuid::from_u128(0x247f));
    let child_id = WorkflowId::new(uuid::Uuid::from_u128(0x2471));
    let (engine, store, _visibility) = shared_engine().await?;
    seed_parent(&store, &parent_id, &parent_run, &child_id).await?;

    let ownership = StaticWorkflowNamespaces::default();
    ownership.record(parent_id.clone(), NAMESPACE)?;
    let resolver = NamespaceResolver::from_parts(
        NamespaceMode::SharedEngine,
        Some(engine),
        Arc::new(ownership),
        Arc::new(StaticScheduleNamespaces::default()),
    );
    let router = workflow_router(server_state(resolver, runtime_config()).await?);

    let response = router
        .oneshot(json_request(
            "/workflows/children",
            &json!({
                "namespace": NAMESPACE,
                "workflow_id": parent_id,
                "run_id": unknown_run,
            }),
        )?)
        .await?;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    Ok(())
}

#[tokio::test]
async fn explicit_and_implicit_parent_runs_project_different_generations() -> TestResult {
    let parent_id = WorkflowId::new(uuid::Uuid::from_u128(0x248));
    let old_run = RunId::new(uuid::Uuid::from_u128(0x2480));
    let new_run = RunId::new(uuid::Uuid::from_u128(0x2481));
    let old_child = WorkflowId::new(uuid::Uuid::from_u128(0x2482));
    let new_child = WorkflowId::new(uuid::Uuid::from_u128(0x2483));
    let (engine, store, _visibility) = shared_engine().await?;
    seed_parent(&store, &parent_id, &old_run, &old_child).await?;
    store
        .append(
            WriteToken::recorder(),
            &parent_id,
            &[
                started(3, &parent_id, &new_run, "fleet_dev")?,
                Event::ChildWorkflowStarted {
                    envelope: envelope(4, &parent_id),
                    child_workflow_id: new_child.clone(),
                    workflow_type: "new_leg".to_owned(),
                    input: payload()?,
                    package_version: version(),
                },
            ],
            2,
        )
        .await?;

    let ownership = StaticWorkflowNamespaces::default();
    for workflow_id in [&parent_id, &old_child, &new_child] {
        ownership.record(workflow_id.clone(), NAMESPACE)?;
    }
    let resolver = NamespaceResolver::from_parts(
        NamespaceMode::SharedEngine,
        Some(engine),
        Arc::new(ownership),
        Arc::new(StaticScheduleNamespaces::default()),
    );
    let router = workflow_router(server_state(resolver, runtime_config()).await?);

    let explicit = router
        .clone()
        .oneshot(json_request(
            "/workflows/children",
            &json!({"namespace": NAMESPACE, "workflow_id": parent_id, "run_id": old_run}),
        )?)
        .await?;
    assert_eq!(explicit.status(), StatusCode::OK);
    let explicit_body: Value = read_json(explicit).await?;
    assert_eq!(
        explicit_body["children"][0]["workflow_id"],
        json!(old_child)
    );

    let implicit = router
        .oneshot(json_request(
            "/workflows/children",
            &json!({"namespace": NAMESPACE, "workflow_id": parent_id}),
        )?)
        .await?;
    assert_eq!(implicit.status(), StatusCode::OK);
    let implicit_body: Value = read_json(implicit).await?;
    assert_eq!(
        implicit_body["children"][0]["workflow_id"],
        json!(new_child)
    );
    Ok(())
}

#[tokio::test]
async fn unauthorized_child_is_omitted_without_projecting_its_history() -> TestResult {
    let parent_id = WorkflowId::new(uuid::Uuid::from_u128(0x245));
    let parent_run = RunId::new(uuid::Uuid::from_u128(0x2450));
    let child_id = WorkflowId::new(uuid::Uuid::from_u128(0x2451));
    let child_run = RunId::new(uuid::Uuid::from_u128(0x2452));
    let (engine, store, _visibility) = shared_engine().await?;
    seed_parent(&store, &parent_id, &parent_run, &child_id).await?;
    seed_child(&store, &child_id, &child_run).await?;

    let ownership = StaticWorkflowNamespaces::default();
    ownership.record(parent_id.clone(), NAMESPACE)?;
    let resolver = NamespaceResolver::from_parts(
        NamespaceMode::SharedEngine,
        Some(engine),
        Arc::new(ownership),
        Arc::new(StaticScheduleNamespaces::default()),
    );
    let router = workflow_router(server_state(resolver, runtime_config()).await?);

    let response = router
        .oneshot(json_request(
            "/workflows/children",
            &json!({
                "namespace": NAMESPACE,
                "workflow_id": parent_id,
                "run_id": parent_run,
            }),
        )?)
        .await?;

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = read_json(response).await?;
    assert_eq!(
        body,
        json!({
            "children": []
        })
    );
    Ok(())
}

#[tokio::test]
async fn recorded_child_without_a_started_run_has_null_status() -> TestResult {
    let parent_id = WorkflowId::new(uuid::Uuid::from_u128(0x246));
    let parent_run = RunId::new(uuid::Uuid::from_u128(0x2460));
    let child_id = WorkflowId::new(uuid::Uuid::from_u128(0x2461));
    let (engine, store, _visibility) = shared_engine().await?;
    seed_parent(&store, &parent_id, &parent_run, &child_id).await?;

    let ownership = StaticWorkflowNamespaces::default();
    ownership.record(parent_id.clone(), NAMESPACE)?;
    ownership.record(child_id.clone(), NAMESPACE)?;
    let resolver = NamespaceResolver::from_parts(
        NamespaceMode::SharedEngine,
        Some(engine),
        Arc::new(ownership),
        Arc::new(StaticScheduleNamespaces::default()),
    );
    let router = workflow_router(server_state(resolver, runtime_config()).await?);

    let response = router
        .oneshot(json_request(
            "/workflows/children",
            &json!({
                "namespace": NAMESPACE,
                "workflow_id": parent_id,
                "run_id": parent_run,
            }),
        )?)
        .await?;

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = read_json(response).await?;
    assert_eq!(
        body,
        json!({
            "children": [{
                "workflow_id": child_id,
                "run_id": null,
                "workflow_type": "builder_leg",
                "display_name": null,
                "status": null,
                "current_activity_id": null,
                "current_attempt": null,
            }]
        })
    );
    Ok(())
}

#[tokio::test]
async fn duplicate_child_start_is_projected_once_in_first_discovery_position() -> TestResult {
    let parent_id = WorkflowId::new(uuid::Uuid::from_u128(0x249));
    let parent_run = RunId::new(uuid::Uuid::from_u128(0x2490));
    let child_id = WorkflowId::new(uuid::Uuid::from_u128(0x2491));
    let child_run = RunId::new(uuid::Uuid::from_u128(0x2492));
    let (engine, store, _visibility) = shared_engine().await?;
    seed_parent(&store, &parent_id, &parent_run, &child_id).await?;
    store
        .append(
            WriteToken::recorder(),
            &parent_id,
            &[Event::ChildWorkflowStarted {
                envelope: envelope(3, &parent_id),
                child_workflow_id: child_id.clone(),
                workflow_type: "duplicate_should_not_replace".to_owned(),
                input: payload()?,
                package_version: version(),
            }],
            2,
        )
        .await?;
    seed_child(&store, &child_id, &child_run).await?;

    let ownership = StaticWorkflowNamespaces::default();
    ownership.record(parent_id.clone(), NAMESPACE)?;
    ownership.record(child_id.clone(), NAMESPACE)?;
    let resolver = NamespaceResolver::from_parts(
        NamespaceMode::SharedEngine,
        Some(engine),
        Arc::new(ownership),
        Arc::new(StaticScheduleNamespaces::default()),
    );
    let response = workflow_router(server_state(resolver, runtime_config()).await?)
        .oneshot(json_request(
            "/workflows/children",
            &json!({"namespace": NAMESPACE, "workflow_id": parent_id, "run_id": parent_run}),
        )?)
        .await?;

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = read_json(response).await?;
    assert_eq!(body["children"].as_array().map(Vec::len), Some(1));
    assert_eq!(body["children"][0]["workflow_type"], "builder_leg");
    Ok(())
}

async fn seed_parent(
    store: &Arc<dyn EventStore>,
    parent_id: &WorkflowId,
    parent_run: &RunId,
    child_id: &WorkflowId,
) -> TestResult {
    let events = vec![
        started(1, parent_id, parent_run, "fleet_dev")?,
        Event::ChildWorkflowStarted {
            envelope: envelope(2, parent_id),
            child_workflow_id: child_id.clone(),
            workflow_type: "builder_leg".to_owned(),
            input: payload()?,
            package_version: version(),
        },
    ];
    store
        .append(WriteToken::recorder(), parent_id, &events, 0)
        .await?;
    Ok(())
}

async fn seed_child(
    store: &Arc<dyn EventStore>,
    child_id: &WorkflowId,
    child_run: &RunId,
) -> TestResult {
    let mut attributes = HashMap::new();
    attributes.insert(
        DISPLAY_NAME_ATTRIBUTE.to_owned(),
        SearchAttributeValue::String("Builder L1".to_owned()),
    );
    let activity_id = ActivityId::from_sequence_position(7);
    let events = vec![
        started(1, child_id, child_run, "builder_leg")?,
        Event::SearchAttributesUpdated {
            envelope: envelope(2, child_id),
            workflow_id: child_id.clone(),
            attributes,
        },
        Event::ActivityScheduled {
            envelope: envelope(3, child_id),
            activity_id: activity_id.clone(),
            activity_type: "build".to_owned(),
            input: payload()?,
            task_queue: "builders".to_owned(),
            node: None,
        },
        Event::ActivityStarted {
            envelope: envelope(4, child_id),
            activity_id,
            attempt: 2,
        },
    ];
    store
        .append(WriteToken::recorder(), child_id, &events, 0)
        .await?;
    Ok(())
}

fn started(
    seq: u64,
    workflow_id: &WorkflowId,
    run_id: &RunId,
    workflow_type: &str,
) -> Result<Event, aion_core::PayloadError> {
    Ok(Event::WorkflowStarted {
        envelope: envelope(seq, workflow_id),
        workflow_type: workflow_type.to_owned(),
        input: payload()?,
        run_id: run_id.clone(),
        parent_run_id: None,
        package_version: version(),
    })
}

fn envelope(seq: u64, workflow_id: &WorkflowId) -> EventEnvelope {
    EventEnvelope {
        seq,
        recorded_at: Utc::now(),
        workflow_id: workflow_id.clone(),
    }
}

fn payload() -> Result<Payload, aion_core::PayloadError> {
    Payload::from_json(&json!({ "fixture": true }))
}

fn version() -> PackageVersion {
    PackageVersion::new("a".repeat(64))
}
