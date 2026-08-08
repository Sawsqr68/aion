//! Shared worker-deployment persistence scenarios.

use std::collections::BTreeSet;
use std::sync::Arc;

use chrono::{TimeZone, Utc};

use super::{WorkerDeploymentRawWrite, WorkerDeploymentReopen};
use crate::{
    DeployedBinaryIdentity, DesiredState, NewWorkerDeployment, PutOutcome, StoreError,
    WorkerArtifactRef, WorkerDeployment, WorkerDeploymentStore,
};

pub(crate) async fn run(
    store: Arc<dyn WorkerDeploymentStore>,
    reopen: Option<WorkerDeploymentReopen>,
    write_raw: WorkerDeploymentRawWrite,
) -> Result<(), StoreError> {
    round_trip_and_nothing_spawns(&store).await?;
    replace(&store).await?;
    desired_state_flip_appends_history(&store).await?;
    absent_answers_and_delete(&store).await?;
    list_ordering(&store).await?;
    unknown_artifact_tag_refuses_decode()?;
    poisoned_row_is_visible_and_removable(&store, &write_raw).await?;
    drop(write_raw);
    restart_survival(store, reopen).await
}

fn deployment(name: &str, desired: DesiredState) -> Result<WorkerDeployment, StoreError> {
    let now = Utc
        .with_ymd_and_hms(2026, 8, 8, 12, 0, 0)
        .single()
        .ok_or_else(|| StoreError::Backend("conformance instant is invalid".to_owned()))?;
    WorkerDeployment::new(
        NewWorkerDeployment {
            name: name.to_owned(),
            artifact: WorkerArtifactRef::Builtin {
                verb: vec!["worker".to_owned(), "shell".to_owned()],
            },
            binary: DeployedBinaryIdentity {
                version: "1.0.0".to_owned(),
                commit: "commit-a".to_owned(),
                dirty: "false".to_owned(),
                content_hash: "hash-a".to_owned(),
            },
            namespaces: BTreeSet::from(["orders".to_owned(), "billing".to_owned()]),
            task_queue: "shell".to_owned(),
            node: Some("node-a".to_owned()),
            desired,
        },
        now,
    )
    .map_err(|error| StoreError::Serialization(error.to_string()))
}

async fn round_trip_and_nothing_spawns(
    store: &Arc<dyn WorkerDeploymentStore>,
) -> Result<(), StoreError> {
    let expected = deployment("suite-round-trip", DesiredState::Running)?;
    let result = store.put_worker_deployment(expected.clone()).await?;
    require(
        result.outcome == PutOutcome::Created,
        "first deployment put must report created",
    )?;
    require(
        result.deployment == expected,
        "first deployment put must return the exact persisted record",
    )?;
    let actual = store.get_worker_deployment(&expected.name).await?;
    require(
        actual.as_ref() == Some(&result.deployment),
        "deployment returned by put must equal the subsequent load",
    )?;
    require(
        actual.and_then(|record| record.last_spawn_binary).is_none(),
        "W-0 must never stamp last_spawn_binary",
    )
}

async fn replace(store: &Arc<dyn WorkerDeploymentStore>) -> Result<(), StoreError> {
    let mut previous = deployment("suite-replace", DesiredState::Running)?;
    previous.last_spawn_binary = Some(DeployedBinaryIdentity {
        version: "0.9.0".to_owned(),
        commit: "spawn-commit".to_owned(),
        dirty: "false".to_owned(),
        content_hash: "spawn-hash".to_owned(),
    });
    previous.change_desired_state(DesiredState::Stopped, previous.updated_at);
    store.put_worker_deployment(previous.clone()).await?;

    let mut replacement = deployment("suite-replace", DesiredState::Running)?;
    "replacement-queue".clone_into(&mut replacement.task_queue);
    "hash-b".clone_into(&mut replacement.binary.content_hash);
    let result = store.put_worker_deployment(replacement).await?;
    require(
        result.outcome == PutOutcome::Replaced,
        "second deployment put must report replaced",
    )?;
    let actual = result.deployment;
    let loaded = store.get_worker_deployment("suite-replace").await?;
    require(
        loaded.as_ref() == Some(&actual),
        "merged deployment returned by put must equal the subsequent load",
    )?;
    require(
        actual.created_at == previous.created_at,
        "replacement must preserve created_at",
    )?;
    require(
        actual.updated_at != previous.updated_at,
        "replacement must move updated_at",
    )?;
    require(
        actual.last_spawn_binary == previous.last_spawn_binary,
        "replacement must preserve last_spawn_binary",
    )?;
    require(
        actual.status_history.len() == previous.status_history.len() + 1,
        "replacement must append exactly one history entry",
    )?;
    require(
        actual.status_history[..previous.status_history.len()] == previous.status_history,
        "replacement must carry forward prior history",
    )?;
    require(
        actual
            .status_history
            .last()
            .map(|entry| entry.status.as_str())
            == Some(WorkerDeployment::REPLACED_STATUS),
        "replacement must append the stable replaced status",
    )?;
    require(
        actual.task_queue == "replacement-queue" && actual.binary.content_hash == "hash-b",
        "replacement must apply newly authored fields",
    )
}

async fn desired_state_flip_appends_history(
    store: &Arc<dyn WorkerDeploymentStore>,
) -> Result<(), StoreError> {
    let record = deployment("suite-desired", DesiredState::Running)?;
    store.put_worker_deployment(record).await?;
    let changed = store
        .set_desired_state("suite-desired", DesiredState::Stopped)
        .await?
        .ok_or_else(|| StoreError::Backend("existing deployment disappeared".to_owned()))?;
    require(
        changed.desired == DesiredState::Stopped,
        "desired state must change",
    )?;
    require(
        changed.status_history.len() == 2,
        "desired change must append history",
    )?;
    require(
        changed.status_history[1].status == WorkerDeployment::DESIRED_STATE_CHANGED_STATUS,
        "desired change must append the stable status token",
    )
}

async fn absent_answers_and_delete(
    store: &Arc<dyn WorkerDeploymentStore>,
) -> Result<(), StoreError> {
    require(
        store.get_worker_deployment("suite-absent").await?.is_none(),
        "absent get must return none",
    )?;
    require(
        store
            .set_desired_state("suite-absent", DesiredState::Stopped)
            .await?
            .is_none(),
        "absent desired-state update must return none",
    )?;
    let absent_delete = store.delete_worker_deployment("suite-absent").await?;
    require(
        !absent_delete.existed && absent_delete.deployment.is_none(),
        "absent delete must report absent with no deployment",
    )?;
    let expected = deployment("suite-delete", DesiredState::Stopped)?;
    store.put_worker_deployment(expected.clone()).await?;
    let deleted = store.delete_worker_deployment("suite-delete").await?;
    require(
        deleted.existed && deleted.deployment.as_ref() == Some(&expected),
        "existing delete must return the decoded removed deployment",
    )?;
    require(
        store.get_worker_deployment("suite-delete").await?.is_none(),
        "deleted deployment must be gone",
    )
}

async fn list_ordering(store: &Arc<dyn WorkerDeploymentStore>) -> Result<(), StoreError> {
    for name in ["suite-list-c", "suite-list-a", "suite-list-b"] {
        store
            .put_worker_deployment(deployment(name, DesiredState::Running)?)
            .await?;
    }
    let listing = store.list_worker_deployments().await?;
    require(listing.undecodable.is_empty(), "valid rows must all decode")?;
    let names: Vec<String> = listing
        .deployments
        .into_iter()
        .filter(|record| record.name.starts_with("suite-list-"))
        .map(|record| record.name)
        .collect();
    require(
        names == ["suite-list-a", "suite-list-b", "suite-list-c"],
        "deployment list must be ordered by name",
    )
}

fn unknown_artifact_tag_refuses_decode() -> Result<(), StoreError> {
    let bytes = deployment("suite-unknown-tag", DesiredState::Running)?.encode()?;
    let mut value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|error| StoreError::Serialization(error.to_string()))?;
    value["artifact"]["type"] = serde_json::Value::String("unknown-kind".to_owned());
    let mutated =
        serde_json::to_vec(&value).map_err(|error| StoreError::Serialization(error.to_string()))?;
    require(
        matches!(
            WorkerDeployment::decode(&mutated),
            Err(StoreError::Serialization(_))
        ),
        "unknown artifact tags must refuse to decode with a typed error",
    )
}

async fn poisoned_row_is_visible_and_removable(
    store: &Arc<dyn WorkerDeploymentStore>,
    write_raw: &WorkerDeploymentRawWrite,
) -> Result<(), StoreError> {
    let good = deployment("suite-poison-good", DesiredState::Running)?;
    store.put_worker_deployment(good.clone()).await?;
    write_raw(
        "suite-poison-bad".to_owned(),
        b"{not-worker-deployment-json".to_vec(),
    )
    .await?;

    let listing = store.list_worker_deployments().await?;
    require(
        listing.deployments.iter().any(|record| record == &good),
        "a poisoned row must not hide decodable rows",
    )?;
    require(
        listing
            .undecodable
            .iter()
            .any(|row| row.name == "suite-poison-bad" && !row.error.trim().is_empty()),
        "a poisoned row must be named with its decode error",
    )?;

    let deleted = store.delete_worker_deployment("suite-poison-bad").await?;
    require(
        deleted.existed && deleted.deployment.is_none(),
        "delete must remove a poisoned row without decoding it",
    )?;
    require(
        !store
            .list_worker_deployments()
            .await?
            .undecodable
            .iter()
            .any(|row| row.name == "suite-poison-bad"),
        "deleted poisoned row must disappear from listings",
    )?;

    write_raw(
        "suite-poison-bad".to_owned(),
        b"still-not-worker-deployment-json".to_vec(),
    )
    .await?;
    let replacement = deployment("suite-poison-bad", DesiredState::Stopped)?;
    let result = store.put_worker_deployment(replacement.clone()).await?;
    require(
        result.outcome == PutOutcome::Replaced,
        "put over a poisoned key must report replaced",
    )?;
    require(
        result.deployment == replacement,
        "put over a poisoned key must return the fresh persisted record",
    )?;
    require(
        store
            .get_worker_deployment("suite-poison-bad")
            .await?
            .as_ref()
            == Some(&replacement),
        "put over a poisoned key must install a fresh decodable record",
    )
}

async fn restart_survival(
    store: Arc<dyn WorkerDeploymentStore>,
    reopen: Option<WorkerDeploymentReopen>,
) -> Result<(), StoreError> {
    let expected = deployment("suite-restart", DesiredState::Stopped)?;
    store.put_worker_deployment(expected.clone()).await?;
    let Some(reopen) = reopen else {
        println!(
            "worker-deployment restart scenario skipped: in-memory backend has no reopenable storage"
        );
        return Ok(());
    };
    drop(store);
    let second_store = reopen().await?;
    let actual = second_store.get_worker_deployment("suite-restart").await?;
    require(
        actual.as_ref() == Some(&expected),
        "a second store instance over the same storage must see the complete record",
    )
}

fn require(condition: bool, message: &str) -> Result<(), StoreError> {
    if condition {
        Ok(())
    } else {
        Err(StoreError::Backend(message.to_owned()))
    }
}
