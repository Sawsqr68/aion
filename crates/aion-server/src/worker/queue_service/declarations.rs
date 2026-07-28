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

impl QueueDeclarations for EngineQueueDeclarations {
    fn declaration_for(&self, task_queue: &str) -> QueueDeclaration {
        match self.engine.declared_task_queues() {
            // Nothing declares any queue: this catalog cannot contradict a
            // dispatch, so it must not refuse one.
            Ok(queues) if queues.is_empty() => QueueDeclaration::Unknown,
            Ok(queues) if queues.contains(task_queue) => QueueDeclaration::Declared,
            Ok(_) => QueueDeclaration::NotDeclared,
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
    use super::*;

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
