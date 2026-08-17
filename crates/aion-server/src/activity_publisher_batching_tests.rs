//! Write-amplification regression: the transcript drain must coalesce.
//!
//! # The defect this pins
//!
//! Every agent telemetry event reaches the durable `O` keyspace through a
//! transcript DRAIN — a task that pulls events off an in-process queue and
//! hands them to [`ActivityEventPublisher`] one at a time. Each hand-off was
//! its own `ObservabilityStore` append, which is its own haematite
//! `append_batch` of ONE payload, which is its own tree commit, which
//! re-persists the ENTIRE containing storage leaf as a new permanent
//! content-addressed blob (forensics 2026-08-17,
//! `docs/tracking/aion-write-amplification-forensics-20260817.md` §2: a 2,359
//! honest-byte append bought a 2,303,416-byte permanent blob, ~977x).
//!
//! So the store cost of a run was linear in EVENT COUNT, not in event bytes.
//! N events = N commits = N whole-leaf rewrites.
//!
//! # The instrument
//!
//! [`CommitCountingStore`] counts durable append CALLS at the
//! [`ObservabilityStore`] boundary. That boundary is 1:1 with tree commits by
//! construction — `aion-store-haematite`'s observability append is a single
//! `haematite::EventStore::append_batch`, documented upstream as "atomically
//! append many payloads to `stream_key` as ONE tree commit". The byte-level
//! half of that claim (one batched append writes one commit's worth of new
//! node blobs on disk, N separate appends write N) is measured against the
//! real store in `aion-store-haematite/tests/observability_batching.rs`; this
//! test measures the publisher-path half, which is where the coalescing has to
//! happen.
//!
//! The subject is the REAL production drain
//! ([`publish_declared_transcript`]), not a synthetic store call: the same
//! function the liminal observability tap and the declared-command executor
//! both drain through.

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use aion_core::{ActivityEvent, ActivityEventKind, ActivityId, MessageRole, RunId, WorkflowId};
use aion_store::{
    ActivityRecord, ActivityStreamKey, ActivityStreamSummary, InMemoryObservabilityStore,
    ObservabilityStore, StoreError,
};
use async_trait::async_trait;
use chrono::Utc;
use uuid::Uuid;

use crate::activity_publisher::ActivityEventPublisher;
use crate::worker::declared_body_transcript::publish_declared_transcript;

/// What a test returns: every fallible step is carried, never unwrapped.
type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Events fed to the drain in one go — one agent's chatty attempt.
const EVENTS: u64 = 64;

/// The batch size the drain is configured with here. A TEST parameter, not a
/// shipped default: production takes `observability.max_batch_events` from the
/// operator's config, which has no default at all.
const MAX_BATCH_EVENTS: u64 = 16;

/// The live-tail buffer these tests give the sequencer. A `const` match rather
/// than an unwrap: the workspace denies panicking accessors in tests too.
const TRANSCRIPT_CAPACITY: std::num::NonZeroUsize = match std::num::NonZeroUsize::new(256) {
    Some(capacity) => capacity,
    None => std::num::NonZeroUsize::MIN,
};

/// An [`ObservabilityStore`] that counts durable append calls — one call is one
/// tree commit is one whole-leaf rewrite — and otherwise defers entirely to the
/// in-memory reference implementation, so sequence semantics are unchanged.
#[derive(Debug, Default)]
struct CommitCountingStore {
    inner: InMemoryObservabilityStore,
    commits: AtomicUsize,
}

impl CommitCountingStore {
    /// Durable append calls observed so far.
    fn commits(&self) -> u64 {
        u64::try_from(self.commits.load(Ordering::SeqCst)).unwrap_or(u64::MAX)
    }
}

#[async_trait]
impl ObservabilityStore for CommitCountingStore {
    async fn append_activity_event(
        &self,
        expected_seq: u64,
        event: &ActivityEvent,
    ) -> Result<u64, StoreError> {
        self.commits.fetch_add(1, Ordering::SeqCst);
        self.inner.append_activity_event(expected_seq, event).await
    }

    async fn activity_head(&self, key: &ActivityStreamKey) -> Result<u64, StoreError> {
        self.inner.activity_head(key).await
    }

    async fn read_activity_events_from(
        &self,
        key: &ActivityStreamKey,
        from_seq: u64,
    ) -> Result<Vec<ActivityRecord>, StoreError> {
        self.inner.read_activity_events_from(key, from_seq).await
    }

    async fn list_activity_streams(
        &self,
        workflow_id: &WorkflowId,
        run_id: &RunId,
    ) -> Result<Vec<ActivityStreamSummary>, StoreError> {
        self.inner.list_activity_streams(workflow_id, run_id).await
    }
}

fn workflow() -> WorkflowId {
    WorkflowId::new(Uuid::from_u128(0xBA7C))
}

fn run() -> RunId {
    RunId::new(Uuid::from_u128(0xB1))
}

fn stream_key() -> ActivityStreamKey {
    ActivityStreamKey::new(workflow(), run(), ActivityId::from_sequence_position(0), 1)
}

fn event(worker_seq: u64) -> ActivityEvent {
    ActivityEvent {
        workflow_id: workflow(),
        run_id: run(),
        activity_id: ActivityId::from_sequence_position(0),
        attempt: 1,
        agent_id: Uuid::from_u128(4),
        agent_role: "batch-regression".to_owned(),
        emitted_at: Utc::now(),
        worker_seq,
        store_seq: None,
        ephemeral: false,
        kind: ActivityEventKind::Message {
            role: MessageRole::Assistant,
            text: format!("event-{worker_seq}"),
        },
    }
}

/// THE WRITE-AMPLIFICATION RED.
///
/// Queue `EVENTS` transcript events, then run the production drain over them.
/// Every event must still be durably retained, in order — and the whole drain
/// must cost at most `ceil(EVENTS / MAX_BATCH_EVENTS)` durable commits.
///
/// Before the fix this asserts 64 commits against a ceiling of 4.
#[tokio::test(flavor = "multi_thread")]
async fn the_transcript_drain_coalesces_events_into_batched_commits() -> TestResult {
    let store = Arc::new(CommitCountingStore::default());
    let publisher = ActivityEventPublisher::new(store.clone(), TRANSCRIPT_CAPACITY);

    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
    for worker_seq in 0..EVENTS {
        sender.send(event(worker_seq))?;
    }
    // Closing the seam is what ends the drain: it returns having offered every
    // queued event to the sequencer.
    drop(sender);
    publish_declared_transcript(publisher.clone(), receiver).await;

    // Coalescing must not cost durability: every event is retained, in the
    // order it was queued, with contiguous store sequences.
    let retained = publisher.replay_from(&stream_key(), 0).await?;
    let worker_sequences: Vec<u64> = retained
        .iter()
        .map(|record| record.event.worker_seq)
        .collect();
    assert_eq!(
        worker_sequences,
        (0..EVENTS).collect::<Vec<u64>>(),
        "every queued event is retained exactly once, in arrival order"
    );
    let store_sequences: Vec<u64> = retained.iter().map(|record| record.store_seq).collect();
    assert_eq!(
        store_sequences,
        (0..EVENTS).collect::<Vec<u64>>(),
        "store_seq stays contiguous and monotonic across a batched append"
    );

    let commits = store.commits();
    let ceiling = EVENTS.div_ceil(MAX_BATCH_EVENTS);
    assert!(
        commits <= ceiling,
        "{EVENTS} queued transcript events must cost at most {ceiling} durable commits \
         at a batch size of {MAX_BATCH_EVENTS}; observed {commits} — one commit per event \
         is one whole-leaf rewrite per event, the write amplification of record"
    );
    Ok(())
}
