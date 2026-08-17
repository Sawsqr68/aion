//! NOI-5 transcript sequencer + fan-out — the durability-critical server bridge.
//!
//! [`ActivityEventPublisher`] is the aion-server's SEQUENCER for the agent
//! observability transcript. It is the counterpart of [`crate::cluster_publisher`]'s
//! `ClusterEventPublisher` for the workflow-agnostic cluster channel, but with one
//! decisive difference the design calls out explicitly (§5.3): the transcript's
//! `store_seq` is **NOT a process-local `AtomicU64`**. A per-process counter resets
//! on restart/failover, so two survivors would mint colliding, non-monotonic
//! sequences. Instead `store_seq` is **commit-allocated**: the server reads the
//! durable `O`-keyspace head, appends at that `expected_seq`, and on a
//! [`StoreError::SequenceConflict`] **re-reads the advanced head and retries**.
//!
//! This read-head -> append(expected_seq) -> on-conflict-retry loop
//! ([`Self::publish`]) is the ONLY thing that keeps `store_seq` monotonic when two
//! writers race for one `(workflow, run, activity, attempt)` stream (a dying worker +
//! an adopting worker after failover, or two concurrent publish calls). It is
//! correctness-critical code, not an implementation detail, and is covered by the
//! two mandatory NOI-5 negative controls: concurrent-writer monotonicity and
//! failover dedup.
//!
//! # What this does and does not persist
//!
//! - **Non-ephemeral events** are durably appended to the `O` keyspace and then
//!   fanned out to the live transcript broadcast (with the assigned `store_seq`).
//! - **Ephemeral events** (token deltas) are **WS-forward-only**: fanned out live,
//!   **never** persisted. They carry `store_seq: None` on the wire, forever.
//!
//! # Live fan-out + resume
//!
//! Persisted events are also broadcast on a bounded `broadcast::Sender<ActivityEvent>`
//! so a connected transcript socket tails them live. A reconnecting client resumes
//! by `store_seq`: [`Self::replay_from`] reads the durable `O` tail from the store,
//! and [`Self::subscribe`] attaches the live broadcast suppressing any event at or
//! below the resume cursor (the gap-free splice contract the cluster channel uses).
//!
//! # Batched commits (write amplification)
//!
//! Events do not reach this sequencer one call at a time from the outside world:
//! they arrive on an in-process queue and a DRAIN task feeds them in
//! ([`ActivityEventPublisher::drain`], used by the liminal observability tap and
//! by the declared-command executor). Every durable append is one storage-tree
//! commit, and one commit re-persists the whole containing leaf as a new
//! permanent blob — so a drain that appended one event per commit made the store
//! cost of a run linear in EVENT COUNT rather than in event bytes (forensics
//! 2026-08-17: 2,359 honest bytes of transcript bought a 2,303,416-byte
//! permanent blob, ~977x; a 136 KB status-sweep run cost 73.7 MB of permanent
//! store, 540x).
//!
//! The drain therefore COALESCES: it takes up to
//! [`TranscriptBatchPolicy::max_batch_events`] events off the queue, waiting at
//! most [`TranscriptBatchPolicy::max_hold`] for the batch to fill, and appends
//! each stream's share as ONE commit ([`Self::publish_all`]). Both values are
//! operator config with **no default** — see the durability contract below.

use std::sync::Arc;
use std::time::Duration;

use aion_core::{ActivityEvent, ActivityEventKind, ProgressDetail};
use aion_store::{ActivityRecord, ActivityStreamKey, ObservabilityStore, StoreError};
use futures::stream::{self, BoxStream};
use tokio::sync::{broadcast, mpsc};

use crate::activity_bounds::{TranscriptBounds, bound_event};

/// A lag item on the transcript broadcast: `skipped` events were dropped because
/// the subscriber fell behind the bounded buffer. Surfaced typed to the client
/// (which then re-resumes from the durable `O` tail), never a silent skip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TranscriptStreamLagged {
    /// Number of transcript events dropped.
    pub skipped: u64,
}

impl std::fmt::Display for TranscriptStreamLagged {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "transcript stream lagged: {} events dropped",
            self.skipped
        )
    }
}

impl std::error::Error for TranscriptStreamLagged {}

/// The durable transcript sequencer + live fan-out for one deployment.
///
/// Cloneable: the broadcast sender and the store handle are shared, so every
/// clone sequences into the same `O` keyspace and fans out to the same live
/// subscribers. The store is the single source of `store_seq` monotonicity; the
/// broadcast is best-effort live tail only.
#[derive(Clone)]
pub struct ActivityEventPublisher {
    store: Arc<dyn ObservabilityStore>,
    live: broadcast::Sender<ActivityEvent>,
    bounds: TranscriptBounds,
    batch: TranscriptBatchPolicy,
}

/// The transcript drain's flush policy: how many queued events one durable
/// commit may carry, and how long the drain may wait to fill that batch.
///
/// # There is no default, deliberately
///
/// Both values are a deployment trade between store cost and transcript
/// latency, so neither is invented here: [`crate::config::ObservabilityConfig`]
/// carries them as `Option`s with no default and the boot path refuses to build
/// a publisher without them (the `websocket.cluster_broadcast_capacity`
/// pattern). A server whose operator has not ruled on them does not start, and
/// says which keys are missing.
///
/// # Durability contract
///
/// **Unchanged for the caller of [`ActivityEventPublisher::publish`] /
/// [`ActivityEventPublisher::publish_all`]:** those return only after the
/// events are durably committed, and an event that has been acked as persisted
/// (a returned `store_seq`) is exactly as durable as before.
///
/// **What `max_hold` extends** is the window in which an event sits in the
/// server's IN-MEMORY transcript queue before the drain commits it. That window
/// already existed and is already lossy: the observability tap hands events to a
/// bounded in-process channel, and everything queued there is lost if the server
/// process dies. `max_hold` lengthens that pre-existing window by at most its own
/// value, and `max_batch_events` bounds how many events can be waiting in it
/// beyond what the channel already held. Nothing that was durable becomes
/// non-durable; a strictly bounded amount of not-yet-durable transcript stays
/// not-yet-durable for strictly bounded longer.
///
/// That trade is acceptable HERE and would not be elsewhere, because the `O`
/// keyspace is observability, never replay authority: workflow correctness lives
/// on the `E`-stream, whose append path is untouched by this policy. Losing the
/// last few hundred milliseconds of an agent's transcript to a kill-9 costs
/// transcript, never a workflow decision — which is the same reason the drain
/// already logs and drops an event the store refuses instead of failing the
/// activity. An operator who wants the old timing sets `max_hold_ms = 0`: the
/// drain then commits whatever is already queued the moment it can, coalescing
/// only what genuinely arrived together, with no added window at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TranscriptBatchPolicy {
    /// Maximum events in one durable commit (`observability.max_batch_events`).
    pub max_batch_events: std::num::NonZeroUsize,
    /// Maximum time the drain waits for a partial batch to fill
    /// (`observability.max_batch_hold_ms`). Zero means "never wait".
    pub max_hold: Duration,
}

impl TranscriptBatchPolicy {
    /// The IDENTITY policy: one event per durable commit, never held.
    ///
    /// This is the pre-batching behaviour, and the only policy that assumes
    /// nothing about a deployment — which is exactly why it is the fallback for
    /// the embedder/test state constructors that bypass config validation and
    /// cannot refuse to build, and the policy under which the single-event
    /// sequencing tests run. A config-driven server never reaches it: it states
    /// its own policy or it does not start.
    pub const UNBATCHED: Self = Self {
        max_batch_events: std::num::NonZeroUsize::MIN,
        max_hold: Duration::ZERO,
    };
}

impl std::fmt::Debug for ActivityEventPublisher {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ActivityEventPublisher")
            .field("live_receivers", &self.live.receiver_count())
            .finish_non_exhaustive()
    }
}

/// The maximum number of `SequenceConflict` retries before a single `publish`
/// gives up. In-process publishers serialize per tap (the liminal drain
/// queue), so conflicts only come from genuine cross-process races — failover
/// adoption, a dying worker racing its adopter — where a handful of writers
/// contend. Exceeding this signals a pathological hot loop and must FAIL
/// CHEAP: the 2026-07-23 flood burned a core spinning 1024-retry loops (each
/// failed backend append also leaving orphaned store nodes behind), so the
/// budget is sized to real contention, not to hope.
const MAX_SEQUENCE_CONFLICT_RETRIES: usize = 16;

impl ActivityEventPublisher {
    /// Build a publisher over `store` with a live broadcast of `capacity` and
    /// the drain's `batch` flush policy.
    ///
    /// `capacity` is the bounded live-tail buffer; a subscriber that lags beyond
    /// it receives one typed [`TranscriptStreamLagged`] then re-resumes from the
    /// durable tail. It must be non-zero (validated by the caller's config).
    ///
    /// `batch` is a REQUIRED argument, not an option with a fallback: the flush
    /// policy is the operator's ruling on store cost against transcript latency
    /// and this type refuses to invent one (see [`TranscriptBatchPolicy`]).
    #[must_use]
    pub fn new(
        store: Arc<dyn ObservabilityStore>,
        capacity: std::num::NonZeroUsize,
        batch: TranscriptBatchPolicy,
    ) -> Self {
        let (live, _receiver) = broadcast::channel(capacity.get());
        Self {
            store,
            live,
            bounds: TranscriptBounds::default(),
            batch,
        }
    }

    /// Replace the default retention bounds with operator-configured ones
    /// (`[observability]` config). Bounds apply to the durable append path
    /// only; ephemeral fan-out is untouched.
    #[must_use]
    pub(crate) fn with_bounds(mut self, bounds: TranscriptBounds) -> Self {
        self.bounds = bounds;
        self
    }

    /// Sequence + persist + fan out one event.
    ///
    /// Ephemeral events are fanned out live with `store_seq: None` and are NEVER
    /// persisted. Non-ephemeral events are appended to the `O` keyspace under the
    /// commit-allocated `store_seq` (via the read-head -> `append(expected_seq)` ->
    /// on-conflict-re-read-head-and-retry loop), then fanned out carrying that
    /// `store_seq`. Returns the assigned `store_seq` for a persisted event, or
    /// `None` for an ephemeral one.
    ///
    /// A send with no live subscribers is not an error (the calm no-dashboard
    /// case); the durable append is the primary artifact.
    ///
    /// # Errors
    /// A [`StoreError`] from the durable append (after exhausting the retry
    /// budget on pathological contention, or any non-conflict backend error).
    pub async fn publish(&self, event: &ActivityEvent) -> Result<Option<u64>, StoreError> {
        let assigned = self.publish_all(std::slice::from_ref(event)).await?;
        // `publish_all` returns one entry per input event, in input order.
        Ok(assigned.into_iter().next().flatten())
    }

    /// Sequence + persist + fan out a batch of events as FEW durable commits as
    /// the batch allows: one commit per distinct
    /// `(workflow, run, activity, attempt)` stream present in `events`.
    ///
    /// Returns one entry per input event, in input order: the assigned
    /// `store_seq` for a persisted event, or `None` for an ephemeral one and for
    /// one past the stream's retention cap — the same per-event answer
    /// [`Self::publish`] gives, because `publish` IS this method over a slice of
    /// one.
    ///
    /// Events keep their arrival order within a stream, and a stream's whole
    /// share of the batch is committed at one `expected_seq` under the same
    /// read-head -> append -> on-conflict-re-read-head-and-retry loop the
    /// single-event path used, so `store_seq` monotonicity and the single-writer
    /// law are untouched: a `SequenceConflict` leaves the WHOLE batch unwritten
    /// (the store contract is all-or-nothing) and the batch is retried at the
    /// observed head.
    ///
    /// # Errors
    /// A [`StoreError`] from a durable append. Streams are independent, so a
    /// failure on one stream does not abandon the others: every stream in the
    /// batch is attempted and the FIRST error is returned once they have all
    /// been tried. (A caller with a single-stream batch — every drain call in
    /// this server — sees exactly the single-event behaviour.)
    pub async fn publish_all(
        &self,
        events: &[ActivityEvent],
    ) -> Result<Vec<Option<u64>>, StoreError> {
        let (outcomes, first_error) = self.publish_all_outcomes(events).await;
        match first_error {
            Some(error) => Err(error),
            None => Ok(outcomes
                .into_iter()
                .map(|outcome| match outcome {
                    EventOutcome::Persisted(store_seq) => Some(store_seq),
                    EventOutcome::NotPersisted | EventOutcome::Refused => None,
                })
                .collect()),
        }
    }

    /// [`Self::publish_all`] with the per-event outcome kept even when a stream
    /// failed, so the drain can count EXACTLY how many events the store refused
    /// rather than reporting an upper bound.
    async fn publish_all_outcomes(
        &self,
        events: &[ActivityEvent],
    ) -> (Vec<EventOutcome>, Option<StoreError>) {
        let mut outcomes: Vec<EventOutcome> = vec![EventOutcome::NotPersisted; events.len()];
        let mut first_error: Option<StoreError> = None;
        // Group the durable events by stream key, preserving arrival order both
        // between groups (first-seen key first) and inside each one.
        let mut groups: Vec<(ActivityStreamKey, Vec<(usize, ActivityEvent)>)> = Vec::new();
        for (index, event) in events.iter().enumerate() {
            if event.ephemeral {
                // WS-forward-only: fan out live with no store_seq, never persist.
                let mut ephemeral = event.clone();
                ephemeral.store_seq = None;
                let send_result = self.live.send(ephemeral);
                drop(send_result);
                continue;
            }
            // Bound the event FIRST so the persisted record, the live fan-out,
            // and every later replay all carry the same bounded shape. A single
            // unrepresentable event is that event's failure, not the batch's.
            let bounded = match bound_event(event, self.bounds.max_event_bytes) {
                Ok(bounded) => bounded,
                Err(error) => {
                    outcomes[index] = EventOutcome::Refused;
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                    continue;
                }
            };
            let key = ActivityStreamKey::of(&bounded);
            match groups.iter_mut().find(|(existing, _)| *existing == key) {
                Some((_, items)) => items.push((index, bounded)),
                None => groups.push((key, vec![(index, bounded)])),
            }
        }
        for (key, items) in groups {
            if let Err(error) = self.persist_group(&key, &items, &mut outcomes).await {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
        (outcomes, first_error)
    }

    /// Commit one stream's share of a batch, honouring the retention cap and the
    /// optimistic-concurrency retry budget.
    ///
    /// `items` are `(index into the caller's batch, bounded event)` pairs in
    /// arrival order, all for `key`; each event's outcome is written back into
    /// `outcomes` at its index — its assigned `store_seq` when persisted,
    /// [`EventOutcome::Refused`] when the store would not take it. A batch that
    /// would cross the per-stream cap is split at the cap: the events below it
    /// commit, the cap marker is written at the cap sequence, and the remainder
    /// is fanned out live-only — exactly the sequence of states the single-event
    /// path produced.
    async fn persist_group(
        &self,
        key: &ActivityStreamKey,
        items: &[(usize, ActivityEvent)],
        outcomes: &mut [EventOutcome],
    ) -> Result<(), StoreError> {
        // Seed the optimistic-concurrency loop from the durable head. On a
        // SequenceConflict a concurrent writer advanced the head between our read
        // and our append, so we re-read the (now advanced) head and retry — this
        // is what keeps store_seq strictly monotonic across racing writers.
        let mut expected_seq = match self.store.activity_head(key).await {
            Ok(head) => head,
            Err(error) => {
                mark_refused(items, outcomes, 0);
                return Err(error);
            }
        };
        let mut committed = 0usize;
        let mut conflicts = 0usize;
        while committed < items.len() {
            // The per-stream retention cap is re-evaluated every iteration: a
            // conflict advances `expected_seq`, which can cross the cap.
            if expected_seq > self.bounds.max_stream_events {
                // Past the cap (the marker at the cap seq is already durable):
                // live streaming continues, persistence stops.
                for (_, event) in &items[committed..] {
                    self.fan_out_live_only(event);
                }
                return Ok(());
            }
            if expected_seq == self.bounds.max_stream_events {
                match self
                    .append_cap_marker(&items[committed].1, expected_seq)
                    .await
                {
                    Ok(()) => {
                        // The marker is durable; the triggering events themselves
                        // are live-only, like everything after them.
                        for (_, event) in &items[committed..] {
                            self.fan_out_live_only(event);
                        }
                        return Ok(());
                    }
                    Err(StoreError::SequenceConflict { found, .. }) => {
                        // A concurrent writer won the cap seq: adopt the head
                        // and re-loop (the cap re-check then routes to drop).
                        expected_seq = found;
                        conflicts += 1;
                    }
                    Err(error) => {
                        mark_refused(items, outcomes, committed);
                        return Err(error);
                    }
                }
                if conflicts >= MAX_SEQUENCE_CONFLICT_RETRIES {
                    break;
                }
                continue;
            }
            // Commit only what fits below the cap; the remainder re-enters the
            // loop at `expected_seq == cap` and takes the marker arm.
            let room = usize::try_from(self.bounds.max_stream_events - expected_seq)
                .unwrap_or(usize::MAX)
                .min(items.len() - committed);
            let batch: Vec<ActivityEvent> = items[committed..committed + room]
                .iter()
                .map(|(_, event)| event.clone())
                .collect();
            match self
                .store
                .append_activity_events(expected_seq, &batch)
                .await
            {
                Ok(_new_head) => {
                    for (offset, (index, event)) in
                        items[committed..committed + room].iter().enumerate()
                    {
                        let store_seq =
                            expected_seq.saturating_add(u64::try_from(offset).unwrap_or(u64::MAX));
                        outcomes[*index] = EventOutcome::Persisted(store_seq);
                        let mut persisted = event.clone();
                        persisted.store_seq = Some(store_seq);
                        let send_result = self.live.send(persisted);
                        drop(send_result);
                    }
                    committed += room;
                    expected_seq =
                        expected_seq.saturating_add(u64::try_from(room).unwrap_or(u64::MAX));
                }
                Err(StoreError::SequenceConflict { found, .. }) => {
                    // The durable head advanced past our expectation: adopt the
                    // observed head and retry this batch there. Nothing of the
                    // batch was written (all-or-nothing store contract).
                    expected_seq = found;
                    conflicts += 1;
                    if conflicts >= MAX_SEQUENCE_CONFLICT_RETRIES {
                        break;
                    }
                }
                Err(error) => {
                    mark_refused(items, outcomes, committed);
                    return Err(error);
                }
            }
        }
        if committed < items.len() {
            mark_refused(items, outcomes, committed);
            return Err(StoreError::Backend(format!(
                "observability append exceeded {MAX_SEQUENCE_CONFLICT_RETRIES} sequence-conflict retries for {key:?}"
            )));
        }
        Ok(())
    }

    /// Drain `receiver` into the durable transcript in COALESCED batches until
    /// the seam closes, returning the number of events the store refused.
    ///
    /// This is the one transcript drain: the liminal observability tap and the
    /// declared-command executor both run their queue through it, so both pay
    /// one commit per batch rather than one per event. Each iteration takes up
    /// to [`TranscriptBatchPolicy::max_batch_events`] events off the queue,
    /// waits at most [`TranscriptBatchPolicy::max_hold`] for a partial batch to
    /// fill (skipping the wait entirely when the hold is zero or the batch is
    /// already full), and commits the batch with [`Self::publish_all`].
    ///
    /// Returns when the sender is dropped AND the queue is empty, so a caller
    /// can await this and know every queued event has been offered to the
    /// sequencer.
    ///
    /// # Losing a transcript never fails the producer
    ///
    /// A store that refuses a batch costs a log line and that batch's events,
    /// never the activity or the command: the refusal is warned once per FAILED
    /// BATCH (bounded by the flush policy — never one log line per line of
    /// output) naming `operation` and the batch's first event, and the total is
    /// returned for the caller's end-of-run summary.
    pub(crate) async fn drain<R: TranscriptEventReceiver>(
        &self,
        receiver: &mut R,
        operation: &'static str,
    ) -> u64 {
        let limit = self.batch.max_batch_events.get();
        let mut buffer: Vec<ActivityEvent> = Vec::with_capacity(limit);
        let mut dropped: u64 = 0;
        loop {
            let mut closed = receiver.recv_many(&mut buffer, limit).await == 0;
            if !closed && buffer.len() < limit && !self.batch.max_hold.is_zero() {
                // Hold the partial batch open briefly so events arriving in the
                // same burst share one commit. The deadline is absolute, so a
                // slow trickle cannot extend the window indefinitely.
                let deadline = tokio::time::Instant::now() + self.batch.max_hold;
                while buffer.len() < limit {
                    let remaining = limit - buffer.len();
                    match tokio::time::timeout_at(
                        deadline,
                        receiver.recv_many(&mut buffer, remaining),
                    )
                    .await
                    {
                        Ok(0) => {
                            closed = true;
                            break;
                        }
                        Ok(_) => {}
                        Err(_elapsed) => break,
                    }
                }
            }
            if !buffer.is_empty() {
                let (outcomes, error) = self.publish_all_outcomes(&buffer).await;
                if let Some(error) = error {
                    let refused = outcomes
                        .iter()
                        .filter(|outcome| matches!(outcome, EventOutcome::Refused))
                        .count();
                    dropped = dropped.saturating_add(u64::try_from(refused).unwrap_or(u64::MAX));
                    let first = buffer.first();
                    tracing::warn!(
                        %error,
                        operation,
                        batch_events = buffer.len(),
                        refused_events = refused,
                        workflow_id = ?first.map(|event| event.workflow_id.to_string()),
                        activity_id = ?first.map(|event| event.activity_id.to_string()),
                        attempt = ?first.map(|event| event.attempt),
                        "transcript drain: the sequencer refused part of a batch; the producer \
                         is unaffected and the refused events are not retained"
                    );
                }
                buffer.clear();
            }
            if closed {
                return dropped;
            }
        }
    }

    /// Fan one non-ephemeral event out live WITHOUT a `store_seq` (past-cap
    /// delivery: the event is real transcript, just not retained).
    fn fan_out_live_only(&self, event: &ActivityEvent) {
        let mut live_only = event.clone();
        live_only.store_seq = None;
        let send_result = self.live.send(live_only);
        drop(send_result);
    }

    /// Durably append the one retention-cap marker record at `cap_seq` (the
    /// stream's `max_stream_events` position) and fan it out with its
    /// `store_seq`. The marker carries the SAME identity fields as the event
    /// that crossed the cap, so it lands in the same stream and attributes to
    /// the same agent.
    async fn append_cap_marker(
        &self,
        event: &ActivityEvent,
        cap_seq: u64,
    ) -> Result<(), StoreError> {
        let cap = self.bounds.max_stream_events;
        let mut marker = event.clone();
        marker.kind = ActivityEventKind::Progress {
            detail: ProgressDetail::Note {
                text: format!(
                    "transcript retention cap reached ({cap} events); further events are live-only and not persisted"
                ),
            },
        };
        let store_seq = self.store.append_activity_event(cap_seq, &marker).await?;
        marker.store_seq = Some(store_seq);
        let send_result = self.live.send(marker);
        drop(send_result);
        Ok(())
    }

    /// Read the durable `O` tail for `key` with `store_seq >= from_seq`.
    ///
    /// The priming read a resuming transcript client replays before splicing onto
    /// the live stream. `from_seq = 0` replays the whole persisted transcript.
    ///
    /// # Errors
    /// A [`StoreError`] from the durable read.
    pub async fn replay_from(
        &self,
        key: &ActivityStreamKey,
        from_seq: u64,
    ) -> Result<Vec<ActivityRecord>, StoreError> {
        self.store.read_activity_events_from(key, from_seq).await
    }

    /// Enumerate the retained transcript streams of ONE RUN of `workflow_id`
    /// from the durable `O` keyspace (empty for a run with none — old runs
    /// simply have no retained transcript).
    ///
    /// The run is required, never an optional filter: a workflow-wide
    /// enumeration over a continue-as-new chain would list several generations'
    /// streams under coordinates that collide pairwise, and the caller could
    /// not tell them apart.
    ///
    /// # Errors
    /// A [`StoreError`] from the durable enumeration.
    pub async fn list_streams(
        &self,
        workflow_id: &aion_core::WorkflowId,
        run_id: &aion_core::RunId,
    ) -> Result<Vec<aion_store::ActivityStreamSummary>, StoreError> {
        self.store.list_activity_streams(workflow_id, run_id).await
    }

    /// Subscribe to the live transcript tail for `key`, suppressing every event
    /// for a DIFFERENT stream and every persisted event already covered by the
    /// resume cursor.
    ///
    /// The broadcast is deployment-wide (one channel), so this filters to `key`'s
    /// `(workflow, run, activity, attempt)` stream — an event from a sibling
    /// continue-as-new generation of the same workflow fails the key comparison
    /// and is suppressed, exactly like a different attempt's.
    ///
    /// `after_seq` dedups the splice seam
    /// exactly like the cluster channel: attach this receiver BEFORE reading the
    /// priming [`Self::replay_from`] tail, so an event that races the priming read
    /// is retained by the receiver and applied after it (deduped on `store_seq`).
    ///
    /// The cursor is an `Option` because `store_seq` is **0-based** (the first
    /// event is `store_seq == 0`): `after_seq = None` is a FRESH subscriber that
    /// has applied nothing and must see every event including `store_seq == 0`;
    /// `after_seq = Some(n)` has already applied through `store_seq == n`, so
    /// events with `store_seq <= n` are suppressed at the seam. Ephemeral events
    /// (which carry `store_seq: None`) for `key` are ALWAYS forwarded live — they
    /// have no sequence to dedup and are never replayed.
    #[must_use]
    pub fn subscribe(
        &self,
        key: ActivityStreamKey,
        after_seq: Option<u64>,
    ) -> BoxStream<'static, Result<ActivityEvent, TranscriptStreamLagged>> {
        let receiver = self.live.subscribe();
        Box::pin(stream::unfold(
            (receiver, key, after_seq),
            |(mut receiver, key, after_seq)| async move {
                loop {
                    match receiver.recv().await {
                        Ok(event) => {
                            if ActivityStreamKey::of(&event) != key {
                                // A different stream's event on the shared
                                // broadcast — another attempt, or another
                                // generation of this same workflow: not for
                                // this subscriber.
                                continue;
                            }
                            match (event.store_seq, after_seq) {
                                // Already-applied persisted event at the splice
                                // seam: suppress it (fall through to re-loop).
                                (Some(seq), Some(cursor)) if seq <= cursor => {}
                                // A live persisted event past the cursor, a fresh
                                // subscriber (no cursor), or an ephemeral (None)
                                // event: forward it.
                                _ => return Some((Ok(event), (receiver, key, after_seq))),
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(skipped)) => {
                            return Some((
                                Err(TranscriptStreamLagged { skipped }),
                                (receiver, key, after_seq),
                            ));
                        }
                        Err(broadcast::error::RecvError::Closed) => return None,
                    }
                }
            },
        ))
    }
}

/// What became of one event handed to [`ActivityEventPublisher::publish_all`].
///
/// Distinguishing "not persisted by design" from "the store refused it" is what
/// lets the drain report an EXACT dropped count instead of an upper bound: an
/// ephemeral or past-the-retention-cap event is `NotPersisted` and is not a
/// loss, while a `Refused` event is transcript the operator asked for and did
/// not get.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EventOutcome {
    /// Deliberately not written: an ephemeral event, or one past the stream's
    /// retention cap. Live fan-out still happened.
    NotPersisted,
    /// Durably committed at this `store_seq`.
    Persisted(u64),
    /// The store would not take it — the only outcome that counts as a loss.
    Refused,
}

/// Mark every item of a group from `from` onwards as refused by the store.
fn mark_refused(items: &[(usize, ActivityEvent)], outcomes: &mut [EventOutcome], from: usize) {
    for (index, _) in &items[from..] {
        outcomes[*index] = EventOutcome::Refused;
    }
}

/// A queue [`ActivityEventPublisher::drain`] can pull whole batches from.
///
/// Exists so the ONE drain implementation serves both transcript seams — the
/// liminal observability tap's bounded channel and the declared-command
/// executor's unbounded one — without either growing its own copy of the
/// coalescing loop. Both tokio receivers already offer the batch primitive;
/// this trait is the two-line adapter that lets one loop take either.
#[async_trait::async_trait]
pub(crate) trait TranscriptEventReceiver: Send {
    /// Move up to `limit` currently-queued events into `buffer`, awaiting the
    /// first one, and return how many were moved. Returns `0` ONLY when the
    /// channel is closed and drained (`limit` is never zero here).
    async fn recv_many(&mut self, buffer: &mut Vec<ActivityEvent>, limit: usize) -> usize;
}

#[async_trait::async_trait]
impl TranscriptEventReceiver for mpsc::Receiver<ActivityEvent> {
    async fn recv_many(&mut self, buffer: &mut Vec<ActivityEvent>, limit: usize) -> usize {
        Self::recv_many(self, buffer, limit).await
    }
}

#[async_trait::async_trait]
impl TranscriptEventReceiver for mpsc::UnboundedReceiver<ActivityEvent> {
    async fn recv_many(&mut self, buffer: &mut Vec<ActivityEvent>, limit: usize) -> usize {
        Self::recv_many(self, buffer, limit).await
    }
}

#[cfg(test)]
#[path = "activity_publisher_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "activity_publisher_batching_tests.rs"]
mod batching_tests;
