//! One supervised managed-worker instance: spawn, observe, restart, stop.
//!
//! The loop here is the whole supervision mechanism. Three properties it is
//! built to keep, each of which has a test:
//!
//! * **A crash brings the worker back.** Bounded by the operator's window and
//!   restart budget — a crash loop escalates to a visible `Failed`, never a
//!   silent retry forever.
//! * **A stop is terminal.** Stop is not a slow restart: the loop returns, and
//!   the only way back is an operator start.
//! * **`Stopped` is earned, not asserted.** It is written only after the whole
//!   process tree was proven gone. A stop whose tree survives reports `Failed`
//!   with what was observed, so the failure mode "the registry says stopped
//!   while the process lives" has no path to a `Stopped` status.
//!
//! ## Why the proof is end-of-file, not a process-group probe
//!
//! A process-group id is the leading child's pid, and a pid is reusable the
//! instant it is reaped. Probing or signalling that id afterwards asks a
//! question about whatever inherited the number — on a busy machine that is
//! not theoretical, and it has been observed answering `EPERM`. So the loop
//! signals only while the leader is unreaped (which the containment core
//! enforces), and PROVES the tree gone by draining the worker's piped output
//! to end-of-file: no process still holding a write end can produce that.
//!
//! What is NOT solved here: `kill -9` of the SERVER runs none of this, so a
//! hard-killed server can leave managed workers behind. The ruled stronger
//! mechanisms — cgroup v2 on Linux, a dead-man launcher on macOS
//! (RUNTIME-OPERATIONS R8) — close that and are not built in this unit.

use std::collections::VecDeque;
use std::process::{ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use aion_store::{DesiredState, WorkerDeploymentStore};
use aion_worker::ContainedChild;
use tokio::io::AsyncBufReadExt;
use tokio::process::Command;
use tokio::sync::watch;
use tokio::time::Instant;

use super::error::SupervisionError;
use super::executable::ManagedExecutable;
use super::policy::SupervisionPolicy;
use super::status::{ManagedWorkerExit, ManagedWorkerState, SpawnedBinary};

/// Everything one instance loop needs, captured at start.
pub(super) struct InstanceConfig {
    /// Deployment primary key.
    pub(super) name: String,
    /// Operator-declared argv tail appended to the executable.
    pub(super) verb: Vec<String>,
    /// Which executable to launch.
    pub(super) executable: ManagedExecutable,
    /// The operator's restart discipline.
    pub(super) policy: SupervisionPolicy,
    /// Durable records, re-read before every restart so an out-of-band desired
    /// state change converges without a server restart.
    pub(super) store: Arc<dyn WorkerDeploymentStore>,
}

/// The live, readable state of one instance.
#[derive(Clone, Debug, Default)]
pub(super) struct InstanceSnapshot {
    /// Live supervision state; `None` only before the loop's first transition.
    pub(super) state: Option<ManagedWorkerState>,
    /// Process id of the running child.
    pub(super) pid: Option<u32>,
    /// Process group the running child leads.
    pub(super) process_group: Option<i32>,
    /// Restarts this instance has performed.
    pub(super) restarts: u32,
    /// How the process last ended.
    pub(super) last_exit: Option<ManagedWorkerExit>,
    /// Last failure worth an operator's attention.
    pub(super) last_error: Option<String>,
    /// Identity of the executable the most recent spawn ran.
    pub(super) spawn_binary: Option<SpawnedBinary>,
    /// Whether a process group was created and has NOT since been proven empty.
    ///
    /// This — not the state token — is what a stop is gated on. It is set at
    /// spawn and cleared only by a confirmed teardown, so "the group is gone"
    /// is always an operating-system observation. Defaulting to false is
    /// correct and load-bearing: an instance that never spawned has nothing to
    /// leak.
    pub(super) containment_unconfirmed: bool,
}

/// State shared between the instance loop and everything that reads it.
#[derive(Debug, Default)]
pub(super) struct InstanceShared {
    snapshot: Mutex<InstanceSnapshot>,
}

impl InstanceShared {
    /// Read the current snapshot.
    fn snapshot(&self) -> Result<InstanceSnapshot, SupervisionError> {
        self.snapshot
            .lock()
            .map(|guard| guard.clone())
            .map_err(|poison| SupervisionError::StatePoisoned {
                detail: poison.to_string(),
            })
    }

    fn mutate<F>(&self, mutation: F)
    where
        F: FnOnce(&mut InstanceSnapshot),
    {
        match self.snapshot.lock() {
            Ok(mut guard) => mutation(&mut guard),
            Err(poison) => {
                // A poisoned status cell means a previous holder panicked mid
                // write. Recovering the guard keeps later transitions visible
                // (readers still get the poison as a typed error from
                // `snapshot`) instead of losing every one of them in silence.
                mutation(&mut poison.into_inner());
            }
        }
    }

    fn set_terminal(&self, state: ManagedWorkerState, detail: Option<String>) {
        self.mutate(|snapshot| {
            snapshot.state = Some(state);
            snapshot.pid = None;
            snapshot.process_group = None;
            if detail.is_some() {
                snapshot.last_error = detail;
            }
        });
    }
}

/// What one spawn-and-watch cycle ended as.
enum Cycle {
    /// The child ended on its own; a restart decision is owed.
    Exited,
    /// A stop was asked for and handled. Terminal either way: the state cell
    /// already says whether the group was proven empty.
    StopHandled,
}

/// Run one instance until it is stopped or fails.
///
/// This task owns every transition written into `shared`; nothing else writes
/// to it while the task lives.
pub(super) async fn run(
    config: InstanceConfig,
    shared: Arc<InstanceShared>,
    mut stop: watch::Receiver<bool>,
) {
    let mut window: VecDeque<Instant> = VecDeque::new();
    let mut restarts = 0_u32;
    loop {
        shared.mutate(|snapshot| {
            snapshot.state = Some(ManagedWorkerState::Starting);
            snapshot.pid = None;
            snapshot.process_group = None;
        });

        match spawn(&config) {
            Ok(spawned) => match observe(&config, &shared, spawned, &mut stop).await {
                Cycle::StopHandled => return,
                Cycle::Exited => {}
            },
            Err(error) => {
                tracing::error!(
                    worker = config.name.as_str(),
                    %error,
                    "managed worker could not be spawned"
                );
                shared.mutate(|snapshot| snapshot.last_error = Some(error.to_string()));
            }
        }

        if !restart_permitted(&config, &shared, &mut window, &mut restarts, &mut stop).await {
            return;
        }
    }
}

fn spawn(config: &InstanceConfig) -> Result<Spawned, SupervisionError> {
    let path = config.executable.resolve()?;
    let binary = config.executable.identify(&path)?;
    let mut command = Command::new(&path);
    command.args(&config.verb);
    // The output is PIPED rather than inherited, and that is a containment
    // decision before it is a logging one: end-of-file on both streams happens
    // only once every process holding the write end has exited, which is the
    // one proof of "the whole tree is gone" that a recycled process-group id
    // cannot forge. The forwarders below keep it bounded — each line is logged
    // and dropped, never accumulated.
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = ContainedChild::spawn(command).map_err(|source| SupervisionError::Process {
        name: config.name.clone(),
        source,
    })?;
    tracing::info!(
        worker = config.name.as_str(),
        executable = binary.path.as_str(),
        content_hash = binary.content_hash.as_str(),
        pid = child.id(),
        "managed worker spawned"
    );
    let output = Output {
        stdout: child
            .take_stdout()
            .map(|pipe| forward(config.name.clone(), "stdout", pipe)),
        stderr: child
            .take_stderr()
            .map(|pipe| forward(config.name.clone(), "stderr", pipe)),
    };
    Ok(Spawned {
        child,
        binary,
        output,
    })
}

/// One spawned instance and the two tasks draining its output.
struct Spawned {
    child: ContainedChild,
    binary: SpawnedBinary,
    output: Output,
}

/// The forwarders whose completion means end-of-file on the worker's streams.
struct Output {
    stdout: Option<tokio::task::JoinHandle<()>>,
    stderr: Option<tokio::task::JoinHandle<()>>,
}

impl Output {
    /// Wait for both streams to reach EOF, bounded by `within`.
    ///
    /// Returns whether they did. `true` is the tree-gone proof; `false` says
    /// something is still holding a write end, which is the honest answer when
    /// a descendant has outlived the worker that spawned it.
    async fn drained(self, within: Duration) -> bool {
        let joined = async {
            for handle in [self.stdout, self.stderr].into_iter().flatten() {
                if handle.await.is_err() {
                    return false;
                }
            }
            true
        };
        (tokio::time::timeout(within, joined).await).unwrap_or(false)
    }
}

/// Read one of the worker's streams to EOF, logging what it says.
///
/// Line by line and never accumulated: a managed worker runs for weeks, and a
/// buffer that grows with its output would be a memory leak with a schedule.
fn forward(
    worker: String,
    stream: &'static str,
    pipe: impl tokio::io::AsyncRead + Unpin + Send + 'static,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut lines = tokio::io::BufReader::new(pipe).lines();
        loop {
            match lines.next_line().await {
                Ok(Some(line)) => {
                    tracing::info!(worker = worker.as_str(), stream, "{line}");
                }
                Ok(None) => return,
                Err(error) => {
                    tracing::warn!(
                        worker = worker.as_str(),
                        stream,
                        %error,
                        "managed worker output could not be read"
                    );
                    return;
                }
            }
        }
    })
}

async fn observe(
    config: &InstanceConfig,
    shared: &Arc<InstanceShared>,
    spawned: Spawned,
    stop: &mut watch::Receiver<bool>,
) -> Cycle {
    let Spawned {
        mut child,
        binary,
        output,
    } = spawned;
    let pid = child.id();
    let group = child.process_group_id();
    shared.mutate(|snapshot| {
        snapshot.state = Some(ManagedWorkerState::Running);
        snapshot.pid = pid;
        snapshot.process_group = group;
        snapshot.spawn_binary = Some(binary);
        // A group now exists. Only a confirmed teardown clears this.
        snapshot.containment_unconfirmed = true;
    });

    let requested = tokio::select! {
        result = child.wait() => {
            if let Err(source) = result {
                let error = SupervisionError::Process {
                    name: config.name.clone(),
                    source,
                };
                tracing::error!(
                    worker = config.name.as_str(),
                    %error,
                    "managed worker could not be waited for"
                );
                shared.mutate(|snapshot| snapshot.last_error = Some(error.to_string()));
            }
            false
        }
        () = wait_for_stop(stop) => true,
    };

    // A stop signals the group FIRST — the leader is still unreaped there, so
    // the id is provably ours to aim at. After a natural exit the leader has
    // been reaped and `terminate` deliberately does nothing; the tree is judged
    // by end-of-file instead, which no recycled id can fake.
    let signalled = child.terminate(config.policy.stop_grace).await;
    let drained = output.drained(config.policy.stop_grace).await;
    let exit = child.exit_status().map(exit_record);
    record_teardown(config, shared, requested, exit, &signalled, drained);
    if requested {
        Cycle::StopHandled
    } else {
        Cycle::Exited
    }
}

/// Write the one truthful account of how this cycle ended.
///
/// `Stopped` is reachable only from a requested teardown whose tree was proven
/// gone, and `containment_unconfirmed` — the flag a stop is actually gated on —
/// is cleared only by that same proof. `drained` IS the proof: both output
/// streams reached end-of-file, which no process still holding a write end
/// could allow.
fn record_teardown(
    config: &InstanceConfig,
    shared: &Arc<InstanceShared>,
    requested: bool,
    exit: Option<ManagedWorkerExit>,
    signalled: &Result<(), aion_worker::ProcessGroupError>,
    drained: bool,
) {
    let failure = match signalled {
        Err(source) => Some(SupervisionError::StopIncomplete {
            name: config.name.clone(),
            detail: source.to_string(),
        }),
        Ok(()) if !drained => Some(SupervisionError::StopIncomplete {
            name: config.name.clone(),
            detail: format!(
                "the worker's output streams did not reach end-of-file within {:?}, so a \
                 descendant is still holding them open and the process tree cannot be \
                 declared gone",
                config.policy.stop_grace
            ),
        }),
        Ok(()) => None,
    };

    if let Some(error) = failure {
        tracing::error!(
            worker = config.name.as_str(),
            %error,
            "managed worker process tree could not be confirmed gone"
        );
        // Deliberately NOT `Stopped`, and `containment_unconfirmed` stays set:
        // an unproven teardown is exactly the case a stop must refuse to call
        // success.
        shared.set_terminal(ManagedWorkerState::Failed, Some(error.to_string()));
        return;
    }

    let ending = exit.map(|mut record| {
        record.requested = requested;
        record
    });
    shared.mutate(|snapshot| {
        snapshot.pid = None;
        snapshot.process_group = None;
        snapshot.containment_unconfirmed = false;
        if ending.is_some() {
            snapshot.last_exit = ending;
        }
        snapshot.state = Some(if requested {
            ManagedWorkerState::Stopped
        } else {
            // `Backoff` the moment the child is gone, not after the restart
            // decision: leaving `Running` here would report a live worker with
            // no pid, which is the exact shape of a status surface nobody can
            // trust.
            ManagedWorkerState::Backoff
        });
    });
    if requested {
        tracing::info!(
            worker = config.name.as_str(),
            "managed worker stopped; its process tree reached end-of-file"
        );
    } else {
        tracing::warn!(worker = config.name.as_str(), "managed worker exited");
    }
}

/// Decide whether to restart, and wait out the backoff if so.
///
/// Returns false when the loop must end: the operator no longer wants this
/// worker running, the record is gone, the crash-loop budget is spent, or a
/// stop arrived while the backoff was being waited out.
async fn restart_permitted(
    config: &InstanceConfig,
    shared: &Arc<InstanceShared>,
    window: &mut VecDeque<Instant>,
    restarts: &mut u32,
    stop: &mut watch::Receiver<bool>,
) -> bool {
    if *stop.borrow_and_update() {
        shared.set_terminal(ManagedWorkerState::Stopped, None);
        return false;
    }
    if !desired_still_running(config, shared).await {
        return false;
    }

    let now = Instant::now();
    let restart_window = config.policy.restart_window;
    window.retain(|at| now.duration_since(*at) < restart_window);
    let already = u32::try_from(window.len()).unwrap_or(u32::MAX);
    if already >= config.policy.max_restarts_per_window.get() {
        let detail = format!(
            "managed worker `{}` restarted {already} times within {restart_window:?} and has \
             spent its `max_restarts_per_window` budget; it will not be restarted again until an \
             operator starts it",
            config.name
        );
        tracing::error!(
            worker = config.name.as_str(),
            %detail,
            "managed worker is in a crash loop"
        );
        shared.set_terminal(ManagedWorkerState::Failed, Some(detail));
        return false;
    }

    let delay = config.policy.backoff_after(already);
    window.push_back(now);
    shared.mutate(|snapshot| {
        snapshot.state = Some(ManagedWorkerState::Backoff);
        snapshot.pid = None;
        snapshot.process_group = None;
    });
    let elapsed = tokio::select! {
        () = tokio::time::sleep(delay) => true,
        () = wait_for_stop(stop) => false,
    };
    if !elapsed {
        shared.set_terminal(ManagedWorkerState::Stopped, None);
        return false;
    }
    *restarts = restarts.saturating_add(1);
    let total = *restarts;
    shared.mutate(|snapshot| snapshot.restarts = total);
    true
}

/// Re-read durable intent. A store failure does NOT stop the fleet: an
/// unreadable record is a reason to keep the last known intent and say so, not
/// a reason to quietly let every managed worker die.
async fn desired_still_running(config: &InstanceConfig, shared: &Arc<InstanceShared>) -> bool {
    match config.store.get_worker_deployment(&config.name).await {
        Ok(Some(record)) => {
            if record.desired == DesiredState::Running {
                return true;
            }
            shared.set_terminal(ManagedWorkerState::Stopped, None);
            false
        }
        Ok(None) => {
            let detail = format!(
                "worker deployment `{}` was deleted while it was running; supervision stopped",
                config.name
            );
            tracing::warn!(
                worker = config.name.as_str(),
                %detail,
                "managed worker record removed"
            );
            shared.set_terminal(ManagedWorkerState::Stopped, Some(detail));
            false
        }
        Err(source) => {
            let error = SupervisionError::Store { source };
            tracing::error!(
                worker = config.name.as_str(),
                %error,
                "managed worker desired state could not be re-read; keeping the last known intent"
            );
            shared.mutate(|snapshot| snapshot.last_error = Some(error.to_string()));
            true
        }
    }
}

async fn wait_for_stop(stop: &mut watch::Receiver<bool>) {
    loop {
        if *stop.borrow_and_update() {
            return;
        }
        if stop.changed().await.is_err() {
            // The sender lives in the handle that owns this task, so a closed
            // channel means the handle is gone: nothing will ever start this
            // instance again, and stopping is the only correct reading.
            return;
        }
    }
}

fn exit_record(status: ExitStatus) -> ManagedWorkerExit {
    #[cfg(unix)]
    let signal = {
        use std::os::unix::process::ExitStatusExt;
        status.signal()
    };
    #[cfg(not(unix))]
    let signal = None;
    ManagedWorkerExit {
        at: chrono::Utc::now(),
        code: status.code(),
        signal,
        requested: false,
    }
}

/// A live instance and the switch that ends it.
#[derive(Debug)]
pub(super) struct InstanceHandle {
    shared: Arc<InstanceShared>,
    stop: watch::Sender<bool>,
    task: tokio::task::JoinHandle<()>,
}

impl InstanceHandle {
    /// Start supervising one deployment.
    pub(super) fn start(config: InstanceConfig) -> Self {
        let shared = Arc::new(InstanceShared::default());
        // Written HERE, synchronously, and not left to the task's first poll.
        // Between `tokio::spawn` and that poll the snapshot would carry no
        // state at all, and a reader arriving in that window would be told
        // something — the honest answer being `Starting`, since the instance
        // demonstrably exists. Reporting anything terminal there would be a
        // supervisor claiming to have given up on work it had not begun.
        shared.mutate(|snapshot| snapshot.state = Some(ManagedWorkerState::Starting));
        let (stop, receiver) = watch::channel(false);
        let task = tokio::spawn(run(config, Arc::clone(&shared), receiver));
        Self { shared, stop, task }
    }

    /// Read what this instance is doing.
    pub(super) fn snapshot(&self) -> Result<InstanceSnapshot, SupervisionError> {
        self.shared.snapshot()
    }

    /// Whether the supervision task has ended.
    pub(super) fn is_finished(&self) -> bool {
        self.task.is_finished()
    }

    /// Ask the instance to stop, and WAIT for it to prove that it did.
    ///
    /// No timeout is applied here and none is needed: the termination ladder is
    /// itself bounded by the operator's `stop_grace` (once for `SIGTERM`, once
    /// for `SIGKILL`) and always resolves — to an empty group, or to a typed
    /// failure. Imposing a second deadline on top would be an invented cap that
    /// could report "stopped" while the ladder was still running.
    ///
    /// The acceptance gate is the OPERATING SYSTEM's answer, not the state
    /// token: a stop succeeds exactly when no process group this instance
    /// created is still unaccounted for. That is deliberate in both
    /// directions — an instance that already gave up on a crash loop has no
    /// process and stops cleanly, while an instance whose termination ladder
    /// failed reports the failure no matter what its state token says. A gate
    /// reading the token instead would let bookkeeping stand in for the fact.
    ///
    /// # Errors
    ///
    /// Returns [`SupervisionError::TaskLost`] when the supervision task ended
    /// abnormally, and [`SupervisionError::StopIncomplete`] when a process
    /// group it created could not be proven empty.
    pub(super) async fn stop(self, name: &str) -> Result<(), SupervisionError> {
        // A send failure means every receiver is gone, which means the loop has
        // already ended; the join below reports what it ended as either way.
        let signalled = self.stop.send(true).is_ok();
        tracing::debug!(worker = name, signalled, "managed worker stop requested");
        if let Err(join) = self.task.await {
            return Err(SupervisionError::TaskLost {
                name: name.to_owned(),
                detail: join.to_string(),
            });
        }
        let snapshot = self.shared.snapshot()?;
        if snapshot.containment_unconfirmed {
            return Err(SupervisionError::StopIncomplete {
                name: name.to_owned(),
                detail: format!(
                    "the process group was not confirmed empty; supervision ended in state {} ({})",
                    snapshot.state.map_or("unknown", ManagedWorkerState::token),
                    snapshot
                        .last_error
                        .as_deref()
                        .unwrap_or("no failure recorded")
                ),
            });
        }
        Ok(())
    }
}
