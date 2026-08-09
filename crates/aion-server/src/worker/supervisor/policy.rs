//! The supervision policy, and the refusal owed when there is none.
//!
//! Every number here is the operator's (ADR-001). Nothing in this module
//! invents a backoff, a ceiling, or a grace period: an unconfigured server
//! supervises nothing and says exactly which keys would change that.

use std::num::NonZeroU32;
use std::time::Duration;

/// The operator-configured restart discipline for managed workers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SupervisionPolicy {
    /// Delay before the FIRST restart in a window.
    pub restart_backoff_initial: Duration,
    /// Ceiling the geometric backoff is clamped to.
    pub restart_backoff_max: Duration,
    /// Geometric growth factor applied per restart already made in the window.
    pub restart_backoff_multiplier: NonZeroU32,
    /// Sliding window over which restarts are counted.
    pub restart_window: Duration,
    /// Restarts permitted within one window before the instance is failed.
    pub max_restarts_per_window: NonZeroU32,
    /// Grace given to each of `SIGTERM` and `SIGKILL` when stopping a group.
    pub stop_grace: Duration,
}

impl SupervisionPolicy {
    /// The delay owed before a restart, given how many restarts already
    /// happened inside the current window.
    ///
    /// `restarts_in_window` counts the restarts already made, so the first
    /// restart of a window (`0` prior) waits exactly
    /// `restart_backoff_initial`. Growth is geometric and saturating, and the
    /// result is always clamped to `restart_backoff_max`.
    #[must_use]
    pub fn backoff_after(&self, restarts_in_window: u32) -> Duration {
        let multiplier = self.restart_backoff_multiplier.get();
        let mut delay = self.restart_backoff_initial;
        for _ in 0..restarts_in_window {
            delay = delay.saturating_mul(multiplier);
            if delay >= self.restart_backoff_max {
                return self.restart_backoff_max;
            }
        }
        delay.min(self.restart_backoff_max)
    }
}

/// The exact remedy an operator is owed when supervision is not configured.
///
/// Held as one constant because it is spoken by three surfaces (the start
/// refusal, the status report, and the boot warning): a remedy that drifts
/// between them names keys that may no longer exist.
pub const UNCOMMISSIONED_REMEDY: &str = "managed-worker supervision is not configured on this \
     server: add a `[worker_supervision]` section naming \
     `restart_backoff_initial_ms`, `restart_backoff_max_ms`, \
     `restart_backoff_multiplier`, `restart_window_ms`, \
     `max_restarts_per_window`, and `stop_grace_ms` (there are no defaults — \
     the restart discipline is an operator decision), then restart the server";

#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;
    use std::time::Duration;

    use super::SupervisionPolicy;

    fn policy(initial_ms: u64, max_ms: u64, multiplier: u32) -> Option<SupervisionPolicy> {
        Some(SupervisionPolicy {
            restart_backoff_initial: Duration::from_millis(initial_ms),
            restart_backoff_max: Duration::from_millis(max_ms),
            restart_backoff_multiplier: NonZeroU32::new(multiplier)?,
            restart_window: Duration::from_secs(60),
            max_restarts_per_window: NonZeroU32::new(3)?,
            stop_grace: Duration::from_secs(1),
        })
    }

    #[test]
    fn first_restart_waits_exactly_the_configured_initial_backoff() -> Result<(), &'static str> {
        let policy = policy(100, 10_000, 2).ok_or("policy must build")?;
        assert_eq!(policy.backoff_after(0), Duration::from_millis(100));
        Ok(())
    }

    #[test]
    fn backoff_grows_geometrically_then_clamps_to_the_ceiling() -> Result<(), &'static str> {
        let policy = policy(100, 400, 2).ok_or("policy must build")?;
        assert_eq!(policy.backoff_after(1), Duration::from_millis(200));
        assert_eq!(policy.backoff_after(2), Duration::from_millis(400));
        assert_eq!(policy.backoff_after(3), Duration::from_millis(400));
        assert_eq!(policy.backoff_after(64), Duration::from_millis(400));
        Ok(())
    }

    /// A multiplier of one is a flat retry cadence, not a growing one, and must
    /// not be mistaken for "no backoff".
    #[test]
    fn a_multiplier_of_one_keeps_the_initial_delay_forever() -> Result<(), &'static str> {
        let policy = policy(250, 10_000, 1).ok_or("policy must build")?;
        assert_eq!(policy.backoff_after(0), Duration::from_millis(250));
        assert_eq!(policy.backoff_after(9), Duration::from_millis(250));
        Ok(())
    }

    /// The growth is saturating: a large multiplier over many restarts must
    /// clamp rather than overflow the duration arithmetic.
    #[test]
    fn extreme_growth_saturates_into_the_ceiling() -> Result<(), &'static str> {
        let policy = policy(1_000, 30_000, u32::MAX).ok_or("policy must build")?;
        assert_eq!(policy.backoff_after(16), Duration::from_secs(30));
        Ok(())
    }
}
