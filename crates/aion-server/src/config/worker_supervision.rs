//! The `[worker_supervision]` section: the operator's restart discipline.
//!
//! There is no default policy and there will not be one. A backoff, a restart
//! budget, and a kill grace are operational decisions with real consequences —
//! a guessed backoff turns a dependency outage into a thundering herd, and a
//! guessed grace turns a slow shutdown into a data-losing `SIGKILL`. So the
//! section is all-or-nothing: write every key and the server supervises;
//! write none and it supervises nothing, loudly (ADR-001).
//!
//! A PARTIAL section is the one thing that is neither: it is refused at load
//! with every missing key named, because silently defaulting the rest is
//! exactly the guess this rule exists to prevent.

use std::num::NonZeroU32;
use std::time::Duration;

use serde::Deserialize;

use crate::error::ServerError;
use crate::worker::supervisor::SupervisionPolicy;

use super::config_error;

/// Managed-worker supervision settings from `[worker_supervision]`.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct WorkerSupervisionConfig {
    /// Delay before the first restart in a window, milliseconds.
    pub restart_backoff_initial_ms: Option<u64>,
    /// Ceiling the geometric backoff is clamped to, milliseconds.
    pub restart_backoff_max_ms: Option<u64>,
    /// Geometric growth factor per restart already made in the window.
    pub restart_backoff_multiplier: Option<u32>,
    /// Sliding window over which restarts are counted, milliseconds.
    pub restart_window_ms: Option<u64>,
    /// Restarts permitted within one window before an instance is failed.
    pub max_restarts_per_window: Option<u32>,
    /// Grace given to each of `SIGTERM` and `SIGKILL` when stopping a managed
    /// worker's process group, milliseconds.
    pub stop_grace_ms: Option<u64>,
}

/// Every key the section needs, in the order an operator writes them.
const REQUIRED_KEYS: &[&str] = &[
    "restart_backoff_initial_ms",
    "restart_backoff_max_ms",
    "restart_backoff_multiplier",
    "restart_window_ms",
    "max_restarts_per_window",
    "stop_grace_ms",
];

impl WorkerSupervisionConfig {
    /// Which required keys this section is missing.
    fn missing(self) -> Vec<&'static str> {
        let present = [
            self.restart_backoff_initial_ms.is_some(),
            self.restart_backoff_max_ms.is_some(),
            self.restart_backoff_multiplier.is_some(),
            self.restart_window_ms.is_some(),
            self.max_restarts_per_window.is_some(),
            self.stop_grace_ms.is_some(),
        ];
        REQUIRED_KEYS
            .iter()
            .zip(present)
            .filter_map(|(key, present)| (!present).then_some(*key))
            .collect()
    }

    /// Resolve the section into a policy.
    ///
    /// `Ok(None)` is the honest reading of an ENTIRELY absent section: the
    /// operator has not commissioned supervision, and the managed-worker
    /// surface refuses with the remedy rather than inventing numbers.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::Config`] naming every missing key for a PARTIAL
    /// section, and naming the offending key for a value that cannot express a
    /// working discipline (a zero interval, a zero budget, a shrinking
    /// backoff, or a ceiling below the initial delay).
    pub fn resolve(self) -> Result<Option<SupervisionPolicy>, ServerError> {
        let missing = self.missing();
        if missing.len() == REQUIRED_KEYS.len() {
            return Ok(None);
        }
        if !missing.is_empty() {
            return config_error(format!(
                "[worker_supervision] is incomplete: it is missing {} — the section has no \
                 defaults, so write every key or remove the section entirely",
                missing.join(", ")
            ));
        }

        let initial = Self::required_duration(self.restart_backoff_initial_ms, REQUIRED_KEYS[0])?;
        let max = Self::required_duration(self.restart_backoff_max_ms, REQUIRED_KEYS[1])?;
        let multiplier = Self::required_factor(self.restart_backoff_multiplier, REQUIRED_KEYS[2])?;
        let window = Self::required_duration(self.restart_window_ms, REQUIRED_KEYS[3])?;
        let budget = Self::required_factor(self.max_restarts_per_window, REQUIRED_KEYS[4])?;
        let grace = Self::required_duration(self.stop_grace_ms, REQUIRED_KEYS[5])?;

        if max < initial {
            return config_error(format!(
                "worker_supervision.restart_backoff_max_ms ({}) must be at least \
                 restart_backoff_initial_ms ({})",
                max.as_millis(),
                initial.as_millis()
            ));
        }

        Ok(Some(SupervisionPolicy {
            restart_backoff_initial: initial,
            restart_backoff_max: max,
            restart_backoff_multiplier: multiplier,
            restart_window: window,
            max_restarts_per_window: budget,
            stop_grace: grace,
        }))
    }

    fn required_duration(value: Option<u64>, key: &'static str) -> Result<Duration, ServerError> {
        match value {
            Some(millis) if millis > 0 => Ok(Duration::from_millis(millis)),
            _ => config_error(format!(
                "worker_supervision.{key} must be greater than zero milliseconds"
            )),
        }
    }

    fn required_factor(value: Option<u32>, key: &'static str) -> Result<NonZeroU32, ServerError> {
        value.and_then(NonZeroU32::new).map_or_else(
            || {
                config_error(format!(
                    "worker_supervision.{key} must be greater than zero"
                ))
            },
            Ok,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{REQUIRED_KEYS, WorkerSupervisionConfig};

    fn complete() -> WorkerSupervisionConfig {
        WorkerSupervisionConfig {
            restart_backoff_initial_ms: Some(100),
            restart_backoff_max_ms: Some(5_000),
            restart_backoff_multiplier: Some(2),
            restart_window_ms: Some(60_000),
            max_restarts_per_window: Some(5),
            stop_grace_ms: Some(2_000),
        }
    }

    #[test]
    fn an_absent_section_commissions_nothing_and_is_not_an_error()
    -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(WorkerSupervisionConfig::default().resolve()?, None);
        Ok(())
    }

    #[test]
    fn a_complete_section_resolves_every_value() -> Result<(), Box<dyn std::error::Error>> {
        let policy = complete()
            .resolve()?
            .ok_or("a complete section must resolve")?;
        assert_eq!(policy.restart_backoff_initial.as_millis(), 100);
        assert_eq!(policy.restart_backoff_max.as_millis(), 5_000);
        assert_eq!(policy.restart_backoff_multiplier.get(), 2);
        assert_eq!(policy.restart_window.as_millis(), 60_000);
        assert_eq!(policy.max_restarts_per_window.get(), 5);
        assert_eq!(policy.stop_grace.as_millis(), 2_000);
        Ok(())
    }

    /// A partial section is the dangerous case: defaulting the rest would
    /// silently choose a restart discipline nobody wrote. Every missing key
    /// must be named, one refusal, so the operator fixes it in one pass.
    #[test]
    fn a_partial_section_is_refused_and_names_every_missing_key()
    -> Result<(), Box<dyn std::error::Error>> {
        let partial = WorkerSupervisionConfig {
            restart_backoff_initial_ms: Some(100),
            ..WorkerSupervisionConfig::default()
        };
        let error = partial
            .resolve()
            .err()
            .ok_or("a partial section must be refused")?
            .to_string();
        for key in &REQUIRED_KEYS[1..] {
            assert!(
                error.contains(key),
                "the refusal must name `{key}`: {error}"
            );
        }
        assert!(
            !error.contains(REQUIRED_KEYS[0]),
            "the refusal must not name a key that IS present: {error}"
        );
        Ok(())
    }

    #[test]
    fn a_zero_interval_is_refused_by_name() -> Result<(), Box<dyn std::error::Error>> {
        for (mutate, key) in [
            (
                (|config: &mut WorkerSupervisionConfig| config.restart_backoff_initial_ms = Some(0))
                    as fn(&mut WorkerSupervisionConfig),
                "restart_backoff_initial_ms",
            ),
            (
                |config: &mut WorkerSupervisionConfig| config.restart_window_ms = Some(0),
                "restart_window_ms",
            ),
            (
                |config: &mut WorkerSupervisionConfig| config.stop_grace_ms = Some(0),
                "stop_grace_ms",
            ),
            (
                |config: &mut WorkerSupervisionConfig| config.restart_backoff_multiplier = Some(0),
                "restart_backoff_multiplier",
            ),
            (
                |config: &mut WorkerSupervisionConfig| config.max_restarts_per_window = Some(0),
                "max_restarts_per_window",
            ),
        ] {
            let mut config = complete();
            mutate(&mut config);
            let error = config
                .resolve()
                .err()
                .ok_or("a zero value must be refused")?
                .to_string();
            assert!(
                error.contains(key),
                "the refusal must name `{key}`: {error}"
            );
        }
        Ok(())
    }

    #[test]
    fn a_ceiling_below_the_initial_backoff_is_refused() -> Result<(), Box<dyn std::error::Error>> {
        let mut config = complete();
        config.restart_backoff_max_ms = Some(10);
        let error = config
            .resolve()
            .err()
            .ok_or("a ceiling below the initial delay must be refused")?
            .to_string();
        assert!(error.contains("restart_backoff_max_ms"));
        assert!(error.contains("restart_backoff_initial_ms"));
        Ok(())
    }
}
