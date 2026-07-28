//! Live poller census for one pool address, and the classification it feeds.
//!
//! The census is taken from the connected-worker registry at the same moment
//! selection misses, so the taxonomy verdict describes the fleet the selection
//! actually saw — never a second, later, disagreeing observation.

use std::time::Duration;

use super::declarations::QueueDeclaration;
use super::taxonomy::QueueServiceReason;

/// What the connected-worker registry holds for one
/// `(namespace, task_queue, activity_type[, node])` address.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PoolCensus {
    /// Workers connected for `(namespace, task_queue)`, whatever they serve.
    pub workers_in_pool: usize,
    /// Of those, workers advertising this activity type.
    pub workers_serving_activity: usize,
    /// Of those, workers that also satisfy the dispatch's node pin. Equal to
    /// [`Self::workers_serving_activity`] for an unpinned dispatch.
    pub compatible_workers: usize,
    /// How long ago a compatible worker was last in service, or `None` when
    /// none has ever been seen for this address in this server's life.
    ///
    /// Zero while a compatible worker is connected right now.
    pub last_compatible_poller_age: Option<Duration>,
}

impl PoolCensus {
    /// Whether the address currently has a worker that could take the dispatch.
    #[must_use]
    pub const fn is_served(&self) -> bool {
        self.compatible_workers > 0
    }
}

/// Classify one selection miss into the taxonomy.
///
/// Returns `None` when the address is in fact served — the caller raced a
/// worker arriving and must simply retry selection rather than report a state
/// that is no longer true.
///
/// Ordering is deliberate: a structurally undeclared queue is reported as
/// [`QueueServiceReason::NoQueueDeclaration`] even when unrelated workers sit
/// in the pool, because that is the deeper fact and the only one that refuses
/// unconditionally. [`QueueDeclaration::Unknown`] never manufactures a
/// structural refusal — the same fail-open discipline worker registration
/// applies when no catalog is reachable.
///
/// [`QueueServiceReason::Saturated`] is never produced here: it is not a
/// selection miss. It is emitted by the delivery path
/// ([`super::delivery::deliver_within_schedule_to_start`]) when a compatible
/// worker is live and refuses intake past the schedule-to-start clock.
#[must_use]
pub fn classify(declaration: QueueDeclaration, census: &PoolCensus) -> Option<QueueServiceReason> {
    if census.is_served() {
        return None;
    }
    if declaration == QueueDeclaration::NotDeclared {
        return Some(QueueServiceReason::NoQueueDeclaration);
    }
    if census.workers_in_pool > 0 {
        return Some(QueueServiceReason::PollersIncompatible);
    }
    Some(QueueServiceReason::NoLivePollers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_served_address_is_not_classified() {
        let census = PoolCensus {
            workers_in_pool: 1,
            workers_serving_activity: 1,
            compatible_workers: 1,
            last_compatible_poller_age: Some(Duration::ZERO),
        };
        assert_eq!(classify(QueueDeclaration::Declared, &census), None);
        assert_eq!(classify(QueueDeclaration::NotDeclared, &census), None);
    }

    #[test]
    fn an_undeclared_queue_is_structural() {
        assert_eq!(
            classify(QueueDeclaration::NotDeclared, &PoolCensus::default()),
            Some(QueueServiceReason::NoQueueDeclaration)
        );
    }

    #[test]
    fn an_empty_pool_on_a_declared_queue_has_no_live_pollers() {
        assert_eq!(
            classify(QueueDeclaration::Declared, &PoolCensus::default()),
            Some(QueueServiceReason::NoLivePollers)
        );
    }

    #[test]
    fn an_unknowable_declaration_never_produces_a_structural_refusal() {
        assert_eq!(
            classify(QueueDeclaration::Unknown, &PoolCensus::default()),
            Some(QueueServiceReason::NoLivePollers)
        );
    }

    #[test]
    fn pollers_that_do_not_cover_the_activity_are_incompatible() {
        let census = PoolCensus {
            workers_in_pool: 2,
            workers_serving_activity: 0,
            compatible_workers: 0,
            last_compatible_poller_age: None,
        };
        assert_eq!(
            classify(QueueDeclaration::Declared, &census),
            Some(QueueServiceReason::PollersIncompatible)
        );
    }

    #[test]
    fn pollers_off_the_pinned_node_are_incompatible() {
        let census = PoolCensus {
            workers_in_pool: 1,
            workers_serving_activity: 1,
            compatible_workers: 0,
            last_compatible_poller_age: Some(Duration::from_secs(9)),
        };
        assert_eq!(
            classify(QueueDeclaration::Declared, &census),
            Some(QueueServiceReason::PollersIncompatible)
        );
    }
}
