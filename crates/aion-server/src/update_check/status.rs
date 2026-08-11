//! The server's memory of its last completed update check.
//!
//! One process-wide slot, shared by the dispatch observer (the writer) and
//! `GET /update-status` (the reader). It starts EMPTY and stays empty until a
//! check genuinely completes: a fresh server has honestly never checked, and
//! representing that as anything but absence — a zero, an epoch date, a blank
//! string — would fabricate a measurement nobody made.
//!
//! Deliberately in-memory only. The recorded value is an observation about
//! the OUTSIDE world (what crates.io publishes), not workflow state; a
//! restart forgetting it is truthful, because the world may have moved while
//! the server was down. The check itself is durable where durability means
//! something: its workflow history and transcript.

use std::sync::{Arc, RwLock};

use chrono::{DateTime, Utc};

/// One completed check's result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LastCheck {
    /// The greatest INSTALLABLE `aion-cli` version — not yanked and not a
    /// prerelease, the newest version a plain `cargo install` would take —
    /// rendered spec-exactly.
    pub latest_known: String,
    /// When the server finished parsing the fetched index.
    pub checked_at: DateTime<Utc>,
}

/// Shared handle over the last-check slot. Cheap to clone; all clones read
/// and write the one slot.
#[derive(Debug, Clone, Default)]
pub struct UpdateStatusState {
    inner: Arc<RwLock<Option<LastCheck>>>,
}

impl UpdateStatusState {
    /// Record a completed check, keeping whichever observation is NEWER.
    ///
    /// Monotonic by `checked_at`: two concurrent checks can complete out of
    /// order (each stamps its own moment before taking the lock), and
    /// replacing unconditionally would let the older observation land last
    /// and be served as the freshest. An equal timestamp records — the later
    /// writer of the same instant is the later observation.
    ///
    /// A poisoned lock is recovered rather than propagated: the slot holds a
    /// plain value, so whatever a panicking holder left behind is still a
    /// coherent `Option`, and refusing to record a NEWER observation over it
    /// would preserve exactly the staler data.
    pub fn record(&self, check: LastCheck) {
        let mut slot = match self.inner.write() {
            Ok(slot) => slot,
            Err(poisoned) => poisoned.into_inner(),
        };
        match slot.as_ref() {
            Some(current) if current.checked_at > check.checked_at => {
                tracing::info!(
                    operation = "update_check.record",
                    kept = %current.checked_at,
                    offered = %check.checked_at,
                    "an older check completed after a newer one; keeping the newer observation"
                );
            }
            _ => *slot = Some(check),
        }
    }

    /// The last completed check, or `None` when no check has ever completed.
    #[must_use]
    pub fn last(&self) -> Option<LastCheck> {
        let slot = match self.inner.read() {
            Ok(slot) => slot,
            Err(poisoned) => poisoned.into_inner(),
        };
        slot.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::{LastCheck, UpdateStatusState};

    /// A fresh slot reports absence — not a zero, not an epoch.
    #[test]
    fn a_fresh_server_has_honestly_never_checked() {
        assert_eq!(UpdateStatusState::default().last(), None);
    }

    /// A fixed, strictly-ordered pair rather than two `Utc::now()` calls:
    /// `record` is monotonic by `checked_at`, so a wall clock stepping
    /// backwards between two live reads could flake a now-based version of
    /// this test (r2-m8). Fixed instants make the ordering part of the
    /// fixture.
    fn ordered_checks() -> Result<(LastCheck, LastCheck), Box<dyn std::error::Error>> {
        let first = LastCheck {
            latest_known: "0.13.7".to_owned(),
            checked_at: "2026-08-11T01:00:00Z".parse()?,
        };
        let second = LastCheck {
            latest_known: "0.14.0".to_owned(),
            checked_at: "2026-08-11T01:00:30Z".parse()?,
        };
        Ok((first, second))
    }

    /// Clones share the one slot, and a later record replaces an earlier one.
    #[test]
    fn record_replaces_and_is_visible_through_every_clone() -> Result<(), Box<dyn std::error::Error>>
    {
        let state = UpdateStatusState::default();
        let reader = state.clone();
        let (first, second) = ordered_checks()?;

        state.record(first.clone());
        assert_eq!(reader.last(), Some(first));

        state.record(second.clone());
        assert_eq!(reader.last(), Some(second));
        Ok(())
    }

    /// Two checks completing out of order leave the NEWER observation
    /// recorded: an older `checked_at` arriving last must not shadow it.
    #[test]
    fn an_out_of_order_older_check_never_shadows_a_newer_one()
    -> Result<(), Box<dyn std::error::Error>> {
        let state = UpdateStatusState::default();
        let (older, newer) = ordered_checks()?;
        state.record(newer.clone());
        state.record(older);
        assert_eq!(state.last(), Some(newer));
        Ok(())
    }
}
