//! `aion worker status | start | stop | restart`: the managed-worker fleet.
//!
//! These verbs drive the server's supervisor over the operator `DeployService`
//! — the same authority and the same stub the deployment records themselves
//! use, so there is one credential story and one endpoint flag rather than a
//! second one invented for workers.
//!
//! Output is the server's own report, re-shaped only enough to read: desired
//! state and actual state are always printed TOGETHER, because either one
//! alone is the misleading half. "desired: running, state: stopped" is a real
//! and important condition, and a surface that prints only one of them cannot
//! express it.

use aion_proto::generated;
use anyhow::Result;
use serde_json::{Value, json};

use crate::deploy::{DeployTarget, deploy_status_error};

/// `aion worker status`: the whole managed fleet, desired joined with actual.
pub(crate) async fn status(target: &DeployTarget) -> Result<Value> {
    let mut client = target.client().await?;
    let response = client
        .list_managed_workers(target.request(generated::ListManagedWorkersRequest {})?)
        .await
        .map_err(|status| deploy_status_error(&status))?
        .into_inner();
    let workers: Vec<Value> = response.workers.iter().map(worker_json).collect();
    Ok(json!({
        "commissioned": response.commissioned,
        // Named even when absent, so a reader never has to know whether a
        // missing key means "supervised" or "the field was not reported".
        "remedy": response.remedy,
        "workers": workers,
        "undecodable": response.undecodable,
    }))
}

/// `aion worker start <name>`: record the intent durably and supervise it.
pub(crate) async fn start(target: &DeployTarget, name: &str) -> Result<Value> {
    let mut client = target.client().await?;
    let response = client
        .start_managed_worker(target.request(request(name))?)
        .await
        .map_err(|status| deploy_status_error(&status))?
        .into_inner();
    Ok(one(&response, name))
}

/// `aion worker stop <name>`: terminal, and only reported once the server has
/// proven the worker's process group empty.
pub(crate) async fn stop(target: &DeployTarget, name: &str) -> Result<Value> {
    let mut client = target.client().await?;
    let response = client
        .stop_managed_worker(target.request(request(name))?)
        .await
        .map_err(|status| deploy_status_error(&status))?
        .into_inner();
    Ok(one(&response, name))
}

/// `aion worker restart <name>`: stop then start, leaving the intent running.
pub(crate) async fn restart(target: &DeployTarget, name: &str) -> Result<Value> {
    let mut client = target.client().await?;
    let response = client
        .restart_managed_worker(target.request(request(name))?)
        .await
        .map_err(|status| deploy_status_error(&status))?
        .into_inner();
    Ok(one(&response, name))
}

fn request(name: &str) -> generated::ManagedWorkerRequest {
    generated::ManagedWorkerRequest {
        name: name.to_owned(),
    }
}

/// Render one mutation's response, or say plainly that the server returned no
/// worker rather than printing `null` and letting a reader guess.
fn one(response: &generated::ManagedWorkerResponse, name: &str) -> Value {
    response.worker.as_ref().map_or_else(
        || json!({ "name": name, "error": "the server returned no worker for this deployment" }),
        worker_json,
    )
}

fn worker_json(worker: &generated::ManagedWorker) -> Value {
    json!({
        "name": worker.name,
        "task_queue": worker.task_queue,
        "desired": worker.desired,
        "state": worker.state,
        "pid": worker.pid,
        "process_group": worker.process_group,
        "restarts": worker.restarts,
        "last_exit": last_exit(worker),
        "last_error": worker.last_error,
        "deployed_content_hash": worker.deployed_content_hash,
        "spawn_content_hash": worker.spawn_content_hash,
        "spawn_path": worker.spawn_path,
        // The one derived field, and the reason both hashes are printed: a
        // restart that picked up a new binary is an UPGRADE, and an operator
        // reading a status page should not have to diff two hex strings by eye.
        "running_upgraded_binary": upgraded(worker),
    })
}

/// Whether the running process is a DIFFERENT binary from the one the
/// deployment recorded. `null` while nothing has been spawned: "not yet known"
/// and "not upgraded" are different answers.
fn upgraded(worker: &generated::ManagedWorker) -> Value {
    worker
        .spawn_content_hash
        .as_ref()
        .map_or(Value::Null, |spawned| {
            Value::Bool(*spawned != worker.deployed_content_hash)
        })
}

fn last_exit(worker: &generated::ManagedWorker) -> Value {
    worker.last_exit_at.as_ref().map_or(Value::Null, |at| {
        json!({
            "at": at,
            "code": worker.last_exit_code,
            "signal": worker.last_exit_signal,
            "requested": worker.last_exit_requested,
        })
    })
}

#[cfg(test)]
mod tests {
    use aion_proto::generated;

    use super::{one, upgraded, worker_json};

    fn worker() -> generated::ManagedWorker {
        generated::ManagedWorker {
            name: "shells".to_owned(),
            task_queue: "shell".to_owned(),
            desired: "running".to_owned(),
            state: "running".to_owned(),
            pid: Some(11),
            process_group: Some(11),
            restarts: 0,
            last_exit_at: None,
            last_exit_code: None,
            last_exit_signal: None,
            last_exit_requested: false,
            last_error: None,
            deployed_content_hash: "aaaa".to_owned(),
            spawn_content_hash: Some("aaaa".to_owned()),
            spawn_path: Some("/usr/local/bin/aion".to_owned()),
        }
    }

    /// The three answers must be distinguishable: matching hashes, differing
    /// hashes, and nothing spawned yet. Collapsing the third into `false` would
    /// report "not upgraded" about a worker that has never run.
    #[test]
    fn the_upgrade_flag_distinguishes_unknown_from_not_upgraded() {
        assert_eq!(upgraded(&worker()), serde_json::Value::Bool(false));

        let mut upgraded_worker = worker();
        upgraded_worker.spawn_content_hash = Some("bbbb".to_owned());
        assert_eq!(upgraded(&upgraded_worker), serde_json::Value::Bool(true));

        let mut never_run = worker();
        never_run.spawn_content_hash = None;
        assert_eq!(upgraded(&never_run), serde_json::Value::Null);
    }

    /// Desired and actual are printed together: a status line carrying only one
    /// of them cannot express "should be running, is not".
    #[test]
    fn desired_and_actual_state_are_both_rendered() {
        let mut stalled = worker();
        stalled.state = "stopped".to_owned();
        let rendered = worker_json(&stalled);
        assert_eq!(rendered["desired"], "running");
        assert_eq!(rendered["state"], "stopped");
    }

    #[test]
    fn a_response_without_a_worker_says_so_by_name() {
        let rendered = one(&generated::ManagedWorkerResponse { worker: None }, "shells");
        assert_eq!(rendered["name"], "shells");
        assert!(rendered["error"].is_string());
    }

    #[test]
    fn an_exit_renders_every_recorded_field() {
        let mut exited = worker();
        exited.last_exit_at = Some("2026-08-09T00:00:00Z".to_owned());
        exited.last_exit_code = Some(3);
        exited.last_exit_requested = false;
        let rendered = worker_json(&exited);
        assert_eq!(rendered["last_exit"]["code"], 3);
        assert_eq!(rendered["last_exit"]["requested"], false);
        assert_eq!(rendered["last_exit"]["at"], "2026-08-09T00:00:00Z");
    }
}
