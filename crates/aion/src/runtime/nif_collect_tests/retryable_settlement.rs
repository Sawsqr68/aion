//! aion#47 — a RETRYABLE `ActivityFailed` is a retry record, not a fan-out
//! member's terminal.
//!
//! The off-thread retry loop
//! (`nif_activity_retry_dispatch.rs::record_retry_event`) appends a
//! **non-terminal** `ActivityFailed` (`ActivityErrorKind::Retryable`) for every
//! failed attempt that still has budget, then re-dispatches the SAME ordinal at
//! the next attempt. The single-activity await path already knows this:
//! `durability/cursor.rs::resolve_activity` walks past a retryable failure to
//! the trail's real end, and `durability/resolver.rs::resolution_from_matched`
//! resolves only a `Terminal` one. The fan-out settlement scan did not: it took
//! the first `ActivityFailed` for the ordinal whatever its `error.kind`.
//!
//! **Every test in this module is RED against the literal prior text — measured,
//! 8/8, alongside the recorder's ninth, in
//! `gate-logs/47-fanout-settlement/evidence/red-unfixed.log`: 43 run, 9 failed.
//! Not one test in THIS module is a control.** The 34 that passed in that same
//! run are the pre-existing settlement / expiry / routing / fan-out suites,
//! carried in the run deliberately so the red set and its baseline are one
//! measurement rather than two. The defect has two faces, and both are red here:
//!
//! 1. a retry record SETTLES a member that should have retried (the row's stated
//!    mechanism); and
//! 2. because the scan takes the FIRST `ActivityFailed` for the ordinal, a retry
//!    record also SHADOWS the genuine terminal that eventually arrives — so the
//!    tests named "…over the retry record" are not restatements of unchanged
//!    behaviour, they are the second red face. Under the prior text they answer
//!    the retry record's `"retryable:boom"` where the trail's real end says
//!    `"exhausted: boom"` or `Cancelled`.
//!
//! The genuine CONTROLS — arms measured GREEN on both sides of the fix, proving
//! the skip is narrow rather than a blanket "ignore every `ActivityFailed`" —
//! live elsewhere and are named with their receipts in the lane report:
//! `durability::recorder::fan_out_tests::fan_out_completion_drops_after_a_terminal_failure`
//! (green pre-fix and post-fix), and the pre-existing settlement suites next
//! door (`settlement.rs`, `expiry.rs`, `routing_*.rs`), whose every recorded
//! failure is `ActivityErrorKind::Terminal` — so the new predicate returns the
//! same answer for every event they carry.

use super::*;

use crate::runtime::nif_collect::OrdinalState;
use crate::runtime::nif_collect_settlement::recorded_terminal;

/// The DISPATCH-STAGING seam (`nif_collect.rs::dispatch_unscheduled`) asks
/// `recorded_terminal` whether a scheduled member is already resolved; a `None`
/// is what re-dispatches an in-flight member after an engine restart.
///
/// A trail that dangles at a retryable attempt record is precisely the crash
/// -mid-retry shape `spawn_completion_task`'s own doc promises replay will
/// re-dispatch. Reading that record as a terminal both refuses the re-dispatch
/// and settles the member `Failed` — the member that would have retried,
/// treated as terminal.
#[test]
fn a_retryable_attempt_record_leaves_the_member_unresolved() -> TestResult {
    let mut history = scheduled_started(0, "a");
    history.push(retryable_failed(0, "retryable:boom", 1));

    assert_eq!(
        recorded_terminal(&history, 0)
            .map_err(|reason| -> Box<dyn std::error::Error> { reason.into() })?,
        None,
        "a retryable attempt record must leave the ordinal unresolved, so recovery re-dispatches it"
    );
    Ok(())
}

/// The SHADOWING face, at the unit seam: a trail's TERMINAL end resolves the
/// ordinal WITH THE TERMINAL'S MESSAGE, over the retry record that precedes it.
///
/// This was written as a trailing arm of the test above under a comment reading
/// "the trail's TERMINAL end still resolves it" — a control's phrasing for an
/// arm that is red, and one the first assertion's panic made unmeasurable
/// anyway. It is neither: under the prior text `recorded_terminal` first-matches
/// the retry record and answers `Failed("retryable:boom")`, so a member that
/// really did fail is reported to workflow code under the wrong message. Split
/// out so the shadowing is measured on its own.
#[test]
fn a_terminal_ending_a_retry_trail_resolves_the_member_over_the_retry_record() -> TestResult {
    let mut history = scheduled_started(0, "a");
    history.push(retryable_failed(0, "retryable:boom", 1));
    history.push(started(0, 2));
    history.push(failed_at_attempt(0, "boom-final", 2));

    assert_eq!(
        recorded_terminal(&history, 0)
            .map_err(|reason| -> Box<dyn std::error::Error> { reason.into() })?,
        Some(OrdinalState::Failed("boom-final".to_owned())),
        "the terminal that ends a retry trail resolves the ordinal, and its message — not the \
         retry record's — is what the settlement reports"
    );
    Ok(())
}

/// `collect_all` must not fail fast on a member whose recorded failure is a
/// retry record. The retry succeeded and its result is in the runtime map,
/// exactly as the completion task delivers it.
#[tokio::test(flavor = "multi_thread")]
async fn a_retryable_attempt_record_does_not_fail_fast_a_collect_all() -> TestResult {
    let mut events = pending_batch(&["a", "b"]);
    events.push(retryable_failed(0, "retryable:boom", 1));
    events.push(started(0, 2));
    events.push(completed(1, r#""done-b""#));
    let harness = CollectHarness::over_events(&events).await?;
    harness
        .deps
        .runtime
        .deliver_activity_completion_message_with_attempt(
            harness.pid,
            "activity:0",
            r#""done-a""#.to_owned(),
            Some(2),
        )?;

    let step = harness.step(CollectKind::All, &specs(&["a", "b"]));

    assert_eq!(
        step,
        Ok(CollectStep::AllCompleted(vec![
            r#""done-a""#.to_owned(),
            r#""done-b""#.to_owned(),
        ])),
        "a retried member's success must settle the batch, not its retry record"
    );
    assert_eq!(
        harness.cancelled_ordinals().await?,
        Vec::<u64>::new(),
        "nothing may be cancelled: no member failed terminally"
    );
    harness.shutdown()
}

/// A retry record must not win a race either — `recorded_race_winner` reads the
/// same failure set the settlement sweep does.
#[tokio::test(flavor = "multi_thread")]
async fn a_retryable_attempt_record_does_not_win_a_race() -> TestResult {
    let mut events = pending_batch(&["a", "b"]);
    events.push(retryable_failed(0, "retryable:boom", 1));
    events.push(started(0, 2));
    let harness = CollectHarness::over_events(&events).await?;

    let step = harness.step(CollectKind::Race, &specs(&["a", "b"]));

    assert_eq!(
        step,
        Ok(CollectStep::Suspend),
        "an open retry trail settles nothing, so the race must stay suspended"
    );
    assert_eq!(
        harness.cancelled_ordinals().await?,
        Vec::<u64>::new(),
        "no winner means no loser cancellations"
    );
    harness.shutdown()
}

/// The aion#47 divergence window, end to end.
///
/// A settlement sweep reads ONE history snapshot, taken at the top of
/// `collect_step`. The off-thread retry loop can append its retry record for a
/// member AFTER that snapshot was taken and BEFORE the sweep appends the
/// member's own terminal. The live pass therefore settles the member from the
/// runtime map; a replayed pass reads the finished trail and settled it
/// `Failed` from the retry record. Same run, two answers.
#[tokio::test(flavor = "multi_thread")]
async fn a_retry_record_landing_mid_sweep_leaves_the_replayed_settlement_unchanged() -> TestResult {
    // LIVE: ordinal 1 is already recorded complete; ordinal 0's successful
    // retry is in the runtime map and its retry record is NOT in this pass's
    // snapshot.
    let mut events = pending_batch(&["a", "b"]);
    events.push(completed(1, r#""done-b""#));
    let live = CollectHarness::over_events(&events).await?;
    live.deps
        .runtime
        .deliver_activity_completion_message_with_attempt(
            live.pid,
            "activity:0",
            r#""done-a""#.to_owned(),
            Some(2),
        )?;

    let live_answer = live.step(CollectKind::All, &specs(&["a", "b"]));
    assert_eq!(
        live_answer,
        Ok(CollectStep::AllCompleted(vec![
            r#""done-a""#.to_owned(),
            r#""done-b""#.to_owned(),
        ])),
        "the live sweep settles ordinal 0 from the runtime map and records its completion"
    );

    let workflow_id = live.workflow_id.clone();
    let run_id = live.handle.run_id().clone();
    let recorded = live.store.read_history(&workflow_id).await?;
    // The retry loop's append lands between the snapshot and the sweep's own
    // terminal append: immediately before ordinal 0's `ActivityCompleted`.
    let completion_index = recorded
        .iter()
        .position(|event| {
            matches!(event, Event::ActivityCompleted { activity_id, .. }
                if activity_id.sequence_position() == 0)
        })
        .ok_or("the live sweep must have recorded ordinal 0's completion")?;
    let mut interleaved = recorded.clone();
    interleaved.insert(completion_index, retryable_failed(0, "retryable:boom", 1));
    live.shutdown()?;

    // REPLAY: a fresh engine epoch over the trail that run actually left.
    let replay = CollectHarness::over_store(
        restored_store(&workflow_id, &interleaved).await?,
        workflow_id,
        run_id,
    )
    .await?;
    let replayed_length = replay.store.read_history(&replay.workflow_id).await?.len();

    assert_eq!(
        replay.step(CollectKind::All, &specs(&["a", "b"])),
        live_answer,
        "the replayed settlement must answer exactly what the live run answered"
    );
    assert_eq!(
        replay.store.read_history(&replay.workflow_id).await?.len(),
        replayed_length,
        "replay must append nothing"
    );
    replay.shutdown()
}

/// The shadowing face at the `collect_all` seam: a terminal failure that ENDS a
/// retry trail fails the batch fast with the TERMINAL's message, not the retry
/// record's.
///
/// NOT a control — RED today, and named for what it demonstrates rather than for
/// what survives. The batch does fail fast either way, so "still fails fast"
/// would have been true and vacuous; what the prior text gets WRONG is the
/// message, `FailFast("retryable:boom")` where the trail's real end says
/// `FailFast("exhausted: boom")`. A member that really did fail is reported to
/// workflow code under a superseded attempt's error.
#[tokio::test(flavor = "multi_thread")]
async fn a_terminal_ending_a_retry_trail_fails_fast_with_the_terminals_message() -> TestResult {
    let mut events = pending_batch(&["a", "b"]);
    events.push(retryable_failed(0, "retryable:boom", 1));
    events.push(started(0, 2));
    events.push(failed_at_attempt(0, "exhausted: boom", 2));
    let harness = CollectHarness::over_events(&events).await?;

    let step = harness.step(CollectKind::All, &specs(&["a", "b"]));

    assert_eq!(
        step,
        Ok(CollectStep::FailFast("exhausted: boom".to_owned()))
    );
    assert_eq!(harness.cancelled_ordinals().await?, vec![1]);
    assert_eq!(harness.pinned(), None);
    harness.shutdown()
}

/// The race half of the shadowing above — also NOT a control, also RED today.
///
/// `recorded_race_winner` is a second first-match scan over the same failure
/// set, so the retry record wins the race under the prior text and the workflow
/// is handed `RaceWon(Err("retryable:boom"))` where the trail's real end says
/// `RaceWon(Err("exhausted: boom"))`. The ordinal that wins is unchanged; the
/// answer it carries is not.
#[tokio::test(flavor = "multi_thread")]
async fn a_terminal_ending_a_retry_trail_wins_a_race_with_the_terminals_message() -> TestResult {
    let mut events = pending_batch(&["a", "b"]);
    events.push(retryable_failed(1, "retryable:boom", 1));
    events.push(started(1, 2));
    events.push(failed_at_attempt(1, "exhausted: boom", 2));
    let harness = CollectHarness::over_events(&events).await?;

    let step = harness.step(CollectKind::Race, &specs(&["a", "b"]));

    assert_eq!(
        step,
        Ok(CollectStep::RaceWon(Err("exhausted: boom".to_owned())))
    );
    assert_eq!(harness.cancelled_ordinals().await?, vec![0]);
    harness.shutdown()
}

/// An `ActivityCancelled` recorded over an open retry trail resolves the member
/// as CANCELLED, not `Failed` — the shadowing face again, at the cancellation
/// terminal.
///
/// NOT a control — RED today, answering `Some(Failed("retryable:boom"))` where
/// the trail's real end says `Some(Cancelled)`. Its job beyond the arms above is
/// the third terminal: the skip must remove retry RECORDS from the scan and
/// nothing else, so an `ActivityCancelled` sitting behind one still resolves the
/// member. (The arms that catch a blanket "ignore every `ActivityFailed`" are
/// the terminal-shadowing three — under that spelling they read past the real
/// terminal too. Both mutants were run; the per-arm verdicts are in the lane
/// report.)
#[tokio::test(flavor = "multi_thread")]
async fn a_cancellation_over_an_open_retry_trail_resolves_the_member_as_cancelled() -> TestResult {
    let mut history = scheduled_started(0, "a");
    history.push(retryable_failed(0, "retryable:boom", 1));
    history.push(Event::ActivityCancelled {
        envelope: placeholder_envelope(),
        activity_id: ActivityId::from_sequence_position(0),
        attempt: 1,
    });

    assert_eq!(
        recorded_terminal(&history, 0)
            .map_err(|reason| -> Box<dyn std::error::Error> { reason.into() })?,
        Some(OrdinalState::Cancelled),
        "a cancellation still resolves an ordinal whose retry trail was open"
    );
    Ok(())
}
