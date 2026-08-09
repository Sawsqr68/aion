//! The one managed-worker projection both transports serve.
//!
//! `GET /workers/managed` and `DeployService.ListManagedWorkers` answer the
//! same question, so they must not be two answers. Both encode the SAME
//! [`ManagedWorkerReport`] the supervisor produces, through the functions here:
//! a status field added to one transport and forgotten on the other is a
//! divergence nothing would catch, and this module is what removes the chance.

use aion_proto::{WireError, generated};

use crate::worker::supervisor::{ManagedWorkerReport, ManagedWorkerStatus, SupervisionError};

/// Map a supervision failure onto the wire, preserving its remedy.
///
/// The `NotCommissioned` message IS the operator remedy, so it must survive
/// the mapping verbatim; losing it would leave a caller with a status code and
/// nothing to do about it.
pub(crate) fn wire_error(error: &SupervisionError) -> WireError {
    let message = error.to_string();
    match error {
        SupervisionError::UnknownDeployment { .. } => WireError::not_found(message),
        SupervisionError::NotCommissioned => WireError::invalid_state(message),
        _ => WireError::backend(message),
    }
}

/// Encode one managed worker for the wire.
pub(crate) fn managed_worker(status: ManagedWorkerStatus) -> generated::ManagedWorker {
    let (last_exit_at, last_exit_code, last_exit_signal, last_exit_requested) =
        status.last_exit.map_or((None, None, None, false), |exit| {
            (
                Some(exit.at.to_rfc3339()),
                exit.code,
                exit.signal,
                exit.requested,
            )
        });
    let (spawn_content_hash, spawn_path) = status.spawn_binary.map_or((None, None), |binary| {
        (Some(binary.content_hash), Some(binary.path))
    });
    generated::ManagedWorker {
        name: status.name,
        task_queue: status.task_queue,
        desired: status.desired.token().to_owned(),
        state: status.state.token().to_owned(),
        pid: status.pid,
        process_group: status.process_group,
        restarts: status.restarts,
        last_exit_at,
        last_exit_code,
        last_exit_signal,
        last_exit_requested,
        last_error: status.last_error,
        deployed_content_hash: status.deployed_binary.content_hash,
        spawn_content_hash,
        spawn_path,
    }
}

/// Encode the whole fleet report for the wire.
pub(crate) fn managed_worker_report(
    report: ManagedWorkerReport,
) -> generated::ListManagedWorkersResponse {
    generated::ListManagedWorkersResponse {
        commissioned: report.commissioned,
        remedy: report.remedy,
        workers: report.workers.into_iter().map(managed_worker).collect(),
        undecodable: report.undecodable,
    }
}

#[cfg(test)]
mod tests {
    use aion_store::{DeployedBinaryIdentity, DesiredState};

    use super::{managed_worker, wire_error};
    use crate::worker::supervisor::{
        ManagedWorkerExit, ManagedWorkerState, ManagedWorkerStatus, SpawnedBinary, SupervisionError,
    };

    fn status() -> ManagedWorkerStatus {
        ManagedWorkerStatus {
            name: "shells".to_owned(),
            task_queue: "shell".to_owned(),
            desired: DesiredState::Running,
            state: ManagedWorkerState::Running,
            pid: Some(4242),
            process_group: Some(4242),
            restarts: 2,
            last_exit: Some(ManagedWorkerExit {
                at: chrono::Utc::now(),
                code: Some(7),
                signal: None,
                requested: false,
            }),
            last_error: Some("something to say".to_owned()),
            deployed_binary: DeployedBinaryIdentity {
                version: "1.0.0".to_owned(),
                commit: "abc".to_owned(),
                dirty: "false".to_owned(),
                content_hash: "deployed".to_owned(),
            },
            spawn_binary: Some(SpawnedBinary {
                path: "/usr/local/bin/aion".to_owned(),
                content_hash: "spawned".to_owned(),
                version: Some("1.1.0".to_owned()),
                commit: Some("def".to_owned()),
                dirty: Some("false".to_owned()),
            }),
        }
    }

    /// The upgrade signal is the two hashes DIFFERING, so both must reach the
    /// wire. Encoding only one of them would make an upgraded restart look
    /// identical to a calm one.
    #[test]
    fn both_binary_identities_survive_the_encoding() {
        let encoded = managed_worker(status());
        assert_eq!(encoded.deployed_content_hash, "deployed");
        assert_eq!(encoded.spawn_content_hash.as_deref(), Some("spawned"));
        assert_eq!(encoded.spawn_path.as_deref(), Some("/usr/local/bin/aion"));
    }

    #[test]
    fn every_reported_field_reaches_the_wire() {
        let encoded = managed_worker(status());
        assert_eq!(encoded.name, "shells");
        assert_eq!(encoded.task_queue, "shell");
        assert_eq!(encoded.desired, "running");
        assert_eq!(encoded.state, "running");
        assert_eq!(encoded.pid, Some(4242));
        assert_eq!(encoded.process_group, Some(4242));
        assert_eq!(encoded.restarts, 2);
        assert_eq!(encoded.last_exit_code, Some(7));
        assert_eq!(encoded.last_exit_signal, None);
        assert!(!encoded.last_exit_requested);
        assert!(encoded.last_exit_at.is_some());
        assert_eq!(encoded.last_error.as_deref(), Some("something to say"));
    }

    /// A worker that has never run encodes as absence, not as zeroes that read
    /// like a real observation.
    #[test]
    fn a_never_run_worker_encodes_absence_rather_than_defaults() {
        let mut status = status();
        status.last_exit = None;
        status.spawn_binary = None;
        status.pid = None;
        status.process_group = None;
        let encoded = managed_worker(status);
        assert_eq!(encoded.last_exit_at, None);
        assert_eq!(encoded.last_exit_code, None);
        assert_eq!(encoded.spawn_content_hash, None);
        assert_eq!(encoded.pid, None);
    }

    #[test]
    fn the_uncommissioned_remedy_survives_the_wire_mapping() {
        let error = wire_error(&SupervisionError::NotCommissioned);
        assert!(error.message.contains("[worker_supervision]"));
        assert!(error.message.contains("stop_grace_ms"));
    }

    #[test]
    fn an_unknown_deployment_maps_to_not_found() {
        let error = wire_error(&SupervisionError::UnknownDeployment {
            name: "absent".to_owned(),
        });
        assert_eq!(error.code, aion_proto::WireErrorCode::NotFound);
    }
}
