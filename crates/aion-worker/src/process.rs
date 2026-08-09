//! Cancellable process-group containment for worker-owned commands.
//!
//! A command enters a fresh process group before user code runs. Cancellation
//! sends `SIGTERM` to the whole group, waits a bounded grace period, escalates
//! to `SIGKILL`, reaps the direct child, and returns `Cancelled` only after an
//! `ESRCH` probe confirms that the group is empty.
//!
//! # Output arrives line by line, not at exit
//!
//! Both output streams are read as the command produces them: every complete
//! line is handed to a [`CommandOutputObserver`] the moment its newline is
//! read, while the same bytes accumulate verbatim into the [`Output`] the
//! command's completion still carries. A command that prints for ten minutes is
//! therefore observable throughout, and its terminal result is byte-identical
//! to what a read-to-end capture would have produced.

use std::future::Future;
use std::io;
use std::process::{Output, Stdio};
use std::time::Duration;

use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::{Child, ChildStderr, ChildStdout, Command};

/// Grace allowed after each group signal before cancellation fails loudly.
///
/// `SIGTERM` receives this interval before escalation. `SIGKILL` receives the
/// same interval for OS-level disappearance and direct-child reaping.
pub const PROCESS_GROUP_TERMINATION_GRACE: Duration = Duration::from_secs(2);

const GROUP_PROBE_INTERVAL: Duration = Duration::from_millis(10);

/// Which of a contained command's two output streams a line arrived on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandStream {
    /// The command's standard output.
    Stdout,
    /// The command's standard error.
    Stderr,
}

impl CommandStream {
    /// The stream's conventional name, used in diagnostics and in the labels an
    /// observer attaches to what it records.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
        }
    }
}

/// Observes a contained command's output one line at a time, AS IT IS WRITTEN.
///
/// [`Self::on_line`] is called once per complete line on either stream, with the
/// line terminator (`\n`, or `\r\n`) already removed and invalid UTF-8 replaced
/// exactly as the completion capture replaces it. A blank line is delivered as
/// an empty string rather than dropped: the observer sees the command's output
/// faithfully, and deciding what is worth keeping belongs to the observer.
///
/// The call happens on the task reading that stream, so an implementation must
/// not block — it hands the line onward (a channel, a counter, a log) and
/// returns. It also must not fail the command: an observer that cannot record a
/// line reports that in its own terms; the command's result is unaffected either
/// way.
pub trait CommandOutputObserver: Send + Sync {
    /// Record one complete output line from `stream`.
    fn on_line(&self, stream: CommandStream, line: &str);
}

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
/// The command's stdout and stderr are captured in full and, as they arrive,
/// streamed line by line to `observer` — so a caller can surface what the
/// command is printing WHILE it runs, not only once it has exited. On
/// cancellation, this function does not return
/// [`CancellableCommandOutput::Cancelled`] until the direct child is reaped and
/// a signal-zero group probe reports `ESRCH`; the lines observed up to that
/// point stand.
///
/// # Errors
///
/// Returns [`ProcessGroupError`] when the command cannot be spawned or observed,
/// when signalling fails, or when the group cannot be confirmed empty after
/// bounded `SIGTERM`/`SIGKILL` handling.
pub async fn run_cancellable_command<C, O>(
    command: Command,
    cancellation: C,
    observer: &O,
) -> Result<CancellableCommandOutput, ProcessGroupError>
where
    C: Future<Output = ()>,
    O: CommandOutputObserver + ?Sized,
{
    #[cfg(unix)]
    {
        run_unix(command, cancellation, observer).await
    }
    #[cfg(not(unix))]
    {
        let _ = (command, cancellation, observer);
        Err(ProcessGroupError::Unsupported)
    }
}

#[cfg(unix)]
async fn run_unix<C, O>(
    mut command: Command,
    cancellation: C,
    observer: &O,
) -> Result<CancellableCommandOutput, ProcessGroupError>
where
    C: Future<Output = ()>,
    O: CommandOutputObserver + ?Sized,
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
        let completion = collect_output(&mut child, stdout, stderr, observer);
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

async fn collect_output<O>(
    child: &mut Child,
    stdout: Option<ChildStdout>,
    stderr: Option<ChildStderr>,
    observer: &O,
) -> Result<Output, ProcessGroupError>
where
    O: CommandOutputObserver + ?Sized,
{
    let wait = async {
        child
            .wait()
            .await
            .map_err(|source| ProcessGroupError::Reap { source })
    };
    let capture = async {
        let stdout = stdout.ok_or(ProcessGroupError::MissingPipe {
            stream: CommandStream::Stdout.name(),
        })?;
        let stderr = stderr.ok_or(ProcessGroupError::MissingPipe {
            stream: CommandStream::Stderr.name(),
        })?;
        tokio::try_join!(
            read_stream(stdout, CommandStream::Stdout, observer),
            read_stream(stderr, CommandStream::Stderr, observer)
        )
    };
    let (status, (stdout, stderr)) = tokio::try_join!(wait, capture)?;
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

/// Read one output stream to EOF, announcing each complete line to `observer` as
/// it arrives and returning every byte read, verbatim.
///
/// The line and the capture are the SAME memory: each read appends to the
/// capture buffer and the line is the slice just appended, so streaming costs no
/// second copy of the command's output and the returned bytes are exactly what a
/// read-to-end capture would have produced — terminators, blank lines, invalid
/// UTF-8 and all.
///
/// Reading stops at the first I/O error, which fails the whole command exactly as
/// it did before: a partially observed stream is never silently accepted as a
/// complete one.
async fn read_stream<R, O>(
    stream: R,
    which: CommandStream,
    observer: &O,
) -> Result<Vec<u8>, ProcessGroupError>
where
    R: AsyncRead + Unpin,
    O: CommandOutputObserver + ?Sized,
{
    let mut reader = BufReader::new(stream);
    let mut bytes = Vec::new();
    loop {
        let line_start = bytes.len();
        let read = reader
            .read_until(b'\n', &mut bytes)
            .await
            .map_err(|source| ProcessGroupError::Read {
                stream: which.name(),
                source,
            })?;
        if read == 0 {
            // EOF. A final line with no terminator was already delivered by the
            // previous iteration, which returned it without a trailing newline.
            return Ok(bytes);
        }
        let line = bytes.get(line_start..).unwrap_or_default();
        observer.on_line(which, &String::from_utf8_lossy(strip_line_ending(line)));
    }
}

/// Remove the line terminator from a chunk read up to and including `\n`.
///
/// A chunk ending the stream without a newline is returned untouched. `\r\n` is
/// treated as one terminator so a command writing DOS line endings does not
/// leave a stray carriage return at the end of every observed line.
fn strip_line_ending(line: &[u8]) -> &[u8] {
    let line = line.strip_suffix(b"\n").unwrap_or(line);
    line.strip_suffix(b"\r").unwrap_or(line)
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

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, PoisonError};
    use std::time::Duration;

    use super::{
        CancellableCommandOutput, CommandOutputObserver, CommandStream, read_stream,
        run_cancellable_command,
    };
    use tokio::process::Command;

    /// What a test returns. Every fallible step is carried rather than
    /// unwrapped, because the workspace denies panicking accessors in test code
    /// as firmly as in library code.
    type TestResult = Result<(), Box<dyn std::error::Error>>;

    /// Records every observed line in arrival order.
    #[derive(Default)]
    struct Recorder {
        lines: Mutex<Vec<(CommandStream, String)>>,
    }

    impl Recorder {
        fn lines(&self) -> Vec<(CommandStream, String)> {
            self.lines
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone()
        }

        /// The lines observed on one stream, in arrival order.
        fn on(&self, stream: CommandStream) -> Vec<String> {
            self.lines()
                .into_iter()
                .filter(|(observed, _)| *observed == stream)
                .map(|(_, line)| line)
                .collect()
        }

        fn saw(&self, stream: CommandStream, line: &str) -> bool {
            self.on(stream).iter().any(|observed| observed == line)
        }
    }

    impl CommandOutputObserver for Recorder {
        fn on_line(&self, stream: CommandStream, line: &str) {
            self.lines
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push((stream, line.to_owned()));
        }
    }

    /// THE PROPERTY THIS BUILD EXISTS FOR: a line written on either stream is
    /// observable WHILE the command is still running, not when it exits.
    ///
    /// The command prints one line on each stream and then sleeps far longer
    /// than the test's patience. The run future is polled concurrently with the
    /// check, so if the lines only materialized at completion the run arm would
    /// win and the test fails by name; a capture that never delivers them fails
    /// on the deadline. Both failure modes are the old read-to-end behaviour.
    #[tokio::test]
    async fn output_lines_are_observed_before_the_command_exits() -> TestResult {
        let recorder = Recorder::default();
        let mut command = Command::new("sh");
        command.arg("-c").arg("echo out; echo err >&2; sleep 30");
        let run = run_cancellable_command(command, std::future::pending::<()>(), &recorder);
        tokio::pin!(run);

        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        loop {
            tokio::select! {
                biased;
                result = &mut run => {
                    drop(result);
                    return Err(
                        "the command ran to completion before its output was observed".into(),
                    );
                }
                () = tokio::time::sleep(Duration::from_millis(5)) => {
                    if recorder.saw(CommandStream::Stdout, "out")
                        && recorder.saw(CommandStream::Stderr, "err")
                    {
                        // Both streams delivered mid-run. Dropping the pinned run
                        // future on the way out kills and reaps the process group,
                        // so the sleeping command is not left behind.
                        return Ok(());
                    }
                    if tokio::time::Instant::now() >= deadline {
                        return Err(format!(
                            "the running command's output never arrived: {:?}",
                            recorder.lines()
                        )
                        .into());
                    }
                }
            }
        }
    }

    /// Streaming must not change what the command's completion carries: the
    /// captured bytes are exactly the stream's, terminators included, and the
    /// observed lines are that same output split on its newlines.
    #[tokio::test]
    async fn the_completion_capture_is_unchanged_by_streaming() -> TestResult {
        let recorder = Recorder::default();
        let mut command = Command::new("sh");
        command
            .arg("-c")
            .arg("printf 'first\\nsecond\\n'; printf 'warned\\n' >&2");
        let outcome =
            run_cancellable_command(command, std::future::pending::<()>(), &recorder).await?;
        let CancellableCommandOutput::Completed(output) = outcome else {
            return Err("the command must complete".into());
        };

        assert_eq!(output.stdout, b"first\nsecond\n");
        assert_eq!(output.stderr, b"warned\n");
        assert_eq!(recorder.on(CommandStream::Stdout), vec!["first", "second"]);
        assert_eq!(recorder.on(CommandStream::Stderr), vec!["warned"]);
        Ok(())
    }

    /// The line splitter's whole contract on one input: CRLF and LF are both one
    /// terminator, a blank line is delivered rather than dropped, a final line
    /// with no terminator is still delivered, invalid UTF-8 is replaced exactly
    /// as the capture replaces it, and every byte read is returned verbatim.
    #[tokio::test]
    async fn every_line_shape_is_delivered_and_the_bytes_are_returned_verbatim() -> TestResult {
        let source: &[u8] = b"plain\ncrlf\r\n\n\xffbad\nno trailing newline";
        let recorder = Recorder::default();

        let bytes = read_stream(source, CommandStream::Stdout, &recorder).await?;

        assert_eq!(bytes, source, "the capture returns every byte, untouched");
        assert_eq!(
            recorder.on(CommandStream::Stdout),
            vec![
                "plain".to_owned(),
                "crlf".to_owned(),
                String::new(),
                String::from_utf8_lossy(b"\xffbad").into_owned(),
                "no trailing newline".to_owned(),
            ]
        );
        Ok(())
    }

    /// A stream that produces nothing observes nothing — no phantom empty line
    /// at EOF.
    #[tokio::test]
    async fn an_empty_stream_observes_nothing() -> TestResult {
        let recorder = Recorder::default();
        let bytes = read_stream(&b""[..], CommandStream::Stderr, &recorder).await?;
        assert!(bytes.is_empty());
        assert!(recorder.lines().is_empty());
        Ok(())
    }
}
