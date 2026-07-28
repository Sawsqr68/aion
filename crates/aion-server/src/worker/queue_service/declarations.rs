//! Whether a task queue is declared by any deployed contract.
//!
//! This reads the SAME durable `.v4` worker-contract records the registration
//! handshake admits workers against (`crate::worker::contracts`) and workflow
//! start admission refuses against (`aion::lifecycle` `NoQueueDeclaration`) —
//! there is no second queue-declaration truth in the server.
//!
//! Fail-open by construction. A server whose catalog declares no queues at all
//! (an engine-less state built from parts, a fresh boot, an embedded engine
//! serving activities in process) can contradict nothing, so it answers
//! [`QueueDeclaration::Unknown`] and no structural refusal is ever
//! manufactured from an absence of information. That is the same discipline
//! the handshake lane adopted after its engine-less registration defect —
//! whose observed symptom was this very seam waiting forever at INFO.

use std::sync::{Arc, OnceLock};

use aion::loader::DeclaredQueues;

/// What the deployed contract records say about a task queue.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueueDeclaration {
    /// At least one retained `.v4` contract declares this queue.
    Declared,
    /// Contracts declaring queues exist, and none of them declares this one.
    NotDeclared,
    /// Nothing in reach can answer: no source installed, no queue-declaring
    /// contract deployed at all, or the catalog could not be read.
    Unknown,
}

/// A reader over the deployed queue declarations.
pub trait QueueDeclarations: Send + Sync {
    /// Answer for one task queue. Implementations never fail: an unreadable
    /// catalog is [`QueueDeclaration::Unknown`], reported by the implementation.
    fn declaration_for(&self, task_queue: &str) -> QueueDeclaration;
}

/// Shared, install-once handle the bridge holds from construction and the boot
/// path fills in once the engine exists.
///
/// Mirrors the outbox-delivery callback install: the dispatcher is built before
/// the engine, so the seam it will consult is handed over afterwards through a
/// clone of this handle rather than by rebuilding the dispatcher.
#[derive(Clone, Default)]
pub struct QueueDeclarationSource {
    inner: Arc<OnceLock<Arc<dyn QueueDeclarations>>>,
}

impl std::fmt::Debug for QueueDeclarationSource {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("QueueDeclarationSource")
            .field("installed", &self.inner.get().is_some())
            .finish()
    }
}

impl QueueDeclarationSource {
    /// Install the reader. A second install is ignored and logged: the source
    /// is process-wide and must not silently change identity.
    pub fn install(&self, source: Arc<dyn QueueDeclarations>) {
        if self.inner.set(source).is_err() {
            tracing::warn!("queue declaration source already installed; ignoring duplicate set");
        }
    }

    /// Whether a reader has been installed.
    #[must_use]
    pub fn is_installed(&self) -> bool {
        self.inner.get().is_some()
    }

    /// Answer for one task queue, or [`QueueDeclaration::Unknown`] when no
    /// reader is installed.
    #[must_use]
    pub fn declaration_for(&self, task_queue: &str) -> QueueDeclaration {
        self.inner
            .get()
            .map_or(QueueDeclaration::Unknown, |source| {
                source.declaration_for(task_queue)
            })
    }
}

/// Reads declarations out of the engine's live workflow catalog.
pub struct EngineQueueDeclarations {
    engine: Arc<aion::Engine>,
}

impl EngineQueueDeclarations {
    /// Build a reader over `engine`'s catalog.
    #[must_use]
    pub const fn new(engine: Arc<aion::Engine>) -> Self {
        Self { engine }
    }
}

impl std::fmt::Debug for EngineQueueDeclarations {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("EngineQueueDeclarations")
    }
}

/// Maps one catalog read to the answer for `task_queue`.
///
/// A positive find is positive. An ABSENCE is only reported as
/// [`QueueDeclaration::NotDeclared`] when the read has standing to make it:
/// a catalog that declares no queue at all contradicts nothing.
fn declaration_from(read: &DeclaredQueues, task_queue: &str) -> QueueDeclaration {
    if read.declares(task_queue) {
        QueueDeclaration::Declared
    } else if read.found_no_declaration() {
        // Nothing declares any queue: this catalog cannot contradict a
        // dispatch, so it must not refuse one.
        QueueDeclaration::Unknown
    } else {
        QueueDeclaration::NotDeclared
    }
}

impl QueueDeclarations for EngineQueueDeclarations {
    fn declaration_for(&self, task_queue: &str) -> QueueDeclaration {
        match self.engine.declared_task_queues() {
            Ok(read) => declaration_from(&read, task_queue),
            Err(error) => {
                tracing::warn!(
                    task_queue,
                    %error,
                    "queue declaration lookup failed; treating the declaration as unknown"
                );
                QueueDeclaration::Unknown
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::super::census::{PoolCensus, classify};
    use super::super::taxonomy::QueueServiceReason;
    use super::*;

    fn queues(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    /// A catalog that decoded every entry has standing to say "nobody declares
    /// this queue", and that refusal stays structural.
    #[test]
    fn a_complete_read_still_reports_an_absent_queue_as_undeclared() {
        let read = DeclaredQueues::new(queues(&["orders"]), Vec::new());
        assert_eq!(
            declaration_from(&read, "checkout"),
            QueueDeclaration::NotDeclared
        );
        assert_eq!(
            declaration_from(&read, "orders"),
            QueueDeclaration::Declared
        );
    }

    #[test]
    fn a_complete_read_that_found_nothing_is_unknown() {
        let read = DeclaredQueues::new(BTreeSet::new(), Vec::new());
        assert_eq!(declaration_from(&read, "orders"), QueueDeclaration::Unknown);
    }

    /// The blocker pin, mapping half. A read that could not decode every entry
    /// has seen only part of the catalog: a queue it did not find may well be
    /// declared by the entry it could not read.
    #[test]
    fn a_queue_absent_from_a_partial_read_is_never_undeclared() {
        let read = DeclaredQueues::new(queues(&["orders"]), vec!["sha256:legacy".to_owned()]);
        assert_eq!(
            declaration_from(&read, "checkout"),
            QueueDeclaration::Unknown
        );
    }

    #[test]
    fn a_positive_find_survives_a_partial_read() {
        let read = DeclaredQueues::new(queues(&["orders"]), vec!["sha256:legacy".to_owned()]);
        assert_eq!(
            declaration_from(&read, "orders"),
            QueueDeclaration::Declared
        );
    }

    /// The blocker pin, consequence half. The mixed catalog the handshake
    /// migration creates must never terminally refuse a dispatch to the
    /// pre-`.v4` package's queue — it is a fleet condition, retryable and
    /// serviceable the moment a compatible worker connects.
    #[test]
    fn a_mixed_catalog_never_structurally_refuses_the_undecodable_queue() {
        let read = DeclaredQueues::new(queues(&["orders"]), vec!["sha256:legacy".to_owned()]);
        let reason = classify(
            declaration_from(&read, "checkout"),
            &PoolCensus {
                workers_in_pool: 1,
                workers_serving_activity: 0,
                compatible_workers: 0,
                last_compatible_poller_age: None,
            },
        );
        assert_ne!(reason, Some(QueueServiceReason::NoQueueDeclaration));
        assert_eq!(reason, Some(QueueServiceReason::PollersIncompatible));
    }

    struct FixedDeclarations(QueueDeclaration);

    impl QueueDeclarations for FixedDeclarations {
        fn declaration_for(&self, _task_queue: &str) -> QueueDeclaration {
            self.0
        }
    }

    #[test]
    fn an_uninstalled_source_answers_unknown() {
        let source = QueueDeclarationSource::default();
        assert!(!source.is_installed());
        assert_eq!(source.declaration_for("general"), QueueDeclaration::Unknown);
    }

    #[test]
    fn an_installed_source_answers_and_is_visible_to_every_clone() {
        let source = QueueDeclarationSource::default();
        source
            .clone()
            .install(Arc::new(FixedDeclarations(QueueDeclaration::NotDeclared)));
        assert!(source.is_installed());
        assert_eq!(
            source.declaration_for("general"),
            QueueDeclaration::NotDeclared
        );
    }

    #[test]
    fn a_duplicate_install_never_changes_the_answer() {
        let source = QueueDeclarationSource::default();
        source.install(Arc::new(FixedDeclarations(QueueDeclaration::Declared)));
        source.install(Arc::new(FixedDeclarations(QueueDeclaration::NotDeclared)));
        assert_eq!(
            source.declaration_for("general"),
            QueueDeclaration::Declared
        );
    }
}
