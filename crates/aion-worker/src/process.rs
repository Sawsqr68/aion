//! Cancellable process-group containment for worker-owned commands.
//!
//! A command enters a fresh process group before user code runs. Cancellation
//! sends `SIGTERM` to the whole group, waits a bounded grace period, escalates
//! to `SIGKILL`, reaps the direct child, and returns `Cancelled` only after an
//! `ESRCH` probe confirms that the group is empty.

use std::future::Future;
use std::io;
use std::process::{Output, Stdio};
use std::time::Duration;

use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::{Child, ChildStderr, ChildStdout, Command};

/// Grace allowed after each group signal before cancellation fails loudly.
///
/// `SIGTERM` receives this interval before escalation. `SIGKILL` receives the
/// same interval for OS-level disappearance and direct-child reaping.
pub const PROCESS_GROUP_TERMINATION_GRACE: Duration = Duration::from_secs(2);

const GROUP_PROBE_INTERVAL: Duration = Duration::from_millis(10);

/// The observable outcome of a contained command.
#[derive(Debug)]
pub enum CancellableCommandOutput {
    /// The command exited and its output pipes reached EOF.
    Completed(Output),
    /// Cancellation killed, reaped, and verified the entire process group.
    Cancelled,
}

/// A failure to start, observe, or completely stop a contained command.
#[derive(Debug, Error)]
pub enum ProcessGroupError {
    /// Process-group containment is unavailable on this target.
    #[error("process-group containment is unsupported on this operating system")]
    Unsupported,
    /// The command could not be spawned.
    #[error("contained command could not be spawned: {source}")]
    Spawn {
        /// Underlying spawn failure.
        #[source]
        source: io::Error,
    },
    /// The spawned child did not expose a process id.
    #[error("contained command spawned without a process id")]
    MissingProcessId,
    /// The platform process id did not fit the Unix `pid_t` representation.
    #[error("contained command process id {pid} is outside the supported range")]
    ProcessIdOutOfRange {
        /// Process id returned by Tokio.
        pid: u32,
    },
    /// A configured output pipe was unexpectedly unavailable.
    #[error("contained command did not expose its piped {stream}")]
    MissingPipe {
        /// Name of the missing stream.
        stream: &'static str,
    },
    /// Reading a captured output stream failed.
    #[error("failed to read contained command {stream}: {source}")]
    Read {
        /// Name of the failed stream.
        stream: &'static str,
        /// Underlying read failure.
        #[source]
        source: io::Error,
    },
    /// Waiting for or reaping the direct child failed.
    #[error("failed to reap contained command: {source}")]
    Reap {
        /// Underlying wait failure.
        #[source]
        source: io::Error,
    },
    /// Sending a signal to the process group failed.
    #[cfg(unix)]
    #[error("failed to send {signal} to process group {process_group}: {source}")]
    Signal {
        /// Process group that should have received the signal.
        process_group: i32,
        /// Signal being sent.
        signal: &'static str,
        /// Underlying Unix error.
        #[source]
        source: nix::errno::Errno,
    },
    /// Probing process-group existence failed.
    #[cfg(unix)]
    #[error("failed to probe process group {process_group}: {source}")]
    Probe {
        /// Process group being probed.
        process_group: i32,
        /// Underlying Unix error.
        #[source]
        source: nix::errno::Errno,
    },
    /// The group remained observable after `SIGKILL` and the verification bound.
    #[error(
        "process group {process_group} remained alive {grace:?} after SIGKILL; cancellation is not complete"
    )]
    GroupStillAlive {
        /// Process group that failed to disappear.
        process_group: i32,
        /// Verification interval after `SIGKILL`.
        grace: Duration,
    },
    /// Command observation failed and the mandatory cleanup also failed.
    #[error("{original}; mandatory process-group cleanup also failed: {cleanup}")]
    CleanupAfterFailure {
        /// Original command observation failure.
        original: Box<ProcessGroupError>,
        /// Cleanup failure proving cancellation could not be confirmed.
        cleanup: Box<ProcessGroupError>,
    },
}

/// Run a command in a fresh process group and make cancellation tree-wide.
///
/// The command's stdout and stderr are captured. On cancellation, this function
/// does not return [`CancellableCommandOutput::Cancelled`] until the direct
/// child is reaped and a signal-zero group probe reports `ESRCH`.
///
/// # Errors
///
/// Returns [`ProcessGroupError`] when the command cannot be spawned or observed,
/// when signalling fails, or when the group cannot be confirmed empty after
/// bounded `SIGTERM`/`SIGKILL` handling.
pub async fn run_cancellable_command<C>(
    command: Command,
    cancellation: C,
) -> Result<CancellableCommandOutput, ProcessGroupError>
where
    C: Future<Output = ()>,
{
    #[cfg(unix)]
    {
        run_unix(command, cancellation).await
    }
    #[cfg(not(unix))]
    {
        let _ = (command, cancellation);
        Err(ProcessGroupError::Unsupported)
    }
}

#[cfg(unix)]
async fn run_unix<C>(
    mut command: Command,
    cancellation: C,
) -> Result<CancellableCommandOutput, ProcessGroupError>
where
    C: Future<Output = ()>,
{
    use std::os::unix::process::CommandExt;

    command.as_std_mut().process_group(0);
    command
        .kill_on_drop(true)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|source| ProcessGroupError::Spawn { source })?;
    let raw_pid = child.id().ok_or(ProcessGroupError::MissingProcessId)?;
    let process_group = i32::try_from(raw_pid)
        .map_err(|_| ProcessGroupError::ProcessIdOutOfRange { pid: raw_pid })?;
    let process_group = nix::unistd::Pid::from_raw(process_group);
    let mut guard = ProcessGroupGuard::new(process_group);
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    tokio::pin!(cancellation);

    let state = {
        let completion = collect_output(&mut child, stdout, stderr);
        tokio::pin!(completion);
        tokio::select! {
            biased;
            () = &mut cancellation => RunState::Cancelled,
            result = &mut completion => RunState::Completed(result),
        }
    };

    match state {
        RunState::Completed(Ok(output)) => {
            guard.disarm();
            Ok(CancellableCommandOutput::Completed(output))
        }
        RunState::Completed(Err(original)) => {
            match terminate_process_group(&mut child, process_group).await {
                Ok(()) => {
                    guard.disarm();
                    Err(original)
                }
                Err(cleanup) => Err(ProcessGroupError::CleanupAfterFailure {
                    original: Box::new(original),
                    cleanup: Box::new(cleanup),
                }),
            }
        }
        RunState::Cancelled => {
            terminate_process_group(&mut child, process_group).await?;
            guard.disarm();
            Ok(CancellableCommandOutput::Cancelled)
        }
    }
}

#[cfg(unix)]
enum RunState {
    Completed(Result<Output, ProcessGroupError>),
    Cancelled,
}

async fn collect_output(
    child: &mut Child,
    stdout: Option<ChildStdout>,
    stderr: Option<ChildStderr>,
) -> Result<Output, ProcessGroupError> {
    let wait = async {
        child
            .wait()
            .await
            .map_err(|source| ProcessGroupError::Reap { source })
    };
    let capture = async {
        let stdout = stdout.ok_or(ProcessGroupError::MissingPipe { stream: "stdout" })?;
        let stderr = stderr.ok_or(ProcessGroupError::MissingPipe { stream: "stderr" })?;
        tokio::try_join!(read_stream(stdout, "stdout"), read_stream(stderr, "stderr"))
    };
    let (status, (stdout, stderr)) = tokio::try_join!(wait, capture)?;
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

async fn read_stream<R>(mut stream: R, name: &'static str) -> Result<Vec<u8>, ProcessGroupError>
where
    R: AsyncRead + Unpin,
{
    let mut bytes = Vec::new();
    stream
        .read_to_end(&mut bytes)
        .await
        .map_err(|source| ProcessGroupError::Read {
            stream: name,
            source,
        })?;
    Ok(bytes)
}

#[cfg(unix)]
async fn terminate_process_group(
    child: &mut Child,
    process_group: nix::unistd::Pid,
) -> Result<(), ProcessGroupError> {
    use nix::sys::signal::Signal;

    signal_group(process_group, Signal::SIGTERM, "SIGTERM")?;
    let deadline = tokio::time::Instant::now() + PROCESS_GROUP_TERMINATION_GRACE;
    if wait_for_group_to_disappear(child, process_group, deadline).await? {
        return Ok(());
    }

    signal_group(process_group, Signal::SIGKILL, "SIGKILL")?;
    let deadline = tokio::time::Instant::now() + PROCESS_GROUP_TERMINATION_GRACE;
    if wait_for_group_to_disappear(child, process_group, deadline).await? {
        return Ok(());
    }
    Err(ProcessGroupError::GroupStillAlive {
        process_group: process_group.as_raw(),
        grace: PROCESS_GROUP_TERMINATION_GRACE,
    })
}

#[cfg(unix)]
async fn wait_for_group_to_disappear(
    child: &mut Child,
    process_group: nix::unistd::Pid,
    deadline: tokio::time::Instant,
) -> Result<bool, ProcessGroupError> {
    loop {
        child
            .try_wait()
            .map_err(|source| ProcessGroupError::Reap { source })?;
        if group_is_gone(process_group)? {
            return Ok(true);
        }
        let now = tokio::time::Instant::now();
        if now >= deadline {
            return Ok(false);
        }
        tokio::time::sleep(GROUP_PROBE_INTERVAL.min(deadline - now)).await;
    }
}

#[cfg(unix)]
fn signal_group(
    process_group: nix::unistd::Pid,
    signal: nix::sys::signal::Signal,
    signal_name: &'static str,
) -> Result<(), ProcessGroupError> {
    match nix::sys::signal::killpg(process_group, signal) {
        Ok(()) | Err(nix::errno::Errno::ESRCH) => Ok(()),
        Err(source) => Err(ProcessGroupError::Signal {
            process_group: process_group.as_raw(),
            signal: signal_name,
            source,
        }),
    }
}

#[cfg(unix)]
fn group_is_gone(process_group: nix::unistd::Pid) -> Result<bool, ProcessGroupError> {
    match nix::sys::signal::killpg(process_group, None::<nix::sys::signal::Signal>) {
        Ok(()) => Ok(false),
        Err(nix::errno::Errno::ESRCH) => Ok(true),
        Err(source) => Err(ProcessGroupError::Probe {
            process_group: process_group.as_raw(),
            source,
        }),
    }
}

#[cfg(unix)]
struct ProcessGroupGuard {
    process_group: nix::unistd::Pid,
    armed: bool,
}

#[cfg(unix)]
impl ProcessGroupGuard {
    const fn new(process_group: nix::unistd::Pid) -> Self {
        Self {
            process_group,
            armed: true,
        }
    }

    const fn disarm(&mut self) {
        self.armed = false;
    }
}

#[cfg(unix)]
impl Drop for ProcessGroupGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        match nix::sys::signal::killpg(self.process_group, nix::sys::signal::Signal::SIGKILL) {
            Ok(()) | Err(nix::errno::Errno::ESRCH) => {}
            Err(source) => tracing::error!(
                process_group = self.process_group.as_raw(),
                %source,
                "failed to kill contained process group while dropping its owner"
            ),
        }
    }
}
