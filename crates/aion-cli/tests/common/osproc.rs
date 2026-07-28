//! Shared OS-process harness for the LSUB-5-B liminal failover gates.
//!
//! These helpers boot real `aion server` OS processes and a real
//! `spike/liminal-fan-worker` OS process, observe readiness on REAL observables
//! (an HTTP health probe, a worker readiness file, server log lines), and reap
//! every spawned process so a test never leaks. They are shared by the
//! connect-half test and the kill-9 reconnect-to-survivor failover test so the
//! two gates exercise the SAME boot/observe/teardown path.
//!
//! ## Output discipline: every spawned process logs to a FILE
//!
//! Children are spawned through [`log_redirect`], which sends stdout AND stderr
//! to one file in the run's work dir. Two reasons, both learned from failures
//! this harness could not diagnose:
//!
//! - A failure must be READABLE. This module used to build its post-mortem by
//!   draining `child.stdout`/`child.stderr`, but the kill-9 gate redirects its
//!   nodes into log files, so those handles are `None` and the drain returned an
//!   empty string EVERY time. A node that died during boot was reported as
//!   `server exited during boot with exit status: 1; output:` with nothing after
//!   it, and the empty capture was read as "the node wrote nothing" when in fact
//!   the node's dying words were sitting in a file on disk. The drain is gone;
//!   failure paths read the log FILE via [`log_tail`].
//! - `Stdio::piped()` with nobody reading it is a hang waiting to happen: a
//!   chatty `RUST_LOG=info` server fills the pipe buffer and blocks on write.
//!
//! ## Preservation is unconditional, never env-gated
//!
//! Each gate runs inside a `tempfile::tempdir()` that is deleted on drop, taking
//! the logs with it. [`preserve_on_failure`] copies the work dir OUT on every
//! failure and names the destination in the error. It is unconditional by
//! design: the env-gated post-mortem hook it replaces did not fire for the run
//! that most needed it, and a specimen that cannot be read teaches nothing.

use std::fmt::Write as _;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::unix::process::ExitStatusExt as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use aion_package::{
    ActionContract, BeamModule, BeamSet, CURRENT_FORMAT_VERSION, DeclaredActivity, Manifest,
    ManifestVersion, PackageBuilder, PackageContract, WorkerContract,
};
use serde_json::json;

/// Error type used across the OS-process harness and its tests.
pub type TestError = Box<dyn std::error::Error>;

/// Deadline for a spawned server to answer its liveness probe.
const BOOT_DEADLINE: Duration = Duration::from_secs(90);
/// Deadline for a reaped child to exit.
const EXIT_DEADLINE: Duration = Duration::from_secs(30);
/// How many trailing log lines a failure quotes inline (the whole file is always
/// preserved, so the inline tail only has to orient the reader).
const TAIL_LINES: usize = 50;

/// The `collect_four` fixture module (same beam the in-process liminal tests load).
const OUTBOX_MODULE: &str = "aion_outbox_fixture";
const OUTBOX_BEAM: &[u8] =
    include_bytes!("../../../aion-server/tests/fixtures/aion_outbox_fixture.beam");
const OUTBOX_SOURCE: &[u8] =
    include_bytes!("../../../aion-server/tests/fixtures/aion_outbox_fixture.erl");

/// The task queue `spike/liminal-fan-worker` polls (`WorkerConfig::task_queue`),
/// and therefore the queue this package's activities must be scoped onto.
const FIXTURE_TASK_QUEUE: &str = "default";
/// The activity types `collect_four` schedules, one per fan-out ordinal — the
/// same four names `spike/liminal-fan-worker` registers handlers for.
const FAN_ACTIVITY_TYPES: [&str; 4] = ["fan:0", "fan:1", "fan:2", "fan:3"];

/// Reserve a loopback port by binding to 0 and dropping the listener.
pub fn reserve_port() -> Result<u16, TestError> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr()?.port())
}

/// Write the `collect_four` package to disk exactly as an operator-supplied
/// `workflow_packages` archive, so the spawned server loads it through its real
/// boot path.
pub fn write_package_archive(dir: &Path) -> Result<PathBuf, TestError> {
    let beams = BeamSet::new(vec![BeamModule::new(OUTBOX_MODULE, OUTBOX_BEAM)])?;
    let manifest = Manifest {
        entry_module: OUTBOX_MODULE.to_owned(),
        entry_function: "collect_four".to_owned(),
        input_schema: json!({ "type": "object" }),
        output_schema: json!({}),
        timeout: Some(Duration::from_secs(30)),
        // The four ordinals `collect_four` actually fans out. This manifest used
        // to name one invented activity, `fixture_activity`, that the fixture
        // never schedules and no worker ever served.
        activities: FAN_ACTIVITY_TYPES
            .iter()
            .map(|activity_type| DeclaredActivity {
                activity_type: (*activity_type).to_owned(),
            })
            .collect(),
        version: ManifestVersion::new("stamped-by-builder"),
        format_version: CURRENT_FORMAT_VERSION,
        additional_workflows: Vec::new(),
    };
    let contract = fixture_contract(&manifest)?;
    let archive =
        PackageBuilder::with_source(manifest, beams, [(OUTBOX_MODULE, OUTBOX_SOURCE.to_vec())])
            .with_contract(contract)
            .write_to_bytes()?;
    let path = dir.join("collect_four.aion");
    std::fs::write(&path, &archive)?;
    Ok(path)
}

/// The fixture's queue-scoped `.v4` contract: the four `fan:N` activities
/// `collect_four` schedules, declared on the queue its worker actually polls.
///
/// Why the archive cannot just carry the manifest-derived record: by design
/// `PackageContract::from_manifest` "never invents a queue", so a manifest's
/// bare activity names land in `unscoped_activities` — and under the server's
/// queue-routed posture an unscoped catalog is a terminal `NO_QUEUE_DECLARATION`
/// at start admission (`aion::lifecycle::start_admission`). That refusal is
/// EARNED: an unserved queue would otherwise wait silently forever. So the
/// derived record is amended rather than bypassed — the same four names move out
/// of `unscoped_activities` and onto the queue that serves them — and the
/// package still loads through the server's real boot path with the `.v4`
/// identity `PackageBuilder` stamps over this exact contract.
///
/// The action schemas come from the SAME generator the worker's typed registry
/// uses, for the SAME Rust types: `collect_four` passes each member the JSON
/// string `"in"` and the handler returns a `String`. Deriving both sides from
/// `activity_descriptor::<String, String>` means the package's declaration and
/// the worker's advertisement cannot drift apart, so registration admission
/// (`WORKER_CONTRACT_MISMATCH`) compares two schemas with one source.
fn fixture_contract(manifest: &Manifest) -> Result<PackageContract, TestError> {
    let mut actions = Vec::with_capacity(FAN_ACTIVITY_TYPES.len());
    for activity_type in FAN_ACTIVITY_TYPES {
        let descriptor = aion_worker::activity_descriptor::<String, String>(activity_type)?;
        actions.push(ActionContract {
            name: descriptor.name,
            input_schema: descriptor.input_schema,
            output_schema: descriptor.output_schema,
            node: None,
            timeout: None,
            retry: None,
            advisory: false,
        });
    }
    let mut contract = PackageContract::from_manifest(manifest);
    contract.workers = vec![WorkerContract {
        task_queue: FIXTURE_TASK_QUEUE.to_owned(),
        actions,
    }];
    contract.unscoped_activities.clear();
    Ok(contract)
}

/// Create `path` and return the `(stdout, stderr)` redirection pair for a child,
/// so everything it prints lands in ONE file the harness can read back after a
/// failure. Never spawn a harness child with `Stdio::piped()`: nothing drains it
/// while the run is in flight (see the module docs).
pub fn log_redirect(path: &Path) -> Result<(Stdio, Stdio), TestError> {
    let file = std::fs::File::create(path)?;
    let duplicate = file.try_clone()?;
    Ok((Stdio::from(file), Stdio::from(duplicate)))
}

/// The last [`TAIL_LINES`] lines of `path`, or an EXPLICIT note saying why there
/// is nothing to show. An empty quotation must never again be mistaken for a
/// process that wrote nothing.
pub fn log_tail(path: &Path) -> String {
    let contents = match std::fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) => return format!("<no readable log at {}: {error}>", path.display()),
    };
    let lines: Vec<&str> = contents.lines().collect();
    if lines.is_empty() {
        return format!("<{} exists but is empty>", path.display());
    }
    let skipped = lines.len().saturating_sub(TAIL_LINES);
    let mut tail = String::new();
    if skipped > 0 {
        let _ = writeln!(tail, "<{skipped} earlier lines omitted>");
    }
    for line in &lines[skipped..] {
        let _ = writeln!(tail, "{line}");
    }
    tail
}

/// How a child ended, with any SIGNAL called out: `kill -9` and `exit 1` are
/// different diagnoses and a boot post-mortem must not blur them.
pub fn describe_exit(status: ExitStatus) -> String {
    if let Some(signal) = status.signal() {
        return format!("killed by signal {signal}");
    }
    match status.code() {
        Some(code) => format!("exit status {code}, no signal"),
        None => format!("neither an exit code nor a signal ({status})"),
    }
}

/// One liveness poll's outcome. Distinguishes "the port is not listening yet"
/// from "the port answered", so a boot failure can say whether the server died
/// BEFORE or AFTER it bound its HTTP listener.
enum Probe {
    /// The port refused the connection: the server has not bound it (yet).
    NotBound,
    /// The port accepted a connection but the exchange did not complete.
    BoundNoResponse,
    /// The server answered; carries the raw HTTP response text.
    Answered(String),
}

/// Issue a raw `GET /health/live` and classify what came back.
fn probe_live(http_port: u16) -> Probe {
    let Ok(mut stream) =
        TcpStream::connect_timeout(&([127, 0, 0, 1], http_port).into(), Duration::from_secs(1))
    else {
        return Probe::NotBound;
    };
    if stream
        .write_all(b"GET /health/live HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .is_err()
    {
        return Probe::BoundNoResponse;
    }
    let mut response = String::new();
    if stream.read_to_string(&mut response).is_err() {
        return Probe::BoundNoResponse;
    }
    Probe::Answered(response)
}

/// Block until the server answers `GET /health/live` with 200 or the boot
/// deadline elapses. On failure the error carries the exit status AND signal,
/// whether the server had bound `http_port` before it died, and the tail of
/// `log_path` — the file the caller redirected the child's output into.
pub fn wait_for_liveness(
    child: &mut Child,
    http_port: u16,
    log_path: &Path,
) -> Result<(), TestError> {
    let started = Instant::now();
    let mut bound = false;
    loop {
        if let Some(status) = child.try_wait()? {
            let phase = if bound { "AFTER" } else { "BEFORE" };
            return Err(format!(
                "server exited during boot {phase} binding 127.0.0.1:{http_port} \
                 ({exit}); last {TAIL_LINES} lines of {log}:\n{tail}",
                exit = describe_exit(status),
                log = log_path.display(),
                tail = log_tail(log_path),
            )
            .into());
        }
        match probe_live(http_port) {
            Probe::NotBound => {}
            Probe::BoundNoResponse => bound = true,
            Probe::Answered(response) => {
                if response.starts_with("HTTP/1.1 200") {
                    return Ok(());
                }
                return Err(format!("liveness probe must answer 200: {response}").into());
            }
        }
        if started.elapsed() > BOOT_DEADLINE {
            let phase = if bound {
                "it had bound the port"
            } else {
                "it never bound the port"
            };
            return Err(format!(
                "server did not answer /health/live within {BOOT_DEADLINE:?} and {phase}; \
                 last {TAIL_LINES} lines of {log}:\n{tail}",
                log = log_path.display(),
                tail = log_tail(log_path),
            )
            .into());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Copy the failing run's work dir somewhere that OUTLIVES the caller's
/// `tempfile::tempdir()` and fold the destination into `error`. Unconditional by
/// design — see the module docs.
pub fn preserve_on_failure(work_dir: &Path, label: &str, error: &TestError) -> TestError {
    match preserve_work_dir(work_dir, label) {
        Ok((dest, copied)) => format!(
            "{error}\n\npreserved this run's files in {dest} ({files})",
            dest = dest.display(),
            files = if copied.is_empty() {
                "no files found".to_owned()
            } else {
                copied.join(", ")
            },
        )
        .into(),
        Err(copy_error) => format!(
            "{error}\n\n<could not preserve the run's logs from {dir}: {copy_error}>",
            dir = work_dir.display(),
        )
        .into(),
    }
}

/// Copy every FILE in `work_dir` (node logs, worker log, configs — the store
/// data dirs are directories and stay behind) into a fresh directory under the
/// cargo target dir. Returns where they landed and what was copied.
fn preserve_work_dir(work_dir: &Path, label: &str) -> Result<(PathBuf, Vec<String>), TestError> {
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
    let dest = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("failover-logs")
        .join(format!("{label}-{pid}-{stamp}", pid = std::process::id()));
    std::fs::create_dir_all(&dest)?;
    let mut copied: Vec<String> = Vec::new();
    for entry in std::fs::read_dir(work_dir)? {
        let entry = entry?;
        if entry.path().is_file() {
            std::fs::copy(entry.path(), dest.join(entry.file_name()))?;
            copied.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    copied.sort();
    Ok((dest, copied))
}

/// Poll `predicate` until it returns true or `deadline` elapses; returns whether
/// it became true. Used for process/log/store observables (never a bare sleep).
pub fn wait_until<F: FnMut() -> bool>(deadline: Duration, mut predicate: F) -> bool {
    let started = Instant::now();
    while started.elapsed() < deadline {
        if predicate() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

/// Kill a child and wait for it to reap, so the test never leaks a process.
pub fn reap(mut child: Child) {
    let _: Result<(), _> = child.kill();
    let _reaped = wait_until(EXIT_DEADLINE, || matches!(child.try_wait(), Ok(Some(_))));
}

/// Build the standalone liminal worker binary (it is NOT a workspace member, so
/// `CARGO_BIN_EXE_*` does not cover it — build it the way the spike script does).
pub fn build_worker_binary() -> Result<PathBuf, TestError> {
    let worker_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("spike")
        .join("liminal-fan-worker");
    let status = Command::new(env!("CARGO"))
        .args(["build"])
        .current_dir(&worker_dir)
        .status()?;
    if !status.success() {
        return Err("failed to build spike/liminal-fan-worker".into());
    }
    let binary = worker_dir
        .join("target")
        .join("debug")
        .join("liminal-fan-worker");
    if !binary.exists() {
        return Err(format!("worker binary not found at {}", binary.display()).into());
    }
    Ok(binary)
}

/// Guards on the harness's own diagnostics. These are the regression pins for
/// the blindness this module used to have: a boot death that reported
/// `output:` and nothing else, and a post-mortem that deleted itself with the
/// tempdir. They spawn `/bin/sh`, not a cluster, so they are fast and not
/// `#[ignore]`d — the harness's ability to REPORT is itself a gate.
#[cfg(test)]
mod harness_diagnostics {
    use super::{TestError, log_redirect, log_tail, preserve_on_failure, reserve_port};
    use std::path::Path;
    use std::process::Command;

    /// Spawn a child that prints to stdout and stderr and exits non-zero, with
    /// its output redirected to `log_path` exactly as the gates redirect theirs.
    fn spawn_doomed_child(log_path: &Path) -> Result<std::process::Child, TestError> {
        let (stdout, stderr) = log_redirect(log_path)?;
        Ok(Command::new("/bin/sh")
            .args(["-c", "echo 'config parse failed'; echo 'boom' >&2; exit 3"])
            .stdout(stdout)
            .stderr(stderr)
            .spawn()?)
    }

    #[test]
    fn a_boot_death_reports_the_exit_status_the_bind_phase_and_the_log() -> Result<(), TestError> {
        let temp = tempfile::tempdir()?;
        let log_path = temp.path().join("node0.log");
        let mut child = spawn_doomed_child(&log_path)?;
        let http_port = reserve_port()?;

        let Err(error) = super::wait_for_liveness(&mut child, http_port, &log_path) else {
            return Err("a child that exits 3 must not be reported as live".into());
        };
        let report = error.to_string();

        // The three things specimen 3's `output:` could not say.
        assert!(
            report.contains("exit status 3"),
            "the exit status must be named: {report}"
        );
        assert!(
            report.contains("BEFORE binding"),
            "the bind phase must be named: {report}"
        );
        assert!(
            report.contains("config parse failed") && report.contains("boom"),
            "the child's stdout AND stderr must be quoted back: {report}"
        );
        Ok(())
    }

    #[test]
    fn an_unreadable_or_empty_log_says_so_instead_of_quoting_nothing() -> Result<(), TestError> {
        let temp = tempfile::tempdir()?;
        let missing = temp.path().join("never-written.log");
        assert!(
            log_tail(&missing).contains("no readable log"),
            "a missing log must be named as missing, not quoted as silence"
        );

        let empty = temp.path().join("empty.log");
        std::fs::write(&empty, "")?;
        assert!(
            log_tail(&empty).contains("exists but is empty"),
            "an empty log must be named as empty, not quoted as silence"
        );
        Ok(())
    }

    #[test]
    fn a_failure_preserves_the_work_dir_beyond_the_tempdir() -> Result<(), TestError> {
        let temp = tempfile::tempdir()?;
        std::fs::write(temp.path().join("node0.log"), "the node's dying words\n")?;
        let preserved =
            preserve_on_failure(temp.path(), "harness-diagnostics", &TestError::from("red"));
        let report = preserved.to_string();
        assert!(
            report.contains("node0.log"),
            "the preserved files must be named: {report}"
        );

        // Drop the tempdir: the copies must outlive it, which is the whole point.
        let dir = temp.path().to_path_buf();
        drop(temp);
        assert!(!dir.exists(), "the tempdir must really be gone");
        let dest = report
            .rsplit_once("preserved this run's files in ")
            .and_then(|(_, tail)| tail.split_once(" ("))
            .map(|(path, _)| Path::new(path).to_path_buf())
            .ok_or_else(|| TestError::from("the report must name the destination directory"))?;
        assert!(
            dest.join("node0.log").is_file(),
            "the preserved copy must still exist at {}",
            dest.display()
        );
        Ok(())
    }
}
