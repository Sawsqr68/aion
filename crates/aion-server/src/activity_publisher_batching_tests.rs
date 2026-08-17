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

use crate::activity_publisher::{ActivityEventPublisher, TranscriptBatchPolicy};
use crate::worker::declared_body_transcript::publish_declared_transcript;

/// What a test returns: every fallible step is carried, never unwrapped.
type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Events fed to the drain in one go — one agent's chatty attempt.
const EVENTS: u64 = 64;

/// The batch size the drain is configured with here. A TEST parameter, not a
/// shipped default: production takes `observability.max_batch_events` from the
/// operator's config, which has no default at all.
const MAX_BATCH_EVENTS: u64 = 16;

/// The flush policy under test. Both values are TEST parameters — production
/// takes `observability.max_batch_events` / `max_batch_hold_ms` from the
/// operator's config, which has no default at all. The hold is generous so the
/// assertion measures coalescing rather than this machine's scheduling.
fn policy() -> Result<TranscriptBatchPolicy, Box<dyn std::error::Error>> {
    Ok(TranscriptBatchPolicy {
        max_batch_events: std::num::NonZeroUsize::new(usize::try_from(MAX_BATCH_EVENTS)?)
            .ok_or("max batch events must be non-zero")?,
        max_hold: std::time::Duration::from_millis(50),
    })
}

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
    async fn append_activity_events(
        &self,
        expected_seq: u64,
        events: &[ActivityEvent],
    ) -> Result<u64, StoreError> {
        self.commits.fetch_add(1, Ordering::SeqCst);
        self.inner
            .append_activity_events(expected_seq, events)
            .await
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
    let publisher = ActivityEventPublisher::new(store.clone(), TRANSCRIPT_CAPACITY, policy()?);

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

/// The liminal observability tap — the seam the forensics measured — drains a
/// BOUNDED channel through the very same loop, so the coalescing proven above
/// is the coalescing the agent transcript path gets. Same assertion, same
/// publisher method, different receiver type.
#[tokio::test(flavor = "multi_thread")]
async fn the_bounded_observability_tap_channel_coalesces_the_same_way() -> TestResult {
    let store = Arc::new(CommitCountingStore::default());
    let publisher = ActivityEventPublisher::new(store.clone(), TRANSCRIPT_CAPACITY, policy()?);

    let (sender, mut receiver) =
        tokio::sync::mpsc::channel::<ActivityEvent>(usize::try_from(EVENTS)?);
    for worker_seq in 0..EVENTS {
        sender.send(event(worker_seq)).await?;
    }
    drop(sender);
    let dropped = publisher.drain(&mut receiver, "observability_tap").await;

    assert_eq!(dropped, 0, "a healthy store refuses nothing");
    assert_eq!(
        publisher.replay_from(&stream_key(), 0).await?.len(),
        usize::try_from(EVENTS)?,
        "every tapped event is retained"
    );
    let commits = store.commits();
    let ceiling = EVENTS.div_ceil(MAX_BATCH_EVENTS);
    assert!(
        commits <= ceiling,
        "the bounded tap must coalesce too: {EVENTS} events, {commits} commits, ceiling \
         {ceiling}"
    );
    Ok(())
}

/// A batch spanning several agents costs ONE commit PER STREAM, not one per
/// event: the durable append is per stream key, so the drain groups by key and
/// each group's whole share lands in a single commit.
#[tokio::test(flavor = "multi_thread")]
async fn a_multi_stream_batch_costs_one_commit_per_stream() -> TestResult {
    let store = Arc::new(CommitCountingStore::default());
    let publisher = ActivityEventPublisher::new(store.clone(), TRANSCRIPT_CAPACITY, policy()?);

    // Three interleaved activities, four events each — the shape a shared
    // server-wide transcript queue actually carries.
    let mut batch = Vec::new();
    for worker_seq in 0..4u64 {
        for activity in 0..3u64 {
            let mut event = event(worker_seq);
            event.activity_id = ActivityId::from_sequence_position(activity);
            batch.push(event);
        }
    }
    let assigned = publisher.publish_all(&batch).await?;

    assert_eq!(assigned.len(), batch.len(), "one answer per input event");
    assert!(
        assigned.iter().all(Option::is_some),
        "every non-ephemeral event of the batch is persisted"
    );
    assert_eq!(
        store.commits(),
        3,
        "three streams cost three commits — not twelve, and not one (a commit \
         addresses exactly one stream)"
    );
    for activity in 0..3u64 {
        let key = ActivityStreamKey::new(
            workflow(),
            run(),
            ActivityId::from_sequence_position(activity),
            1,
        );
        let retained = publisher.replay_from(&key, 0).await?;
        assert_eq!(
            retained
                .iter()
                .map(|record| record.event.worker_seq)
                .collect::<Vec<u64>>(),
            (0..4).collect::<Vec<u64>>(),
            "each stream keeps its own events in arrival order"
        );
    }
    Ok(())
}

/// Ephemeral events inside a batch are still never persisted: they are fanned
/// out live and dropped from the commit, and the durable events around them
/// keep contiguous sequences.
#[tokio::test(flavor = "multi_thread")]
async fn ephemeral_events_inside_a_batch_are_never_persisted() -> TestResult {
    let store = Arc::new(CommitCountingStore::default());
    let publisher = ActivityEventPublisher::new(store.clone(), TRANSCRIPT_CAPACITY, policy()?);

    let mut batch = Vec::new();
    for worker_seq in 0..6u64 {
        let mut event = event(worker_seq);
        event.ephemeral = worker_seq % 2 == 1;
        batch.push(event);
    }
    let assigned = publisher.publish_all(&batch).await?;

    assert_eq!(
        assigned,
        vec![Some(0), None, Some(1), None, Some(2), None],
        "the durable events take contiguous sequences; the ephemeral ones take none"
    );
    assert_eq!(store.commits(), 1, "one commit for the durable remainder");
    assert_eq!(
        publisher
            .replay_from(&stream_key(), 0)
            .await?
            .iter()
            .map(|record| record.event.worker_seq)
            .collect::<Vec<u64>>(),
        vec![0, 2, 4],
        "only the non-ephemeral events are retained"
    );
    Ok(())
}

/// The per-stream retention cap survives batching: a batch that crosses the cap
/// commits the events below it, writes the ONE cap marker at the cap sequence,
/// and leaves the remainder live-only — the same sequence of durable states the
/// single-event path produced.
#[tokio::test(flavor = "multi_thread")]
async fn a_batch_that_crosses_the_retention_cap_splits_at_the_cap() -> TestResult {
    let store = Arc::new(CommitCountingStore::default());
    let publisher = ActivityEventPublisher::new(store.clone(), TRANSCRIPT_CAPACITY, policy()?)
        .with_bounds(crate::activity_bounds::TranscriptBounds {
            max_event_bytes: 64 * 1024,
            max_stream_events: 3,
        });

    let batch: Vec<ActivityEvent> = (0..6).map(event).collect();
    let assigned = publisher.publish_all(&batch).await?;

    assert_eq!(
        assigned,
        vec![Some(0), Some(1), Some(2), None, None, None],
        "events below the cap persist; everything from the cap on is live-only"
    );
    let retained = publisher.replay_from(&stream_key(), 0).await?;
    assert_eq!(
        retained.len(),
        4,
        "three events plus exactly one cap marker are retained"
    );
    match &retained[3].event.kind {
        aion_core::ActivityEventKind::Progress {
            detail: aion_core::ProgressDetail::Note { text },
        } => assert!(
            text.contains("retention cap reached"),
            "the marker says why persistence stopped: {text}"
        ),
        other => return Err(format!("expected the cap marker, found {other:?}").into()),
    }
    Ok(())
}

/// The hold window is what lets events that arrive SEPARATELY still share a
/// commit. With a hold configured, a producer trickling events into the queue
/// while the drain waits pays one commit for the burst; the same trickle under
/// the identity policy pays one commit per event.
#[tokio::test(flavor = "multi_thread")]
async fn a_configured_hold_coalesces_a_trickle_the_identity_policy_would_not() -> TestResult {
    async fn trickle(publisher: &ActivityEventPublisher) -> TestResult {
        let (sender, mut receiver) = tokio::sync::mpsc::channel::<ActivityEvent>(64);
        let feeder = tokio::spawn(async move {
            for worker_seq in 0..4u64 {
                if sender.send(event(worker_seq)).await.is_err() {
                    return;
                }
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        });
        publisher.drain(&mut receiver, "hold-window").await;
        feeder.await?;
        Ok(())
    }

    // A hold generously longer than the 5 ms trickle: the whole burst lands in
    // one batch.
    let held_store = Arc::new(CommitCountingStore::default());
    let held = ActivityEventPublisher::new(
        held_store.clone(),
        TRANSCRIPT_CAPACITY,
        TranscriptBatchPolicy {
            max_batch_events: std::num::NonZeroUsize::new(64).ok_or("non-zero")?,
            max_hold: std::time::Duration::from_millis(500),
        },
    );
    trickle(&held).await?;

    // The identity policy — the pre-batching behaviour — cannot coalesce it.
    let unheld_store = Arc::new(CommitCountingStore::default());
    let unheld = ActivityEventPublisher::new(
        unheld_store.clone(),
        TRANSCRIPT_CAPACITY,
        TranscriptBatchPolicy::UNBATCHED,
    );
    trickle(&unheld).await?;

    assert_eq!(
        unheld_store.commits(),
        4,
        "the control arm pays one commit per trickled event"
    );
    assert!(
        held_store.commits() < unheld_store.commits(),
        "a configured hold must coalesce a trickle the identity policy cannot: held \
         {} commits against {} unheld",
        held_store.commits(),
        unheld_store.commits()
    );
    assert_eq!(
        held.replay_from(&stream_key(), 0).await?.len(),
        4,
        "holding never loses an event that the drain went on to commit"
    );
    Ok(())
}
