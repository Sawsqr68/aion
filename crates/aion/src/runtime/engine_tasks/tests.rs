use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use aion_core::{RunId, WorkflowId};

use super::{ArmOutcome, CompletionRetryKey, EngineTaskRuntime};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Sets a flag when the future is dropped (completion or abort).
struct DropFlag(Arc<AtomicBool>);

impl Drop for DropFlag {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

fn park_forever(flag: Arc<AtomicBool>) -> impl Future<Output = ()> + Send + 'static {
    // The guard is captured at construction, not at first poll: a task
    // aborted before it ever runs still drops its future, and the flag
    // must observe that.
    let guard = DropFlag(flag);
    async move {
        let _guard = guard;
        loop {
            tokio::time::sleep(Duration::from_secs(3600)).await;
        }
    }
}

#[test]
fn arming_is_idempotent_per_key() -> TestResult {
    let tasks = EngineTaskRuntime::new()?;
    let parent = 7;
    let child = WorkflowId::new_v4();
    let flag = Arc::new(AtomicBool::new(false));

    assert_eq!(
        tasks.arm_watch(parent, child.clone(), park_forever(Arc::clone(&flag))),
        ArmOutcome::Armed
    );
    // 🔴 `AlreadyArmed`, NOT merely "refused". The two refusals mean
    // opposite things about whether the work has an owner, and while this
    // returned a `bool` this line passed for either — a shutdown-gated
    // refusal here would have read as idempotence.
    assert_eq!(
        tasks.arm_watch(parent, child.clone(), park_forever(Arc::clone(&flag))),
        ArmOutcome::AlreadyArmed
    );
    assert_eq!(tasks.armed_watch_count(), 1);

    // A different child under the same parent is its own watcher.
    assert_eq!(
        tasks.arm_watch(
            parent,
            WorkflowId::new_v4(),
            park_forever(Arc::clone(&flag))
        ),
        ArmOutcome::Armed
    );
    assert_eq!(tasks.armed_watch_count(), 2);
    tasks.shutdown();
    Ok(())
}

#[test]
fn abort_watch_disarms_a_single_key() -> TestResult {
    let tasks = EngineTaskRuntime::new()?;
    let child = WorkflowId::new_v4();
    let other = WorkflowId::new_v4();
    let flag = Arc::new(AtomicBool::new(false));
    assert_eq!(
        tasks.arm_watch(3, child.clone(), park_forever(Arc::clone(&flag))),
        ArmOutcome::Armed
    );
    assert_eq!(
        tasks.arm_watch(3, other.clone(), park_forever(Arc::clone(&flag))),
        ArmOutcome::Armed
    );

    tasks.abort_watch(3, &child);

    assert_eq!(tasks.armed_watch_count(), 1);
    // The remaining key is the other child: re-arming it is still a
    // no-op, re-arming the aborted one is accepted.
    assert_eq!(
        tasks.arm_watch(3, other, park_forever(Arc::clone(&flag))),
        ArmOutcome::AlreadyArmed
    );
    assert_eq!(
        tasks.arm_watch(3, child, park_forever(Arc::clone(&flag))),
        ArmOutcome::Armed
    );
    tasks.shutdown();
    Ok(())
}

#[test]
fn abort_for_parent_leaves_other_parents_armed() -> TestResult {
    let tasks = EngineTaskRuntime::new()?;
    let flag = Arc::new(AtomicBool::new(false));
    for parent in [31, 31, 32] {
        assert_eq!(
            tasks.arm_watch(
                parent,
                WorkflowId::new_v4(),
                park_forever(Arc::clone(&flag))
            ),
            ArmOutcome::Armed
        );
    }

    tasks.abort_watches_for_parent(31);

    assert_eq!(tasks.armed_watch_count(), 1);
    tasks.shutdown();
    Ok(())
}

/// A release issued for one task cannot evict a different task's entry.
///
/// 🔴 READ THE SCOPE NARROWLY — AN EARLIER REVISION OF THIS COMMENT DID NOT.
/// It claimed the race "is not hypothetical and has no other guard", and
/// justified that with a reopen reusing the run id so successive tasks claim
/// one key. **That justification is retracted.** [`CompletionRetryKey`] now
/// carries the monitor pid, and a reopened run's successor lease is a
/// different pid — so the reopen story produces two DIFFERENT keys and
/// cannot produce this collision at all.
///
/// What remains is real but smaller, and worth stating exactly. Two tasks
/// can still hold one key in succession for a single lease: a retry that
/// ends without recording (an unretryable failure, an epoch close) leaves a
/// finished handle behind until its drop guard runs, and a later process
/// exit for that SAME lease arms again — [`EngineTaskRuntime::arm`]'s
/// `Occupied` + `is_finished` arm deliberately REPLACES the dead handle with
/// a fresh one. The outgoing task's release runs in its own drop, which can
/// land after that replacement. Removing by key alone would then delete the
/// live successor's registration, the map would report the lease as unowned
/// while a retry for it was still running, and the next arm would spawn a
/// SECOND writer of that run's terminal. Invariant 3 lost to bookkeeping,
/// with no bad append anywhere to point at.
///
/// Arming-order alone cannot pin this: the claim-on-first-poll in
/// `lifecycle::completion_retry` stops a REFUSED arm from releasing, but a
/// release from a task that genuinely ran and genuinely held the key is not
/// a refusal, and nothing upstream of this function distinguishes it. So
/// the check is measured here, directly, against both a foreign id and the
/// real one.
#[test]
fn a_release_from_a_foreign_task_cannot_evict_a_live_completion_retry() -> TestResult {
    let tasks = EngineTaskRuntime::new()?;
    let run = CompletionRetryKey {
        workflow_id: WorkflowId::new_v4(),
        run_id: RunId::new_v4(),
        monitor_pid: 1,
    };
    let flag = Arc::new(AtomicBool::new(false));

    // The foreign id is a REAL task id from this same runtime, taken from a
    // task that has already finished — which is exactly the shape a
    // superseded predecessor's release carries. A fabricated id would prove
    // less: it could be rejected by something other than the comparison.
    let (foreign_tx, foreign_rx) = std::sync::mpsc::channel();
    assert_eq!(
        tasks.arm_spawn_retry(WorkflowId::new_v4(), async move {
            let _ = foreign_tx.send(tokio::task::id());
        }),
        ArmOutcome::Armed
    );
    let foreign = foreign_rx.recv_timeout(Duration::from_secs(10))?;

    let (live_tx, live_rx) = std::sync::mpsc::channel();
    assert_eq!(
        tasks.arm_completion_retry(run.clone(), {
            let flag = Arc::clone(&flag);
            async move {
                let _ = live_tx.send(tokio::task::id());
                park_forever(flag).await;
            }
        }),
        ArmOutcome::Armed
    );
    let live = live_rx.recv_timeout(Duration::from_secs(10))?;
    assert_ne!(
        foreign, live,
        "control: the two ids must differ, or the assertions below cannot tell the identity \
         check from an unconditional removal"
    );
    assert_eq!(tasks.armed_completion_retry_count(), 1);

    tasks.remove_completion_retry(&run, foreign);

    assert_eq!(
        tasks.armed_completion_retry_count(),
        1,
        "a release issued for a different task evicted the live retry's registration; the run \
         now reads as unowned while a retry for it is still running, and the next arm for it \
         would spawn a second writer of the same terminal"
    );

    // Positive control: the check is an identity comparison, not a refusal
    // to remove anything. The real owner's release DOES clear the entry.
    tasks.remove_completion_retry(&run, live);
    assert_eq!(
        tasks.armed_completion_retry_count(),
        0,
        "the owning task's own release must clear its entry, or a finished retry would pin \
         its run's key forever and no later exit for that run could ever arm"
    );

    tasks.shutdown();
    Ok(())
}

/// 🔴 A REOPENED RUN'S SUCCESSOR LEASE MUST BE ABLE TO ARM ITS OWN RETRY.
///
/// Review 8 found this as a silent permanent zombie, and it is the reason
/// [`CompletionRetryKey`] carries a pid. A reopen REUSES the run id, so with
/// the old `(WorkflowId, RunId)` key the successor lease's arm collided with
/// its own superseded predecessor's still-sleeping retry and came back
/// `AlreadyArmed` — "somebody owns this terminal". The incumbent then stood
/// down without writing, because `monitor_stands_down` sees its pid
/// superseded. The successor's terminal was never recorded, the run
/// projected `Running` for the life of the epoch, and the only trace was a
/// `debug!` line asserting the opposite.
///
/// The three assertions are one property split by what each can catch:
///
/// - **The control.** Same workflow, same run, SAME pid arms once and is
///   refused the second time. Without it, a key that ignored the run
///   entirely would also pass the decisive assertion below while destroying
///   the single-writer guarantee this map exists for.
/// - **The decisive one.** Same workflow, same run, DIFFERENT pid is
///   `Armed`, not `AlreadyArmed`.
/// - **The count.** Both leases hold a registration simultaneously. Two
///   armed retries under different pids is the correct state, not a leak:
///   the superseded one stands down by the identity check it already
///   performs, so exactly one writes.
///
/// The defect this reconstructs is a key that does not distinguish two
/// leases over the same run. A mutation built out of the new field (for
/// example always passing the same pid) would reconstruct nothing and pass.
///
/// 🔴 The literal prior key — deleting the `monitor_pid` FIELD — cannot be
/// run against this test. The test constructs the key with that field and
/// the control reads it, so deleting it does not compile, and **a mutation
/// that fails to compile is a NON-RUN: neither a kill nor a survivor.** The
/// executable equivalent, and the one actually run, keeps the field and
/// hand-writes `PartialEq`/`Hash` to ignore it — same observable key
/// collapse, and it compiles because every struct literal still type-checks.
/// Under it this test fails at "the decisive one"
/// (`left: AlreadyArmed, right: Armed`) and NOT at the control, which is the
/// point of the control being a statement about the fixture rather than a
/// second copy of the subject.
#[test]
fn a_reopened_runs_successor_lease_can_arm_its_own_completion_retry() -> TestResult {
    let tasks = EngineTaskRuntime::new()?;
    let workflow_id = WorkflowId::new_v4();
    let run_id = RunId::new_v4();
    let predecessor = CompletionRetryKey {
        workflow_id: workflow_id.clone(),
        run_id: run_id.clone(),
        monitor_pid: 11,
    };
    // Same run, new lease — what `respawn_and_register` installs after a
    // reopen. Only the pid differs, which is the whole point.
    let successor = CompletionRetryKey {
        workflow_id,
        run_id,
        monitor_pid: 12,
    };
    // CONTROL — and it is deliberately a statement about the FIXTURE, not
    // about the key's equality.
    //
    // 🔴 Its first form was `assert_ne!(predecessor, successor)`, which is
    // the same claim the decisive assertion below tests. That made the test
    // useless against the mutation it exists to catch: a key that stopped
    // distinguishing leases collapses `predecessor == successor`, the
    // control fires FIRST, and the decisive assertion never executes — so
    // it measured nothing, and the reader is handed a message about test
    // setup instead of about the defect. A control must survive the
    // mutation the test is aimed at; if it cannot, it is not a control, it
    // is a second copy of the subject.
    assert_ne!(
        predecessor.monitor_pid, successor.monitor_pid,
        "control: the fixture must have built two DIFFERENT leases — same workflow, same \
         run, different monitor pid — or there is no reopen here to test"
    );
    assert_eq!(
        (&predecessor.workflow_id, &predecessor.run_id),
        (&successor.workflow_id, &successor.run_id),
        "control: and they must be the SAME run, or this is two unrelated workflows and the \
         collision the reopen causes never arises"
    );

    let predecessor_flag = Arc::new(AtomicBool::new(false));
    assert_eq!(
        tasks.arm_completion_retry(
            predecessor.clone(),
            park_forever(Arc::clone(&predecessor_flag))
        ),
        ArmOutcome::Armed,
        "the superseded lease's retry is armed and still sleeping on its backoff"
    );

    // CONTROL: the same lease arming twice is still refused. This is the
    // guarantee the pid must not have weakened.
    let duplicate_flag = Arc::new(AtomicBool::new(false));
    assert_eq!(
        tasks.arm_completion_retry(predecessor, park_forever(Arc::clone(&duplicate_flag))),
        ArmOutcome::AlreadyArmed,
        "control: one LEASE must still never hold two retries — if this is `Armed` the pid \
         did not narrow the key, it replaced it, and one run's terminal has two writers"
    );

    // DECISIVE: the successor lease is a different writer and owes its own
    // terminal, so it must get its own retry.
    let successor_flag = Arc::new(AtomicBool::new(false));
    assert_eq!(
        tasks.arm_completion_retry(successor, park_forever(Arc::clone(&successor_flag))),
        ArmOutcome::Armed,
        "THE DECISIVE ONE: a reopened run's successor lease was refused as `AlreadyArmed` by \
         its own superseded predecessor, whose retry then stood down without writing — so \
         the successor's terminal was never recorded and the run projected Running forever"
    );

    assert_eq!(
        tasks.armed_completion_retry_count(),
        2,
        "both leases hold a registration: the superseded one until it stands down, the \
         successor until it writes"
    );

    tasks.shutdown();
    Ok(())
}

#[test]
fn shutdown_gates_new_arms_and_awaits_aborted_tasks() -> TestResult {
    let tasks = EngineTaskRuntime::new()?;
    let watch_flag = Arc::new(AtomicBool::new(false));
    let retry_flag = Arc::new(AtomicBool::new(false));
    let completion_flag = Arc::new(AtomicBool::new(false));
    let child = WorkflowId::new_v4();
    let run = CompletionRetryKey {
        workflow_id: WorkflowId::new_v4(),
        run_id: RunId::new_v4(),
        monitor_pid: 1,
    };
    assert_eq!(
        tasks.arm_watch(9, child.clone(), park_forever(Arc::clone(&watch_flag))),
        ArmOutcome::Armed
    );
    assert_eq!(
        tasks.arm_spawn_retry(child.clone(), park_forever(Arc::clone(&retry_flag))),
        ArmOutcome::Armed
    );
    assert_eq!(
        tasks.arm_completion_retry(run.clone(), park_forever(Arc::clone(&completion_flag))),
        ArmOutcome::Armed
    );
    assert_eq!(tasks.armed_spawn_retry_count(), 1);
    assert_eq!(tasks.armed_completion_retry_count(), 1);

    tasks.shutdown();

    // Awaited, not just aborted: by the time shutdown returns, both task
    // futures have been dropped to quiescence.
    assert!(
        watch_flag.load(Ordering::Acquire),
        "watcher task must be fully dropped before the epoch closes"
    );
    assert!(
        retry_flag.load(Ordering::Acquire),
        "spawn-retry task must be fully dropped before the epoch closes"
    );
    // The completion retry is the ONLY task kind that appends terminal
    // events, so it is the one whose survival past the epoch would be a
    // second writer against a successor engine over the same store
    // (invariant 3). It was the kind `Drop` originally forgot.
    assert!(
        completion_flag.load(Ordering::Acquire),
        "completion-retry task must be fully dropped before the epoch closes"
    );
    assert_eq!(tasks.armed_watch_count(), 0);
    assert_eq!(tasks.armed_spawn_retry_count(), 0);
    assert_eq!(tasks.armed_completion_retry_count(), 0);
    // The gate holds: nothing can be armed after shutdown, and every
    // refusal names the epoch as the reason.
    //
    // 🔴 `EpochClosed`, not just "refused". This is the half of the
    // distinction that matters to an operator: nothing owns this work and
    // nothing in this process will, until a successor's startup sweep runs.
    // Paired with `arming_is_idempotent_per_key`'s `AlreadyArmed`
    // assertion, the two tests can no longer both pass with the two
    // refusals swapped — which is exactly what the `bool` allowed.
    assert_eq!(
        tasks.arm_watch(9, child.clone(), park_forever(Arc::clone(&watch_flag))),
        ArmOutcome::EpochClosed
    );
    assert_eq!(
        tasks.arm_spawn_retry(child, park_forever(Arc::clone(&retry_flag))),
        ArmOutcome::EpochClosed
    );
    assert_eq!(
        tasks.arm_completion_retry(run, park_forever(Arc::clone(&completion_flag))),
        ArmOutcome::EpochClosed
    );
    Ok(())
}

/// Dropping the registry cancels an armed completion retry.
///
/// `shutdown` is the explicit close; `Drop` is what runs when an engine is
/// released without one, and it must cover the terminal-appending task kind
/// too — the one that can double-write a history past the epoch that owned
/// it.
///
/// 🔴 Read what this does and does not control. The property is real and
/// worth pinning — an engine released without an explicit shutdown must not
/// leave a task that appends terminal events running. But the mechanism that
/// delivers it is the owned runtime's drop, not the `completion_retries`
/// abort sweep in `Drop`: deleting that sweep leaves this test green.
///
/// So this is a pin, not a control for that line, and it is not cited as
/// one. A line with no independent observable has no independent control,
/// and saying so is cheaper than a test that passes for a reason other than
/// the one it names.
#[test]
fn dropping_the_registry_aborts_completion_retries_too() -> TestResult {
    let flag = Arc::new(AtomicBool::new(false));
    {
        let tasks = EngineTaskRuntime::new()?;
        assert_eq!(
            tasks.arm_completion_retry(
                CompletionRetryKey {
                    workflow_id: WorkflowId::new_v4(),
                    run_id: RunId::new_v4(),
                    monitor_pid: 1,
                },
                park_forever(Arc::clone(&flag)),
            ),
            ArmOutcome::Armed
        );
        assert_eq!(tasks.armed_completion_retry_count(), 1);
    }
    // `Drop` releases the runtime without blocking, so the abort is
    // observed rather than awaited. Poll for it instead of asserting on a
    // race we deliberately did not synchronize.
    let aborted = (0..500).any(|_| {
        if flag.load(Ordering::Acquire) {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        false
    });
    assert!(
        aborted,
        "dropping the registry must abort the armed completion retry"
    );
    Ok(())
}

#[tokio::test]
async fn shutdown_is_safe_from_inside_a_host_async_context() -> TestResult {
    let tasks = EngineTaskRuntime::new()?;
    let flag = Arc::new(AtomicBool::new(false));
    assert_eq!(
        tasks.arm_watch(11, WorkflowId::new_v4(), park_forever(Arc::clone(&flag))),
        ArmOutcome::Armed
    );

    // Engine::shutdown runs in whatever context the embedder calls it
    // from — including a current-thread tokio test like this one.
    tasks.shutdown();

    assert!(flag.load(Ordering::Acquire));
    Ok(())
}

#[tokio::test]
async fn drop_backstop_aborts_without_blocking() -> TestResult {
    let flag = Arc::new(AtomicBool::new(false));
    {
        let tasks = EngineTaskRuntime::new()?;
        assert_eq!(
            tasks.arm_watch(12, WorkflowId::new_v4(), park_forever(Arc::clone(&flag))),
            ArmOutcome::Armed
        );
        // Dropped without shutdown: the backstop must abort the task and
        // release the runtime without panicking in this async context.
    }
    // Background shutdown is asynchronous; the abort lands promptly.
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while !flag.load(Ordering::Acquire) {
        if std::time::Instant::now() > deadline {
            return Err("drop backstop never aborted the armed task".into());
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    Ok(())
}
