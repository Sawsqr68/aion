//! Durable timer service: schedule, wheel arm, and `TimerFired` delivery.

use std::sync::Arc;

use aion_core::{Event, EventEnvelope, TimerCancelCause, TimerId, WorkflowId};
use aion_store::{ReadableEventStore, StoreError};
use chrono::{DateTime, Utc};
use dashmap::DashSet;

use crate::engine_seam::{
    EngineHandle, EngineSeamError, RecordOutcome, TimerWheelEntry, WorkflowMailboxMessage,
    WorkflowResidency,
};
use crate::time::deadline::{DeadlineHandler, deadline_run_id, is_deadline_timer};

/// Durable timer scheduling and wheel-fire handling.
///
/// The service owns the AT live path for timers. Workflow-issued `TimerStarted` events are recorded
/// by AD's resume-live handoff before this service is called; this service persists only the durable
/// timer row and later asynchronous arrival/cancellation history through the engine recorder seam.
pub struct TimerService {
    engine: Arc<dyn EngineHandle>,
    store: Arc<dyn ReadableEventStore>,
    recorded_at: fn() -> DateTime<Utc>,
    /// Per-timer first-recorded-wins coordinator shared across EVERY service
    /// instance the production bridge hands out. Cancel and fire obtain
    /// SEPARATE service instances (the live wheel constructs one, `Engine::cancel`
    /// another), so a per-instance set would not exclude them; a shared `Arc`
    /// makes a cancel and a fire for the same timer mutually exclude — the
    /// #cancel-vs-fire race the review flagged. Bare unit-test services get their
    /// own set, which is correct for a single-instance test.
    terminal_updates: Arc<DashSet<(WorkflowId, TimerId)>>,
    /// Engine-registered handler for reserved `deadline:{run_id}` fires.
    ///
    /// `None` on a bare service (unit tests): a deadline fire is then a typed
    /// error, never a silent generic `TimerFired`. The production bridge sets it
    /// via [`Self::with_deadline_handler`] when constructing the service.
    deadline_handler: Option<Arc<dyn DeadlineHandler>>,
}

struct TerminalUpdateSlot<'a> {
    terminal_updates: &'a DashSet<(WorkflowId, TimerId)>,
    key: (WorkflowId, TimerId),
}

impl Drop for TerminalUpdateSlot<'_> {
    fn drop(&mut self) {
        self.terminal_updates.remove(&self.key);
    }
}

/// Errors returned by [`TimerService`].
#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum TimerServiceError {
    /// Durable timer storage or history inspection failed.
    #[error("timer store operation failed: {0}")]
    Store(#[from] StoreError),

    /// Engine seam operation failed.
    #[error("timer engine operation failed: {0}")]
    Engine(#[from] EngineSeamError),

    /// A reserved `deadline:{run_id}` timer fired but could not be routed to a
    /// registered deadline handler (or the handler failed).
    ///
    /// Never a silent generic fire: a deadline timer that reaches
    /// [`TimerService::fire_timer`] without a handler — or whose handler errors —
    /// surfaces here so the caller (live wheel or `recover_due`) observes the
    /// failure rather than recording a spurious `TimerFired`.
    #[error("deadline timer routing failed: {0}")]
    Deadline(String),
}

impl TimerService {
    /// Creates a durable timer service from the engine seam and timer store.
    #[must_use]
    pub fn new(engine: Arc<dyn EngineHandle>, store: Arc<dyn ReadableEventStore>) -> Self {
        Self::with_recorded_at(engine, store, Utc::now)
    }

    /// Creates a durable timer service with an injected history timestamp source.
    #[must_use]
    pub fn with_recorded_at(
        engine: Arc<dyn EngineHandle>,
        store: Arc<dyn ReadableEventStore>,
        recorded_at: fn() -> DateTime<Utc>,
    ) -> Self {
        Self {
            engine,
            store,
            recorded_at,
            terminal_updates: Arc::new(DashSet::new()),
            deadline_handler: None,
        }
    }

    /// Replaces this service's per-timer terminal-update coordinator with a
    /// shared one, returning the service for chaining.
    ///
    /// The production timer bridge owns ONE coordinator and hands it to every
    /// [`TimerService`] it constructs, so a cancel obtained from one service and
    /// a fire obtained from another still serialize per timer (first-recorded
    /// wins). Without this, each service would guard against itself only.
    #[must_use]
    pub fn with_terminal_updates(
        mut self,
        terminal_updates: Arc<DashSet<(WorkflowId, TimerId)>>,
    ) -> Self {
        self.terminal_updates = terminal_updates;
        self
    }

    /// Registers the engine-side deadline handler for reserved `deadline:{run_id}`
    /// fires, returning the service for chaining.
    ///
    /// The production timer bridge calls this so both the live wheel and
    /// `recover_due` (which share [`Self::fire_timer`]) demux a deadline fire to
    /// the handler instead of recording a generic `TimerFired`.
    #[must_use]
    pub fn with_deadline_handler(mut self, handler: Arc<dyn DeadlineHandler>) -> Self {
        self.deadline_handler = Some(handler);
        self
    }

    /// Schedules a durable timer and arms the live wheel when the workflow is resident.
    ///
    /// The operation persists the durable timer row and arms the wheel when needed. The
    /// command-issued `TimerStarted` recorder event is appended by AD's resume-live handoff before
    /// AE/AT reaches this service, so this method deliberately does not record it again.
    ///
    /// # Errors
    ///
    /// Returns [`TimerServiceError`] when durable storage, recording, residency resolution, or wheel
    /// arming fails.
    pub async fn schedule(
        &self,
        workflow_id: WorkflowId,
        timer_id: TimerId,
        fire_at: DateTime<Utc>,
    ) -> Result<(), TimerServiceError> {
        self.store
            .schedule_timer(&workflow_id, &timer_id, fire_at)
            .await?;

        if let WorkflowResidency::Resident(process) = self.engine.resolve_workflow(&workflow_id)? {
            self.engine.arm_timer(TimerWheelEntry {
                process,
                timer_id,
                fire_at,
            })?;
        }

        Ok(())
    }

    /// Cancels a durable timer that has not already reached a terminal timer state.
    ///
    /// Already-fired and already-cancelled timers are treated as idempotent no-ops. For active
    /// resident timers the live wheel is disarmed through the engine seam before `TimerCancelled` is
    /// recorded through the workflow recorder seam. Non-resident timers still record the cancellation
    /// so recovery/replay can suppress a later fire.
    ///
    /// Anonymous timers are accepted: authors can never address one (the SDK's `cancel_timer`
    /// takes a `TimerRef` minted by `start_timer`, which is always named), but the engine settles
    /// `with_timeout` scope deadlines — anonymous by construction — through this first-recorded-wins
    /// race against [`Self::fire_timer`].
    ///
    /// # Errors
    ///
    /// Returns [`TimerServiceError`] when history inspection, residency resolution, wheel disarming,
    /// or event recording fails.
    pub async fn cancel(
        &self,
        workflow_id: WorkflowId,
        timer_id: TimerId,
        cause: TimerCancelCause,
    ) -> Result<(), TimerServiceError> {
        let key = (workflow_id.clone(), timer_id.clone());
        let terminal_update_slot = self.wait_for_terminal_update_slot(key).await;

        let result = self.cancel_guarded(workflow_id, timer_id, cause).await;
        drop(terminal_update_slot);
        result
    }

    async fn cancel_guarded(
        &self,
        workflow_id: WorkflowId,
        timer_id: TimerId,
        cause: TimerCancelCause,
    ) -> Result<(), TimerServiceError> {
        if !self.timer_is_live(&workflow_id, &timer_id).await? {
            return Ok(());
        }

        if let WorkflowResidency::Resident(process) = self.engine.resolve_workflow(&workflow_id)? {
            self.engine.disarm_timer(process, &timer_id)?;
        }

        let event = Event::TimerCancelled {
            envelope: self.next_envelope(&workflow_id).await?,
            timer_id,
            cause,
        };
        self.engine.record_workflow_event(&workflow_id, event)?;

        Ok(())
    }

    /// Handles a live timer-wheel fire.
    ///
    /// `TimerFired` is recorded before any mailbox delivery. If the workflow is no longer resident,
    /// the recorded event remains the durable observation that replay/recovery can surface later.
    /// A fire whose `TimerFired` is ALREADY the timer's last recorded event (an earlier append
    /// landed while its acknowledgement was lost — aion#145) is not a no-op: for a resident
    /// workflow the fire re-enters the recorder seam, which reconciles the recorder's sequence
    /// forward without appending, and the owed mailbox wake is delivered.
    ///
    /// # Errors
    ///
    /// Returns [`TimerServiceError`] when history inspection, recording, residency resolution, or
    /// live mailbox delivery fails.
    pub async fn fire_timer(
        &self,
        workflow_id: WorkflowId,
        timer_id: TimerId,
        fire_at: DateTime<Utc>,
    ) -> Result<(), TimerServiceError> {
        let key = (workflow_id.clone(), timer_id.clone());
        let terminal_update_slot = self.wait_for_terminal_update_slot(key).await;

        let result = self
            .fire_timer_guarded(workflow_id, timer_id, fire_at)
            .await;
        drop(terminal_update_slot);
        result
    }

    async fn wait_for_terminal_update_slot(
        &self,
        key: (WorkflowId, TimerId),
    ) -> TerminalUpdateSlot<'_> {
        loop {
            if self.terminal_updates.insert(key.clone()) {
                return TerminalUpdateSlot {
                    terminal_updates: self.terminal_updates.as_ref(),
                    key,
                };
            }
            tokio::task::yield_now().await;
        }
    }

    async fn fire_timer_guarded(
        &self,
        workflow_id: WorkflowId,
        timer_id: TimerId,
        fire_at: DateTime<Utc>,
    ) -> Result<(), TimerServiceError> {
        // WHY the timer is not live decides what a fire still owes (aion#145).
        // A cancelled or absent timer owes nothing; a timer whose last event is
        // already `TimerFired` is the ack-loss shape — the durable record
        // landed while the recording call's acknowledgement was lost, so the
        // mailbox wake (and the recorder's sequence repair) may still be owed.
        // This service-layer read is the cheap gate that keeps genuine no-ops
        // (cancelled/absent/retired) out of the recorder seam; the bridge
        // re-checks the same fact under the recorder lock, and that check is
        // the authoritative one.
        let history = self.store.read_history(&workflow_id).await?;
        match timer_disposition_in_active_segment(&history, &timer_id) {
            TimerDisposition::Live => {}
            TimerDisposition::Fired if !is_deadline_timer(&timer_id) => {
                return self
                    .redeliver_recorded_fire(workflow_id, timer_id, fire_at)
                    .await;
            }
            // A retired deadline (which never records `TimerFired` through this
            // path), a cancelled timer, or a timer with no event in the active
            // segment: nothing is owed, exactly as before the #145 fix.
            TimerDisposition::Fired | TimerDisposition::Cancelled | TimerDisposition::Absent => {
                return Ok(());
            }
        }

        // Demux a reserved workflow-deadline timer out of the generic
        // record-then-deliver path (both the live wheel and `recover_due` reach
        // here): it never records a `TimerFired` — the registered handler records
        // `WorkflowTimedOut` and tears the run down instead.
        if is_deadline_timer(&timer_id) {
            return self.fire_deadline(workflow_id, timer_id).await;
        }

        let event = Event::TimerFired {
            envelope: self.next_envelope(&workflow_id).await?,
            timer_id: timer_id.clone(),
        };
        // Deliver the mailbox wake only when the durable record exists. The
        // recorder seam refuses a late fire that lands after the run terminated
        // (`RefusedTerminal`), recording nothing; waking the process then would
        // reschedule a workflow that has already reached its terminal — the
        // post-terminal wake this gate closes. `AlreadyRecorded` is the
        // opposite case: the record exists (an earlier acknowledgement-lost
        // append landed), so the wake is owed exactly as for `Recorded`.
        match self.engine.record_workflow_event(&workflow_id, event)? {
            RecordOutcome::RefusedTerminal => return Ok(()),
            RecordOutcome::Recorded | RecordOutcome::AlreadyRecorded => {}
        }

        if let WorkflowResidency::Resident(process) = self.engine.resolve_workflow(&workflow_id)? {
            self.engine.deliver_workflow_message(
                process,
                WorkflowMailboxMessage::TimerFired { timer_id, fire_at },
            )?;
        }

        Ok(())
    }

    /// Completes a fire whose durable `TimerFired` already exists but whose
    /// delivery — and possibly the recorder's own sequence advance — was lost
    /// (aion#145): the incident's ack-lost append, or a wake that failed after
    /// a fully recorded fire.
    ///
    /// Only a RESIDENT workflow owes a live wake, and only its still-held
    /// Recorder can be carrying the stale-low sequence the ack loss leaves
    /// behind: a non-resident workflow's replay on residency restore rebuilds
    /// its recorder from the durable head and consumes the recorded fire, so
    /// for it this is a clean no-op, exactly as before the fix.
    ///
    /// For the resident case the fire is put back through the recorder seam
    /// rather than woken directly: the bridge's check under the recorder lock
    /// is the authoritative "already recorded" decision, and it is where the
    /// recorder's in-memory sequence is reconciled forward to the durable head
    /// — without that repair the woken workflow's next append would mint a
    /// stale sequence and die on `SequenceConflict`, wedging the run one event
    /// later. The wake itself is a pure wake (the suspended await re-resolves
    /// from history), so a duplicate delivery is harmless by design.
    async fn redeliver_recorded_fire(
        &self,
        workflow_id: WorkflowId,
        timer_id: TimerId,
        fire_at: DateTime<Utc>,
    ) -> Result<(), TimerServiceError> {
        let WorkflowResidency::Resident(process) = self.engine.resolve_workflow(&workflow_id)?
        else {
            return Ok(());
        };

        let event = Event::TimerFired {
            envelope: self.next_envelope(&workflow_id).await?,
            timer_id: timer_id.clone(),
        };
        match self.engine.record_workflow_event(&workflow_id, event)? {
            // The run reached a terminal after the fire landed: the recorded
            // fire is inert history now, and no wake may reschedule the run.
            RecordOutcome::RefusedTerminal => return Ok(()),
            RecordOutcome::Recorded | RecordOutcome::AlreadyRecorded => {}
        }

        self.engine.deliver_workflow_message(
            process,
            WorkflowMailboxMessage::TimerFired {
                timer_id: timer_id.clone(),
                fire_at,
            },
        )?;
        tracing::info!(
            %workflow_id,
            %timer_id,
            "timer fire was already durably recorded; delivered the owed mailbox wake"
        );
        Ok(())
    }

    /// Route a live reserved-deadline fire to the registered handler.
    ///
    /// Called only for a `deadline:{run_id}` timer that passed the liveness
    /// guard. A missing handler or an unparseable run id is a typed
    /// [`TimerServiceError::Deadline`] — never a silent generic fire — and the
    /// handler's own failure is surfaced the same way. The handler re-checks the
    /// run's terminal under the recorder lock, so it loses cleanly to a
    /// concurrent completion.
    async fn fire_deadline(
        &self,
        workflow_id: WorkflowId,
        timer_id: TimerId,
    ) -> Result<(), TimerServiceError> {
        let handler = self.deadline_handler.as_ref().ok_or_else(|| {
            TimerServiceError::Deadline(format!(
                "no deadline handler registered for {timer_id} on workflow {workflow_id}"
            ))
        })?;
        let run_id = deadline_run_id(&timer_id).ok_or_else(|| {
            TimerServiceError::Deadline(format!(
                "malformed deadline timer {timer_id} on workflow {workflow_id}"
            ))
        })?;
        handler
            .on_deadline_elapsed(workflow_id, run_id)
            .await
            .map_err(|error| TimerServiceError::Deadline(error.to_string()))
    }

    /// Whether the timer is currently live (started and not since retired) in
    /// the workflow's active run segment, by last-event-wins.
    ///
    /// A timer belongs to the run that recorded its `TimerStarted`, and
    /// anonymous timer identities are run-scoped ordinals that replacement
    /// runs (continue-as-new) re-allocate from zero. Scoping the check to
    /// the latest run segment keeps a stale fire from a finished run from
    /// recording into — or suppressing — the replacement run's identically
    /// named timer.
    ///
    /// Liveness is decided by the *last* timer event for the id (see
    /// [`timer_disposition_in_active_segment`], the per-id view of the model
    /// [`live_timers_in_active_segment`] enumerates): a re-armed named timer
    /// (`TimerStarted(T), TimerFired(T), TimerStarted(T)`) is correctly live
    /// again rather than judged terminal forever by the earlier `TimerFired`.
    async fn timer_is_live(
        &self,
        workflow_id: &WorkflowId,
        timer_id: &TimerId,
    ) -> Result<bool, StoreError> {
        let history = self.store.read_history(workflow_id).await?;
        Ok(matches!(
            timer_disposition_in_active_segment(&history, timer_id),
            TimerDisposition::Live
        ))
    }

    async fn next_envelope(&self, workflow_id: &WorkflowId) -> Result<EventEnvelope, StoreError> {
        let history = self.store.read_history(workflow_id).await?;
        let seq = history.iter().map(Event::seq).max().unwrap_or_default() + 1;
        Ok(EventEnvelope {
            seq,
            recorded_at: (self.recorded_at)(),
            workflow_id: workflow_id.clone(),
        })
    }
}

/// The live timer ids in the workflow's active run segment, by last-event-wins.
///
/// Scans forward from the latest `WorkflowStarted` (the active run segment) and
/// lets the *last* event for each timer id decide its liveness: a `TimerStarted`
/// (re)arms it, a `TimerFired`/`TimerCancelled` retires it. This means a *named*
/// timer that fired or was cancelled and then re-armed within the same segment
/// (`TimerStarted(T), TimerFired(T), TimerStarted(T)`) is correctly reported live
/// again, rather than judged terminal forever by the earlier terminal event.
///
/// Start order is preserved and a timer id started more than once is deduped, so
/// the result is a stable, history-derived (and therefore replay-deterministic)
/// view of which timers are outstanding. This is the single liveness model shared
/// by [`TimerService::timer_is_live`] (firing/cancel guard) and the cancel-path
/// enumerator in `engine::api`, so the two cannot diverge.
pub(crate) fn live_timers_in_active_segment(history: &[Event]) -> Vec<TimerId> {
    let mut live: Vec<TimerId> = Vec::new();
    for event in active_segment(history) {
        match event {
            Event::TimerStarted { timer_id, .. } if !live.contains(timer_id) => {
                live.push(timer_id.clone());
            }
            Event::TimerFired { timer_id, .. } | Event::TimerCancelled { timer_id, .. } => {
                live.retain(|id| id != timer_id);
            }
            _ => {}
        }
    }
    live
}

/// The workflow's active run segment: everything from the latest
/// `WorkflowStarted` (the whole history when none is recorded — bare fixtures
/// and coordinator histories).
///
/// The single segment anchor shared by [`live_timers_in_active_segment`] and
/// [`timer_disposition_in_active_segment`], so the enumerating and the per-id
/// view of the liveness model cannot disagree about where the active run
/// begins.
fn active_segment(history: &[Event]) -> &[Event] {
    let segment_start = history
        .iter()
        .rposition(|event| matches!(event, Event::WorkflowStarted { .. }))
        .unwrap_or(0);
    &history[segment_start..]
}

/// The recorded fate of ONE timer in the workflow's active run segment, by the
/// same last-event-wins rule as [`live_timers_in_active_segment`].
///
/// [`live_timers_in_active_segment`] can only answer "live or not"; the fire
/// path needs to know WHY a timer is not live (aion#145): a timer whose last
/// event is `TimerFired` already has its durable record — the fire's mailbox
/// wake may still be owed — while a cancelled or absent timer owes nothing.
/// This is the per-id view of the SAME model, not a fork of it: same segment
/// anchor ([`active_segment`]), same last-event-wins traversal, so for every
/// history and timer id, `Live` here if and only if the id appears in
/// [`live_timers_in_active_segment`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TimerDisposition {
    /// The timer's last event in the active segment is `TimerStarted`: live.
    Live,
    /// The timer's last event in the active segment is `TimerFired`: the
    /// durable fire record exists (its mailbox wake may or may not have been
    /// delivered — history cannot tell, which is why delivery is a pure,
    /// duplicate-safe wake).
    Fired,
    /// The timer's last event in the active segment is `TimerCancelled`.
    Cancelled,
    /// The timer has no event in the active segment (never started there, or
    /// started only in a prior, closed run segment).
    Absent,
}

/// Computes [`TimerDisposition`] for `timer_id` over `history`.
pub(crate) fn timer_disposition_in_active_segment(
    history: &[Event],
    timer_id: &TimerId,
) -> TimerDisposition {
    let mut disposition = TimerDisposition::Absent;
    for event in active_segment(history) {
        match event {
            Event::TimerStarted { timer_id: id, .. } if id == timer_id => {
                disposition = TimerDisposition::Live;
            }
            Event::TimerFired { timer_id: id, .. } if id == timer_id => {
                disposition = TimerDisposition::Fired;
            }
            Event::TimerCancelled { timer_id: id, .. } if id == timer_id => {
                disposition = TimerDisposition::Cancelled;
            }
            _ => {}
        }
    }
    disposition
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use aion_core::{Event, EventEnvelope, RunId, TimerCancelCause, TimerId, WorkflowId};
    use aion_store::{InMemoryStore, ReadableEventStore, StoreError, WritableEventStore};
    use chrono::{DateTime, Utc};

    use super::{
        TimerDisposition, TimerService, TimerServiceError, live_timers_in_active_segment,
        timer_disposition_in_active_segment,
    };
    use crate::engine_seam::test_support::{
        DeliveredWorkflowMessage, FakeEngineHandle, FakeEngineOperation,
    };
    use crate::engine_seam::{
        EngineHandle, TimerWheelEntry, WorkflowProcessHandle, WorkflowResidency,
    };
    use crate::time::deadline::{DeadlineHandler, DeadlineHandlerError, deadline_timer_id};

    fn instant(offset_seconds: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(1_700_000_000 + offset_seconds, 0).unwrap_or_default()
    }

    fn workflow_id() -> WorkflowId {
        WorkflowId::new_v4()
    }

    fn timer_id() -> TimerId {
        TimerId::anonymous(7)
    }

    fn service() -> (Arc<InMemoryStore>, Arc<FakeEngineHandle>, TimerService) {
        let concrete_store = Arc::new(InMemoryStore::default());
        let recorder_store: Arc<dyn WritableEventStore> = concrete_store.clone();
        let readable_store: Arc<dyn ReadableEventStore> = concrete_store.clone();
        let engine = Arc::new(FakeEngineHandle::recording_to(recorder_store));
        let service = TimerService::with_recorded_at(engine.clone(), readable_store, recorded_at);
        (concrete_store, engine, service)
    }

    fn recorded_at() -> DateTime<Utc> {
        instant(1)
    }

    async fn history(
        store: &InMemoryStore,
        workflow_id: &WorkflowId,
    ) -> Result<Vec<Event>, StoreError> {
        store.read_history(workflow_id).await
    }

    fn count_timer_fired(events: &[Event], timer_id: &TimerId) -> usize {
        events
            .iter()
            .filter(|event| {
                matches!(event, Event::TimerFired { timer_id: recorded, .. } if recorded == timer_id)
            })
            .count()
    }

    fn timer_started_event(workflow_id: &WorkflowId, timer_id: &TimerId, seq: u64) -> Event {
        Event::TimerStarted {
            envelope: EventEnvelope {
                seq,
                recorded_at: instant(0),
                workflow_id: workflow_id.clone(),
            },
            timer_id: timer_id.clone(),
            fire_at: instant(5),
        }
    }

    fn workflow_started_event(workflow_id: &WorkflowId, seq: u64) -> Event {
        Event::WorkflowStarted {
            envelope: EventEnvelope {
                seq,
                recorded_at: instant(0),
                workflow_id: workflow_id.clone(),
            },
            workflow_type: "fixture".to_owned(),
            input: aion_core::Payload::new(aion_core::ContentType::Json, b"null".to_vec()),
            run_id: aion_core::RunId::new_v4(),
            parent_run_id: None,
            parent_workflow_id: None,
            package_version: aion_core::PackageVersion::new("a".repeat(64)),
        }
    }

    fn timer_fired_event(workflow_id: &WorkflowId, timer_id: &TimerId, seq: u64) -> Event {
        Event::TimerFired {
            envelope: EventEnvelope {
                seq,
                recorded_at: instant(0),
                workflow_id: workflow_id.clone(),
            },
            timer_id: timer_id.clone(),
        }
    }

    fn timer_cancelled_event(workflow_id: &WorkflowId, timer_id: &TimerId, seq: u64) -> Event {
        Event::TimerCancelled {
            cause: TimerCancelCause::WorkflowIntent,
            envelope: EventEnvelope {
                seq,
                recorded_at: instant(0),
                workflow_id: workflow_id.clone(),
            },
            timer_id: timer_id.clone(),
        }
    }

    fn make_named(name: &str) -> TimerId {
        // The name is a non-empty literal, so construction never fails; the
        // anonymous fallback only exists to keep the helper total without an
        // `unwrap`/`expect` (disallowed by clippy in this crate).
        TimerId::named(name).unwrap_or_else(|_| TimerId::anonymous(0))
    }

    fn named_timer_id() -> TimerId {
        make_named("review-deadline")
    }

    // --- `live_timers_in_active_segment` / `timer_is_live` semantics ---

    #[test]
    fn started_timer_is_live() {
        let workflow_id = workflow_id();
        let timer_id = named_timer_id();
        let history = vec![
            workflow_started_event(&workflow_id, 0),
            timer_started_event(&workflow_id, &timer_id, 1),
        ];
        assert_eq!(live_timers_in_active_segment(&history), vec![timer_id]);
    }

    #[test]
    fn started_then_fired_timer_is_dead() {
        let workflow_id = workflow_id();
        let timer_id = named_timer_id();
        let history = vec![
            workflow_started_event(&workflow_id, 0),
            timer_started_event(&workflow_id, &timer_id, 1),
            timer_fired_event(&workflow_id, &timer_id, 2),
        ];
        assert!(live_timers_in_active_segment(&history).is_empty());
    }

    #[test]
    fn started_then_cancelled_timer_is_dead() {
        let workflow_id = workflow_id();
        let timer_id = named_timer_id();
        let history = vec![
            workflow_started_event(&workflow_id, 0),
            timer_started_event(&workflow_id, &timer_id, 1),
            timer_cancelled_event(&workflow_id, &timer_id, 2),
        ];
        assert!(live_timers_in_active_segment(&history).is_empty());
    }

    #[test]
    fn restarted_named_timer_after_fire_is_live() {
        // The bug fix: a named timer that fired then was re-armed in the same run
        // segment must be live again (last-event-wins), not judged terminal forever
        // by the earlier `TimerFired`.
        let workflow_id = workflow_id();
        let timer_id = named_timer_id();
        let history = vec![
            workflow_started_event(&workflow_id, 0),
            timer_started_event(&workflow_id, &timer_id, 1),
            timer_fired_event(&workflow_id, &timer_id, 2),
            timer_started_event(&workflow_id, &timer_id, 3),
        ];
        assert_eq!(
            live_timers_in_active_segment(&history),
            vec![timer_id],
            "a re-armed named timer is live again"
        );
    }

    #[test]
    fn restarted_named_timer_after_cancel_is_live() {
        let workflow_id = workflow_id();
        let timer_id = named_timer_id();
        let history = vec![
            workflow_started_event(&workflow_id, 0),
            timer_started_event(&workflow_id, &timer_id, 1),
            timer_cancelled_event(&workflow_id, &timer_id, 2),
            timer_started_event(&workflow_id, &timer_id, 3),
        ];
        assert_eq!(live_timers_in_active_segment(&history), vec![timer_id]);
    }

    #[test]
    fn prior_run_segment_timer_is_not_live() {
        // A timer started in a run segment that a later `WorkflowStarted` closed
        // (continue-as-new) is out of scope for the active segment.
        let workflow_id = workflow_id();
        let prior = named_timer_id();
        let current = make_named("current-deadline");
        let history = vec![
            workflow_started_event(&workflow_id, 0),
            timer_started_event(&workflow_id, &prior, 1),
            // New run segment begins; the prior timer must not be surfaced.
            workflow_started_event(&workflow_id, 2),
            timer_started_event(&workflow_id, &current, 3),
        ];
        assert_eq!(live_timers_in_active_segment(&history), vec![current]);
    }

    /// The per-id disposition view (aion#145) must agree with the enumerating
    /// liveness model on every shape: `Live` exactly when the id appears in
    /// [`live_timers_in_active_segment`], with the not-live cases split by WHY.
    /// Each case asserts both views so the two traversals cannot drift.
    #[test]
    fn disposition_tracks_the_last_event_for_the_id_and_agrees_with_liveness() {
        let workflow_id = workflow_id();
        let timer_id = named_timer_id();
        let other = make_named("unrelated");
        let assert_agrees = |history: &[Event], expected: TimerDisposition| {
            assert_eq!(
                timer_disposition_in_active_segment(history, &timer_id),
                expected
            );
            assert_eq!(
                live_timers_in_active_segment(history).contains(&timer_id),
                expected == TimerDisposition::Live,
                "the per-id disposition and the enumerating model disagree on liveness"
            );
        };

        // Started → live.
        let mut history = vec![
            workflow_started_event(&workflow_id, 0),
            timer_started_event(&workflow_id, &timer_id, 1),
        ];
        assert_agrees(&history, TimerDisposition::Live);

        // Fired at head → the durable record exists (the aion#145 shape).
        history.push(timer_fired_event(&workflow_id, &timer_id, 2));
        assert_agrees(&history, TimerDisposition::Fired);

        // A fire for an UNRELATED id must not disturb this timer's disposition.
        history.push(timer_fired_event(&workflow_id, &other, 3));
        assert_agrees(&history, TimerDisposition::Fired);

        // Re-armed after the fire → live again (last-event-wins).
        history.push(timer_started_event(&workflow_id, &timer_id, 4));
        assert_agrees(&history, TimerDisposition::Live);

        // Cancelled at head → retired, owed nothing.
        history.push(timer_cancelled_event(&workflow_id, &timer_id, 5));
        assert_agrees(&history, TimerDisposition::Cancelled);

        // A new run segment closes the book: the id is absent from the active
        // segment even though the prior segment fired and cancelled it.
        history.push(workflow_started_event(&workflow_id, 6));
        assert_agrees(&history, TimerDisposition::Absent);

        // And with no events at all it was absent to begin with.
        assert_agrees(&[], TimerDisposition::Absent);
    }

    #[tokio::test]
    async fn re_armed_named_timer_fires_again() -> Result<(), TimerServiceError> {
        // End-to-end firing-path guard: with last-event-wins, a re-armed named
        // timer is live, so `fire_timer` records a second `TimerFired` and
        // delivers it — rather than silently no-opping under the old
        // `any`-semantics.
        let process = WorkflowProcessHandle::new(42);
        let (store, engine, service) = service();
        let workflow_id = workflow_id();
        let timer_id = named_timer_id();
        let fire_at = instant(110);
        engine.set_residency(workflow_id.clone(), WorkflowResidency::Resident(process))?;
        engine.record_workflow_event(
            &workflow_id,
            timer_started_event(&workflow_id, &timer_id, 1),
        )?;
        engine
            .record_workflow_event(&workflow_id, timer_fired_event(&workflow_id, &timer_id, 2))?;
        engine.record_workflow_event(
            &workflow_id,
            timer_started_event(&workflow_id, &timer_id, 3),
        )?;

        service
            .fire_timer(workflow_id.clone(), timer_id.clone(), fire_at)
            .await?;

        assert_eq!(
            count_timer_fired(&history(&store, &workflow_id).await?, &timer_id),
            2,
            "the re-armed timer fires again, recording a second TimerFired"
        );
        assert_eq!(engine.delivered_messages()?.len(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn schedule_records_timer_row_without_timer_started_event()
    -> Result<(), TimerServiceError> {
        let (store, _engine, service) = service();
        let workflow_id = workflow_id();
        let timer_id = timer_id();
        let fire_at = instant(10);

        service
            .schedule(workflow_id.clone(), timer_id.clone(), fire_at)
            .await?;

        let expired = store.expired_timers(fire_at).await?;
        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0].workflow_id, workflow_id);
        assert_eq!(expired[0].timer_id, timer_id);
        assert_eq!(expired[0].fire_at, fire_at);

        assert!(history(&store, &workflow_id).await?.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn schedule_arms_wheel_for_resident_workflow() -> Result<(), TimerServiceError> {
        let process = WorkflowProcessHandle::new(42);
        let (_store, engine, service) = service();
        let workflow_id = workflow_id();
        let timer_id = timer_id();
        let fire_at = instant(20);
        engine.set_residency(workflow_id.clone(), WorkflowResidency::Resident(process))?;

        service
            .schedule(workflow_id, timer_id.clone(), fire_at)
            .await?;

        assert_eq!(
            engine.armed_timers()?,
            vec![TimerWheelEntry {
                process,
                timer_id,
                fire_at
            }]
        );
        Ok(())
    }

    #[tokio::test]
    async fn schedule_for_nonresident_records_without_arming() -> Result<(), TimerServiceError> {
        let (store, engine, service) = service();
        let workflow_id = workflow_id();
        let timer_id = timer_id();
        let fire_at = instant(30);
        engine.set_residency(workflow_id.clone(), WorkflowResidency::NonResident)?;

        service
            .schedule(workflow_id.clone(), timer_id, fire_at)
            .await?;

        assert!(engine.armed_timers()?.is_empty());
        assert!(history(&store, &workflow_id).await?.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn fire_records_timer_fired_then_delivers_mailbox_message()
    -> Result<(), TimerServiceError> {
        let process = WorkflowProcessHandle::new(42);
        let (store, engine, service) = service();
        let workflow_id = workflow_id();
        let timer_id = timer_id();
        let fire_at = instant(40);
        engine.set_residency(workflow_id.clone(), WorkflowResidency::Resident(process))?;
        engine.record_workflow_event(
            &workflow_id,
            timer_started_event(&workflow_id, &timer_id, 1),
        )?;

        service
            .fire_timer(workflow_id.clone(), timer_id.clone(), fire_at)
            .await?;

        assert_eq!(
            count_timer_fired(&history(&store, &workflow_id).await?, &timer_id),
            1
        );
        assert_eq!(
            engine.delivered_messages()?,
            vec![(
                process,
                DeliveredWorkflowMessage::TimerFired {
                    timer_id: timer_id.clone(),
                    fire_at
                }
            )]
        );
        assert!(matches!(
            engine.operations()?.as_slice(),
            [
                FakeEngineOperation::EventRecorded {
                    event: Event::TimerStarted { .. },
                    ..
                },
                FakeEngineOperation::EventRecorded {
                    workflow_id: recorded_workflow_id,
                    event: Event::TimerFired { timer_id: recorded_timer_id, .. },
                },
                FakeEngineOperation::Delivered {
                    process: delivered_process,
                    message: DeliveredWorkflowMessage::TimerFired { timer_id: delivered_timer_id, .. },
                }
            ] if recorded_workflow_id == &workflow_id
                && recorded_timer_id == &timer_id
                && delivered_process == &process
                && delivered_timer_id == &timer_id
        ));
        Ok(())
    }

    #[tokio::test]
    async fn fire_records_without_delivery_when_workflow_becomes_nonresident()
    -> Result<(), TimerServiceError> {
        let (store, engine, service) = service();
        let workflow_id = workflow_id();
        let timer_id = timer_id();
        let fire_at = instant(50);
        engine.set_residency(workflow_id.clone(), WorkflowResidency::NonResident)?;
        engine.record_workflow_event(
            &workflow_id,
            timer_started_event(&workflow_id, &timer_id, 1),
        )?;

        service
            .fire_timer(workflow_id.clone(), timer_id.clone(), fire_at)
            .await?;

        assert_eq!(
            count_timer_fired(&history(&store, &workflow_id).await?, &timer_id),
            1
        );
        assert!(engine.delivered_messages()?.is_empty());
        Ok(())
    }

    /// aion#145: a second fire of an already-fired timer records nothing new
    /// but RE-DELIVERS the owed wake to a resident workflow. Delivery is a pure
    /// wake (the suspended await re-resolves from history), so a duplicate is
    /// harmless — while the pre-fix silent no-op is exactly what wedged the
    /// incident's workflows: the durable `TimerFired` existed and the resident
    /// process waited forever on a wake that never came. Mutation-sensitive:
    /// reverting the `Fired`-disposition branch to a plain `Ok(())` leaves one
    /// delivery; a second append would raise the fired count to two.
    #[tokio::test]
    async fn firing_same_timer_twice_records_once_and_redelivers_the_wake()
    -> Result<(), TimerServiceError> {
        let process = WorkflowProcessHandle::new(42);
        let (store, engine, service) = service();
        let workflow_id = workflow_id();
        let timer_id = timer_id();
        let fire_at = instant(60);
        engine.set_residency(workflow_id.clone(), WorkflowResidency::Resident(process))?;
        engine.record_workflow_event(
            &workflow_id,
            timer_started_event(&workflow_id, &timer_id, 1),
        )?;

        service
            .fire_timer(workflow_id.clone(), timer_id.clone(), fire_at)
            .await?;
        // The second fire re-enters the recorder seam, which answers
        // `AlreadyRecorded` without appending (the fake implements the seam
        // contract; the real bridge's under-lock decision — including the
        // recorder-sequence reconciliation — is pinned in
        // `nif_timer_bridge_tests`).
        service
            .fire_timer(workflow_id.clone(), timer_id.clone(), fire_at)
            .await?;

        assert_eq!(
            count_timer_fired(&history(&store, &workflow_id).await?, &timer_id),
            1,
            "the recorded fire must never be appended a second time"
        );
        assert_eq!(
            engine.delivered_messages()?.len(),
            2,
            "the second fire re-delivers the owed wake instead of silently no-opping"
        );
        Ok(())
    }

    /// aion#145 Parts 2+3, service-seam mapping: a fire whose `TimerFired` is
    /// already the timer's last recorded event (the incident's ack-lost append)
    /// must NOT no-op for a resident workflow — it re-enters the recorder seam
    /// (which answers `AlreadyRecorded` without appending) and then delivers
    /// the owed wake. Mutation-sensitive both ways: reverting the
    /// `Fired`-disposition branch to `Ok(())` delivers nothing, and mapping
    /// `AlreadyRecorded` like `RefusedTerminal` delivers nothing.
    #[tokio::test]
    async fn already_recorded_fire_delivers_owed_wake_without_second_append()
    -> Result<(), TimerServiceError> {
        let process = WorkflowProcessHandle::new(42);
        let (store, engine, service) = service();
        let workflow_id = workflow_id();
        let timer_id = timer_id();
        let fire_at = instant(140);
        engine.set_residency(workflow_id.clone(), WorkflowResidency::Resident(process))?;
        engine.record_workflow_event(
            &workflow_id,
            timer_started_event(&workflow_id, &timer_id, 1),
        )?;
        engine
            .record_workflow_event(&workflow_id, timer_fired_event(&workflow_id, &timer_id, 2))?;

        service
            .fire_timer(workflow_id.clone(), timer_id.clone(), fire_at)
            .await?;

        assert_eq!(
            count_timer_fired(&history(&store, &workflow_id).await?, &timer_id),
            1,
            "the already-recorded fire must not be appended again"
        );
        assert_eq!(
            engine.delivered_messages()?,
            vec![(
                process,
                DeliveredWorkflowMessage::TimerFired { timer_id, fire_at }
            )],
            "the owed mailbox wake must be delivered"
        );
        Ok(())
    }

    /// aion#145 test matrix row 3: fired-but-undelivered for a NON-resident
    /// workflow is a clean no-op — no wake is attempted (there is no live
    /// process to wake) and the recorder seam is not re-entered: replay on
    /// residency restore rebuilds the recorder from the durable head and
    /// consumes the recorded fire. Load-bearing assertions: exactly one
    /// durable `TimerFired`, and an empty delivery log.
    #[tokio::test]
    async fn already_recorded_fire_for_nonresident_workflow_wakes_nothing()
    -> Result<(), TimerServiceError> {
        let (store, engine, service) = service();
        let workflow_id = workflow_id();
        let timer_id = timer_id();
        engine.set_residency(workflow_id.clone(), WorkflowResidency::NonResident)?;
        engine.record_workflow_event(
            &workflow_id,
            timer_started_event(&workflow_id, &timer_id, 1),
        )?;
        engine
            .record_workflow_event(&workflow_id, timer_fired_event(&workflow_id, &timer_id, 2))?;

        service
            .fire_timer(workflow_id.clone(), timer_id.clone(), instant(150))
            .await?;

        assert_eq!(
            count_timer_fired(&history(&store, &workflow_id).await?, &timer_id),
            1,
            "a non-resident redelivery must not re-enter the recorder seam"
        );
        assert!(
            engine.delivered_messages()?.is_empty(),
            "no wake is attempted for a non-resident workflow"
        );
        Ok(())
    }

    /// aion#145: the redelivery path still honors the post-terminal refusal.
    /// A recorded fire whose run has since reached a terminal gets NO wake —
    /// the recorder seam answers `RefusedTerminal` and the recorded fire is
    /// inert history. Mutation-sensitive: delivering the wake regardless of the
    /// refusal would reschedule a terminated workflow.
    #[tokio::test]
    async fn already_recorded_fire_after_run_terminal_delivers_no_wake()
    -> Result<(), TimerServiceError> {
        let process = WorkflowProcessHandle::new(42);
        let (store, engine, service) = service();
        let workflow_id = workflow_id();
        let timer_id = timer_id();
        engine.set_residency(workflow_id.clone(), WorkflowResidency::Resident(process))?;
        engine.record_workflow_event(
            &workflow_id,
            timer_started_event(&workflow_id, &timer_id, 1),
        )?;
        engine
            .record_workflow_event(&workflow_id, timer_fired_event(&workflow_id, &timer_id, 2))?;
        engine.refuse_next_record_as_terminal()?;

        service
            .fire_timer(workflow_id.clone(), timer_id.clone(), instant(160))
            .await?;

        assert_eq!(
            count_timer_fired(&history(&store, &workflow_id).await?, &timer_id),
            1
        );
        assert!(
            engine.delivered_messages()?.is_empty(),
            "a post-terminal redelivery must not wake the terminated run"
        );
        Ok(())
    }

    #[tokio::test]
    async fn firing_cancelled_timer_is_noop() -> Result<(), TimerServiceError> {
        let process = WorkflowProcessHandle::new(42);
        let (store, engine, service) = service();
        let workflow_id = workflow_id();
        let timer_id = timer_id();
        let fire_at = instant(70);
        engine.set_residency(workflow_id.clone(), WorkflowResidency::Resident(process))?;
        engine.record_workflow_event(
            &workflow_id,
            timer_started_event(&workflow_id, &timer_id, 1),
        )?;
        let cancelled = Event::TimerCancelled {
            cause: TimerCancelCause::WorkflowIntent,
            envelope: EventEnvelope {
                seq: 2,
                recorded_at: instant(69),
                workflow_id: workflow_id.clone(),
            },
            timer_id: timer_id.clone(),
        };
        engine.record_workflow_event(&workflow_id, cancelled)?;

        service
            .fire_timer(workflow_id.clone(), timer_id.clone(), fire_at)
            .await?;

        let history = history(&store, &workflow_id).await?;
        assert_eq!(count_timer_fired(&history, &timer_id), 0);
        assert!(engine.delivered_messages()?.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn fire_resolves_residency_at_fire_time() -> Result<(), TimerServiceError> {
        let process = WorkflowProcessHandle::new(42);
        let (store, engine, service) = service();
        let workflow_id = workflow_id();
        let timer_id = timer_id();
        let fire_at = instant(80);

        engine.set_residency(workflow_id.clone(), WorkflowResidency::Resident(process))?;
        engine.set_residency(workflow_id.clone(), WorkflowResidency::NonResident)?;
        engine.record_workflow_event(
            &workflow_id,
            timer_started_event(&workflow_id, &timer_id, 1),
        )?;
        service
            .fire_timer(workflow_id.clone(), timer_id.clone(), fire_at)
            .await?;

        assert_eq!(
            count_timer_fired(&history(&store, &workflow_id).await?, &timer_id),
            1
        );
        assert!(engine.delivered_messages()?.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn firing_unstarted_timer_records_nothing() -> Result<(), TimerServiceError> {
        let process = WorkflowProcessHandle::new(42);
        let (store, engine, service) = service();
        let workflow_id = workflow_id();
        let timer_id = timer_id();
        engine.set_residency(workflow_id.clone(), WorkflowResidency::Resident(process))?;

        service
            .fire_timer(workflow_id.clone(), timer_id.clone(), instant(90))
            .await?;

        assert!(history(&store, &workflow_id).await?.is_empty());
        assert!(engine.delivered_messages()?.is_empty());
        Ok(())
    }

    /// A deadline handler that records each fire and can be told to fail.
    struct RecordingDeadlineHandler {
        calls: std::sync::Mutex<Vec<(WorkflowId, RunId)>>,
        fail: bool,
    }

    impl RecordingDeadlineHandler {
        fn new(fail: bool) -> Self {
            Self {
                calls: std::sync::Mutex::new(Vec::new()),
                fail,
            }
        }

        fn calls(&self) -> Result<Vec<(WorkflowId, RunId)>, TimerServiceError> {
            self.calls
                .lock()
                .map(|calls| calls.clone())
                .map_err(|error| TimerServiceError::Deadline(error.to_string()))
        }
    }

    #[async_trait::async_trait]
    impl DeadlineHandler for RecordingDeadlineHandler {
        async fn on_deadline_elapsed(
            &self,
            workflow_id: WorkflowId,
            run_id: RunId,
        ) -> Result<(), DeadlineHandlerError> {
            self.calls
                .lock()
                .map_err(|error| DeadlineHandlerError(error.to_string()))?
                .push((workflow_id, run_id));
            if self.fail {
                Err(DeadlineHandlerError(
                    "deliberate handler failure".to_owned(),
                ))
            } else {
                Ok(())
            }
        }
    }

    fn service_with_handler(
        handler: Arc<dyn DeadlineHandler>,
    ) -> (Arc<InMemoryStore>, Arc<FakeEngineHandle>, TimerService) {
        let concrete_store = Arc::new(InMemoryStore::default());
        let recorder_store: Arc<dyn WritableEventStore> = concrete_store.clone();
        let readable_store: Arc<dyn ReadableEventStore> = concrete_store.clone();
        let engine = Arc::new(FakeEngineHandle::recording_to(recorder_store));
        let service = TimerService::with_recorded_at(engine.clone(), readable_store, recorded_at)
            .with_deadline_handler(handler);
        (concrete_store, engine, service)
    }

    /// A live reserved deadline fire is demuxed to the registered handler with
    /// the id-encoded run, and records NO `TimerFired` and delivers nothing.
    #[tokio::test]
    async fn deadline_fire_routes_to_handler_and_records_no_timer_fired()
    -> Result<(), TimerServiceError> {
        let run_id = RunId::new_v4();
        let deadline_id = deadline_timer_id(&run_id)
            .map_err(|error| TimerServiceError::Deadline(error.to_string()))?;
        let handler = Arc::new(RecordingDeadlineHandler::new(false));
        let (store, engine, service) = service_with_handler(handler.clone());
        let workflow_id = workflow_id();
        let fire_at = instant(120);
        engine.set_residency(
            workflow_id.clone(),
            WorkflowResidency::Resident(WorkflowProcessHandle::new(9)),
        )?;
        engine.record_workflow_event(
            &workflow_id,
            timer_started_event(&workflow_id, &deadline_id, 1),
        )?;

        service
            .fire_timer(workflow_id.clone(), deadline_id.clone(), fire_at)
            .await?;

        assert_eq!(handler.calls()?, vec![(workflow_id.clone(), run_id)]);
        assert_eq!(
            count_timer_fired(&history(&store, &workflow_id).await?, &deadline_id),
            0,
            "a deadline fire never records TimerFired"
        );
        assert!(engine.delivered_messages()?.is_empty());
        Ok(())
    }

    /// A deadline fire with no handler registered is a typed error — never a
    /// silent generic fire.
    #[tokio::test]
    async fn deadline_fire_without_handler_is_typed_error() -> Result<(), TimerServiceError> {
        let run_id = RunId::new_v4();
        let deadline_id = deadline_timer_id(&run_id)
            .map_err(|error| TimerServiceError::Deadline(error.to_string()))?;
        let (store, engine, service) = service();
        let workflow_id = workflow_id();
        engine.record_workflow_event(
            &workflow_id,
            timer_started_event(&workflow_id, &deadline_id, 1),
        )?;

        let result = service
            .fire_timer(workflow_id.clone(), deadline_id.clone(), instant(120))
            .await;

        assert!(
            matches!(result, Err(TimerServiceError::Deadline(_))),
            "unhandled deadline fire must be a typed error, got {result:?}"
        );
        assert_eq!(
            count_timer_fired(&history(&store, &workflow_id).await?, &deadline_id),
            0
        );
        Ok(())
    }

    /// A handler failure surfaces as a typed deadline error to the caller.
    #[tokio::test]
    async fn deadline_handler_failure_surfaces_as_typed_error() -> Result<(), TimerServiceError> {
        let run_id = RunId::new_v4();
        let deadline_id = deadline_timer_id(&run_id)
            .map_err(|error| TimerServiceError::Deadline(error.to_string()))?;
        let handler = Arc::new(RecordingDeadlineHandler::new(true));
        let (_store, engine, service) = service_with_handler(handler);
        let workflow_id = workflow_id();
        engine.record_workflow_event(
            &workflow_id,
            timer_started_event(&workflow_id, &deadline_id, 1),
        )?;

        let result = service
            .fire_timer(workflow_id, deadline_id, instant(120))
            .await;

        assert!(matches!(result, Err(TimerServiceError::Deadline(_))));
        Ok(())
    }

    /// A fire the recorder refuses as a post-terminal late arrival records
    /// nothing AND delivers no wake. Mutation-sensitive: the timer is live so the
    /// pre-check passes and the fire reaches the recorder seam, which returns
    /// `RefusedTerminal`; delivering the mailbox wake regardless of that outcome
    /// would reschedule a terminated workflow and fail this test.
    #[tokio::test]
    async fn refused_terminal_fire_records_nothing_and_delivers_no_wake()
    -> Result<(), TimerServiceError> {
        let process = WorkflowProcessHandle::new(42);
        let (store, engine, service) = service();
        let workflow_id = workflow_id();
        let timer_id = timer_id();
        engine.set_residency(workflow_id.clone(), WorkflowResidency::Resident(process))?;
        engine.record_workflow_event(
            &workflow_id,
            timer_started_event(&workflow_id, &timer_id, 1),
        )?;
        engine.refuse_next_record_as_terminal()?;

        service
            .fire_timer(workflow_id.clone(), timer_id.clone(), instant(130))
            .await?;

        assert_eq!(
            count_timer_fired(&history(&store, &workflow_id).await?, &timer_id),
            0,
            "a refused fire records no TimerFired"
        );
        assert!(
            engine.delivered_messages()?.is_empty(),
            "a refused fire delivers no wake"
        );
        Ok(())
    }

    /// Two services obtained separately but sharing ONE terminal-update
    /// coordinator (as the production bridge hands out) serialize a cancel and a
    /// fire of the same timer: exactly one terminal timer event is recorded, never
    /// both. A `Barrier` forces genuine overlap — both actors are released
    /// together after setup — and the loop runs each direction. Mutation-sensitive:
    /// a per-service coordinator would let both read the timer live and record a
    /// `TimerFired` AND a `TimerCancelled`.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn shared_coordinator_serializes_cancel_and_fire_across_services()
    -> Result<(), TimerServiceError> {
        use dashmap::DashSet;
        use tokio::sync::Barrier;

        for _ in 0..20 {
            let process = WorkflowProcessHandle::new(42);
            let concrete_store = Arc::new(InMemoryStore::default());
            let recorder_store: Arc<dyn WritableEventStore> = concrete_store.clone();
            let readable: Arc<dyn ReadableEventStore> = concrete_store.clone();
            let engine = Arc::new(FakeEngineHandle::recording_to(recorder_store));
            let coordinator = Arc::new(DashSet::new());
            let service_a =
                TimerService::with_recorded_at(engine.clone(), readable.clone(), recorded_at)
                    .with_terminal_updates(Arc::clone(&coordinator));
            let service_b =
                TimerService::with_recorded_at(engine.clone(), readable.clone(), recorded_at)
                    .with_terminal_updates(Arc::clone(&coordinator));

            let workflow_id = workflow_id();
            let timer_id = timer_id();
            let fire_at = instant(200);
            engine.set_residency(workflow_id.clone(), WorkflowResidency::Resident(process))?;
            engine.record_workflow_event(
                &workflow_id,
                timer_started_event(&workflow_id, &timer_id, 1),
            )?;

            let gate = Arc::new(Barrier::new(2));
            let (cancel_gate, fire_gate) = (Arc::clone(&gate), gate);
            let (cancel_wf, cancel_timer) = (workflow_id.clone(), timer_id.clone());
            let cancel = async move {
                cancel_gate.wait().await;
                service_a
                    .cancel(cancel_wf, cancel_timer, TimerCancelCause::WorkflowIntent)
                    .await
            };
            let (fire_wf, fire_timer) = (workflow_id.clone(), timer_id.clone());
            let fire = async move {
                fire_gate.wait().await;
                service_b.fire_timer(fire_wf, fire_timer, fire_at).await
            };
            let (cancel_result, fire_result) = tokio::join!(cancel, fire);
            cancel_result?;
            fire_result?;

            let history = history(&concrete_store, &workflow_id).await?;
            let terminal_timer_events = history
                .iter()
                .filter(|event| {
                    matches!(
                        event,
                        Event::TimerFired { timer_id: recorded, .. }
                        | Event::TimerCancelled { timer_id: recorded, .. }
                            if recorded == &timer_id
                    )
                })
                .count();
            assert_eq!(
                terminal_timer_events, 1,
                "first-recorded wins across shared services: {history:#?}"
            );
        }
        Ok(())
    }

    #[tokio::test]
    async fn firing_prior_run_timer_after_continue_as_new_is_noop() -> Result<(), TimerServiceError>
    {
        let process = WorkflowProcessHandle::new(42);
        let (store, engine, service) = service();
        let workflow_id = workflow_id();
        let timer_id = timer_id();
        engine.set_residency(workflow_id.clone(), WorkflowResidency::Resident(process))?;
        // Run 1 started the timer; run 2's WorkflowStarted closes that segment.
        engine.record_workflow_event(
            &workflow_id,
            timer_started_event(&workflow_id, &timer_id, 1),
        )?;
        engine.record_workflow_event(&workflow_id, workflow_started_event(&workflow_id, 2))?;

        service
            .fire_timer(workflow_id.clone(), timer_id.clone(), instant(100))
            .await?;

        assert_eq!(
            count_timer_fired(&history(&store, &workflow_id).await?, &timer_id),
            0
        );
        assert!(engine.delivered_messages()?.is_empty());
        Ok(())
    }
}
