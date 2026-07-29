//! One contained command run: its log file, its exit code, and its live
//! progress channel.
//!
//! Every command this worker runs goes through [`run_logged`]. It is built on
//! the SDK's [`run_cancellable_command`], so a cancelled workflow takes the
//! whole process group down with it rather than orphaning a Cargo build on the
//! gate host. Nothing is discarded: stdout and stderr are both written to the
//! leg's log file, and the real exit code is always returned.

use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Output};
use std::time::{Duration, Instant};

use aion_core::{Payload, PayloadError};
use aion_worker::{
    ActivityContext, CancellableCommandOutput, ProcessGroupError, WorkerError,
    run_cancellable_command,
};
use thiserror::Error;
use tokio::process::Command;

/// How many trailing log lines travel back in `output_tail`.
///
/// Mirrors the landed `cargo-gates` worker: enough to read a failure without
/// shipping a whole build log through the event store. The full log always
/// stays on the gate host at `log_path`.
pub const OUTPUT_TAIL_LINES: usize = 40;

/// Exit code reported when no process status could be observed at all.
pub const NO_PROCESS_EXIT_CODE: i32 = -1;

/// A command that failed to run, be observed, or be logged.
#[derive(Debug, Error)]
pub enum CommandError {
    /// The log file could not be prepared or written.
    #[error("could not write the leg log at {path}: {source}")]
    Log {
        /// Log file the worker was trying to write.
        path: PathBuf,
        /// Underlying filesystem failure.
        #[source]
        source: std::io::Error,
    },
    /// The command could not be spawned, observed, or contained.
    #[error("`{program}` could not be run in {cwd}: {source}")]
    Process {
        /// Program the worker tried to run.
        program: String,
        /// Directory it was to run in.
        cwd: PathBuf,
        /// Underlying containment failure.
        #[source]
        source: ProcessGroupError,
    },
    /// The workflow cancelled the activity; the process group is confirmed dead.
    #[error("`{program}` was cancelled and its process group was terminated")]
    Cancelled {
        /// Program that was running when cancellation arrived.
        program: String,
    },
    /// A progress heartbeat could not be attached to the activity.
    #[error("progress heartbeat for `{label}` could not be sent: {source}")]
    Heartbeat {
        /// Leg or step the heartbeat belonged to.
        label: String,
        /// Underlying worker failure.
        #[source]
        source: WorkerError,
    },
    /// A progress heartbeat payload could not be encoded.
    #[error("progress heartbeat payload for `{label}` could not be encoded: {source}")]
    HeartbeatPayload {
        /// Leg or step the heartbeat belonged to.
        label: String,
        /// Underlying encoding failure.
        #[source]
        source: PayloadError,
    },
}

/// A command to run on the gate host.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandSpec {
    /// Program to execute.
    pub program: String,
    /// Arguments, already resolved — no shell, no word splitting.
    pub args: Vec<String>,
    /// Environment entries set for this run only.
    pub env: Vec<(String, String)>,
    /// Directory the command runs in.
    pub cwd: PathBuf,
}

impl CommandSpec {
    /// The exact invocation, rendered for the log header.
    #[must_use]
    pub fn rendered(&self) -> String {
        let mut rendered = String::new();
        for (key, value) in &self.env {
            rendered.push_str(key);
            rendered.push('=');
            rendered.push_str(value);
            rendered.push(' ');
        }
        rendered.push_str(&self.program);
        for arg in &self.args {
            rendered.push(' ');
            rendered.push_str(arg);
        }
        rendered
    }
}

/// What a completed command produced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandRun {
    /// Real exit code, or the signal-derived code on a signalled exit.
    pub exit_code: i32,
    /// Captured standard output, verbatim — the machine-readable stream.
    pub stdout: String,
    /// Captured standard error, verbatim — never discarded.
    pub stderr: String,
    /// Absolute path of the log file holding both streams.
    pub log_path: PathBuf,
    /// Last [`OUTPUT_TAIL_LINES`] lines of the written log body.
    pub output_tail: String,
}

/// Runs one command to completion, logging it and heartbeating while it runs.
///
/// The heartbeat ticker only fires while the command is still running, so the
/// server can tell a grinding build from a dead worker. The log file is written
/// once the command finishes, and it always contains the exact invocation, both
/// output streams, and the exit code.
///
/// # Errors
///
/// Returns [`CommandError`] when the command cannot be spawned or contained,
/// when the activity was cancelled, when a heartbeat cannot be sent, or when the
/// log file cannot be written.
pub async fn run_logged(
    spec: &CommandSpec,
    log_path: &Path,
    label: &str,
    context: &ActivityContext,
    progress_interval: Duration,
) -> Result<CommandRun, CommandError> {
    let started = Instant::now();
    let output = run_contained(spec, label, context, progress_interval, started).await?;
    let exit_code = exit_code(output.status);
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let body = log_body(spec, &stdout, &stderr, exit_code, started.elapsed());
    write_log(log_path, &body)?;
    Ok(CommandRun {
        exit_code,
        stdout,
        stderr,
        log_path: log_path.to_path_buf(),
        output_tail: tail(&body),
    })
}

async fn run_contained(
    spec: &CommandSpec,
    label: &str,
    context: &ActivityContext,
    progress_interval: Duration,
    started: Instant,
) -> Result<Output, CommandError> {
    let mut command = Command::new(&spec.program);
    command.args(&spec.args).current_dir(&spec.cwd);
    for (key, value) in &spec.env {
        command.env(key, value);
    }
    let run = run_cancellable_command(command, context.cancelled());
    tokio::pin!(run);
    let mut ticker = tokio::time::interval(progress_interval);
    // The first tick of a Tokio interval completes immediately; consume it so the
    // first progress beat lands one interval into the run, not at spawn time.
    ticker.tick().await;
    let outcome = loop {
        tokio::select! {
            outcome = &mut run => break outcome,
            _ = ticker.tick() => beat(label, context, started.elapsed())?,
        }
    };
    match outcome {
        Ok(CancellableCommandOutput::Completed(output)) => Ok(output),
        Ok(CancellableCommandOutput::Cancelled) => Err(CommandError::Cancelled {
            program: spec.program.clone(),
        }),
        Err(source) => Err(CommandError::Process {
            program: spec.program.clone(),
            cwd: spec.cwd.clone(),
            source,
        }),
    }
}

fn beat(label: &str, context: &ActivityContext, elapsed: Duration) -> Result<(), CommandError> {
    let detail = Payload::from_json(&serde_json::json!({
        "leg": label,
        "state": "running",
        "elapsed_seconds": elapsed.as_secs(),
    }))
    .map_err(|source| CommandError::HeartbeatPayload {
        label: label.to_owned(),
        source,
    })?;
    context
        .heartbeat(Some(detail))
        .map_err(|source| CommandError::Heartbeat {
            label: label.to_owned(),
            source,
        })
}

fn log_body(
    spec: &CommandSpec,
    stdout: &str,
    stderr: &str,
    exit_code: i32,
    elapsed: Duration,
) -> String {
    format!(
        "# cwd: {}\n# argv: {}\n\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}\n--- exit: {exit_code} after {:.3}s ---\n",
        spec.cwd.display(),
        spec.rendered(),
        elapsed.as_secs_f64(),
    )
}

fn write_log(log_path: &Path, body: &str) -> Result<(), CommandError> {
    if let Some(parent) = log_path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| CommandError::Log {
            path: log_path.to_path_buf(),
            source,
        })?;
    }
    std::fs::write(log_path, body).map_err(|source| CommandError::Log {
        path: log_path.to_path_buf(),
        source,
    })
}

/// Returns the last [`OUTPUT_TAIL_LINES`] lines of `output`.
#[must_use]
pub fn tail(output: &str) -> String {
    let mut lines = output
        .lines()
        .rev()
        .take(OUTPUT_TAIL_LINES)
        .collect::<Vec<_>>();
    lines.reverse();
    lines.join("\n")
}

/// Maps a process status onto a reportable exit code.
#[cfg(unix)]
#[must_use]
pub fn exit_code(status: ExitStatus) -> i32 {
    use std::os::unix::process::ExitStatusExt;
    match (status.code(), status.signal()) {
        (Some(code), _) => code,
        (None, Some(signal)) => 128 + signal,
        (None, None) => NO_PROCESS_EXIT_CODE,
    }
}

/// Maps a process status onto a reportable exit code.
#[cfg(not(unix))]
#[must_use]
pub fn exit_code(status: ExitStatus) -> i32 {
    status.code().unwrap_or(NO_PROCESS_EXIT_CODE)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::Duration;

    use super::{CommandSpec, log_body, tail};

    fn spec() -> CommandSpec {
        CommandSpec {
            program: "cargo".to_owned(),
            args: vec!["check".to_owned(), "--workspace".to_owned()],
            env: vec![("CARGO_TERM_COLOR".to_owned(), "never".to_owned())],
            cwd: PathBuf::from("/srv/gates/checkouts/aion"),
        }
    }

    #[test]
    fn the_rendered_invocation_is_the_real_argv() {
        assert_eq!(
            spec().rendered(),
            "CARGO_TERM_COLOR=never cargo check --workspace"
        );
    }

    #[test]
    fn the_log_body_keeps_both_streams_and_the_exit_code() {
        let body = log_body(
            &spec(),
            "{\"reason\":\"x\"}",
            "warning: y",
            101,
            Duration::from_millis(1500),
        );
        assert!(body.contains("# argv: CARGO_TERM_COLOR=never cargo check --workspace"));
        assert!(body.contains("--- stdout ---\n{\"reason\":\"x\"}"));
        assert!(body.contains("--- stderr ---\nwarning: y"));
        assert!(body.contains("--- exit: 101 after 1.500s ---"));
    }

    #[test]
    fn the_tail_is_bounded_to_the_last_forty_lines() {
        let output = (1..=50)
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let bounded = tail(&output);
        assert_eq!(bounded.lines().count(), 40);
        assert_eq!(bounded.lines().next(), Some("11"));
        assert_eq!(bounded.lines().last(), Some("50"));
    }
}
