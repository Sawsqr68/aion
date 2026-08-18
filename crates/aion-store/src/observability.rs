//! The durable observability (`O`) keyspace contract — NOI-5's durability spine.
//!
//! This module defines the persistence contract for the agent-observability
//! transcript: an **append-only, per-`(workflow, run, activity, attempt)`** stream of
//! [`aion_core::ActivityEvent`] records that survives kill-9 and failover, is
//! replayable by `store_seq`, and is **never** part of the workflow replay log.
//!
//! # The `O` keyspace is NOT the `E`-stream (LOCKED)
//!
//! Workflow replay authority lives exclusively on the `E`-stream (the
//! [`crate::WritableEventStore`] append path). An [`ActivityRecord`] is an
//! observability record: the replay decoder never scans this keyspace and could
//! not decode one of these records as an `Event` even if it did (different region
//! tag, different schema). The byte-level disjointness is what makes "durable but
//! non-replay-authoritative" a *guarantee*, not a hope — see
//! `aion-store-haematite`'s `observability` module for the `O` (0x4F) region tag
//! and the disjointness test.
//!
//! # Single-writer, server-allocated `store_seq`
//!
//! `store_seq` is **not** allocated by the store: it is a caller-supplied
//! `expected_seq` under optimistic concurrency, exactly like the workflow-history
//! append path. [`ObservabilityStore::append_activity_events`] returns the
//! `SequenceConflict` the server's sequencer re-reads-head-and-retries on. The
//! server is the *single writer* to this keyspace, so monotonicity is enforced by
//! the server's read-head -> append(expected_seq) -> retry loop, not by any magic
//! in the store. This mirrors [`StoreError::SequenceConflict`] on the workflow
//! path and is why the store deliberately does not auto-allocate an id.
//!
//! # One append, many events (write amplification)
//!
//! The append primitive takes a SLICE of same-stream events, not one event,
//! because a durable backend pays per COMMIT, not per byte: haematite
//! re-persists the whole containing storage leaf as a new permanent blob on
//! every commit, so appending N events one at a time costs N whole-leaf
//! rewrites of a leaf that can reach tens of megabytes (forensics 2026-08-17:
//! 2,359 honest bytes bought a 2,303,416-byte blob). One batched append of N
//! same-stream events is ONE commit and therefore one rewrite. The single-event
//! [`ObservabilityStore::append_activity_event`] remains as a documented
//! one-line convenience over the slice form for callers that genuinely hold a
//! single event; it is not a second implementation.

use async_trait::async_trait;

use aion_core::{ActivityEvent, ActivityId, RunId, WorkflowId};

use crate::StoreError;

/// The durable key of one observability stream: a `(workflow, run, activity,
/// attempt)` quad. Every [`ActivityRecord`] for one running agent attempt shares
/// this key and is ordered by `store_seq` within it.
///
/// # Why the run axis exists
///
/// A continue-as-new chain reuses one [`WorkflowId`] across generations, and
/// BOTH of the remaining axes restart inside each new run: activity ordinals
/// count from `0` again and attempts from `1`. A `(workflow, activity, attempt)`
/// key is therefore not unique across a chain — generation two's first event
/// would be appended onto generation one's stream head, silently fusing two
/// transcripts into one. The run is the axis that separates them, so it is a
/// required component of the key with no `None` arm.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ActivityStreamKey {
    /// The workflow the activity belongs to.
    pub workflow_id: WorkflowId,
    /// The concrete run of that workflow — the second key axis. Two generations
    /// of one continue-as-new chain are DISTINCT streams even when their
    /// `(activity, attempt)` coordinates coincide, which they routinely do.
    pub run_id: RunId,
    /// The activity within the workflow.
    pub activity_id: ActivityId,
    /// The attempt number — the fourth key axis (NOI-0). Two attempts of one
    /// activity are DISTINCT streams; a within-attempt failover shares one stream
    /// (so a dying + adopting worker's events dedupe), while a retry is a new
    /// attempt and therefore a new stream.
    pub attempt: u32,
}

impl ActivityStreamKey {
    /// Build a stream key from its four components.
    #[must_use]
    pub const fn new(
        workflow_id: WorkflowId,
        run_id: RunId,
        activity_id: ActivityId,
        attempt: u32,
    ) -> Self {
        Self {
            workflow_id,
            run_id,
            activity_id,
            attempt,
        }
    }

    /// The stream key an [`ActivityEvent`] belongs to.
    #[must_use]
    pub fn of(event: &ActivityEvent) -> Self {
        Self {
            workflow_id: event.workflow_id.clone(),
            run_id: event.run_id.clone(),
            activity_id: event.activity_id.clone(),
            attempt: event.attempt,
        }
    }
}

/// A durably persisted observability event: an [`ActivityEvent`] with its
/// server-stamped `store_seq` guaranteed present.
///
/// The wire envelope carries `store_seq: Option<u64>` (`None` until persisted);
/// once read back from the `O` keyspace the sequence is always present, so this
/// record exposes it as a non-optional field alongside the event.
#[derive(Clone, Debug, PartialEq)]
pub struct ActivityRecord {
    /// The monotonic, server-allocated sequence assigned at durable commit.
    pub store_seq: u64,
    /// The persisted event. Its `store_seq` field is populated to match
    /// [`Self::store_seq`] so a record read back is self-describing.
    pub event: ActivityEvent,
}

/// One retained transcript stream of a workflow run: its key and its head
/// (the number of durably retained records / the next `store_seq`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActivityStreamSummary {
    /// The stream's `(workflow, run, activity, attempt)` key — self-describing,
    /// so a summary read out of one enumeration names the run it came from.
    pub key: ActivityStreamKey,
    /// Next `store_seq` to be written == count of retained records.
    pub head: u64,
}

/// Durable, append-only observability keyspace contract.
///
/// Implemented by the haematite backend for production and by
/// [`InMemoryObservabilityStore`] for tests + conformance. The server is the
/// single writer; every method keys on the `(workflow, run, activity, attempt)`
/// quad, never on the workflow alone and never on a run-ambiguous triple.
#[async_trait]
pub trait ObservabilityStore: Send + Sync + 'static {
    /// Atomically append `events` — all belonging to ONE
    /// `(workflow, run, activity, attempt)` stream — starting at `expected_seq`
    /// (the current head the caller believes it holds), as a SINGLE durable
    /// commit.
    ///
    /// The `i`-th event is assigned `store_seq == expected_seq + i`. On success
    /// returns the stream's new head (`expected_seq + events.len()`), which is
    /// the next `store_seq` to be written. On a stale expectation returns
    /// [`StoreError::SequenceConflict`] with the actual head, leaving the stream
    /// **entirely** unchanged — a batch is all-or-nothing, so the server's
    /// sequencer can re-read the advanced head and retry the whole batch.
    ///
    /// An EMPTY slice is a no-op that commits nothing and returns
    /// `expected_seq`: there is no partial state to reason about and no commit
    /// to pay for.
    ///
    /// **Every event must share one stream key** — the durable backend addresses
    /// one stream per commit, so a mixed slice is a caller bug and is refused
    /// with [`StoreError::Backend`] rather than silently split (silently
    /// splitting would re-introduce the per-event commit this method exists to
    /// remove, and would assign sequences from the wrong stream's head).
    /// **Ephemeral events must never be passed here** — they are
    /// WS-forward-only and are filtered out before this call.
    ///
    /// # Errors
    /// [`StoreError::SequenceConflict`] on a stale `expected_seq`;
    /// [`StoreError::Backend`] when the slice mixes stream keys; otherwise a
    /// backend or serialization error.
    async fn append_activity_events(
        &self,
        expected_seq: u64,
        events: &[ActivityEvent],
    ) -> Result<u64, StoreError>;

    /// Append ONE event at `expected_seq`, returning its assigned `store_seq`
    /// (which equals `expected_seq`).
    ///
    /// A one-line convenience over [`Self::append_activity_events`] for callers
    /// that genuinely hold a single event — never a second implementation, and
    /// never the way to write N events (that is N commits; see the module docs).
    ///
    /// # Errors
    /// As [`Self::append_activity_events`].
    async fn append_activity_event(
        &self,
        expected_seq: u64,
        event: &ActivityEvent,
    ) -> Result<u64, StoreError> {
        self.append_activity_events(expected_seq, std::slice::from_ref(event))
            .await?;
        Ok(expected_seq)
    }

    /// Read the current head (next `store_seq` to be written) for `key`.
    ///
    /// An unwritten stream reads head `0`. The server's sequencer seeds its
    /// retry loop from this value.
    ///
    /// # Errors
    /// A backend or serialization error.
    async fn activity_head(&self, key: &ActivityStreamKey) -> Result<u64, StoreError>;

    /// Read every record for `key` with `store_seq >= from_seq`, in order.
    ///
    /// This is the resume-by-`store_seq` primitive: a reconnecting transcript
    /// client replays from its last-seen cursor without paying for the whole
    /// stream. An unwritten stream (or a `from_seq` beyond the head) reads empty.
    ///
    /// # Errors
    /// A backend or serialization error.
    async fn read_activity_events_from(
        &self,
        key: &ActivityStreamKey,
        from_seq: u64,
    ) -> Result<Vec<ActivityRecord>, StoreError>;

    /// Enumerate every retained transcript stream of ONE RUN of `workflow_id`,
    /// ordered by `(activity_id, attempt)` ascending. A run with no retained
    /// transcript reads empty (old runs simply have none).
    ///
    /// The run is a required argument, not an optional filter: a workflow-wide
    /// enumeration over a continue-as-new chain would return several
    /// generations' streams under coordinates that collide pairwise, which is
    /// exactly the ambiguity this keyspace exists to remove.
    ///
    /// # Errors
    /// A backend or serialization error.
    async fn list_activity_streams(
        &self,
        workflow_id: &WorkflowId,
        run_id: &RunId,
    ) -> Result<Vec<ActivityStreamSummary>, StoreError>;
}

/// An in-memory [`ObservabilityStore`] reference implementation for tests.
///
/// Enforces the SAME optimistic-concurrency contract the haematite backend does:
/// an append with a stale `expected_seq` returns [`StoreError::SequenceConflict`]
/// and writes nothing, so the server's retry loop can be exercised without a real
/// database. A `std::sync::Mutex` serializes the read-compare-write so two racing
/// appends on one stream cannot both win — the same single-shard-actor guarantee
/// the haematite backend gives.
#[derive(Debug, Default)]
pub struct InMemoryObservabilityStore {
    streams:
        std::sync::Mutex<std::collections::HashMap<ActivityStreamKeyBytes, Vec<ActivityRecord>>>,
}

/// A hashable, owned encoding of [`ActivityStreamKey`] for the in-memory map:
/// `(workflow uuid, run uuid, activity ordinal, attempt)`, in the SAME axis
/// order the durable `O`-region key encodes, so the two implementations agree on
/// which events share a stream and on the order an enumeration returns them in.
type ActivityStreamKeyBytes = (uuid::Uuid, uuid::Uuid, u64, u32);

fn key_bytes(key: &ActivityStreamKey) -> ActivityStreamKeyBytes {
    (
        key.workflow_id.as_uuid(),
        key.run_id.as_uuid(),
        key.activity_id.sequence_position(),
        key.attempt,
    )
}

/// The next `store_seq` for an in-memory stream = its record count.
///
/// A `Vec` length that does not fit in `u64` is unrepresentable on any supported
/// target (a 64-bit `usize` maxes at `u64::MAX`), so the saturating conversion is
/// exact in practice; it is written as a fallible convert to satisfy the
/// deny-level pedantic cast lints without an `as` cast.
fn stream_head(stream: &[ActivityRecord]) -> u64 {
    u64::try_from(stream.len()).unwrap_or(u64::MAX)
}

#[async_trait]
impl ObservabilityStore for InMemoryObservabilityStore {
    async fn append_activity_events(
        &self,
        expected_seq: u64,
        events: &[ActivityEvent],
    ) -> Result<u64, StoreError> {
        let Some(first) = events.first() else {
            // An empty batch commits nothing and leaves the head where it is.
            return Ok(expected_seq);
        };
        let key = ActivityStreamKey::of(first);
        for event in events {
            if ActivityStreamKey::of(event) != key {
                return Err(StoreError::Backend(format!(
                    "observability batch mixes stream keys: {key:?} and {:?} — one batch \
                     addresses exactly one stream",
                    ActivityStreamKey::of(event)
                )));
            }
        }
        let mut streams = self.streams.lock().map_err(|error| {
            StoreError::Backend(format!("observability mutex poisoned: {error}"))
        })?;
        let stream = streams.entry(key_bytes(&key)).or_default();
        let head = stream_head(stream);
        if head != expected_seq {
            // All-or-nothing: nothing of this batch is written.
            return Err(StoreError::SequenceConflict {
                expected: expected_seq,
                found: head,
            });
        }
        for (offset, event) in events.iter().enumerate() {
            let store_seq = head.saturating_add(u64::try_from(offset).unwrap_or(u64::MAX));
            let mut event = event.clone();
            event.store_seq = Some(store_seq);
            stream.push(ActivityRecord { store_seq, event });
        }
        Ok(stream_head(stream))
    }

    async fn activity_head(&self, key: &ActivityStreamKey) -> Result<u64, StoreError> {
        let streams = self.streams.lock().map_err(|error| {
            StoreError::Backend(format!("observability mutex poisoned: {error}"))
        })?;
        Ok(streams
            .get(&key_bytes(key))
            .map_or(0, |stream| stream_head(stream)))
    }

    async fn read_activity_events_from(
        &self,
        key: &ActivityStreamKey,
        from_seq: u64,
    ) -> Result<Vec<ActivityRecord>, StoreError> {
        let streams = self.streams.lock().map_err(|error| {
            StoreError::Backend(format!("observability mutex poisoned: {error}"))
        })?;
        Ok(streams
            .get(&key_bytes(key))
            .map_or_else(Vec::new, |stream| {
                stream
                    .iter()
                    .filter(|record| record.store_seq >= from_seq)
                    .cloned()
                    .collect()
            }))
    }

    async fn list_activity_streams(
        &self,
        workflow_id: &WorkflowId,
        run_id: &RunId,
    ) -> Result<Vec<ActivityStreamSummary>, StoreError> {
        let streams = self.streams.lock().map_err(|error| {
            StoreError::Backend(format!("observability mutex poisoned: {error}"))
        })?;
        let mut summaries: Vec<ActivityStreamSummary> = streams
            .iter()
            .filter(|((workflow, run, _activity, _attempt), _records)| {
                *workflow == workflow_id.as_uuid() && *run == run_id.as_uuid()
            })
            .map(
                |(&(workflow, run, activity_seq, attempt), records)| ActivityStreamSummary {
                    key: ActivityStreamKey::new(
                        WorkflowId::new(workflow),
                        RunId::new(run),
                        ActivityId::from_sequence_position(activity_seq),
                        attempt,
                    ),
                    head: stream_head(records),
                },
            )
            .collect();
        summaries.sort_by_key(|summary| {
            (
                summary.key.activity_id.sequence_position(),
                summary.key.attempt,
            )
        });
        Ok(summaries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aion_core::{ActivityEventKind, MessageRole};
    use chrono::Utc;
    use uuid::Uuid;

    fn workflow() -> WorkflowId {
        WorkflowId::new(Uuid::from_u128(1))
    }

    /// The first generation of the continue-as-new chain under test.
    fn generation_one() -> RunId {
        RunId::new(Uuid::from_u128(0x11))
    }

    /// The second generation: SAME workflow, different run.
    fn generation_two() -> RunId {
        RunId::new(Uuid::from_u128(0x22))
    }

    fn event(attempt: u32, worker_seq: u64, text: &str) -> ActivityEvent {
        ActivityEvent {
            workflow_id: workflow(),
            run_id: generation_one(),
            activity_id: ActivityId::from_sequence_position(3),
            attempt,
            agent_id: Uuid::from_u128(9),
            agent_role: "orchestrator".to_owned(),
            emitted_at: Utc::now(),
            worker_seq,
            store_seq: None,
            ephemeral: false,
            kind: ActivityEventKind::Message {
                role: MessageRole::Assistant,
                text: text.to_owned(),
            },
        }
    }

    fn key(attempt: u32) -> ActivityStreamKey {
        ActivityStreamKey::new(
            workflow(),
            generation_one(),
            ActivityId::from_sequence_position(3),
            attempt,
        )
    }

    #[tokio::test]
    async fn append_assigns_contiguous_store_seq_from_zero() -> Result<(), StoreError> {
        let store = InMemoryObservabilityStore::default();
        let key = key(0);
        assert_eq!(store.activity_head(&key).await?, 0);
        assert_eq!(store.append_activity_event(0, &event(0, 1, "a")).await?, 0);
        assert_eq!(store.append_activity_event(1, &event(0, 2, "b")).await?, 1);
        assert_eq!(store.activity_head(&key).await?, 2);
        let records = store.read_activity_events_from(&key, 0).await?;
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].store_seq, 0);
        assert_eq!(records[0].event.store_seq, Some(0));
        assert_eq!(records[1].store_seq, 1);
        Ok(())
    }

    #[tokio::test]
    async fn stale_expected_seq_conflicts_and_writes_nothing() -> Result<(), StoreError> {
        let store = InMemoryObservabilityStore::default();
        store.append_activity_event(0, &event(0, 1, "a")).await?;
        // Re-appending at the already-consumed seq 0 conflicts against head 1.
        let conflict = store.append_activity_event(0, &event(0, 2, "dup")).await;
        assert_eq!(
            conflict,
            Err(StoreError::SequenceConflict {
                expected: 0,
                found: 1
            })
        );
        let key = ActivityStreamKey::of(&event(0, 0, ""));
        // Nothing partial was written: still exactly one record.
        assert_eq!(store.read_activity_events_from(&key, 0).await?.len(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn attempts_are_disjoint_streams() -> Result<(), StoreError> {
        let store = InMemoryObservabilityStore::default();
        store
            .append_activity_event(0, &event(0, 1, "attempt-0"))
            .await?;
        // A different attempt is a fresh stream with its own head at 0.
        store
            .append_activity_event(0, &event(1, 1, "attempt-1"))
            .await?;
        assert_eq!(store.activity_head(&key(0)).await?, 1);
        assert_eq!(store.activity_head(&key(1)).await?, 1);
        Ok(())
    }

    /// THE RUN-SCOPING INVARIANT at the store contract.
    ///
    /// Two generations of one continue-as-new chain emit from the SAME
    /// `(workflow, activity ordinal 0, attempt 1)` coordinates — which is what
    /// actually happens, because ordinals restart at `0` and attempts at `1` in
    /// each new run. Both appends must therefore succeed at `expected_seq == 0`
    /// (each is its own stream head), the two events must land under DIFFERENT
    /// keys, and a read scoped to generation two must return EXACTLY ONE event:
    /// its own. Under the pre-run-axis key the second append would have
    /// conflicted against generation one's head and then fused onto its stream.
    #[tokio::test]
    async fn two_generations_of_one_chain_never_share_a_stream() -> Result<(), StoreError> {
        let store = InMemoryObservabilityStore::default();
        let ordinal_zero = ActivityId::from_sequence_position(0);
        let mut first = event(1, 1, "generation one");
        first.activity_id = ordinal_zero.clone();
        let mut second = first.clone();
        second.run_id = generation_two();
        second.kind = ActivityEventKind::Message {
            role: MessageRole::Assistant,
            text: "generation two".to_owned(),
        };

        // Both are the FIRST event of their own stream: both append at seq 0.
        assert_eq!(store.append_activity_event(0, &first).await?, 0);
        assert_eq!(store.append_activity_event(0, &second).await?, 0);

        let first_key = ActivityStreamKey::of(&first);
        let second_key = ActivityStreamKey::of(&second);
        assert_ne!(
            first_key, second_key,
            "one chain's two generations must not share a stream key"
        );
        // The keys differ ONLY in the run axis — the collision this guards.
        assert_eq!(first_key.workflow_id, second_key.workflow_id);
        assert_eq!(first_key.activity_id, second_key.activity_id);
        assert_eq!(first_key.attempt, second_key.attempt);

        let second_generation = store.read_activity_events_from(&second_key, 0).await?;
        assert_eq!(
            second_generation.len(),
            1,
            "a read scoped to generation two must return exactly its own event"
        );
        assert_eq!(second_generation[0].event.run_id, generation_two());
        // And generation one is likewise untouched by generation two's append.
        let first_generation = store.read_activity_events_from(&first_key, 0).await?;
        assert_eq!(first_generation.len(), 1);
        assert_eq!(first_generation[0].event.run_id, generation_one());
        Ok(())
    }

    /// Two activities x two attempts of run one plus one stream of run two:
    /// listing run one yields exactly its three streams, ordered by
    /// `(activity, attempt)` ascending, each with the correct head. The run-two
    /// stream shares a workflow with them and must NOT appear.
    #[tokio::test]
    async fn list_activity_streams_orders_by_activity_then_attempt() -> Result<(), StoreError> {
        let store = InMemoryObservabilityStore::default();
        let event_for = |activity_seq: u64, attempt: u32, run: RunId| {
            let mut event = event(attempt, 1, "x");
            event.run_id = run;
            event.activity_id = ActivityId::from_sequence_position(activity_seq);
            event
        };
        // run one: activity 3 attempt 0 (two records), activity 3 attempt 1
        // (one), activity 5 attempt 0 (one). Inserted deliberately out of order.
        store
            .append_activity_event(0, &event_for(5, 0, generation_one()))
            .await?;
        store
            .append_activity_event(0, &event_for(3, 1, generation_one()))
            .await?;
        store
            .append_activity_event(0, &event_for(3, 0, generation_one()))
            .await?;
        store
            .append_activity_event(1, &event_for(3, 0, generation_one()))
            .await?;
        // run two of the SAME workflow: one stream that must not leak in.
        store
            .append_activity_event(0, &event_for(3, 0, generation_two()))
            .await?;

        let summaries = store
            .list_activity_streams(&workflow(), &generation_one())
            .await?;
        let listed: Vec<(u64, u32, u64)> = summaries
            .iter()
            .map(|summary| {
                (
                    summary.key.activity_id.sequence_position(),
                    summary.key.attempt,
                    summary.head,
                )
            })
            .collect();
        assert_eq!(listed, vec![(3, 0, 2), (3, 1, 1), (5, 0, 1)]);
        assert!(
            summaries
                .iter()
                .all(|summary| summary.key.run_id == generation_one()),
            "every summary names the run it was enumerated for"
        );
        Ok(())
    }

    #[tokio::test]
    async fn list_activity_streams_is_empty_for_unknown_workflow() -> Result<(), StoreError> {
        let store = InMemoryObservabilityStore::default();
        store.append_activity_event(0, &event(0, 1, "a")).await?;
        let summaries = store
            .list_activity_streams(&WorkflowId::new(Uuid::from_u128(99)), &generation_one())
            .await?;
        assert!(summaries.is_empty(), "an unwritten workflow lists empty");
        // A known workflow with an unwritten RUN is likewise empty, not the
        // sibling generation's streams.
        let other_run = store
            .list_activity_streams(&workflow(), &generation_two())
            .await?;
        assert!(other_run.is_empty(), "an unwritten run lists empty");
        Ok(())
    }

    /// The batch contract: one append, contiguous sequences from `expected_seq`,
    /// the new head returned, order preserved, each record self-describing.
    #[tokio::test]
    async fn a_batch_assigns_contiguous_sequences_and_returns_the_new_head()
    -> Result<(), StoreError> {
        let store = InMemoryObservabilityStore::default();
        let batch: Vec<ActivityEvent> = (0..4).map(|seq| event(0, seq, "batched")).collect();
        assert_eq!(store.append_activity_events(0, &batch).await?, 4);
        let records = store.read_activity_events_from(&key(0), 0).await?;
        assert_eq!(
            records
                .iter()
                .map(|record| (
                    record.store_seq,
                    record.event.store_seq,
                    record.event.worker_seq
                ))
                .collect::<Vec<(u64, Option<u64>, u64)>>(),
            vec![
                (0, Some(0), 0),
                (1, Some(1), 1),
                (2, Some(2), 2),
                (3, Some(3), 3)
            ],
            "the i-th event lands at expected_seq + i, in order, self-describing"
        );
        // A second batch continues from the returned head.
        let more: Vec<ActivityEvent> = (4..6).map(|seq| event(0, seq, "more")).collect();
        assert_eq!(store.append_activity_events(4, &more).await?, 6);
        assert_eq!(store.activity_head(&key(0)).await?, 6);
        Ok(())
    }

    /// All-or-nothing: a stale `expected_seq` writes NO event of the batch, so a
    /// losing writer never leaves a partial batch for the retry to duplicate.
    #[tokio::test]
    async fn a_conflicted_batch_writes_none_of_its_events() -> Result<(), StoreError> {
        let store = InMemoryObservabilityStore::default();
        store
            .append_activity_event(0, &event(0, 1, "first"))
            .await?;
        let batch: Vec<ActivityEvent> = (2..6).map(|seq| event(0, seq, "losing")).collect();
        assert_eq!(
            store.append_activity_events(0, &batch).await,
            Err(StoreError::SequenceConflict {
                expected: 0,
                found: 1
            })
        );
        assert_eq!(
            store.read_activity_events_from(&key(0), 0).await?.len(),
            1,
            "nothing partial was written"
        );
        Ok(())
    }

    /// One batch addresses ONE stream: a mixed slice is refused whole, never
    /// silently split (which would re-introduce the per-event commit) and never
    /// appended under the wrong stream's head.
    #[tokio::test]
    async fn a_batch_that_mixes_streams_is_refused() -> Result<(), StoreError> {
        let store = InMemoryObservabilityStore::default();
        let mixed = vec![event(0, 1, "attempt-0"), event(1, 2, "attempt-1")];
        match store.append_activity_events(0, &mixed).await {
            Err(StoreError::Backend(message)) => {
                assert!(message.contains("mixes stream keys"), "{message}");
            }
            other => {
                return Err(StoreError::Backend(format!(
                    "expected a refusal: {other:?}"
                )));
            }
        }
        assert_eq!(store.activity_head(&key(0)).await?, 0);
        assert_eq!(store.activity_head(&key(1)).await?, 0);
        Ok(())
    }

    /// An empty batch is a no-op that returns the head it was given.
    #[tokio::test]
    async fn an_empty_batch_is_a_no_op() -> Result<(), StoreError> {
        let store = InMemoryObservabilityStore::default();
        assert_eq!(store.append_activity_events(0, &[]).await?, 0);
        assert_eq!(store.activity_head(&key(0)).await?, 0);
        Ok(())
    }

    #[tokio::test]
    async fn read_from_resumes_by_store_seq() -> Result<(), StoreError> {
        let store = InMemoryObservabilityStore::default();
        for seq in 0..5u64 {
            store
                .append_activity_event(seq, &event(0, seq, "x"))
                .await?;
        }
        let key = ActivityStreamKey::of(&event(0, 0, ""));
        let tail = store.read_activity_events_from(&key, 3).await?;
        assert_eq!(tail.len(), 2);
        assert_eq!(tail[0].store_seq, 3);
        assert_eq!(tail[1].store_seq, 4);
        Ok(())
    }
}
