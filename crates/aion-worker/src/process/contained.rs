//! The process-group containment core.
//!
//! Every process this distribution owns — a worker-owned command, a supervised
//! managed worker — enters a FRESH process group before user code runs, so
//! stopping it stops the whole tree it went on to build. There is exactly one
//! implementation of that discipline, and it is here: a second copy is how a
//! grandchild comes to outlive the thing that spawned it.
//!
//! # The rule the whole module is built around
//!
//! **No signal is ever sent to the group after the leading child has been
//! reaped.** A process-group id IS the leader's pid, and a pid is free for the
//! operating system to reuse the moment it is reaped. Signalling afterwards
//! therefore aims at whatever took the number — on a busy machine that is not
//! hypothetical, and the consequence would be `SIGKILL` delivered to an
//! unrelated process group.
//!
//! So [`ContainedChild`] tracks whether it has reaped, both signals of the
//! termination ladder are sent BEFORE the reap, and the drop guard stands down
//! once reaping has happened. What survives after a reap is a read-only probe
//! used for REPORTING, which can never act on a wrong answer.
//!
//! Killing the group and PROVING it empty are therefore separate operations.
//! [`ContainedChild::terminate`] does the first; the second is either
//! [`ContainedChild::confirm_group_gone`] — `killpg(pgid, 0)` reporting `ESRCH`
//! (gone) or `EPERM` (equally conclusive: this process can always signal its
//! own descendants, so being refused means the id belongs to someone else now)
//! — or end-of-file on captured output, which no surviving descendant holding
//! the write end could produce. A caller with pipes should prefer EOF: it
//! cannot be confused by a recycled id at all.

use std::io;
use std::process::ExitStatus;
use std::time::Duration;

use thiserror::Error;
use tokio::process::{Child, ChildStderr, ChildStdout, Command};

/// Grace allowed after `SIGTERM` before the ladder escalates to `SIGKILL`.
///
/// Also the window a caller is given to confirm the group empty after the
/// signals have been sent.
pub const PROCESS_GROUP_TERMINATION_GRACE: Duration = Duration::from_secs(2);

/// Cadence for the two polling loops: waiting for the leader to exit, and
/// confirming the group empty. A mechanism detail of the ladder, not an
/// operator knob.
const GROUP_PROBE_INTERVAL: Duration = Duration::from_millis(10);

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
    /// The group remained observable after the signals and the bounded wait.
    #[error(
        "process group {process_group} was still observable {grace:?} after termination; \
         it could not be confirmed empty"
    )]
    GroupStillAlive {
        /// Process group that failed to disappear.
        process_group: i32,
        /// Confirmation interval that elapsed.
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

/// The process group a [`ContainedChild`] leads.
#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProcessGroupId(nix::unistd::Pid);

/// The process group a [`ContainedChild`] leads. Never constructed on a target
/// without process groups: [`ContainedChild::spawn`] refuses first.
#[cfg(not(unix))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProcessGroupId;

#[cfg(unix)]
impl ProcessGroupId {
    /// The raw Unix process-group id.
    #[must_use]
    pub const fn as_raw(self) -> i32 {
        self.0.as_raw()
    }
}

/// A spawned child leading its own process group, with tree-wide termination.
///
/// While the leader is unreaped and the handle is armed, dropping it `SIGKILL`s
/// the group — the containment guarantee under task abort. Once reaped, the
/// guard stands down: the id is no longer safely ours to signal.
#[derive(Debug)]
pub struct ContainedChild {
    child: Child,
    process_group: ProcessGroupId,
    armed: bool,
    reaped: bool,
    exit_status: Option<ExitStatus>,
}

impl ContainedChild {
    /// Spawn `command` as the leader of a fresh process group.
    ///
    /// The caller configures stdio before calling: piping it captures output,
    /// inheriting it sends the child's output wherever this process's goes.
    ///
    /// # Errors
    ///
    /// Returns [`ProcessGroupError::Unsupported`] on a target without process
    /// groups, [`ProcessGroupError::Spawn`] when the command cannot start, and
    /// [`ProcessGroupError::MissingProcessId`] or
    /// [`ProcessGroupError::ProcessIdOutOfRange`] when the started child cannot
    /// be addressed as a group.
    pub fn spawn(command: Command) -> Result<Self, ProcessGroupError> {
        #[cfg(unix)]
        {
            spawn_unix(command)
        }
        #[cfg(not(unix))]
        {
            drop(command);
            Err(ProcessGroupError::Unsupported)
        }
    }

    /// The direct child's process id, while it has not been reaped.
    #[must_use]
    pub fn id(&self) -> Option<u32> {
        self.child.id()
    }

    /// The raw process-group id, on a target that has process groups.
    ///
    /// `None` off Unix, where [`Self::spawn`] refuses in the first place.
    #[must_use]
    pub const fn process_group_id(&self) -> Option<i32> {
        #[cfg(unix)]
        {
            Some(self.process_group.as_raw())
        }
        #[cfg(not(unix))]
        {
            None
        }
    }

    /// The exit status recorded by whichever call reaped the leading child.
    #[must_use]
    pub const fn exit_status(&self) -> Option<ExitStatus> {
        self.exit_status
    }

    /// Take the captured stdout pipe, when the command was configured with one.
    pub fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.child.stdout.take()
    }

    /// Take the captured stderr pipe, when the command was configured with one.
    pub fn take_stderr(&mut self) -> Option<ChildStderr> {
        self.child.stderr.take()
    }

    /// Wait for the DIRECT child to exit, REAPING it.
    ///
    /// After this returns, the process-group id is no longer safely ours: see
    /// the module header. Callers that still need to stop the tree must use
    /// [`Self::wait_for_exit`] instead, which observes the exit without
    /// consuming it.
    ///
    /// # Errors
    ///
    /// Returns [`ProcessGroupError::Reap`] when waiting fails.
    pub async fn wait(&mut self) -> Result<ExitStatus, ProcessGroupError> {
        let status = self
            .child
            .wait()
            .await
            .map_err(|source| ProcessGroupError::Reap { source })?;
        self.reaped = true;
        self.exit_status = Some(status);
        Ok(status)
    }

    /// Stop the whole group, then reap the leading child.
    ///
    /// `SIGTERM` → `grace` → `SIGKILL`, BOTH sent while the leader is unreaped,
    /// so the group being signalled is provably the one this handle created.
    /// Once the leader has already been reaped no signal is sent at all: the id
    /// may belong to anyone by then, and a stop that cannot be aimed is a stop
    /// that must not be attempted.
    ///
    /// The `SIGKILL` is unconditional rather than conditional on surviving the
    /// `SIGTERM`. Checking first would mean either reaping the leader (which
    /// forfeits the right to signal) or reading a probe that a live-or-zombie
    /// leader always answers "alive" to — so the check cannot work, and an
    /// extra signal to an empty group costs nothing.
    ///
    /// This says nothing about whether the group ended up EMPTY. Proving that
    /// is the caller's job: [`Self::confirm_group_gone`] where there is nothing
    /// else to go on, or end-of-file on captured output, which no surviving
    /// descendant holding the write end could produce.
    ///
    /// # Errors
    ///
    /// Returns [`ProcessGroupError::Unsupported`] off Unix,
    /// [`ProcessGroupError::Signal`] when the OS refuses to signal, and
    /// [`ProcessGroupError::Reap`] when the leader cannot be reaped.
    pub async fn terminate(&mut self, grace: Duration) -> Result<(), ProcessGroupError> {
        #[cfg(unix)]
        {
            if !self.reaped {
                signal_group(
                    self.process_group.0,
                    nix::sys::signal::Signal::SIGTERM,
                    "SIGTERM",
                )?;
                tokio::time::sleep(grace).await;
                signal_group(
                    self.process_group.0,
                    nix::sys::signal::Signal::SIGKILL,
                    "SIGKILL",
                )?;
                // The status is kept on the handle by `wait`; callers read it
                // back through `exit_status`.
                self.wait().await?;
            }
            self.armed = false;
            Ok(())
        }
        #[cfg(not(unix))]
        {
            drop(grace);
            Err(ProcessGroupError::Unsupported)
        }
    }

    /// Poll until the group is provably gone, or the window closes.
    ///
    /// A read, never an act — which is what makes it safe after the leader has
    /// been reaped, where acting on a wrong answer would be the disaster.
    ///
    /// # Errors
    ///
    /// Returns [`ProcessGroupError::Unsupported`] off Unix,
    /// [`ProcessGroupError::Probe`] when the probe itself fails, and
    /// [`ProcessGroupError::GroupStillAlive`] when the window closes with the
    /// group still observable.
    pub async fn confirm_group_gone(&self, within: Duration) -> Result<(), ProcessGroupError> {
        #[cfg(unix)]
        {
            let deadline = tokio::time::Instant::now() + within;
            loop {
                if group_is_gone(self.process_group.0)? {
                    return Ok(());
                }
                let now = tokio::time::Instant::now();
                if now >= deadline {
                    return Err(ProcessGroupError::GroupStillAlive {
                        process_group: self.process_group.0.as_raw(),
                        grace: within,
                    });
                }
                tokio::time::sleep(GROUP_PROBE_INTERVAL.min(deadline - now)).await;
            }
        }
        #[cfg(not(unix))]
        {
            drop(within);
            Err(ProcessGroupError::Unsupported)
        }
    }

    /// Release the drop guard without signalling anything.
    ///
    /// For a caller that has established the group is gone by other means — the
    /// captured pipes reaching EOF, for instance, which no descendant holding
    /// the write end could allow.
    pub const fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for ContainedChild {
    fn drop(&mut self) {
        // Reaped means the id is no longer provably ours (module header), so the
        // guard stands down rather than signalling a group it cannot identify.
        if !self.armed || self.reaped {
            return;
        }
        // `ESRCH` is the group having departed, and `EPERM` is equally
        // conclusive — the module's own doctrine ([`confirm_group_gone`],
        // [`signal_group`]): this process can always signal its own live
        // descendants, so a permission refusal means only unsignalable
        // remnants (zombies awaiting the reap) remain. Both are the group
        // being gone, not a failed kill; reporting `EPERM` as an error here
        // once dressed a routine server shutdown in a spurious ERROR line.
        #[cfg(unix)]
        match nix::sys::signal::killpg(self.process_group.0, nix::sys::signal::Signal::SIGKILL) {
            Ok(()) | Err(nix::errno::Errno::ESRCH | nix::errno::Errno::EPERM) => {}
            Err(source) => tracing::error!(
                process_group = self.process_group.0.as_raw(),
                %source,
                "failed to kill contained process group while dropping its owner"
            ),
        }
    }
}

#[cfg(unix)]
fn spawn_unix(mut command: Command) -> Result<ContainedChild, ProcessGroupError> {
    use std::os::unix::process::CommandExt;

    command.as_std_mut().process_group(0);
    command.kill_on_drop(true);
    let child = command
        .spawn()
        .map_err(|source| ProcessGroupError::Spawn { source })?;
    let raw_pid = child.id().ok_or(ProcessGroupError::MissingProcessId)?;
    let process_group = i32::try_from(raw_pid)
        .map_err(|_| ProcessGroupError::ProcessIdOutOfRange { pid: raw_pid })?;
    Ok(ContainedChild {
        child,
        process_group: ProcessGroupId(nix::unistd::Pid::from_raw(process_group)),
        armed: true,
        reaped: false,
        exit_status: None,
    })
}

#[cfg(unix)]
fn signal_group(
    process_group: nix::unistd::Pid,
    signal: nix::sys::signal::Signal,
    signal_name: &'static str,
) -> Result<(), ProcessGroupError> {
    match nix::sys::signal::killpg(process_group, signal) {
        // `ESRCH` is an empty group. `EPERM` is a group with no SIGNALABLE
        // member, which on macOS is what a group reduced to its own unreaped
        // zombie leader reports — the overwhelmingly common shape a moment
        // after the worker exits. Both mean the same thing to a caller trying
        // to stop a tree: there is nothing left here for this process to kill.
        // Reporting either as a failure to send is what turned an ordinary
        // cancellation into a spurious error.
        Ok(()) | Err(nix::errno::Errno::ESRCH | nix::errno::Errno::EPERM) => Ok(()),
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
        // `ESRCH` is the group having departed. `EPERM` is equally conclusive:
        // this process can always signal its own descendants, so an id it may
        // NOT signal is an id that has been recycled by somebody else — either
        // way the group this handle created is gone.
        Err(nix::errno::Errno::ESRCH | nix::errno::Errno::EPERM) => Ok(true),
        Err(source) => Err(ProcessGroupError::Probe {
            process_group: process_group.as_raw(),
            source,
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::process::Stdio;
    use std::time::Duration;

    use tokio::process::Command;

    use super::{ContainedChild, PROCESS_GROUP_TERMINATION_GRACE};

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn shell(script: &str) -> Command {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", script]);
        command.stdout(Stdio::null()).stderr(Stdio::null());
        command
    }

    fn group_alive(process_group: i32) -> bool {
        std::process::Command::new("/bin/sh")
            .args(["-c", &format!("kill -0 -{process_group} 2>/dev/null")])
            .status()
            .is_ok_and(|status| status.success())
    }

    /// A grandchild dies with the group, and the group is PROVEN gone rather
    /// than assumed so.
    #[tokio::test]
    async fn terminate_takes_the_whole_group() -> TestResult {
        let mut child = ContainedChild::spawn(shell("sleep 300 & sleep 300"))?;
        let group = child
            .process_group_id()
            .ok_or("a spawned child leads a group")?;
        assert!(group_alive(group));
        child.terminate(Duration::from_millis(20)).await?;
        child
            .confirm_group_gone(PROCESS_GROUP_TERMINATION_GRACE)
            .await?;
        assert!(!group_alive(group), "the group survived a confirmed stop");
        assert!(
            child.exit_status().is_some(),
            "terminate must reap the leader it killed"
        );
        Ok(())
    }

    /// Once the leader has been reaped, no signal may be sent — the id could
    /// belong to anyone. `terminate` must become a no-op rather than take aim
    /// at a number it can no longer vouch for. The safety property is asserted
    /// through an OBSERVABLE consequence: skipping the ladder means skipping
    /// the grace it would otherwise sleep through.
    #[tokio::test]
    async fn a_reaped_child_is_never_signalled_again() -> TestResult {
        let mut child = ContainedChild::spawn(shell("exit 0"))?;
        let status = child.wait().await?;
        assert_eq!(status.code(), Some(0));
        let started = std::time::Instant::now();
        child.terminate(Duration::from_secs(30)).await?;
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "a reaped child must skip the ladder entirely, not sleep through it"
        );
        Ok(())
    }

    /// The exit status is recorded by whichever call reaped, and a later
    /// `terminate` neither loses it nor overwrites it.
    ///
    /// Deliberately driven through a NATURAL exit rather than by racing the
    /// ladder against a short-lived command: `terminate` sends `SIGTERM`
    /// immediately, so a command that has not managed to exit yet is signalled
    /// and reports no exit code at all. That is correct behaviour, not a
    /// defect, and a test that assumed otherwise would only be asserting that
    /// the machine was idle.
    #[tokio::test]
    async fn the_exit_status_is_recorded_by_whichever_call_reaped() -> TestResult {
        let mut child = ContainedChild::spawn(shell("exit 5"))?;
        assert_eq!(child.wait().await?.code(), Some(5));
        assert_eq!(
            child.exit_status().and_then(|status| status.code()),
            Some(5)
        );
        child.terminate(Duration::from_millis(20)).await?;
        assert_eq!(
            child.exit_status().and_then(|status| status.code()),
            Some(5),
            "a terminate after the reap must not disturb the recorded status"
        );
        Ok(())
    }
}
