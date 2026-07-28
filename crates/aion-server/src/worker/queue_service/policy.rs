//! Per-dispatch service policy and the two service clocks.
//!
//! R1 rules two policies and two clocks, and this module is where a dispatch's
//! pair is resolved from what the operator actually wrote.
//!
//! **strict** is the default, everywhere, always: a dispatch to a queue that is
//! structurally unserved refuses instead of starting a lie. **`durable_pending`**
//! is a written opt-in per queue — the run is marked unserved immediately and
//! visibly, then parks durably until a worker arrives.
//!
//! The two clocks are never conflated:
//!
//! * the **service-availability deadline** bounds waiting for a compatible
//!   worker to EXIST (`NO_LIVE_POLLERS` / `POLLERS_INCOMPATIBLE`);
//! * the **schedule-to-start timeout** bounds the hand-off to a worker that
//!   already exists (`SATURATED`).
//!
//! Both are `Option`: unset means unbounded, which is exactly today's
//! behaviour and the only honest default — a server does not get to invent the
//! deadline an operator never declared. What DID change is that an unbounded
//! wait is now loud, typed, and queryable instead of silent.

use std::time::Duration;

use serde::Deserialize;

/// What a dispatch does when its queue is not being served.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum QueueServicePolicy {
    /// Refuse rather than admit work onto a queue that cannot serve it.
    /// Structural unservability refuses immediately; a fleet condition refuses
    /// when the service-availability deadline expires. The default.
    #[default]
    Strict,
    /// Accept, mark the run unserved immediately and visibly, and park durably
    /// until a compatible worker arrives. Written opt-in, per queue. Structural
    /// unservability still refuses — a queue no deployment declares can never
    /// be served, so pending on it would be the lie the policy exists to avoid.
    DurablePending,
}

impl QueueServicePolicy {
    /// The canonical spelling used in config and in logs.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Strict => "strict",
            Self::DurablePending => "durable_pending",
        }
    }
}

impl std::fmt::Display for QueueServicePolicy {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// One written per-queue policy opt-in.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct QueueServiceOverride {
    /// Namespace the override applies to, or `None` for every namespace.
    #[serde(default)]
    pub namespace: Option<String>,
    /// Task queue the override applies to.
    pub task_queue: String,
    /// Policy this queue is served under.
    pub policy: QueueServicePolicy,
}

/// The operator's queue-service settings.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct QueueServiceConfig {
    /// Policy for every queue with no written override. `strict`.
    pub default_policy: QueueServicePolicy,
    /// Written per-queue opt-ins. The first entry matching a dispatch wins, so
    /// a namespace-scoped entry placed before a namespace-less one is honoured.
    pub overrides: Vec<QueueServiceOverride>,
    /// How long a dispatch waits for a compatible worker to exist before a
    /// typed `WorkerUnavailable` refusal. Unset = unbounded.
    #[serde(default, with = "optional_duration_millis")]
    pub service_availability_deadline: Option<Duration>,
    /// How long a dispatch tries to hand its task to a live compatible worker
    /// before a typed `SATURATED` refusal. Unset = one attempt, exactly as
    /// before.
    #[serde(default, with = "optional_duration_millis")]
    pub schedule_to_start_timeout: Option<Duration>,
}

impl QueueServiceConfig {
    /// The policy governing one dispatch address.
    #[must_use]
    pub fn policy_for(&self, namespace: &str, task_queue: &str) -> QueueServicePolicy {
        self.overrides
            .iter()
            .find(|entry| {
                entry.task_queue == task_queue
                    && entry
                        .namespace
                        .as_ref()
                        .is_none_or(|scoped| scoped == namespace)
            })
            .map_or(self.default_policy, |entry| entry.policy)
    }

    /// Whether the service-availability deadline governs this dispatch.
    ///
    /// `durable_pending` is precisely the declaration that this dispatch waits
    /// for its worker, so the availability deadline — the strict policy's
    /// instrument for refusing rather than lying — does not cut it short. The
    /// schedule-to-start clock still applies under both policies: it bounds a
    /// hand-off to a worker that already exists, which is a different question.
    #[must_use]
    pub const fn availability_deadline_for(&self, policy: QueueServicePolicy) -> Option<Duration> {
        match policy {
            QueueServicePolicy::Strict => self.service_availability_deadline,
            QueueServicePolicy::DurablePending => None,
        }
    }
}

/// Milliseconds-or-absent duration fields, matching the `duration_millis`
/// convention the rest of the server config uses.
mod optional_duration_millis {
    use std::time::Duration;

    use serde::{Deserialize, Deserializer};

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let millis = Option::<u64>::deserialize(deserializer)?;
        Ok(millis.map(Duration::from_millis))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_is_the_default_policy_and_no_clock_is_invented() {
        let config = QueueServiceConfig::default();
        assert_eq!(config.default_policy, QueueServicePolicy::Strict);
        assert_eq!(
            config.policy_for("default", "general"),
            QueueServicePolicy::Strict
        );
        assert_eq!(config.service_availability_deadline, None);
        assert_eq!(config.schedule_to_start_timeout, None);
    }

    #[test]
    fn a_written_override_opts_one_queue_into_durable_pending() {
        let config = QueueServiceConfig {
            overrides: vec![QueueServiceOverride {
                namespace: None,
                task_queue: "general".to_owned(),
                policy: QueueServicePolicy::DurablePending,
            }],
            ..QueueServiceConfig::default()
        };
        assert_eq!(
            config.policy_for("default", "general"),
            QueueServicePolicy::DurablePending
        );
        // Every other queue keeps the strict default.
        assert_eq!(
            config.policy_for("default", "billing"),
            QueueServicePolicy::Strict
        );
    }

    #[test]
    fn an_override_can_be_scoped_to_one_namespace() {
        let config = QueueServiceConfig {
            overrides: vec![QueueServiceOverride {
                namespace: Some("lab".to_owned()),
                task_queue: "general".to_owned(),
                policy: QueueServicePolicy::DurablePending,
            }],
            ..QueueServiceConfig::default()
        };
        assert_eq!(
            config.policy_for("lab", "general"),
            QueueServicePolicy::DurablePending
        );
        assert_eq!(
            config.policy_for("prod", "general"),
            QueueServicePolicy::Strict
        );
    }

    #[test]
    fn the_availability_deadline_bounds_strict_only() {
        let config = QueueServiceConfig {
            service_availability_deadline: Some(Duration::from_secs(30)),
            ..QueueServiceConfig::default()
        };
        assert_eq!(
            config.availability_deadline_for(QueueServicePolicy::Strict),
            Some(Duration::from_secs(30))
        );
        assert_eq!(
            config.availability_deadline_for(QueueServicePolicy::DurablePending),
            None
        );
    }

    #[test]
    fn queue_service_settings_deserialize_from_the_written_form() -> Result<(), toml::de::Error> {
        let config: QueueServiceConfig = toml::from_str(
            r#"
            default_policy = "strict"
            service_availability_deadline = 45000
            schedule_to_start_timeout = 5000

            [[overrides]]
            task_queue = "general"
            policy = "durable_pending"
            "#,
        )?;
        assert_eq!(config.default_policy, QueueServicePolicy::Strict);
        assert_eq!(
            config.service_availability_deadline,
            Some(Duration::from_secs(45))
        );
        assert_eq!(
            config.schedule_to_start_timeout,
            Some(Duration::from_secs(5))
        );
        assert_eq!(
            config.policy_for("default", "general"),
            QueueServicePolicy::DurablePending
        );
        Ok(())
    }
}
