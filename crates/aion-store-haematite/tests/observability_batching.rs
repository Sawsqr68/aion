//! The byte-level half of the write-amplification claim, against the real store.
//!
//! `aion-server`'s drain regression counts durable append CALLS. That is only
//! worth counting because each call is one storage-tree commit and each commit
//! writes NEW permanent node blobs — a whole rebuilt leaf plus its spine — which
//! is the amplification the 2026-08-17 forensics measured (a 2,359-byte append
//! bought a 2,303,416-byte blob). These tests measure that half directly, by
//! counting the content-addressed node files haematite leaves on disk.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use aion_core::{ActivityEvent, ActivityEventKind, ActivityId, MessageRole, RunId, WorkflowId};
use aion_store::{ActivityStreamKey, ObservabilityStore, StoreError};
use aion_store_haematite::HaematiteStore;
use chrono::Utc;
use uuid::Uuid;

static DATABASE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Events per arm. Small enough to stay fast, large enough that "one commit per
/// event" and "one commit for all of them" cannot be confused for each other.
const EVENTS: u64 = 16;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn unique_temp_dir(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let counter = DATABASE_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "aion-store-haematite-{name}-{}-{nanos}-{counter}",
        std::process::id()
    ))
}

/// Count the immutable content-addressed node files under a haematite data
/// directory — `shard-N/store/xx/<hash>`. These are the blobs a commit mints and
/// nothing ever deletes, so the DELTA across an operation is exactly what that
/// operation cost the disk, permanently. WAL and metadata files live outside
/// `store/` and are excluded so the count measures node blobs only.
fn node_blob_count(data_dir: &Path) -> Result<usize, StoreError> {
    fn walk(directory: &Path, under_store: bool, count: &mut usize) -> std::io::Result<()> {
        for entry in std::fs::read_dir(directory)? {
            let entry = entry?;
            let path = entry.path();
            let file_type = entry.file_type()?;
            if file_type.is_dir() {
                let is_store = under_store || entry.file_name() == std::ffi::OsStr::new("store");
                walk(&path, is_store, count)?;
            } else if under_store {
                *count += 1;
            }
        }
        Ok(())
    }
    let mut count = 0;
    walk(data_dir, false, &mut count)
        .map_err(|error| StoreError::Backend(format!("counting node blobs: {error}")))?;
    Ok(count)
}

/// Two DISTINCT streams so the arms never share a head or a leaf history.
fn event(activity_seq: u64, worker_seq: u64) -> ActivityEvent {
    ActivityEvent {
        workflow_id: WorkflowId::new(Uuid::from_u128(0xBA7C)),
        run_id: RunId::new(Uuid::from_u128(0xB1)),
        activity_id: ActivityId::from_sequence_position(activity_seq),
        attempt: 1,
        agent_id: Uuid::from_u128(9),
        agent_role: "batch-cost".to_owned(),
        emitted_at: Utc::now(),
        worker_seq,
        store_seq: None,
        ephemeral: false,
        kind: ActivityEventKind::Message {
            role: MessageRole::Assistant,
            text: format!("event-{activity_seq}-{worker_seq}"),
        },
    }
}

fn key(activity_seq: u64) -> ActivityStreamKey {
    ActivityStreamKey::new(
        WorkflowId::new(Uuid::from_u128(0xBA7C)),
        RunId::new(Uuid::from_u128(0xB1)),
        ActivityId::from_sequence_position(activity_seq),
        1,
    )
}

/// THE COST MEASUREMENT, both arms in one database.
///
/// Arm A (the CONTROL, the behaviour of record): append `EVENTS` events one at
/// a time. Arm B: append the same number as ONE batch. Both arms write the same
/// events into equivalent, independent streams of the same store, so the only
/// difference is how many commits paid for them.
#[tokio::test(flavor = "multi_thread")]
async fn a_batched_append_costs_one_commit_of_blobs_not_n() -> TestResult {
    let data_dir = unique_temp_dir("obs-batch-cost");
    let store = HaematiteStore::create(&data_dir, haematite::NodeCacheBudget::Unlimited)?;

    // Arm A: one event per append — one tree commit each.
    let before_single = node_blob_count(&data_dir)?;
    for worker_seq in 0..EVENTS {
        store
            .append_activity_event(worker_seq, &event(1, worker_seq))
            .await?;
    }
    let single_cost = node_blob_count(&data_dir)? - before_single;

    // Arm B: the same events, one append.
    let batch: Vec<ActivityEvent> = (0..EVENTS).map(|worker_seq| event(2, worker_seq)).collect();
    let before_batch = node_blob_count(&data_dir)?;
    let new_head = store.append_activity_events(0, &batch).await?;
    let batch_cost = node_blob_count(&data_dir)? - before_batch;

    assert_eq!(new_head, EVENTS, "a batch of n advances the head by n");
    assert_eq!(
        store.activity_head(&key(2)).await?,
        EVENTS,
        "the batched stream's durable head counts every event"
    );
    let records = store.read_activity_events_from(&key(2), 0).await?;
    assert_eq!(
        records
            .iter()
            .map(|record| record.store_seq)
            .collect::<Vec<u64>>(),
        (0..EVENTS).collect::<Vec<u64>>(),
        "the i-th event of a batch is readable at store_seq == expected_seq + i"
    );
    assert_eq!(
        records
            .iter()
            .map(|record| record.event.worker_seq)
            .collect::<Vec<u64>>(),
        (0..EVENTS).collect::<Vec<u64>>(),
        "batched events keep their order"
    );

    // A THIRD arm, taken last so the tree is at least as large: quadruple the
    // batch. Nothing here is a guessed constant — the claim is that a commit's
    // blob cost does not grow with the number of events it carries, and that is
    // measured against the store, not asserted against a magic number.
    let big: Vec<ActivityEvent> = (0..EVENTS * 4)
        .map(|worker_seq| event(7, worker_seq))
        .collect();
    let before_big = node_blob_count(&data_dir)?;
    store.append_activity_events(0, &big).await?;
    let big_batch_cost = node_blob_count(&data_dir)? - before_big;

    // The measurement itself.
    assert!(
        single_cost >= usize::try_from(EVENTS)?,
        "the control arm must cost at least one new node blob per append; \
         {EVENTS} appends cost {single_cost} blobs"
    );
    assert!(
        batch_cost < single_cost,
        "one batched append of {EVENTS} events must cost strictly fewer permanent node \
         blobs than the same events appended one at a time: batch {batch_cost}, control \
         {single_cost}"
    );
    assert!(
        big_batch_cost < single_cost,
        "a commit's blob cost is bounded by the TREE, not by the events it carries: {} \
         events in one commit cost {big_batch_cost} blobs, while {EVENTS} events in \
         {EVENTS} commits cost {single_cost}",
        EVENTS * 4
    );
    Ok(())
}

/// A batch is all-or-nothing: a stale `expected_seq` writes NOTHING and mints no
/// blob, so a losing writer never leaves half a batch (or a rewritten leaf)
/// behind.
#[tokio::test(flavor = "multi_thread")]
async fn a_conflicted_batch_writes_nothing_and_costs_nothing() -> TestResult {
    let data_dir = unique_temp_dir("obs-batch-conflict");
    let store = HaematiteStore::create(&data_dir, haematite::NodeCacheBudget::Unlimited)?;
    store.append_activity_event(0, &event(3, 0)).await?;

    let before = node_blob_count(&data_dir)?;
    let batch: Vec<ActivityEvent> = (1..4).map(|worker_seq| event(3, worker_seq)).collect();
    // Head is 1; appending at 0 is the stale expectation a racing writer sees.
    let conflict = store.append_activity_events(0, &batch).await;
    assert_eq!(
        conflict,
        Err(StoreError::SequenceConflict {
            expected: 0,
            found: 1
        }),
        "a stale expected_seq reports the actual head"
    );
    assert_eq!(
        node_blob_count(&data_dir)?,
        before,
        "a conflicted batch mints no node blob at all"
    );
    assert_eq!(
        store.activity_head(&key(3)).await?,
        1,
        "and leaves the stream exactly as it was"
    );
    Ok(())
}

/// A batch addresses ONE stream. A slice that mixes keys is a caller bug and is
/// refused before anything is written — never silently split (which would
/// re-introduce the per-event commit) and never appended under the wrong
/// stream's head.
#[tokio::test(flavor = "multi_thread")]
async fn a_batch_that_mixes_streams_is_refused() -> TestResult {
    let data_dir = unique_temp_dir("obs-batch-mixed");
    let store = HaematiteStore::create(&data_dir, haematite::NodeCacheBudget::Unlimited)?;
    let before = node_blob_count(&data_dir)?;

    let mixed = vec![event(4, 0), event(5, 1)];
    let error = store.append_activity_events(0, &mixed).await;
    match error {
        Err(StoreError::Backend(message)) => assert!(
            message.contains("mixes stream keys"),
            "the refusal names the defect: {message}"
        ),
        other => return Err(format!("a mixed-stream batch must be refused: {other:?}").into()),
    }
    assert_eq!(
        node_blob_count(&data_dir)?,
        before,
        "a refused batch mints no node blob"
    );
    assert_eq!(store.activity_head(&key(4)).await?, 0);
    assert_eq!(store.activity_head(&key(5)).await?, 0);
    Ok(())
}

/// An empty batch is a no-op: no commit, no blob, head unchanged.
#[tokio::test(flavor = "multi_thread")]
async fn an_empty_batch_commits_nothing() -> TestResult {
    let data_dir = unique_temp_dir("obs-batch-empty");
    let store = HaematiteStore::create(&data_dir, haematite::NodeCacheBudget::Unlimited)?;
    store.append_activity_event(0, &event(6, 0)).await?;
    let before = node_blob_count(&data_dir)?;

    assert_eq!(store.append_activity_events(1, &[]).await?, 1);
    assert_eq!(
        node_blob_count(&data_dir)?,
        before,
        "an empty batch pays for no commit"
    );
    assert_eq!(store.activity_head(&key(6)).await?, 1);
    Ok(())
}
