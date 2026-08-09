//! Supervision behaviour, proven against real OS processes.
//!
//! These tests drive `/bin/sh` rather than the server's own executable through
//! the [`ManagedExecutable::Path`] seam. That is not a convenience: the
//! production executable IS the running binary, and under `cargo test` the
//! running binary is the test harness — a supervisor pointed at it would
//! re-execute the suite inside itself.
//!
//! Liveness is checked by asking the OPERATING SYSTEM (`kill -0`), never by
//! reading the supervisor's own bookkeeping. A status test that consults the
//! same cell it is validating proves only that a field was assigned.

use std::collections::BTreeSet;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use aion_store::{
    DeployedBinaryIdentity, DesiredState, InMemoryStore, NewWorkerDeployment, WorkerArtifactRef,
    WorkerDeployment, WorkerDeploymentStore,
};

use super::error::SupervisionError;
use super::executable::ManagedExecutable;
use super::fleet::WorkerSupervisor;
use super::policy::SupervisionPolicy;
use super::status::{ManagedWorkerState, ManagedWorkerStatus};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// How long a test waits for a real process transition before declaring
/// failure. Generous on purpose: this decides how long a BROKEN supervisor
/// takes to report, never whether a working one passes.
const BUDGET: Duration = Duration::from_secs(20);
const POLL: Duration = Duration::from_millis(20);

/// The executable every supervision test drives: a harmless shell, never the
/// test binary that `ManagedExecutable::CurrentServer` would resolve to.
fn shell() -> ManagedExecutable {
    ManagedExecutable::Path(PathBuf::from("/bin/sh"))
}

fn policy(initial_ms: u64, budget: u32) -> Result<SupervisionPolicy, &'static str> {
    Ok(SupervisionPolicy {
        restart_backoff_initial: Duration::from_millis(initial_ms),
        restart_backoff_max: Duration::from_millis(initial_ms),
        restart_backoff_multiplier: NonZeroU32::new(1).ok_or("multiplier")?,
        restart_window: Duration::from_secs(600),
        max_restarts_per_window: NonZeroU32::new(budget).ok_or("budget")?,
        stop_grace: Duration::from_secs(2),
    })
}

fn deployment(name: &str, script: &str) -> Result<WorkerDeployment, Box<dyn std::error::Error>> {
    Ok(WorkerDeployment::new(
        NewWorkerDeployment {
            name: name.to_owned(),
            artifact: WorkerArtifactRef::Builtin {
                verb: vec!["-c".to_owned(), script.to_owned()],
            },
            binary: DeployedBinaryIdentity {
                version: "test".to_owned(),
                commit: "test".to_owned(),
                dirty: "false".to_owned(),
                content_hash: "deploy-time-hash".to_owned(),
            },
            namespaces: BTreeSet::from(["default".to_owned()]),
            task_queue: "shell".to_owned(),
            node: None,
            desired: DesiredState::Running,
        },
        chrono::Utc::now(),
    )?)
}

/// A supervisor over one in-memory store, driving `/bin/sh`. The cluster
/// publisher is a fresh test-local channel; [`supervisor_with_publisher`]
/// exposes it to tests that assert emissions.
async fn supervisor_with(
    records: &[WorkerDeployment],
) -> Result<Arc<WorkerSupervisor>, Box<dyn std::error::Error>> {
    let (supervisor, publisher) = supervisor_with_publisher(records).await?;
    drop(publisher);
    Ok(supervisor)
}

/// [`supervisor_with`], keeping a handle on the cluster channel the supervisor
/// publishes desired-state writes to.
async fn supervisor_with_publisher(
    records: &[WorkerDeployment],
) -> Result<
    (
        Arc<WorkerSupervisor>,
        crate::cluster_publisher::ClusterEventPublisher,
    ),
    Box<dyn std::error::Error>,
> {
    let store: Arc<dyn WorkerDeploymentStore> = Arc::new(InMemoryStore::default());
    for record in records {
        drop(store.put_worker_deployment(record.clone()).await?);
    }
    let publisher = crate::cluster_publisher::ClusterEventPublisher::new(
        std::num::NonZeroUsize::new(64).ok_or("publisher capacity")?,
    );
    Ok((
        Arc::new(WorkerSupervisor::new(store, publisher.clone())),
        publisher,
    ))
}

/// Ask the OS whether a pid is alive.
fn pid_alive(pid: u32) -> bool {
    shell_test(&format!("kill -0 {pid} 2>/dev/null"))
}

/// Ask the OS whether a process GROUP still has members.
fn group_alive(process_group: i32) -> bool {
    shell_test(&format!("kill -0 -{process_group} 2>/dev/null"))
}

fn shell_test(script: &str) -> bool {
    std::process::Command::new("/bin/sh")
        .args(["-c", script])
        .status()
        .is_ok_and(|status| status.success())
}

fn lines(path: &Path) -> usize {
    std::fs::read_to_string(path).map_or(0, |content| content.lines().count())
}

async fn wait_until<F>(mut condition: F, what: &str) -> TestResult
where
    F: FnMut() -> bool,
{
    let deadline = Instant::now() + BUDGET;
    while Instant::now() < deadline {
        if condition() {
            return Ok(());
        }
        tokio::time::sleep(POLL).await;
    }
    Err(format!("timed out after {BUDGET:?} waiting for {what}").into())
}

/// Poll the REPORT until one worker's state satisfies `wanted`.
async fn wait_for_state<F>(
    supervisor: &WorkerSupervisor,
    name: &str,
    wanted: F,
    what: &str,
) -> TestResult
where
    F: Fn(ManagedWorkerState) -> bool,
{
    let deadline = Instant::now() + BUDGET;
    while Instant::now() < deadline {
        if wanted(status_of(supervisor, name).await?.state) {
            return Ok(());
        }
        tokio::time::sleep(POLL).await;
    }
    Err(format!("timed out after {BUDGET:?} waiting for {what}").into())
}

async fn status_of(
    supervisor: &WorkerSupervisor,
    name: &str,
) -> Result<ManagedWorkerStatus, Box<dyn std::error::Error>> {
    supervisor
        .report()
        .await?
        .workers
        .into_iter()
        .find(|worker| worker.name == name)
        .ok_or_else(|| format!("no status for `{name}`").into())
}

/// W-3's headline: kill the process, it comes back.
///
/// The oracle is the FILE, written by the child itself once per start — a
/// counter the supervisor cannot fake. A restart count read off the supervisor
/// would be the supervisor grading its own homework.
#[tokio::test]
async fn a_crashing_worker_is_restarted() -> TestResult {
    let directory = tempfile::tempdir()?;
    let marker = directory.path().join("starts");
    let record = deployment(
        "crasher",
        &format!("echo start >> {}; exit 3", marker.display()),
    )?;
    let supervisor = supervisor_with(std::slice::from_ref(&record)).await?;
    assert!(supervisor.commission(policy(20, 50)?, shell()));

    drop(supervisor.start("crasher").await?);
    wait_until(|| lines(&marker) >= 3, "three independent starts").await?;

    let status = status_of(&supervisor, "crasher").await?;
    assert!(
        status.restarts >= 2,
        "restarts were not counted: {status:?}"
    );
    let exit = status.last_exit.ok_or("an exit must have been recorded")?;
    assert_eq!(exit.code, Some(3), "the real exit code must be reported");
    assert!(!exit.requested, "a crash is not a requested ending");

    assert!(supervisor.shutdown().await.is_empty());
    Ok(())
}

/// A stop is terminal. The distinguishing evidence is the marker file NOT
/// growing after the stop: a supervisor that treated stop as a slow restart
/// would still be reporting `Stopped` the moment it was asked.
#[tokio::test]
async fn a_stop_is_terminal_and_the_worker_is_not_restarted() -> TestResult {
    let directory = tempfile::tempdir()?;
    let marker = directory.path().join("starts");
    let record = deployment(
        "sleeper",
        &format!("echo start >> {}; sleep 300", marker.display()),
    )?;
    let supervisor = supervisor_with(std::slice::from_ref(&record)).await?;
    assert!(supervisor.commission(policy(20, 50)?, shell()));

    drop(supervisor.start("sleeper").await?);
    wait_until(|| lines(&marker) >= 1, "the first start").await?;
    let running = status_of(&supervisor, "sleeper").await?;
    let group = running
        .process_group
        .ok_or("a running worker leads a group")?;

    let stopped = supervisor.stop("sleeper").await?;
    assert_eq!(stopped.state, ManagedWorkerState::Stopped);
    assert_eq!(stopped.desired, DesiredState::Stopped);
    assert!(
        !group_alive(group),
        "the process group survived a stop that reported success"
    );

    // Well past several backoff intervals: a restart would have happened by now.
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert_eq!(
        lines(&marker),
        1,
        "the worker was restarted after a terminal stop"
    );
    assert_eq!(
        status_of(&supervisor, "sleeper").await?.state,
        ManagedWorkerState::Stopped
    );
    Ok(())
}

/// Status truthfulness: the pid the server reports is a pid the OS agrees is
/// alive, and it stops being reported once the process is gone.
#[tokio::test]
async fn reported_status_matches_what_the_operating_system_says() -> TestResult {
    let record = deployment("truthful", "sleep 300")?;
    let supervisor = supervisor_with(std::slice::from_ref(&record)).await?;
    assert!(supervisor.commission(policy(20, 50)?, shell()));
    drop(supervisor.start("truthful").await?);

    wait_for_state(
        &supervisor,
        "truthful",
        |state| state == ManagedWorkerState::Running,
        "the worker to report Running",
    )
    .await?;

    let status = status_of(&supervisor, "truthful").await?;
    let pid = status.pid.ok_or("a running worker has a pid")?;
    assert!(pid_alive(pid), "the reported pid is not a live process");
    assert_eq!(status.desired, DesiredState::Running);
    assert_eq!(status.deployed_binary.content_hash, "deploy-time-hash");
    let spawned = status.spawn_binary.ok_or("a spawn identity is captured")?;
    assert_eq!(spawned.path, "/bin/sh");
    assert_ne!(
        spawned.content_hash, status.deployed_binary.content_hash,
        "the spawn identity must be measured, not copied from the record"
    );

    drop(supervisor.stop("truthful").await?);
    wait_until(|| !pid_alive(pid), "the process to disappear").await?;
    let after = status_of(&supervisor, "truthful").await?;
    assert_eq!(after.pid, None, "a stopped worker must report no pid");
    Ok(())
}

/// A crash loop is bounded and LOUD: the budget stops the restarts and the
/// status says which knob stopped them.
#[tokio::test]
async fn a_crash_loop_spends_its_budget_and_fails_visibly() -> TestResult {
    let directory = tempfile::tempdir()?;
    let marker = directory.path().join("starts");
    let record = deployment(
        "loop",
        &format!("echo start >> {}; exit 1", marker.display()),
    )?;
    let supervisor = supervisor_with(std::slice::from_ref(&record)).await?;
    assert!(supervisor.commission(policy(5, 2)?, shell()));

    drop(supervisor.start("loop").await?);
    wait_for_state(
        &supervisor,
        "loop",
        ManagedWorkerState::is_terminal,
        "the crash loop to be given up on",
    )
    .await?;

    let status = status_of(&supervisor, "loop").await?;
    assert_eq!(
        status.state,
        ManagedWorkerState::Failed,
        "supervision gave up for the wrong reason: {status:?}"
    );
    let detail = status.last_error.ok_or("a failure must be explained")?;
    assert!(
        detail.contains("max_restarts_per_window"),
        "the escalation must name the knob that bounded it: {detail}"
    );
    // One initial start plus exactly the budgeted restarts, and no more.
    let observed = lines(&marker);
    assert_eq!(observed, 3, "expected 1 start + 2 budgeted restarts");
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(
        lines(&marker),
        observed,
        "a failed instance restarted again"
    );
    Ok(())
}

/// R8's orphan scan: the whole tree dies, not just the process the supervisor
/// happens to hold a handle to.
#[tokio::test]
async fn a_grandchild_dies_with_the_worker() -> TestResult {
    let directory = tempfile::tempdir()?;
    let grandchild_pid = directory.path().join("grandchild.pid");
    let record = deployment(
        "tree",
        &format!(
            "sleep 300 & echo $! > {}; sleep 300",
            grandchild_pid.display()
        ),
    )?;
    let supervisor = supervisor_with(std::slice::from_ref(&record)).await?;
    assert!(supervisor.commission(policy(20, 50)?, shell()));
    drop(supervisor.start("tree").await?);

    wait_until(
        || lines(&grandchild_pid) >= 1,
        "the grandchild to record itself",
    )
    .await?;
    let recorded = std::fs::read_to_string(&grandchild_pid)?;
    let pid: u32 = recorded.trim().parse()?;
    assert!(pid_alive(pid), "the grandchild should be running");

    drop(supervisor.stop("tree").await?);
    wait_until(
        || !pid_alive(pid),
        "the grandchild to die with the worker it was spawned from",
    )
    .await?;
    Ok(())
}

/// Server shutdown drains the fleet: every instance stopped, every group empty,
/// and no failure reported. An empty failure list IS the no-orphan claim.
#[tokio::test]
async fn shutdown_drains_every_supervised_worker() -> TestResult {
    let first = deployment("one", "sleep 300")?;
    let second = deployment("two", "sleep 300")?;
    let supervisor = supervisor_with(&[first, second]).await?;
    assert!(supervisor.commission(policy(20, 50)?, shell()));

    let started = supervisor.reconcile().await?;
    assert_eq!(
        started, 2,
        "both desired-Running deployments must supervise"
    );

    let mut groups = Vec::new();
    for name in ["one", "two"] {
        wait_for_state(
            &supervisor,
            name,
            |state| state == ManagedWorkerState::Running,
            "both workers to be running",
        )
        .await?;
        groups.push(
            status_of(&supervisor, name)
                .await?
                .process_group
                .ok_or("a running worker leads a group")?,
        );
    }

    let failures = supervisor.shutdown().await;
    assert!(
        failures.is_empty(),
        "shutdown could not prove the fleet stopped: {failures:?}"
    );
    for group in groups {
        assert!(!group_alive(group), "group {group} survived shutdown");
    }
    // Shutdown is not an operator stop: durable intent is untouched, so the
    // next boot brings the fleet back.
    for name in ["one", "two"] {
        assert_eq!(
            status_of(&supervisor, name).await?.desired,
            DesiredState::Running
        );
    }
    Ok(())
}

/// An uncommissioned server refuses, names the remedy, and reports the fleet as
/// unsupervised rather than as stopped — the two must not read alike.
#[tokio::test]
async fn an_uncommissioned_server_refuses_and_names_the_remedy() -> TestResult {
    let record = deployment("unsupervised", "sleep 300")?;
    let supervisor = supervisor_with(std::slice::from_ref(&record)).await?;

    let error = supervisor
        .start("unsupervised")
        .await
        .err()
        .ok_or("an uncommissioned server must refuse to start a worker")?;
    assert!(matches!(error, SupervisionError::NotCommissioned));
    assert!(error.to_string().contains("[worker_supervision]"));

    let report = supervisor.report().await?;
    assert!(!report.commissioned);
    assert!(report.remedy.is_some());
    assert_eq!(
        report
            .workers
            .first()
            .ok_or("the deployment must still be visible")?
            .state,
        ManagedWorkerState::Uncommissioned
    );
    Ok(())
}

/// Starting twice starts one process. The oracle is again the marker file.
#[tokio::test]
async fn starting_an_already_supervised_worker_does_not_start_a_second_one() -> TestResult {
    let directory = tempfile::tempdir()?;
    let marker = directory.path().join("starts");
    let record = deployment(
        "single",
        &format!("echo start >> {}; sleep 300", marker.display()),
    )?;
    let supervisor = supervisor_with(std::slice::from_ref(&record)).await?;
    assert!(supervisor.commission(policy(20, 50)?, shell()));

    drop(supervisor.start("single").await?);
    wait_until(|| lines(&marker) >= 1, "the first start").await?;
    drop(supervisor.start("single").await?);
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(lines(&marker), 1, "a second process was started");

    assert!(supervisor.shutdown().await.is_empty());
    Ok(())
}

/// Stopping a worker that already gave up must SUCCEED. The acceptance gate is
/// "no process group is unaccounted for", and a crash-looped instance has no
/// process — gating on the state token instead would refuse this stop and tell
/// an operator their already-dead worker could not be stopped.
#[tokio::test]
async fn stopping_a_crash_failed_worker_succeeds_and_records_the_intent() -> TestResult {
    let record = deployment("gaveup", "exit 1")?;
    let supervisor = supervisor_with(std::slice::from_ref(&record)).await?;
    assert!(supervisor.commission(policy(5, 1)?, shell()));

    drop(supervisor.start("gaveup").await?);
    wait_for_state(
        &supervisor,
        "gaveup",
        |state| state == ManagedWorkerState::Failed,
        "the worker to give up",
    )
    .await?;

    let stopped = supervisor.stop("gaveup").await?;
    assert_eq!(stopped.desired, DesiredState::Stopped);
    assert_eq!(stopped.state, ManagedWorkerState::Stopped);
    Ok(())
}

#[tokio::test]
async fn an_unknown_deployment_is_a_typed_refusal_on_every_verb() -> TestResult {
    let supervisor = supervisor_with(&[]).await?;
    assert!(supervisor.commission(policy(20, 50)?, shell()));
    for error in [
        supervisor.start("absent").await.err(),
        supervisor.stop("absent").await.err(),
        supervisor.restart("absent").await.err(),
    ] {
        let error = error.ok_or("an absent deployment must be refused")?;
        assert!(
            matches!(error, SupervisionError::UnknownDeployment { .. }),
            "unexpected refusal: {error}"
        );
    }
    Ok(())
}

/// 🔴 Desired-state writes are published from the SUPERVISOR, so every
/// transport that drives it (HTTP and gRPC alike) inherits the same live-feed
/// event the worker-deployment desired-state endpoint emits — and a lifecycle
/// call that writes nothing durable emits nothing. The ordering proves the
/// negative: the restart between start and stop flips no desired state, so
/// the event after the start's must be the stop's.
#[tokio::test]
async fn desired_state_writes_publish_cluster_events_and_non_writes_emit_none() -> TestResult {
    use futures::StreamExt as _;

    let mut record = deployment("published", "sleep 300")?;
    record.desired = DesiredState::Stopped;
    let (supervisor, publisher) = supervisor_with_publisher(&[record]).await?;
    assert!(supervisor.commission(policy(20, 5)?, shell()));
    let mut events = publisher.subscribe(0);

    drop(supervisor.start("published").await?);
    drop(supervisor.restart("published").await?);
    drop(supervisor.stop("published").await?);

    let started = events
        .next()
        .await
        .ok_or("the start flip must publish an event")?
        .map_err(|lagged| format!("cluster stream lagged: {lagged:?}"))?;
    assert!(
        matches!(
            started,
            aion_core::ClusterEvent::WorkerDeploymentDesiredStateChanged {
                ref name,
                desired_state,
                ..
            } if name == "published" && desired_state == DesiredState::Running
        ),
        "unexpected first event: {started:?}"
    );
    let stopped = events
        .next()
        .await
        .ok_or("the stop must publish an event")?
        .map_err(|lagged| format!("cluster stream lagged: {lagged:?}"))?;
    assert!(
        matches!(
            stopped,
            aion_core::ClusterEvent::WorkerDeploymentDesiredStateChanged {
                ref name,
                desired_state,
                ..
            } if name == "published" && desired_state == DesiredState::Stopped
        ),
        "the restart of a running deployment must not have emitted: {stopped:?}"
    );
    Ok(())
}
