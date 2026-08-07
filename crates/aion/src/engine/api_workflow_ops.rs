//! The workflow operations [`Engine`] exposes to a caller.
//!
//! Split from [`super::api`], which keeps the engine's own lifecycle — its
//! construction, its seam accessors, shard adoption and shutdown. Everything
//! here acts on a WORKFLOW rather than on the engine: starting, cancelling,
//! continuing as new, reopening, pausing, awaiting a result, and listing.

use std::collections::HashMap;
use std::sync::Arc;

use aion_core::{
    Payload, RunId, SearchAttributeValue, TimerCancelCause, WorkflowError, WorkflowFilter,
    WorkflowId, WorkflowSummary,
};

use crate::EngineError;
use crate::lifecycle::continue_as_new::{self, ContinueAsNewContext, ContinueAsNewRequest};
use crate::lifecycle::reopen::{self, ReopenWorkflowContext};
use crate::lifecycle::start::{self, StartWorkflowContext};
use crate::lifecycle::terminate::{self, TerminateWorkflowContext};
use crate::lifecycle::transition;
use crate::registry::{TerminalOutcome, WorkflowHandle};
use crate::time::timer_service::live_timers_in_active_segment;

use super::api::{Engine, terminal_outcome_from_history, workflow_not_found};

impl Engine {
    /// Start a loaded workflow type as a new BEAM process.
    ///
    /// `search_attributes` are validated against the engine's configured
    /// [`aion_core::SearchAttributeSchema`] and recorded atomically with the
    /// `WorkflowStarted` event, so visibility metadata can never be lost to a
    /// crash between start and a later attribute update.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::ShuttingDown`] after shutdown begins, and
    /// [`EngineError::Durability`] when a search attribute is unregistered or
    /// mistyped (nothing is appended and no process is spawned). Otherwise
    /// delegates to the start lifecycle transition and returns its typed errors.
    pub async fn start_workflow(
        &self,
        workflow_type: &str,
        input: Payload,
        search_attributes: HashMap<String, SearchAttributeValue>,
        namespace: String,
    ) -> Result<WorkflowHandle, EngineError> {
        self.start_workflow_with_id(
            workflow_type,
            input,
            search_attributes,
            namespace,
            None,
            None,
        )
        .await
    }

    /// Start a loaded workflow type, optionally with a caller-chosen
    /// `workflow_id` and/or R-4 steered-start `routing_key`.
    ///
    /// The request-routing edge supplies `workflow_id` to *place* a new start on
    /// a shard this node owns: the R-1 unsteered-start remint (any locally-owned
    /// shard) or, for a steered start, an id the edge derived on the
    /// `routing_key`'s shard before deciding to run locally. So a `start` whose
    /// id would otherwise hash to a non-owned shard never fences. When
    /// `workflow_id` is `None` this is identical to [`Self::start_workflow`]: the
    /// lifecycle mints a fresh `WorkflowId`, so the default single-node path is
    /// unchanged.
    ///
    /// `routing_key` is the caller-chosen steered-start key recorded on the start
    /// options. Shard derivation for the cluster path is performed at the edge
    /// (which holds the concrete cluster store); here it is threaded through for
    /// API completeness and direct callers.
    ///
    /// # Errors
    ///
    /// Identical to [`Self::start_workflow`]. A supplied `workflow_id` is treated
    /// as a fresh execution; the caller is responsible for choosing an unused id.
    pub async fn start_workflow_with_id(
        &self,
        workflow_type: &str,
        input: Payload,
        search_attributes: HashMap<String, SearchAttributeValue>,
        namespace: String,
        workflow_id: Option<WorkflowId>,
        routing_key: Option<String>,
    ) -> Result<WorkflowHandle, EngineError> {
        let operation = self.shutdown_gate.begin_start()?;
        let result = start::start_workflow_with_options(
            StartWorkflowContext {
                store: self.store(),
                visibility_store: self.visibility_store(),
                catalog: Arc::clone(&self.catalog),
                runtime: Arc::clone(&self.runtime),
                supervision: Arc::clone(&self.supervision),
                registry: Arc::clone(&self.registry),
                signal_handoff: Some(self.signal_handoff()),
                search_attribute_schema: Arc::clone(&self.search_attribute_schema),
                monitor_tokio_handle: tokio::runtime::Handle::current(),
            },
            workflow_type,
            input,
            start::StartWorkflowOptions {
                namespace: Some(namespace),
                search_attributes,
                workflow_id,
                routing_key,
                // THE start boundary: every transport (HTTP, gRPC, WebSocket,
                // CLI, in-process client) reaches the engine through here, and
                // this is the only start path whose input came from outside.
                input_admission: start::InputAdmission::Declared,
                ..start::StartWorkflowOptions::default()
            },
        )
        .await;
        drop(operation);
        result
    }

    /// Resume a suspended workflow run and flush deferred signals through its mailbox.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::WorkflowNotFound`] when the `(workflow, run)` pair
    /// is absent, or registry errors from the residency transition. Deferred
    /// delivery failures are logged and dropped because signals are already durable.
    pub fn resume_workflow(
        &self,
        id: &WorkflowId,
        run: &RunId,
    ) -> Result<WorkflowHandle, EngineError> {
        let handle = transition::resume(self.registry(), id, run)?;
        if let Err(error) = self.signal_handoff.deliver_deferred(self, id) {
            tracing::warn!(
                workflow_id = %id,
                run_id = %run,
                error = %error,
                "failed to flush deferred signals after workflow resume"
            );
        }
        Ok(handle)
    }

    /// Cancel a live workflow run by killing its runtime process.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::ShuttingDown`] after shutdown begins, and
    /// [`EngineError::WorkflowNotFound`] when the `(workflow, run)` pair
    /// is not live. Other typed errors come from the cancel transition.
    pub async fn cancel(
        &self,
        id: &WorkflowId,
        run: &RunId,
        reason: impl Into<String>,
    ) -> Result<(), EngineError> {
        let operation = self.shutdown_gate.begin_operation()?;
        // Tear down the run's in-flight durable timers BEFORE the cancel
        // transition. Cancellation that leaves a live timer behind orphans it:
        // recovery later tries to fire it against a workflow that no longer
        // exists. See `cancel_inflight_timers` for the ordering constraints.
        self.cancel_inflight_timers(id).await;
        let result = terminate::cancel(
            TerminateWorkflowContext {
                runtime: &self.runtime,
                store: self.store(),
                visibility_store: self.visibility_store(),
                registry: &self.registry,
                catalog: &self.catalog,
            },
            id,
            run,
            reason,
        )
        .await;
        // Cancel of a Paused run must release the dispatch hold so its held rows
        // are not leaked forever (#204, GATE-4). Removal is unconditional and
        // idempotent: a non-paused run is simply absent from the set.
        if result.is_ok() {
            self.paused_runs.remove(id);
        }
        drop(operation);
        result
    }

    /// Cancel the workflow's in-flight durable timers, routed through the
    /// production [`crate::time::TimerService`] so each records a
    /// `TimerCancelled` (and disarms the resident wheel) under the service's
    /// terminal-update guard. Once `TimerCancelled` is in history the timer is
    /// dead everywhere — a later wheel or recovery fire no-ops on the liveness
    /// check — so a cancelled workflow no longer leaves orphaned timers that
    /// brick startup recovery.
    ///
    /// Ordering matters and is the reason this lives in `Engine::cancel` rather
    /// than inside `terminate::cancel`:
    /// * It runs **before** `terminate::cancel`, while the workflow is still in
    ///   the registry (before `terminate::cancel`'s final `registry.remove`), so
    ///   the timer bridge's registry lookup succeeds. `UnknownWorkflow` is raised
    ///   only when the workflow is absent from the registry entirely — a
    ///   suspended (non-resident-but-registered) workflow is fine: its wheel
    ///   disarm is skipped but `TimerCancelled` is still recorded.
    /// * It runs **outside** `terminate::cancel`'s recorder lock —
    ///   `TimerService::cancel` re-acquires that same per-handle lock to record,
    ///   and the tokio mutex is not reentrant.
    ///
    /// Best-effort by design: every failure path here is backstopped by
    /// `recover_due`'s orphaned-timer skip (see [`crate::time`]'s recovery
    /// module), so it is logged but never fails the cancel. The only residual
    /// orphan window — a timer armed in the instant between enumeration and the
    /// process kill — is absorbed by that same recovery skip.
    async fn cancel_inflight_timers(&self, id: &WorkflowId) {
        let timer_service = match crate::runtime::nif_timer_bridge::installed_timer_service(
            self.runtime.nif_state(),
        ) {
            Ok(service) => service,
            Err(error) => {
                tracing::warn!(
                    %error,
                    workflow_id = %id,
                    "timer service unavailable during cancel; any in-flight timers will be skipped by recovery"
                );
                return;
            }
        };
        let history = match self.store.read_history(id).await {
            Ok(history) => history,
            Err(error) => {
                tracing::warn!(
                    %error,
                    workflow_id = %id,
                    "could not read history for timer cleanup during cancel; any in-flight timers will be skipped by recovery"
                );
                return;
            }
        };
        for timer_id in live_timers_in_active_segment(&history) {
            // A reserved workflow-deadline timer is retired PERMANENTLY
            // (`WorkflowIntent`): reopen must never resurrect it (a
            // `CancelTeardown` deadline would be re-armed at its original
            // `fire_at` by `rearmable_timers`). Every other in-flight timer is
            // ordinary cancel-teardown bookkeeping that reopen re-arms.
            let cause = if crate::time::is_deadline_timer(&timer_id) {
                TimerCancelCause::WorkflowIntent
            } else {
                TimerCancelCause::CancelTeardown
            };
            if let Err(error) = timer_service
                .cancel(id.clone(), timer_id.clone(), cause)
                .await
            {
                tracing::warn!(
                    %error,
                    workflow_id = %id,
                    %timer_id,
                    "failed to cancel in-flight timer during workflow cancel; recovery will skip it if orphaned"
                );
            }
        }
    }

    /// Continue a live workflow run as a new run under the same workflow id.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::ShuttingDown`] after shutdown begins, and
    /// [`EngineError::WorkflowNotFound`] when the `(workflow, run)` pair
    /// is not live. Other typed errors come from the continue-as-new transition.
    pub async fn continue_as_new(
        &self,
        id: &WorkflowId,
        run: &RunId,
        input: Payload,
        workflow_type: Option<String>,
    ) -> Result<WorkflowHandle, EngineError> {
        let operation = self.shutdown_gate.begin_operation()?;
        let result = continue_as_new::continue_as_new(
            ContinueAsNewContext {
                store: self.store(),
                visibility_store: Arc::clone(&self.visibility_store),
                catalog: Arc::clone(&self.catalog),
                runtime: &self.runtime,
                supervision: Arc::clone(&self.supervision),
                registry: &self.registry,
                search_attribute_schema: Arc::clone(&self.search_attribute_schema),
            },
            id,
            run,
            ContinueAsNewRequest {
                input,
                workflow_type,
            },
        )
        .await;
        drop(operation);
        result
    }

    /// Reopen a terminal-`Failed` or terminal-`Cancelled` run and re-drive it.
    ///
    /// Appends a single `WorkflowReopened` that supersedes the run's terminal
    /// event (returning it to Running), then respawns and re-drives the SAME run
    /// through the existing recovery path so replay returns every recorded result
    /// and only the reopened / in-flight step re-dispatches live, in the
    /// workflow's own namespace. Takes only a workflow id and run; the reopened
    /// steps and the namespace are derived from history.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::ShuttingDown`] after shutdown begins,
    /// [`EngineError::WorkflowNotFound`] when no history exists for the pair, and
    /// [`EngineError::InvalidState`] when the run is not a reopenable terminal
    /// (not terminal, terminal for Completed/`TimedOut`, or already Running).
    pub async fn reopen_workflow(
        &self,
        id: &WorkflowId,
        run: &RunId,
    ) -> Result<WorkflowHandle, EngineError> {
        let operation = self.shutdown_gate.begin_operation()?;
        let result = reopen::reopen(
            ReopenWorkflowContext {
                store: self.store(),
                visibility_store: Arc::clone(&self.visibility_store),
                catalog: Arc::clone(&self.catalog),
                runtime: &self.runtime,
                supervision: Arc::clone(&self.supervision),
                registry: &self.registry,
                search_attribute_schema: Arc::clone(&self.search_attribute_schema),
            },
            id,
            run,
        )
        .await;
        drop(operation);
        result
    }

    /// The shared dispatch-hold set for durable pause (#204).
    ///
    /// Handed to the outbox dispatcher at wiring time so a held (paused) run's
    /// rows are never claimed, and rebuilt from [`aion_store::EventStore::list_paused`] at
    /// startup/adoption.
    #[must_use]
    pub fn paused_runs(&self) -> crate::lifecycle::PausedRuns {
        self.paused_runs.clone()
    }

    /// Rebuild the dispatch-hold set from durable state (startup / shard
    /// adoption). A run projecting `Paused` is excluded from `list_active`
    /// respawn for free; this repopulates the hold so its pre-pause outbox rows
    /// stay unclaimed after a restart.
    ///
    /// # Errors
    ///
    /// Returns store errors from the `list_paused` scan.
    pub async fn rebuild_paused_runs(&self) -> Result<(), EngineError> {
        let paused = self.store.list_paused().await?;
        self.paused_runs.replace_all(paused);
        Ok(())
    }

    fn pause_context(&self) -> crate::lifecycle::PauseWorkflowContext<'_> {
        crate::lifecycle::PauseWorkflowContext {
            store: self.store(),
            visibility_store: Arc::clone(&self.visibility_store),
            catalog: Arc::clone(&self.catalog),
            runtime: &self.runtime,
            supervision: Arc::clone(&self.supervision),
            registry: &self.registry,
            search_attribute_schema: Arc::clone(&self.search_attribute_schema),
            paused_runs: self.paused_runs.clone(),
        }
    }

    /// Pause a live `Running` run, durably holding NEW activity dispatch (#204).
    ///
    /// Appends `WorkflowPaused` through the resident handle's own recorder and
    /// inserts the run into the dispatch-hold set; the resident process stays
    /// alive and keeps recording (timer fires, signals, drained completions).
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::ShuttingDown`] after shutdown begins,
    /// [`EngineError::WorkflowNotFound`] when the pair has no history / no
    /// resident handle, and [`EngineError::InvalidState`] — naming the actual
    /// status — when the run is not `Running`.
    pub async fn pause_workflow(
        &self,
        id: &WorkflowId,
        run: &RunId,
        reason: Option<String>,
        operator: Option<String>,
    ) -> Result<WorkflowHandle, EngineError> {
        let operation = self.shutdown_gate.begin_operation()?;
        let result =
            crate::lifecycle::pause::pause(&self.pause_context(), id, run, reason, operator).await;
        drop(operation);
        result
    }

    /// Resume a `Paused` run, releasing the dispatch hold (#204).
    ///
    /// Named `resume_paused_workflow` to avoid colliding with the existing
    /// residency-flip [`Engine::resume_workflow`]. Appends `WorkflowResumed`,
    /// removes the run from the dispatch-hold set, and — when the run crashed
    /// while paused and is no longer resident — respawns it via the reopen
    /// recovery path, re-arming unfired timers. The ordinary sweep then claims
    /// the released rows.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::ShuttingDown`] after shutdown begins,
    /// [`EngineError::WorkflowNotFound`] when the pair has no history, and
    /// [`EngineError::InvalidState`] — naming the actual status — when the run is
    /// not `Paused`.
    pub async fn resume_paused_workflow(
        &self,
        id: &WorkflowId,
        run: &RunId,
        operator: Option<String>,
    ) -> Result<WorkflowHandle, EngineError> {
        let operation = self.shutdown_gate.begin_operation()?;
        let result =
            crate::lifecycle::pause::resume(&self.pause_context(), id, run, operator).await;
        drop(operation);
        result
    }

    /// Await a workflow run's terminal result.
    ///
    /// Already-terminal histories return immediately. Live workflows await their
    /// completion notifier. Unknown workflow/run pairs return not found.
    ///
    /// # Errors
    ///
    /// Returns store, registry, or runtime channel errors as typed [`EngineError`]
    /// variants, or [`EngineError::WorkflowNotFound`] when no live handle or
    /// terminal history exists for the requested pair.
    pub async fn result(
        &self,
        id: &WorkflowId,
        run: &RunId,
    ) -> Result<Result<Payload, WorkflowError>, EngineError> {
        let history = self.store.read_history(id).await?;
        if let Some(outcome) = terminal_outcome_from_history(&history) {
            return Ok(outcome_to_result(outcome));
        }

        let handle = match self.registry.get(id, run)? {
            Some(handle) => handle,
            // Registration birth window: the run is durably started but its
            // handle insert has not landed yet (see
            // `Engine::handle_after_birth_window`).
            None => self
                .handle_after_birth_window(id, run, &history)
                .await?
                .ok_or_else(|| workflow_not_found(id, run))?,
        };
        let mut receiver = handle.completion().subscribe();
        loop {
            if let Some(outcome) = receiver.borrow().clone() {
                return Ok(outcome_to_result(outcome));
            }
            if receiver.changed().await.is_err() {
                if let Some(outcome) =
                    terminal_outcome_from_history(&self.store.read_history(id).await?)
                {
                    return Ok(outcome_to_result(outcome));
                }
                return Err(EngineError::Runtime {
                    reason: format!(
                        "completion channel closed before workflow `{id}/{run}` finished"
                    ),
                });
            }
        }
    }

    /// List live and terminal workflow summaries matching `filter`.
    ///
    /// Store projections are authoritative; live registry entries are projected
    /// from durable history before being merged and deduplicated.
    ///
    /// # Errors
    ///
    /// Returns typed store or registry errors when visibility data cannot be read.
    pub async fn list_workflows(
        &self,
        filter: WorkflowFilter,
    ) -> Result<Vec<WorkflowSummary>, EngineError> {
        let mut summaries = self
            .store
            .query(&filter)
            .await?
            .into_iter()
            .map(|summary| (summary.workflow_id.clone(), summary))
            .collect::<HashMap<_, _>>();

        for handle in self.registry.list()? {
            let history = self.store.read_history(handle.workflow_id()).await?;
            self.registry
                .reconcile(handle.workflow_id(), handle.run_id(), &history)?;
            if let Some(summary) = WorkflowSummary::from_history(&history) {
                if filter.matches(&summary) {
                    summaries.insert(summary.workflow_id.clone(), summary);
                }
            }
        }

        let mut summaries = summaries.into_values().collect::<Vec<_>>();
        summaries.sort_by(|left, right| {
            left.started_at.cmp(&right.started_at).then_with(|| {
                left.workflow_id
                    .to_string()
                    .cmp(&right.workflow_id.to_string())
            })
        });
        Ok(summaries)
    }
}

fn outcome_to_result(outcome: TerminalOutcome) -> Result<Payload, WorkflowError> {
    match outcome {
        TerminalOutcome::Completed(payload) => Ok(payload),
        TerminalOutcome::Failed(error) => Err(error),
        TerminalOutcome::Cancelled(reason) => Err(WorkflowError {
            message: format!("workflow cancelled: {reason}"),
            details: None,
        }),
        TerminalOutcome::TimedOut(timeout) => Err(WorkflowError {
            message: format!("workflow timed out: {timeout}"),
            details: None,
        }),
        TerminalOutcome::ContinuedAsNew { parent_run_id, .. } => Err(WorkflowError {
            message: format!("workflow continued as new from run {parent_run_id}"),
            details: None,
        }),
    }
}
