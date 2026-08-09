//! What the server will say about a managed worker, and what it refuses to say.
//!
//! The status is a JOIN of two different kinds of truth: the durable record's
//! desired state, and the live instance's actual state. Reporting one without
//! the other is how "it says stopped" comes to mean "nobody looked". Every
//! actual state below is written by the instance loop only after the fact it
//! names has been established — `Running` after a spawn returned a pid,
//! `Stopped` only after the process group was probed empty.

use aion_store::{DeployedBinaryIdentity, DesiredState};
use serde::{Deserialize, Serialize};

/// What a managed worker instance is actually doing right now.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ManagedWorkerState {
    /// A spawn is in progress; no pid has been observed yet.
    Starting,
    /// A contained child is alive and leading its process group.
    Running,
    /// The child exited and a restart is waiting out the configured backoff.
    Backoff,
    /// No process is running and none is wanted: either the deployment desires
    /// `Stopped`, or a stop confirmed the process group empty.
    Stopped,
    /// Supervision gave up: the restart budget for the window was exhausted, or
    /// stopping could not be confirmed. Terminal until an operator acts.
    Failed,
    /// The deployment desires `Running` but this server has no supervision
    /// policy, so nothing is supervising it. The remedy travels with the report.
    Uncommissioned,
}

impl ManagedWorkerState {
    /// Stable lowercase token used on the wire and in status history.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Backoff => "backoff",
            Self::Stopped => "stopped",
            Self::Failed => "failed",
            Self::Uncommissioned => "uncommissioned",
        }
    }

    /// Whether the supervisor has stopped acting on this instance.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Stopped | Self::Failed)
    }
}

/// Identity of the executable one spawn actually ran.
///
/// The content hash is taken from the bytes on disk AT SPAWN, which is the
/// whole point: a restart that picks up an upgraded binary changes this hash
/// while the deployment record's deploy-time hash stays put, so the swap is a
/// visible, reportable fact instead of an invisible one.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SpawnedBinary {
    /// Path of the executable that was launched.
    pub path: String,
    /// Lowercase hexadecimal SHA-256 digest of the executable bytes at spawn.
    pub content_hash: String,
    /// Package version stamped into the binary. Present only when the spawned
    /// executable is this server's own binary, which is the only executable
    /// whose build stamp this process can honestly claim to know.
    pub version: Option<String>,
    /// Source commit stamped into the binary, under the same condition.
    pub commit: Option<String>,
    /// Dirty-state token stamped into the binary, under the same condition.
    pub dirty: Option<String>,
}

/// How a managed worker process last ended.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ManagedWorkerExit {
    /// When the exit was observed.
    pub at: chrono::DateTime<chrono::Utc>,
    /// Exit code, when the process exited normally.
    pub code: Option<i32>,
    /// Terminating signal number, when a signal ended the process.
    pub signal: Option<i32>,
    /// Whether the supervisor itself asked for this ending.
    pub requested: bool,
}

/// One managed worker: what was asked for, and what is true.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ManagedWorkerStatus {
    /// Deployment primary key.
    pub name: String,
    /// Task queue the deployment binds.
    pub task_queue: String,
    /// Durable operator intent.
    pub desired: DesiredState,
    /// Live supervision state.
    pub state: ManagedWorkerState,
    /// Process id of the running child, while one is running.
    pub pid: Option<u32>,
    /// Process group id the child leads, while one is running.
    pub process_group: Option<i32>,
    /// Restarts performed since this instance was started.
    pub restarts: u32,
    /// How the process last ended, when it has ended at least once.
    pub last_exit: Option<ManagedWorkerExit>,
    /// The last failure worth an operator's attention, verbatim.
    pub last_error: Option<String>,
    /// Identity of the binary the deployment named AT DEPLOY TIME.
    pub deployed_binary: DeployedBinaryIdentity,
    /// Identity of the binary the CURRENT (or most recent) spawn actually ran.
    ///
    /// A restart across an upgrade is meant to be a visible fact rather than a
    /// silent swap, so this is captured per spawn and reported beside the
    /// deploy-time identity: the two disagreeing is the upgrade, stated.
    pub spawn_binary: Option<SpawnedBinary>,
}

/// The whole managed-worker surface for this server.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ManagedWorkerReport {
    /// Whether a supervision policy is configured on this server.
    pub commissioned: bool,
    /// The remedy for an uncommissioned server, present exactly when
    /// `commissioned` is false. Absent when there is nothing to remedy.
    pub remedy: Option<String>,
    /// One entry per durable deployment, ordered by name.
    pub workers: Vec<ManagedWorkerStatus>,
    /// Names of durable rows that are PRESENT but could not be decoded, and are
    /// therefore supervised by nothing. Reported rather than dropped: a row the
    /// server cannot read is exactly the row an operator needs told about.
    pub undecodable: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::ManagedWorkerState;

    #[test]
    fn tokens_are_distinct_and_terminality_matches_them() {
        let states = [
            ManagedWorkerState::Starting,
            ManagedWorkerState::Running,
            ManagedWorkerState::Backoff,
            ManagedWorkerState::Stopped,
            ManagedWorkerState::Failed,
            ManagedWorkerState::Uncommissioned,
        ];
        let mut tokens = states.map(ManagedWorkerState::token).to_vec();
        tokens.sort_unstable();
        tokens.dedup();
        assert_eq!(tokens.len(), states.len());

        assert!(ManagedWorkerState::Stopped.is_terminal());
        assert!(ManagedWorkerState::Failed.is_terminal());
        assert!(!ManagedWorkerState::Running.is_terminal());
        assert!(!ManagedWorkerState::Backoff.is_terminal());
        assert!(!ManagedWorkerState::Starting.is_terminal());
        // Uncommissioned is not terminal: commissioning the server and starting
        // the deployment is exactly the state it is waiting for.
        assert!(!ManagedWorkerState::Uncommissioned.is_terminal());
    }
}
