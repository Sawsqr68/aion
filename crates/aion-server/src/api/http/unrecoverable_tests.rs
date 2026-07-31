//! Tests for the fleet-wide degraded-residency read (#117).

use std::sync::Arc;

use aion::registry::UnrecoverableRun;
use aion_core::WorkflowId;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono::{DateTime, TimeDelta, Utc};
use tower::ServiceExt;

use super::super::router::workflow_router;
use super::super::test_support::{runtime_config, server_state, shared_engine};
use super::collect;
use crate::{
    CallerIdentity, NamespaceResolver, ServerState, StaticScheduleNamespaces,
    StaticWorkflowNamespaces, config::NamespaceMode,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const NAMESPACE: &str = "default";
const OTHER_NAMESPACE: &str = "tenant-b";

/// The state, its engine, and the ownership fixture the namespace attributions
/// are written into — the handler reads all three and a test needs to seed two
/// of them.
struct Fleet {
    state: ServerState,
    engine: Arc<aion::Engine>,
    ownership: Arc<StaticWorkflowNamespaces>,
}

async fn fleet() -> Result<Fleet, Box<dyn std::error::Error>> {
    let (engine, store, visibility) = shared_engine().await?;
    std::hint::black_box((store, visibility));
    let ownership = Arc::new(StaticWorkflowNamespaces::default());
    let resolver = NamespaceResolver::from_parts(
        NamespaceMode::SharedEngine,
        Some(Arc::clone(&engine)),
        ownership.clone(),
        Arc::new(StaticScheduleNamespaces::default()),
    );
    let mut runtime = runtime_config();
    runtime.auth.enabled = false;
    let state = server_state(resolver, runtime).await?;
    Ok(Fleet {
        state,
        engine,
        ownership,
    })
}

/// Deterministic observation stamps: the ordering assertion is about the field,
/// so the values must not depend on when the test ran.
///
/// Built by offsetting the epoch rather than by calendar construction. A
/// calendar constructor returns an `Option`, and the only non-panicking way to
/// unwrap it collapses every failure to one default value — which would make
/// two stamps EQUAL, drop the ordering test onto its random-uuid tiebreak, and
/// flake instead of failing. This form cannot produce that.
fn observed(minute: i64) -> DateTime<Utc> {
    DateTime::UNIX_EPOCH + TimeDelta::minutes(minute)
}

fn entry(reason: &str, minute: i64) -> UnrecoverableRun {
    UnrecoverableRun {
        workflow_type: String::from("rig$deadbeef"),
        reason: reason.to_owned(),
        observed_at: observed(minute),
    }
}

/// Record one degraded run, attributed to `namespace` unless it is `None`.
fn degrade(
    fleet: &Fleet,
    workflow_id: &WorkflowId,
    namespace: Option<&str>,
    entry: UnrecoverableRun,
) -> TestResult {
    if let Some(namespace) = namespace {
        fleet.ownership.record(workflow_id.clone(), namespace)?;
    }
    fleet
        .engine
        .registry()
        .unrecoverable()
        .record(workflow_id.clone(), entry)?;
    Ok(())
}

async fn read(
    state: ServerState,
) -> Result<(StatusCode, serde_json::Value), Box<dyn std::error::Error>> {
    let response = workflow_router(state)
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/workflows/unrecoverable")
                .body(Body::empty())?,
        )
        .await?;
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await?;
    Ok((status, serde_json::from_slice(&bytes)?))
}

/// The control, and it runs first: an engine that recovered everything answers
/// with an empty list, not an error and not a phantom row.
#[tokio::test]
async fn a_healthy_engine_reports_no_unrecoverable_runs() -> TestResult {
    let (status, body) = read(fleet().await?.state).await?;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, serde_json::json!([]));
    Ok(())
}

/// The whole point of the endpoint: the id an operator could not obtain any
/// other way, with the reason recovery recorded and a remedy that names both
/// options.
#[tokio::test]
async fn a_degraded_run_is_readable_with_its_id_reason_and_remedy() -> TestResult {
    let fleet = fleet().await?;
    let workflow_id = WorkflowId::new_v4();
    degrade(
        &fleet,
        &workflow_id,
        Some(NAMESPACE),
        entry("pinned version 13958627 is not loaded", 0),
    )?;

    let (status, body) = read(fleet.state).await?;

    assert_eq!(status, StatusCode::OK);
    let rows = body.as_array().ok_or("response is not an array")?;
    assert_eq!(rows.len(), 1, "{body}");
    let row = &rows[0];
    assert_eq!(row["workflow_id"], workflow_id.to_string());
    assert_eq!(row["namespace"], NAMESPACE);
    assert_eq!(row["workflow_type"], "rig$deadbeef");
    assert_eq!(row["reason"], "pinned version 13958627 is not loaded");
    assert_eq!(row["observed_at"], observed(0).to_rfc3339());
    // Both remedies must be named. A list that only offered `cancel` would push
    // operators toward destroying runs a redeploy would have saved.
    let remedy = row["remedy"].as_str().ok_or("remedy is not a string")?;
    assert!(remedy.contains("redeploy"), "{remedy}");
    assert!(remedy.contains("cancel"), "{remedy}");
    Ok(())
}

/// The staleness guard, end to end: an entry cleared because the engine observed
/// the run resident leaves the read. Without this the surface would accumulate a
/// scar list and send operators to redeploy healthy runs.
#[tokio::test]
async fn a_recovered_run_leaves_the_read() -> TestResult {
    let fleet = fleet().await?;
    let workflow_id = WorkflowId::new_v4();
    degrade(
        &fleet,
        &workflow_id,
        Some(NAMESPACE),
        entry("pinned version is not loaded", 0),
    )?;
    // Non-vacuity: it really was reported before it was cleared.
    let (_, before) = read(fleet.state.clone()).await?;
    assert_eq!(before.as_array().map(Vec::len), Some(1), "{before}");

    assert!(
        fleet
            .engine
            .registry()
            .unrecoverable()
            .clear(&workflow_id)?,
        "the fixture must really have removed an entry"
    );
    let (status, after) = read(fleet.state).await?;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(after, serde_json::json!([]));
    Ok(())
}

/// Rows come back oldest observation first, so the run that has been broken
/// longest is the one an operator reads first.
#[tokio::test]
async fn rows_are_ordered_by_when_the_failure_was_observed() -> TestResult {
    let fleet = fleet().await?;
    let newest = WorkflowId::new_v4();
    let oldest = WorkflowId::new_v4();
    // Recorded newest-first, so passing by luck of insertion order is not
    // available to the implementation.
    degrade(&fleet, &newest, Some(NAMESPACE), entry("second", 9))?;
    degrade(&fleet, &oldest, Some(NAMESPACE), entry("first", 1))?;

    let (_, body) = read(fleet.state).await?;

    let rows = body.as_array().ok_or("response is not an array")?;
    assert_eq!(rows.len(), 2, "{body}");
    assert_eq!(rows[0]["workflow_id"], oldest.to_string(), "{body}");
    assert_eq!(rows[1]["workflow_id"], newest.to_string(), "{body}");
    Ok(())
}

/// The existence-leak boundary. A caller granted one namespace sees its own
/// degraded run and is never told the other tenant's exists.
///
/// This goes through `collect` rather than the router because the HTTP surface
/// with auth off resolves every caller to the all-access operator; minting a
/// scoped credential would exercise the token path instead of the filter under
/// test.
#[tokio::test]
async fn a_caller_never_learns_of_a_degraded_run_it_cannot_access() -> TestResult {
    let fleet = fleet().await?;
    let mine = WorkflowId::new_v4();
    let theirs = WorkflowId::new_v4();
    degrade(&fleet, &mine, Some(NAMESPACE), entry("mine", 0))?;
    degrade(&fleet, &theirs, Some(OTHER_NAMESPACE), entry("theirs", 1))?;

    let caller = CallerIdentity::new("tenant-a-subject", [String::from(NAMESPACE)]);
    let rows = collect(&fleet.state, &caller).await?;

    let ids: Vec<&str> = rows.iter().map(|row| row.workflow_id.as_str()).collect();
    assert_eq!(
        ids,
        vec![mine.to_string().as_str()],
        "a tenant must see its own degraded run and only that"
    );

    // The discriminating half: the same fleet, read by the operator, really does
    // hold both — so the filter above dropped a row that was there to drop.
    let operator = CallerIdentity::operator("operator");
    assert_eq!(
        collect(&fleet.state, &operator).await?.len(),
        2,
        "the fixture must hold both runs, or the tenant assertion is vacuous"
    );
    Ok(())
}

/// A run that recorded no owning namespace is visible to the operator and to
/// nobody else.
///
/// Both halves are load-bearing. Hiding it from everyone would recreate the
/// original defect for exactly the runs most likely to hit it — an unattributed
/// run matches no enumerated grant, so it would be permanently invisible to the
/// only person who can fix it. Showing it to an enumerated caller would leak a
/// run outside their tenancy.
#[tokio::test]
async fn an_unattributed_run_reaches_the_operator_and_stops_there() -> TestResult {
    let fleet = fleet().await?;
    let orphan = WorkflowId::new_v4();
    degrade(&fleet, &orphan, None, entry("no namespace recorded", 0))?;

    let operator = CallerIdentity::operator("operator");
    let seen = collect(&fleet.state, &operator).await?;
    assert_eq!(seen.len(), 1, "the operator must see an unattributed run");
    assert_eq!(seen[0].workflow_id, orphan.to_string());
    assert!(
        seen[0].namespace.is_none(),
        "an unattributed run must report a null namespace rather than inventing one"
    );

    let tenant = CallerIdentity::new("tenant-a-subject", [String::from(NAMESPACE)]);
    assert!(
        collect(&fleet.state, &tenant).await?.is_empty(),
        "an enumerated caller must not be told an unattributed run exists"
    );
    Ok(())
}
