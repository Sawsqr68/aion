//! Durability coverage for the `HaematiteStore` observability (`O`) keyspace.
//!
//! These tests exercise the real haematite backend behind the
//! [`aion_store::ObservabilityStore`] contract: append/read/head round-trips,
//! the `SequenceConflict` optimistic-concurrency signal the server's sequencer
//! retries on, within-attempt dedup, and — the load-bearing durability guarantee
//! — that an `O`-region record survives a database reopen AND is structurally
//! undecodable as a workflow-history `Event`.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use aion_core::{
    ActivityEvent, ActivityEventKind, ActivityId, Event, MessageRole, RunId, WorkflowId,
};
use aion_store::{ActivityStreamKey, ObservabilityStore, ReadableEventStore, StoreError};
use aion_store_haematite::HaematiteStore;
use chrono::Utc;
use uuid::Uuid;

static DATABASE_COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_temp_dir(name: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let counter = DATABASE_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "aion-store-haematite-{name}-{}-{nanos}-{counter}",
        std::process::id()
    ))
}

fn workflow() -> WorkflowId {
    WorkflowId::new(Uuid::from_u128(0xABCD))
}

/// The first generation of the continue-as-new chain under test.
fn generation_one() -> RunId {
    RunId::new(Uuid::from_u128(0x11))
}

/// The second generation: SAME workflow id, new run.
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

#[tokio::test(flavor = "multi_thread")]
async fn append_read_head_round_trip() -> Result<(), StoreError> {
    let store = HaematiteStore::create(
        unique_temp_dir("obs-round-trip"),
        haematite::NodeCacheBudget::Unlimited,
    )?;
    assert_eq!(store.activity_head(&key(0)).await?, 0);
    assert_eq!(store.append_activity_event(0, &event(0, 1, "a")).await?, 0);
    assert_eq!(store.append_activity_event(1, &event(0, 2, "b")).await?, 1);
    assert_eq!(store.append_activity_event(2, &event(0, 3, "c")).await?, 2);
    assert_eq!(store.activity_head(&key(0)).await?, 3);

    let all = store.read_activity_events_from(&key(0), 0).await?;
    assert_eq!(all.len(), 3);
    for (expected, record) in (0u64..).zip(all.iter()) {
        assert_eq!(record.store_seq, expected);
        assert_eq!(record.event.store_seq, Some(expected));
    }
    // Resume by store_seq.
    let tail = store.read_activity_events_from(&key(0), 2).await?;
    assert_eq!(tail.len(), 1);
    assert_eq!(tail[0].store_seq, 2);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn stale_expected_seq_conflicts_and_writes_nothing() -> Result<(), StoreError> {
    let store = HaematiteStore::create(
        unique_temp_dir("obs-conflict"),
        haematite::NodeCacheBudget::Unlimited,
    )?;
    store.append_activity_event(0, &event(0, 1, "a")).await?;
    // Re-appending at the already-consumed seq 0 conflicts against head 1 — the
    // exact signal the server sequencer re-reads-head-and-retries on.
    let conflict = store.append_activity_event(0, &event(0, 2, "dup")).await;
    assert_eq!(
        conflict,
        Err(StoreError::SequenceConflict {
            expected: 0,
            found: 1
        })
    );
    // Nothing partial written.
    assert_eq!(store.read_activity_events_from(&key(0), 0).await?.len(), 1);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn attempts_are_disjoint_streams() -> Result<(), StoreError> {
    let store = HaematiteStore::create(
        unique_temp_dir("obs-attempts"),
        haematite::NodeCacheBudget::Unlimited,
    )?;
    store
        .append_activity_event(0, &event(0, 1, "attempt-0"))
        .await?;
    store
        .append_activity_event(0, &event(1, 1, "attempt-1"))
        .await?;
    assert_eq!(store.activity_head(&key(0)).await?, 1);
    assert_eq!(store.activity_head(&key(1)).await?, 1);
    let a0 = store.read_activity_events_from(&key(0), 0).await?;
    let a1 = store.read_activity_events_from(&key(1), 0).await?;
    assert_eq!(a0.len(), 1);
    assert_eq!(a1.len(), 1);
    // Same store_seq space, different attempt streams.
    assert_eq!(a0[0].store_seq, 0);
    assert_eq!(a1[0].store_seq, 0);
    Ok(())
}

/// Enumeration is scoped to ONE RUN's retained streams, ordered by
/// `(activity, attempt)`, and survives a database reopen — the "open it an hour
/// later" acceptance at the store layer.
///
/// Two things must be excluded, and the fixture supplies both: another
/// workflow's stream, and a SIBLING GENERATION of this same workflow whose
/// `(activity, attempt)` coordinate collides with one of the listed streams.
/// The second is the case a workflow-scoped enumeration could not distinguish.
#[tokio::test(flavor = "multi_thread")]
async fn list_activity_streams_enumerates_only_the_runs_streams_and_survives_reopen()
-> Result<(), StoreError> {
    let workflow_b = WorkflowId::new(Uuid::from_u128(0xBEEF));
    let event_for =
        |workflow: &WorkflowId, activity_seq: u64, attempt: u32, worker_seq: u64| ActivityEvent {
            workflow_id: workflow.clone(),
            activity_id: ActivityId::from_sequence_position(activity_seq),
            ..event(attempt, worker_seq, "listed")
        };
    let sibling_generation = |activity_seq: u64, attempt: u32, worker_seq: u64| ActivityEvent {
        run_id: generation_two(),
        activity_id: ActivityId::from_sequence_position(activity_seq),
        ..event(attempt, worker_seq, "sibling")
    };
    let dir = unique_temp_dir("obs-list");
    {
        let store = HaematiteStore::create(&dir, haematite::NodeCacheBudget::Unlimited)?;
        // wf-A: two streams (activity 3 attempt 0 with two records; activity 5
        // attempt 2 with one record). wf-B: one stream that must not leak.
        store
            .append_activity_event(0, &event_for(&workflow(), 3, 0, 1))
            .await?;
        store
            .append_activity_event(1, &event_for(&workflow(), 3, 0, 2))
            .await?;
        store
            .append_activity_event(0, &event_for(&workflow(), 5, 2, 3))
            .await?;
        store
            .append_activity_event(0, &event_for(&workflow_b, 3, 0, 1))
            .await?;
        // A SIBLING GENERATION of wf-A under coordinates that collide with one
        // of its streams: same workflow, same activity, same attempt, new run.
        // It must not leak into wf-A's generation-one enumeration.
        store
            .append_activity_event(0, &sibling_generation(3, 0, 9))
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
        assert_eq!(
            listed,
            vec![(3, 0, 2), (5, 2, 1)],
            "exactly wf-A generation one's streams, ordered, with correct heads"
        );
        store
            .event_store()
            .database()
            .commit()
            .map_err(|error| StoreError::Backend(format!("commit failed: {error}")))?;
    }
    // Reopen the SAME on-disk database: the enumeration is identical.
    let reopened = HaematiteStore::open(&dir, haematite::NodeCacheBudget::Unlimited)?;
    let summaries = reopened
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
    assert_eq!(
        listed,
        vec![(3, 0, 2), (5, 2, 1)],
        "enumeration survives restart"
    );
    // A workflow with nothing retained reads empty, not an error.
    assert!(
        reopened
            .list_activity_streams(&WorkflowId::new(Uuid::from_u128(0xF00D)), &generation_one())
            .await?
            .is_empty()
    );
    // Generation two of wf-A enumerates exactly its OWN one stream, and it is
    // the colliding `(activity 3, attempt 0)` coordinate — proof the two
    // generations are separately addressable rather than fused.
    let sibling = reopened
        .list_activity_streams(&workflow(), &generation_two())
        .await?;
    let sibling_listed: Vec<(u64, u32, u64)> = sibling
        .iter()
        .map(|summary| {
            (
                summary.key.activity_id.sequence_position(),
                summary.key.attempt,
                summary.head,
            )
        })
        .collect();
    assert_eq!(sibling_listed, vec![(3, 0, 1)]);
    Ok(())
}

/// THE RUN-SCOPING INVARIANT against the real backend, and against the exact
/// coordinates a continue-as-new chain actually produces.
///
/// Generation two of a chain starts its activity ordinals at `0` again and its
/// attempts at `1` again, under the SAME workflow id. Both generations here
/// therefore emit from `(workflow, ordinal 0, attempt 1)`. The invariant:
///
/// - both appends succeed at `expected_seq == 0` (each is its own stream head);
/// - the two records land under DIFFERENT durable keys;
/// - a read scoped to generation two returns EXACTLY ONE event — its own;
/// - and this holds across a database reopen, because the run is in the KEY.
///
/// Under the pre-run-axis 29-byte key, generation two's append would have
/// collided against generation one's head and — after the publisher's
/// re-read-head-and-retry — been appended onto generation one's stream, fusing
/// the two transcripts. That is the defect this pins closed.
#[tokio::test(flavor = "multi_thread")]
async fn two_generations_of_one_chain_never_share_a_durable_stream() -> Result<(), StoreError> {
    let dir = unique_temp_dir("obs-run-scoping");
    let ordinal_zero = ActivityId::from_sequence_position(0);
    let first = ActivityEvent {
        activity_id: ordinal_zero.clone(),
        ..event(1, 1, "generation one")
    };
    let second = ActivityEvent {
        run_id: generation_two(),
        ..first.clone()
    };
    let first_key = ActivityStreamKey::of(&first);
    let second_key = ActivityStreamKey::of(&second);
    // The two keys differ ONLY in the run axis — the collision under test.
    assert_ne!(first_key, second_key);
    assert_eq!(first_key.workflow_id, second_key.workflow_id);
    assert_eq!(first_key.activity_id, second_key.activity_id);
    assert_eq!(first_key.attempt, second_key.attempt);

    {
        let store = HaematiteStore::create(&dir, haematite::NodeCacheBudget::Unlimited)?;
        // Both are the FIRST event of their own stream: both append at seq 0.
        assert_eq!(store.append_activity_event(0, &first).await?, 0);
        assert_eq!(
            store.append_activity_event(0, &second).await?,
            0,
            "generation two's first event must not conflict against generation one's head"
        );
        store
            .event_store()
            .database()
            .commit()
            .map_err(|error| StoreError::Backend(format!("commit failed: {error}")))?;
    }

    let reopened = HaematiteStore::open(&dir, haematite::NodeCacheBudget::Unlimited)?;
    let second_generation = reopened.read_activity_events_from(&second_key, 0).await?;
    assert_eq!(
        second_generation.len(),
        1,
        "a read scoped to generation two must return exactly its own event"
    );
    assert_eq!(second_generation[0].event.run_id, generation_two());
    let first_generation = reopened.read_activity_events_from(&first_key, 0).await?;
    assert_eq!(first_generation.len(), 1);
    assert_eq!(first_generation[0].event.run_id, generation_one());
    // Each stream's head counts ITS OWN records: the retention cap is therefore
    // measured per (run, activity, attempt), not per fused chain.
    assert_eq!(reopened.activity_head(&first_key).await?, 1);
    assert_eq!(reopened.activity_head(&second_key).await?, 1);
    Ok(())
}

/// THE durability guarantee (§7.5 / §0.6): an `O`-region record survives a
/// database reopen AND the workflow-history replay path (`read_history`) cannot
/// see it — the `O` key is byte-disjoint from the `E`-stream, so replay never
/// scans it and could not decode it as an `Event`. This is what makes
/// "durable but non-replay-authoritative" a structural fact.
#[tokio::test(flavor = "multi_thread")]
async fn observability_records_survive_reopen_and_are_invisible_to_replay() -> Result<(), StoreError>
{
    let dir = unique_temp_dir("obs-durable");
    {
        let store = HaematiteStore::create(&dir, haematite::NodeCacheBudget::Unlimited)?;
        store
            .append_activity_event(0, &event(0, 1, "durable-a"))
            .await?;
        store
            .append_activity_event(1, &event(0, 2, "durable-b"))
            .await?;
        store
            .event_store()
            .database()
            .commit()
            .map_err(|error| StoreError::Backend(format!("commit failed: {error}")))?;
    }
    // Reopen the SAME on-disk database.
    let reopened = HaematiteStore::open(&dir, haematite::NodeCacheBudget::Unlimited)?;

    // The observability transcript survived kill-and-reopen.
    let records = reopened.read_activity_events_from(&key(0), 0).await?;
    assert_eq!(
        records.len(),
        2,
        "observability records must survive reopen"
    );
    assert_eq!(records[0].store_seq, 0);
    assert_eq!(records[1].store_seq, 1);

    // The workflow-history replay path cannot see them: `read_history` decodes
    // ONLY the `E`-stream, and the `O` key is not an `E`-stream key. The workflow
    // has zero replay-authoritative history despite two durable `O` records.
    let history: Vec<Event> = reopened.read_history(&workflow()).await?;
    assert!(
        history.is_empty(),
        "observability records must never appear in workflow replay history: {history:?}"
    );
    Ok(())
}
