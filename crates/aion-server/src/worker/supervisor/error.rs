//! Typed managed-worker supervision failures.
//!
//! Every variant names the thing that went wrong AND, where an operator can
//! act, what to do about it. A refusal that does not name its remedy is a
//! refusal the operator has to guess at, and guessing is how a fleet ends up
//! hand-started again.

use thiserror::Error;

use super::policy::UNCOMMISSIONED_REMEDY;

/// A managed-worker lifecycle operation that could not be carried out.
#[derive(Debug, Error)]
pub enum SupervisionError {
    /// No supervision policy is configured, so nothing may be started.
    #[error("{UNCOMMISSIONED_REMEDY}")]
    NotCommissioned,
    /// The named deployment does not exist.
    #[error(
        "worker deployment `{name}` was not found; create it first with a PUT to /worker-deployments/{name}"
    )]
    UnknownDeployment {
        /// Deployment primary key the caller asked for.
        name: String,
    },
    /// The durable deployment store refused or failed.
    #[error("worker deployment store failed: {source}")]
    Store {
        /// Underlying store failure.
        #[source]
        source: aion_store::StoreError,
    },
    /// This server's own executable could not be identified.
    #[error("could not resolve the running server executable: {detail}")]
    ExecutableUnresolved {
        /// Operating-system failure text.
        detail: String,
    },
    /// The executable bytes could not be read for identity capture.
    #[error("could not read managed worker executable `{path}`: {detail}")]
    ExecutableUnreadable {
        /// Path that could not be read.
        path: String,
        /// Operating-system failure text.
        detail: String,
    },
    /// The contained child could not be spawned or observed.
    #[error("managed worker `{name}` process failed: {source}")]
    Process {
        /// Deployment primary key.
        name: String,
        /// Underlying containment failure.
        #[source]
        source: aion_worker::ProcessGroupError,
    },
    /// A stop was asked for and could not be PROVEN complete.
    ///
    /// This exists so that "stopped" is never reported on the strength of
    /// bookkeeping: the only path to a `Stopped` status runs through a process
    /// group that a signal-zero probe found empty, and every other outcome
    /// arrives here instead.
    #[error("managed worker `{name}` could not be confirmed stopped: {detail}")]
    StopIncomplete {
        /// Deployment primary key.
        name: String,
        /// What was observed instead of an empty process group.
        detail: String,
    },
    /// A supervision task ended abnormally (panicked or was aborted).
    #[error("managed worker `{name}` supervision task ended abnormally: {detail}")]
    TaskLost {
        /// Deployment primary key.
        name: String,
        /// Join failure text.
        detail: String,
    },
    /// A supervisor lock was poisoned by a panicking holder.
    #[error("managed worker supervision state is poisoned: {detail}")]
    StatePoisoned {
        /// Poison report.
        detail: String,
    },
}

impl SupervisionError {
    /// The operator remedy carried by this failure, when it has one.
    #[must_use]
    pub const fn remedy(&self) -> Option<&'static str> {
        match self {
            Self::NotCommissioned => Some(UNCOMMISSIONED_REMEDY),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SupervisionError;

    /// The uncommissioned refusal is the one an operator meets first, and it is
    /// useless unless it names the section AND every key that section needs.
    #[test]
    fn the_uncommissioned_refusal_names_the_section_and_every_required_key() {
        let message = SupervisionError::NotCommissioned.to_string();
        for expected in [
            "[worker_supervision]",
            "restart_backoff_initial_ms",
            "restart_backoff_max_ms",
            "restart_backoff_multiplier",
            "restart_window_ms",
            "max_restarts_per_window",
            "stop_grace_ms",
        ] {
            assert!(
                message.contains(expected),
                "the refusal must name `{expected}`: {message}"
            );
        }
        assert_eq!(
            SupervisionError::NotCommissioned.remedy(),
            Some(super::UNCOMMISSIONED_REMEDY)
        );
    }

    #[test]
    fn an_unknown_deployment_refusal_names_the_deployment_and_the_way_to_create_it() {
        let message = SupervisionError::UnknownDeployment {
            name: "shells".to_owned(),
        }
        .to_string();
        assert!(message.contains("shells"));
        assert!(message.contains("/worker-deployments/shells"));
    }
}
