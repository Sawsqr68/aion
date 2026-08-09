//! Cancellable process-group containment for worker-owned commands.
//!
//! A command enters a fresh process group before user code runs
//! ([`ContainedChild`], the one containment core). Cancellation sends
//! `SIGTERM` to the whole group, waits a bounded grace period, escalates to
//! `SIGKILL`, reaps the direct child, and returns `Cancelled` only after the
//! group is confirmed gone.
//!
//! Every reap here goes through [`ContainedChild::wait`] rather than the raw
//! child, because the core tracks reaping to decide whether signalling the
//! group is still legitimate. Reaping behind its back is precisely how a
//! signal comes to be aimed at a recycled process-group id.

use std::future::Future;
use std::process::{Output, Stdio};

use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::{ChildStderr, ChildStdout, Command};

use super::contained::{ContainedChild, PROCESS_GROUP_TERMINATION_GRACE, ProcessGroupError};

/// The observable outcome of a contained command.
#[derive(Debug)]
pub enum CancellableCommandOutput {
    /// The command exited and its output pipes reached EOF.
    Completed(Output),
    /// Cancellation killed, reaped, and verified the entire process group.
    Cancelled,
}

/// Run a command in a fresh process group and make cancellation tree-wide.
///
/// The command's stdout and stderr are captured. On cancellation, this function
/// does not return [`CancellableCommandOutput::Cancelled`] until the direct
/// child is reaped and the process group is confirmed gone.
///
/// # Errors
///
/// Returns [`ProcessGroupError`] when the command cannot be spawned or observed,
/// when signalling fails, or when the group cannot be confirmed empty after
/// bounded `SIGTERM`/`SIGKILL` handling.
pub async fn run_cancellable_command<C>(
    mut command: Command,
    cancellation: C,
) -> Result<CancellableCommandOutput, ProcessGroupError>
where
    C: Future<Output = ()>,
{
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut contained = ContainedChild::spawn(command)?;
    let stdout = contained.take_stdout();
    let stderr = contained.take_stderr();
    tokio::pin!(cancellation);

    let state = {
        let completion = collect_output(&mut contained, stdout, stderr);
        tokio::pin!(completion);
        tokio::select! {
            biased;
            () = &mut cancellation => RunState::Cancelled,
            result = &mut completion => RunState::Completed(result),
        }
    };

    match state {
        RunState::Completed(Ok(output)) => {
            // Both pipes reached EOF, so no descendant still holds a write end:
            // the group is established empty without a probe.
            contained.disarm();
            Ok(CancellableCommandOutput::Completed(output))
        }
        RunState::Completed(Err(original)) => match stop_and_confirm(&mut contained).await {
            Ok(()) => Err(original),
            Err(cleanup) => Err(ProcessGroupError::CleanupAfterFailure {
                original: Box::new(original),
                cleanup: Box::new(cleanup),
            }),
        },
        RunState::Cancelled => {
            stop_and_confirm(&mut contained).await?;
            Ok(CancellableCommandOutput::Cancelled)
        }
    }
}

/// Kill the group, then PROVE it gone before anyone is told it is.
///
/// The two halves are separate operations in the core precisely so that this
/// composition is explicit: `Cancelled` is a claim about the whole tree, and it
/// is not made until the probe supports it.
async fn stop_and_confirm(contained: &mut ContainedChild) -> Result<(), ProcessGroupError> {
    contained.terminate(PROCESS_GROUP_TERMINATION_GRACE).await?;
    contained
        .confirm_group_gone(PROCESS_GROUP_TERMINATION_GRACE)
        .await
}

enum RunState {
    Completed(Result<Output, ProcessGroupError>),
    Cancelled,
}

async fn collect_output(
    contained: &mut ContainedChild,
    stdout: Option<ChildStdout>,
    stderr: Option<ChildStderr>,
) -> Result<Output, ProcessGroupError> {
    let capture = async {
        let stdout = stdout.ok_or(ProcessGroupError::MissingPipe { stream: "stdout" })?;
        let stderr = stderr.ok_or(ProcessGroupError::MissingPipe { stream: "stderr" })?;
        tokio::try_join!(read_stream(stdout, "stdout"), read_stream(stderr, "stderr"))
    };
    let (status, (stdout, stderr)) = tokio::try_join!(contained.wait(), capture)?;
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
