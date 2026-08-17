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

use std::time::Duration;

use aion_package::ActionBodyContract;

use aion::ActivityDispatcher as _;

use super::DeclaredBodyLookup;
use super::tests::{TestResult, dispatcher_with_root, reached_names, request};
use crate::worker::workspace_root::WorkspaceRoot;

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
    let (decorated, reached, _transcript) = dispatcher_with_root(
        DeclaredBodyLookup::Declared(ActionBodyContract::Run {
            command: blocking_command(&marker),
        }),
        Err("terminal:the worker path must never be reached".to_owned()),
        WorkspaceRoot::from_resolution(Ok(std::env::temp_dir())),
    );
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
