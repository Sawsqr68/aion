use super::*;

/// PIN (c) — honesty: dead-letter fires on GENUINE delivery failure (here: no
/// worker exists for the pool at all), and it is LOUD to the workflow — the
/// failure reaches the delivery callback rather than leaving the run
/// `Running` in silence forever.
///
/// The durable row transition intentionally precedes callback delivery. This
/// pin therefore waits for both phases instead of mistaking the observable
/// `Failed` intermediate state for completion of the dead-letter sequence.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn dead_letter_is_genuine_and_loud() -> Result<(), TestError> {
    let rig = PinRig::start("genuine-failure", 3, false).await?;
    let row = pending_row(1_000, "undeliverable", 0)?;
    rig.stage(std::slice::from_ref(&row)).await?;

    // Genuine failure: no worker for the pool. Budget (3 attempts, ms-scale
    // backoff) exhausts within a few sweeps. `Failed` is persisted before the
    // callback runs, so neither observation alone is the completed contract.
    let started = Instant::now();
    let deadline = Duration::from_secs(30);
    let mut dead_lettered = false;
    loop {
        dead_lettered |= rig.row_status(&row.dispatch_key).await? == Some(OutboxStatus::Failed);
        if dead_lettered && rig.callback.failure_count() > 0 {
            break;
        }
        if started.elapsed() >= deadline {
            rig.teardown().await?;
            if dead_lettered {
                return Err(test_error(
                    "SILENT DEAD-LETTER: the row dead-lettered but nothing reached the \
                     delivery callback — the workflow reports Running forever (the \
                     liveness lie); dead-letter must surface an explicit \
                     infrastructure failure to the workflow",
                ));
            }
            return Err(test_error(
                "GUARD BROKEN: a genuinely undeliverable row (no worker for the pool) \
                 never dead-lettered — dead-letter must remain reachable for real \
                 delivery failures",
            ));
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // Never executed, never completed: there was no worker.
    if rig.ledger.total() != 0 {
        rig.teardown().await?;
        return Err(test_error(
            "no worker existed, yet an execution was recorded",
        ));
    }
    if rig.callback.completion_count() != 0 {
        rig.teardown().await?;
        return Err(test_error(
            "no worker existed, yet a completion was recorded",
        ));
    }

    // Exactly one loud failure must survive dispatcher shutdown; this catches
    // both silence and duplicate failure delivery.
    let callback = Arc::clone(&rig.callback);
    rig.teardown().await?;
    let failures = callback.failure_count();
    if failures != 1 {
        return Err(test_error(format!(
            "expected exactly one dead-letter failure callback, recorded {failures}"
        )));
    }
    Ok(())
}

/// PIN (d) — the exhibit: the wedge shape of run `0756ecd5`. One fast leg and
/// two long legs staged together against ONE serial worker. Every leg must
/// complete exactly once; none may dead-letter; the fast leg's success must
/// not be survivorship over the long legs' corpses.
///
/// RED today: the fast leg completes inside the bounded awaiter, the long
/// legs burn their whole attempt budgets against the serial worker (each
/// sweep holds the sibling rows behind a 30s bounded await — the same
/// sequential stall the exhibit's timeline shows between 12:35:38 and
/// 12:39:09) and dead-letter while the worker grinds doomed duplicates.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn wedge_shape_regression_0756ecd5() -> Result<(), TestError> {
    let rig = PinRig::start("wedge-shape", 3, true).await?;
    let rows = vec![
        pending_row(5_000, "fast-check", 0)?,
        pending_row(45_000, "long-clippy", 0)?,
        pending_row(45_000, "long-nextest", 0)?,
    ];
    rig.stage(&rows).await?;

    // Deadline: serial execution of all three legs (5+45+45 = 95s) plus
    // delivery margin. Red paths trip earlier: first duplicate at ~t50s,
    // first dead-letter within the scaled 3-attempt budget.
    let verdict = wait_for_exactly_once(
        &rig,
        &rows,
        &["fast-check", "long-clippy", "long-nextest"],
        Duration::from_secs(400),
    )
    .await;
    rig.teardown().await?;
    verdict
}
