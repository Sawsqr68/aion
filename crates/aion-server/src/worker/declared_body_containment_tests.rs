//! Containment of a declared body the SERVER is executing.
//!
//! The worker path already contains its commands completely: a fresh process
//! group per command, `SIGTERM` → grace → `SIGKILL` on cancellation, and a
//! `Cancelled` verdict that is not returned until the group has been PROVEN
//! gone ([`aion_worker::process`], `cancellable.rs` / `contained.rs`). The
//! server path runs its commands through that same executor, so the mechanism
//! is present — what these tests hold is that the server actually REACHES it.
//!
//! Every subject here blocks until it is signalled. A command that could finish
//! on its own would let a green appear for the wrong reason on a slow machine;
//! `sleep 600` cannot, so a dispatch that returns at all is one the server
//! stopped.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use aion_package::ActionBodyContract;

use aion::ActivityDispatcher as _;

use super::tests::{TestResult, dispatcher_with_attempts, reached_names, request};
use super::{DeclaredBodyLookup, DeclaredCommandDispatcher};
use crate::worker::declared_body_cancel::DeclaredCommandAttempts;
use crate::worker::heartbeat::HeartbeatTracker;
use crate::worker::registry::ConnectedWorkerRegistry;
use crate::worker::workspace_root::WorkspaceRoot;
use crate::worker::{InFlightCancellation, cancel_in_flight_activities};

/// How long a test waits for a stopped attempt to come back.
///
/// Not a bound on anything the server does — the server's own bound is the
/// authored one, and its ladder is the worker path's
/// `PROCESS_GROUP_TERMINATION_GRACE` (2s to escalate, 2s more to confirm). This
/// is only the test's patience, sized an order of magnitude above the longest
/// honest stop so a green never depends on the machine being idle. The subject
/// sleeps for ten minutes, so a failure here is a genuine "it never stopped",
/// never a slow one.
const STOP_DEADLINE: Duration = Duration::from_secs(60);

/// Cadence for the tests' two polling barriers.
const POLL: Duration = Duration::from_millis(20);

/// The per-attempt bound these tests AUTHOR onto their dispatch.
///
/// A fixture, not a default: the server invents no bound of its own, and a
/// dispatch that authors none is unbounded exactly as it always was. This is
/// the number a workflow document would have written.
const AUTHORED_BOUND: Duration = Duration::from_secs(1);

/// A declared body that cannot end by itself, whose process group has more in
/// it than the direct child.
///
/// `marker` is touched first so a test can wait for the command to be genuinely
/// running before acting on it, and its path is spliced into the command line
/// so [`tree_alive`] can find the process by a token unique to this run.
fn blocking_command(marker: &std::path::Path) -> String {
    format!(
        "sh -c 'touch {}; sleep 600 & sleep 600'",
        marker.to_string_lossy()
    )
}

/// Whether any process on this machine still carries `token` in its command
/// line.
///
/// The oracle for "the command is gone" is the operating system's own process
/// table, not the dispatcher's word for it. `ps` is asked for every process's
/// arguments and the token is matched here, so the query itself never carries
/// the token and can never match itself.
fn tree_alive(token: &str) -> Result<bool, Box<dyn std::error::Error>> {
    let listing = std::process::Command::new("/bin/ps")
        .args(["-Ao", "args="])
        .output()?;
    if !listing.status.success() {
        return Err(format!(
            "could not read the process table: ps exited {:?}",
            listing.status.code()
        )
        .into());
    }
    Ok(String::from_utf8_lossy(&listing.stdout).contains(token))
}

/// Kill anything still carrying `token`, so a FAILING test leaves no ten-minute
/// sleep behind on the machine.
///
/// Only ever reached on the failure path: a passing test has already proven the
/// tree gone.
fn reap_leftovers(token: &str) {
    match std::process::Command::new("/usr/bin/pkill")
        .args(["-f", token])
        .status()
    {
        // `pkill` exits 1 when it matched nothing, which is the good case.
        Ok(_) => {}
        Err(error) => tracing::warn!(
            %error,
            token,
            "could not reap a failing containment test's leftover processes"
        ),
    }
}

/// Wait until the command has genuinely started, or fail by name.
///
/// The marker is touched by the command's own first word, so its appearance is
/// the command speaking rather than the test guessing.
async fn await_started(marker: &std::path::Path) -> TestResult {
    let deadline = tokio::time::Instant::now() + STOP_DEADLINE;
    while !marker.exists() {
        if tokio::time::Instant::now() >= deadline {
            return Err(format!(
                "the declared command never started: {} was never created",
                marker.display()
            )
            .into());
        }
        tokio::time::sleep(POLL).await;
    }
    Ok(())
}

/// The heartbeat window the tests' tracker is built with.
///
/// Inert here: no worker is registered and no activity is tracked, because a
/// declared body has neither. The tracker exists only so these tests drive the
/// REAL cancel entry point rather than a convenient half of it.
const HEARTBEAT_WINDOW: Duration = Duration::from_secs(5);

/// The dispatcher under test, the record of whether the worker path was reached,
/// and the registry the run's cancel signals through.
///
/// The registry is built HERE, beside the dispatcher, because the production
/// boot builds exactly one for the two of them: `ServerState` hands the same
/// instance to the declared-body dispatcher and to
/// `cancel_in_flight_activities`. A test that gave the cancel path a registry
/// the executor never joined would be testing an empty room.
fn containment_dispatcher(
    marker: &std::path::Path,
) -> (
    DeclaredCommandDispatcher,
    Arc<Mutex<Vec<String>>>,
    DeclaredCommandAttempts,
) {
    let attempts = DeclaredCommandAttempts::new(crate::shutdown::DrainState::default());
    let (decorated, reached, _transcript) = dispatcher_with_attempts(
        DeclaredBodyLookup::Declared(ActionBodyContract::Run {
            command: blocking_command(marker),
        }),
        Err("terminal:the worker path must never be reached".to_owned()),
        WorkspaceRoot::from_resolution(Ok(std::env::temp_dir())),
        attempts.clone(),
    );
    (decorated, reached, attempts)
}

/// Stop everything this node is executing for `workflow_id`, through the one
/// entry point the cancel handler uses.
///
/// The tracker and the worker registry are empty and stay empty: what is under
/// test is the OTHER execution path, the one with no worker to ask.
fn stop_in_flight_work(
    attempts: &DeclaredCommandAttempts,
    workflow_id: &aion_core::WorkflowId,
) -> Result<InFlightCancellation, crate::error::ServerError> {
    let tracker = HeartbeatTracker::new(HEARTBEAT_WINDOW);
    let registry = ConnectedWorkerRegistry::default();
    cancel_in_flight_activities(&tracker, &registry, attempts, workflow_id)
}

/// The dispatch config a workflow document authoring `timeout <bound>` produces
/// (#223): the SDK's `activity_dispatch.config`, whose `timeout_ms` key is the
/// one ruled per-attempt bound in this workspace.
fn bounded_config(bound: Duration) -> String {
    format!(
        r#"{{"retry":null,"timeout_ms":{},"heartbeat_ms":null,"labels":{{}}}}"#,
        bound.as_millis()
    )
}

/// THE BOUND MUST REACH THE PROCESS. A dispatch that authored a per-attempt
/// timeout must end the server-executed command at that bound, and the command's
/// process must be gone when the attempt reports.
///
/// The engine already applies the authored bound to the dispatch FUTURE
/// (`nif_activity_retry_dispatch::deliver_one_attempt`), and says in its own
/// words what that does and does not achieve: "the dispatch future is DROPPED,
/// which stops this run waiting and nothing more... the worker-side call runs on
/// to its own end and its result is discarded." For a declared body that call is
/// a process on THIS machine, so an unwired bound leaves a command running with
/// nothing left that can stop it — the run has already been told it timed out.
#[tokio::test(flavor = "multi_thread")]
async fn an_authored_bound_ends_the_server_executed_command() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let marker = scratch.path().join("started");
    let token = marker.to_string_lossy().into_owned();
    let (decorated, reached, _attempts) = containment_dispatcher(&marker);
    let mut dispatch = request("blocker", "{}");
    dispatch.config = bounded_config(AUTHORED_BOUND);

    let handle = tokio::task::spawn_blocking(move || decorated.dispatch(dispatch));
    await_started(&marker).await?;

    let Ok(joined) = tokio::time::timeout(STOP_DEADLINE, handle).await else {
        reap_leftovers(&token);
        return Err(
            "the attempt outlived its authored per-attempt bound: the bound reaches the \
             dispatch future but not the process the server started"
                .into(),
        );
    };
    let Err(error) = joined? else {
        reap_leftovers(&token);
        return Err("a command stopped on its bound must fail the dispatch".into());
    };
    assert!(
        error.starts_with("timeout:"),
        "an attempt ended on its authored bound must report the engine's own timeout \
         vocabulary, not an anonymous failure: {error}"
    );
    assert!(
        !tree_alive(&token)?,
        "the bound ended the wait but left the command running"
    );
    assert!(
        reached_names(&reached).is_empty(),
        "a bodied action must not fall through to the worker path"
    );
    Ok(())
}

/// A CANCELLED RUN MUST STOP THE BODY THIS SERVER IS RUNNING.
///
/// Cancelling a workflow records the fact, kills the workflow's VM process, and
/// settles its outbox — and then asks whoever is executing the run's activities
/// to stop (#233). "Whoever" was only ever the connected workers: a declared
/// body is executed at the dispatch seam by the server itself, is held by no
/// worker, and is tracked by no heartbeat, so the cancel path could not see it.
/// The console reported `Cancelled` truthfully while the command kept the
/// machine.
///
/// The subject blocks until it is signalled, so a dispatch that returns at all
/// is one the cancel stopped, and the failure it returns is
/// `run_cancellable_command`'s own — which is not produced until the process
/// group has been PROVEN gone.
#[tokio::test(flavor = "multi_thread")]
async fn cancelling_the_run_stops_the_server_executed_command() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let marker = scratch.path().join("started");
    let token = marker.to_string_lossy().into_owned();
    let (decorated, reached, attempts) = containment_dispatcher(&marker);
    let dispatch = request("blocker", "{}");
    let workflow_id = dispatch.workflow_id.clone();

    let handle = tokio::task::spawn_blocking(move || decorated.dispatch(dispatch));
    await_started(&marker).await?;

    let stopped = stop_in_flight_work(&attempts, &workflow_id)?;

    let Ok(joined) = tokio::time::timeout(STOP_DEADLINE, handle).await else {
        reap_leftovers(&token);
        return Err(
            "cancelling the run left its server-executed command running: the executing \
             attempt is invisible to the cancel path"
                .into(),
        );
    };
    let Err(error) = joined? else {
        reap_leftovers(&token);
        return Err("a cancelled command must fail the dispatch".into());
    };
    assert!(
        error.starts_with("terminal:"),
        "a cancelled attempt must not be retried: {error}"
    );
    assert!(
        error.contains("cancelled") && error.contains("process group"),
        "the failure must be the containment core's own, so it cannot be produced \
         without the group having been proven gone: {error}"
    );
    assert!(
        !tree_alive(&token)?,
        "the dispatch reported a stop the process table does not agree with"
    );
    assert_eq!(
        stopped.declared_attempts.len(),
        1,
        "the cancel must NAME the server-executed attempt it stopped, not stop it \
         silently: {stopped:?}"
    );
    assert_eq!(
        stopped.declared_attempts[0].workflow_id, workflow_id,
        "the named attempt must be the run's own"
    );
    assert!(
        stopped.worker_requests.is_empty(),
        "no worker held this activity, so no worker may be asked about it"
    );
    assert!(
        reached_names(&reached).is_empty(),
        "a bodied action must not fall through to the worker path"
    );
    Ok(())
}

/// A cancel must not reach a bystander. Another run's server-executed body is
/// neither named nor signalled, and goes on to end on its OWN terms.
///
/// "Its own terms" is its authored bound, which is what lets this test have a
/// blocking subject and still terminate: the bystander ends with the timeout
/// vocabulary, never the cancellation one, and the difference between those two
/// strings is the whole assertion.
#[tokio::test(flavor = "multi_thread")]
async fn cancelling_one_run_leaves_another_runs_body_alone() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let marker = scratch.path().join("started");
    let token = marker.to_string_lossy().into_owned();
    let (decorated, _reached, attempts) = containment_dispatcher(&marker);
    let mut bystander = request("blocker", "{}");
    bystander.config = bounded_config(AUTHORED_BOUND);
    let cancelled_run = aion_core::WorkflowId::new_v4();

    let handle = tokio::task::spawn_blocking(move || decorated.dispatch(bystander));
    await_started(&marker).await?;

    let stopped = stop_in_flight_work(&attempts, &cancelled_run)?;
    assert!(
        stopped.declared_attempts.is_empty(),
        "another run's cancel must signal nothing here: {stopped:?}"
    );

    let Ok(joined) = tokio::time::timeout(STOP_DEADLINE, handle).await else {
        reap_leftovers(&token);
        return Err("the bystander's command never ended on its own bound".into());
    };
    let Err(error) = joined? else {
        reap_leftovers(&token);
        return Err("the blocking subject cannot complete on its own".into());
    };
    assert!(
        error.starts_with("timeout:"),
        "the bystander must end on its own bound, not on another run's cancel: {error}"
    );
    assert!(
        !tree_alive(&token)?,
        "the bystander's own bound must still have stopped its process"
    );
    Ok(())
}
