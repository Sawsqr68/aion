//! The terminal-writer reservation: the sole writer for a run that can never
//! obtain a handle (#117(c)).
//!
//! # What this is, and why it is not a handle
//!
//! A run whose pinned package version no longer loads is skipped by startup
//! recovery, never becomes resident, and therefore never obtains a
//! [`WorkflowHandle`] — so it never obtains a `Recorder`, so it can never be
//! cancelled, so it stays `Running` forever. Cancel, the operator's only lever,
//! is the one thing refused.
//!
//! The obvious fix — give the run a handle so it has a recorder — is dead, and
//! was eliminated by measurement before this was written.
//! [`WorkflowHandleParts`] requires a `pid` and a `loaded_version`: precisely
//! the two facts an unrecoverable run cannot supply. Three independent
//! subsystems read those fields as facts (the startup recovery skip, the
//! live-version set gating package unload, and delivery routing), so a
//! fabricated value corrupts each of them.
//!
//! > The cheap route is worse than the expensive one.
//!
//! This is the expensive one. It carries **no pid, no `loaded_version`, and no
//! residency**, and it is invisible to [`Registry::live_pid`] — nothing that
//! reads the registry for a live process can see it, because there is no
//! process.
//!
//! # Why it is a guard, and what that buys
//!
//! The reservation is an RAII guard borrowed from the registry. It cannot be
//! cloned, and [`Drop`] releases the slot unconditionally. That matters more
//! than it looks: a reservation that could be *taken* and never *released*
//! would wedge the `(workflow, run)` pair against every future writer for the
//! life of the process — the same defect class this exists to fix, reborn one
//! level up. Release is therefore not a call any caller can forget.
//!
//! It is also **process-local and non-durable**: the [`Registry`] holds no store
//! handle and no persistence of any kind, and is rebuilt empty by `Default` at
//! every boot. A crash between reservation and append leaves no reservation (the
//! registry is gone) and no append (the store never received one) — exactly
//! where the system was.
//!
//! # Exactly one append
//!
//! [`Self::record_cancelled`] **consumes** the reservation. There is no other
//! way to reach the recorder it holds, so the type itself licenses one terminal
//! transition and then ends. That transition is byte-for-byte the shape every
//! other terminal writer records: `WorkflowCancelled`, then the run's
//! outstanding declared-timeout deadline retired under the same recorder — a
//! cancelled run must not leave an armed deadline behind.
//!
//! [`WorkflowHandle`]: super::handle::WorkflowHandle
//! [`WorkflowHandleParts`]: super::handle::WorkflowHandleParts
//! [`Registry::live_pid`]: super::table::Registry::live_pid

use std::sync::Arc;

use aion_core::{Event, RunId, WorkflowId};
use aion_store::EventStore;
use chrono::{DateTime, Utc};

use crate::EngineError;
use crate::durability::Recorder;

use super::table::Registry;

/// The sole writer for a `(workflow, run)` pair that holds no handle and never
/// will under this build.
///
/// Obtained from [`Registry::reserve_terminal_writer`], which grants it only
/// when the registry can prove — under its own lock, at that moment — that the
/// workflow has no live handle and no other reservation. Held exclusively
/// against both for as long as this value lives.
///
/// `Debug` is written by hand: `dyn EventStore` is not `Debug`, and the store is
/// an implementation detail of the append rather than part of the reservation's
/// identity. What identifies it is the pair it holds.
///
/// [`Registry::reserve_terminal_writer`]: super::table::Registry::reserve_terminal_writer
pub struct TerminalWriterReservation<'a> {
    registry: &'a Registry,
    workflow_id: WorkflowId,
    run_id: RunId,
    store: Arc<dyn EventStore>,
}

impl<'a> TerminalWriterReservation<'a> {
    /// Builds the guard. Private to the registry: the reservation is only ever
    /// minted by the registry operation that won the slot, so a guard cannot
    /// exist without the exclusion that makes it safe.
    pub(super) fn new(
        registry: &'a Registry,
        workflow_id: WorkflowId,
        run_id: RunId,
        store: Arc<dyn EventStore>,
    ) -> Self {
        Self {
            registry,
            workflow_id,
            run_id,
            store,
        }
    }

    /// The workflow this reservation holds the writer slot for.
    #[must_use]
    pub fn workflow_id(&self) -> &WorkflowId {
        &self.workflow_id
    }

    /// The run this reservation holds the writer slot for.
    #[must_use]
    pub fn run_id(&self) -> &RunId {
        &self.run_id
    }

    /// Records the run's cancellation, then ends.
    ///
    /// Consuming `self` is what makes "exactly one terminal append" a property
    /// of the type rather than a rule the caller is trusted to follow: the
    /// recorder is minted here, used once, and dropped with the guard.
    ///
    /// `history` must be the run's history as read **after** the reservation was
    /// granted. The head it yields is what the recorder resumes at, and the
    /// outstanding-deadline lookup reads the same slice, so both derive from one
    /// consistent view. Reading before the reservation would admit a window in
    /// which a departing handle appended and left the head stale.
    ///
    /// # Errors
    ///
    /// Returns the recorder's typed [`EngineError`] if either append fails. A
    /// `SequenceConflict` here would mean a second writer existed — it is an
    /// alarm, not a mechanism: the exclusion that prevents it is the registry's,
    /// and this call does not depend on the alarm firing to be correct.
    pub async fn record_cancelled(
        self,
        history: &[Event],
        recorded_at: DateTime<Utc>,
        reason: String,
    ) -> Result<(), EngineError> {
        let head = history.last().map(Event::seq).unwrap_or_default();
        let mut recorder =
            Recorder::resume_at(self.workflow_id.clone(), Arc::clone(&self.store), head);
        recorder
            .record_workflow_cancelled(recorded_at, reason)
            .await?;
        // The same retirement every other terminal writer performs (`complete`,
        // `fail`, the resident `cancel`). A never-alive run can still carry an
        // armed declared-timeout deadline, and leaving it armed orphans a timer
        // that later fires against a cancelled run.
        crate::time::retire_run_deadline(&mut recorder, history, &self.run_id).await?;
        Ok(())
    }
}

impl std::fmt::Debug for TerminalWriterReservation<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TerminalWriterReservation")
            .field("workflow_id", &self.workflow_id)
            .field("run_id", &self.run_id)
            .finish_non_exhaustive()
    }
}

impl Drop for TerminalWriterReservation<'_> {
    fn drop(&mut self) {
        // Release is unconditional and cannot be forgotten. A poisoned registry
        // lock is the one case that cannot be reported here — `Drop` returns
        // nothing — so it is logged rather than swallowed. The process is
        // already in trouble if this fires: every other registry operation is
        // returning `RegistryPoisoned` too.
        if let Err(error) = self
            .registry
            .release_terminal_writer(&self.workflow_id, &self.run_id)
        {
            tracing::error!(
                workflow_id = %self.workflow_id,
                run_id = %self.run_id,
                error = %error,
                "could not release the terminal-writer reservation; the registry lock is poisoned \
                 and this (workflow, run) pair will refuse a writer for the life of this process"
            );
        }
    }
}
