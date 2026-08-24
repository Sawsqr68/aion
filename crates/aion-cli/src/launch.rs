//! The bare `aion` launcher: start the workflow server in the background,
//! wait until it answers its own liveness probe, report in plain language
//! where everything lives, and open the ops console in the browser.
//!
//! This is the newcomer's whole path: install from crates.io, type `aion`,
//! get a running server and a console — the server itself scaffolds
//! `<AION_HOME>/config.toml` on a first boot that finds no config (#180),
//! and this launcher reports that in words rather than JSON. `--foreground`
//! keeps the server in this terminal instead; `--no-open` skips the browser.

use std::net::SocketAddr;
use std::path::Path;
use std::process::{ExitCode, Stdio};
use std::time::Instant;

use aion_server::config::{
    CliOverrides, FIRST_RUN_CONFIG, ServerConfig, StoreBackend, StoreConfig, aion_home,
};

use crate::console::{self, HealthProbe};

/// Interval between liveness probes while waiting for the spawned server.
const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(200);

/// How many trailing server-log lines a failure report quotes.
const LOG_TAIL_LINES: usize = 40;

/// How many bytes are read from the end of the (append-forever) server log to
/// find those lines. 64 KiB comfortably covers 40 JSON log lines.
const LOG_TAIL_WINDOW_BYTES: u64 = 64 * 1024;

/// A launcher failure: what to tell the operator and which exit code it
/// carries (2 for configuration/refusal; a spawned server's own exit code,
/// or 1, for a server that failed).
struct LaunchFailure {
    /// Exit code for the process.
    code: u8,
    /// Plain-language report, printed to stderr.
    message: String,
}

impl LaunchFailure {
    /// A configuration or refusal failure (exit 2, matching `aion server`'s
    /// config exit code).
    fn refusal(message: String) -> Self {
        Self { code: 2, message }
    }

    /// A runtime failure of the spawned server. When the server itself
    /// exited, its own code is propagated (so config-refusal 2 stays
    /// distinguishable from a crash); otherwise 1.
    fn runtime(status: Option<std::process::ExitStatus>, message: String) -> Self {
        let code = status
            .and_then(|status| status.code())
            .and_then(|code| u8::try_from(code).ok())
            .filter(|&code| code != 0)
            .unwrap_or(1);
        Self { code, message }
    }
}

/// Run the bare `aion` launcher. `--foreground` delegates to the ordinary
/// in-process server path with browser-open semantics; the default spawns
/// `aion server` detached and reports once it is live.
pub async fn run(foreground: bool, no_open: bool) -> ExitCode {
    if foreground {
        return run_foreground(no_open).await;
    }
    match run_background(no_open).await {
        Ok(code) => code,
        Err(failure) => {
            eprintln!("aion: {}", failure.message);
            ExitCode::from(failure.code)
        }
    }
}

/// `aion --foreground`: the ordinary server path, in this terminal, with the
/// console opened once the listener is live (unless `--no-open`).
async fn run_foreground(no_open: bool) -> ExitCode {
    crate::harness::announce_composed_harness();
    if !no_open {
        crate::server::spawn_browser_open(&CliOverrides::default());
    }
    aion_server::run(CliOverrides::default()).await
}

/// The background path: resolve the address the server will bind, refuse or
/// short-circuit on what already holds it, spawn, wait for liveness, report.
async fn run_background(no_open: bool) -> Result<ExitCode, LaunchFailure> {
    // Resolve the effective config exactly the way the spawned server will
    // (discovery + environment overlay + defaults) so launcher and server
    // cannot disagree about the address. A first run with no config resolves
    // the built-in defaults, and the config the server then scaffolds (#180)
    // carries those same addresses — a coincidence pinned by aion-server's
    // `the_template_addresses_equal_the_built_in_defaults`.
    let overrides = CliOverrides::default();
    let config = ServerConfig::load(&overrides).map_err(|error| {
        LaunchFailure::refusal(format!(
            "could not resolve the server configuration: {error}"
        ))
    })?;
    let home = aion_home().map_err(|error| {
        LaunchFailure::refusal(format!("could not resolve the Aion home: {error}"))
    })?;
    let (store, runtime) = config.into_parts();
    let address = runtime.listen.http;
    let url = console::served_url(address);

    match console::probe_health(address).await {
        HealthProbe::Live => {
            println!("aion is already running.");
            println!("console: {url}");
            open_console(no_open, &url);
            return Ok(ExitCode::SUCCESS);
        }
        HealthProbe::NotAion(answer) => {
            return Err(LaunchFailure::refusal(not_aion_report(
                address,
                answer.as_deref(),
            )));
        }
        HealthProbe::Down => {}
    }

    // Remember which config layer exists BEFORE the spawn, so the report can
    // say whether the server's first boot scaffolded one (#180). `--config`
    // is not offered on the bare launcher, so the layers are `./aion.toml`
    // then `<home>/config.toml`.
    let working_dir = std::env::current_dir().map_err(|error| {
        LaunchFailure::refusal(format!("could not resolve the current directory: {error}"))
    })?;
    let project_config = working_dir.join("aion.toml");
    let home_config = home.path.join("config.toml");
    let pre_existing = [&project_config, &home_config]
        .into_iter()
        .find(|path| path.exists())
        .cloned();

    let log_path = home.path.join("server.log");
    let mut child = spawn_server(&home.path, &log_path)?;

    // Wait for liveness, watching the child so an early death is reported
    // with its own words (the log tail) instead of a silent timeout.
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                // Two racing launchers: the loser's child exits on the bind
                // conflict AFTER the winner's server went live. That is not a
                // failure of aion — probe again and report the survivor
                // rather than quoting a healthy server's log as a corpse.
                if matches!(console::probe_health(address).await, HealthProbe::Live) {
                    println!("aion is already running (another launch won the race).");
                    println!("console: {url}");
                    open_console(no_open, &url);
                    return Ok(ExitCode::SUCCESS);
                }
                return Err(LaunchFailure::runtime(
                    Some(status),
                    format!(
                        "the server exited during startup ({status}); its log ends with:\n{}\nfull log: {}",
                        log_tail(&log_path),
                        log_path.display()
                    ),
                ));
            }
            Ok(None) => {}
            Err(error) => {
                return Err(LaunchFailure::runtime(
                    None,
                    format!(
                        "could not watch the server process: {error}; its log is {}",
                        log_path.display()
                    ),
                ));
            }
        }
        if matches!(console::probe_health(address).await, HealthProbe::Live) {
            break;
        }
        if started.elapsed() >= console::LIVE_BUDGET {
            // Still starting is not proven dead: leave the process running
            // rather than destroy a boot that may be seconds from ready, and
            // hand the operator everything needed to watch or stop it.
            return Err(LaunchFailure::runtime(
                None,
                format!(
                    "the server has not answered {url}health/live within {}s; it is still \
                     running as process {pid} — watch its log ({log}) or stop it with `aion server stop`. \
                     The log ends with:\n{tail}",
                    console::LIVE_BUDGET.as_secs(),
                    pid = child.id(),
                    log = log_path.display(),
                    tail = log_tail(&log_path)
                ),
            ));
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }

    let config_line = config_report_line(pre_existing.as_deref(), &project_config, &home_config);
    let pid_line = pid_report_line(&home.path, child.id());
    print!(
        "{}",
        ready_report(&store, &config_line, &url, &log_path, &pid_line)
    );
    open_console(no_open, &url);
    Ok(ExitCode::SUCCESS)
}

/// Spawn `aion server` (this same executable) detached: its own process
/// group, stdin closed, stdout+stderr appended owner-only to
/// `<home>/server.log`.
///
/// Deliberately NOT the worker SDK's contained-child shape: kill-on-drop
/// would take the server down the moment this launcher exits, which is the
/// opposite of the point.
fn spawn_server(home: &Path, log_path: &Path) -> Result<std::process::Child, LaunchFailure> {
    provision_home(home)?;
    let mut log_options = std::fs::OpenOptions::new();
    log_options.create(true).append(true);
    // The log carries whatever the server says — workflow names, paths,
    // addresses — so it gets the same owner-only mode the server gives the
    // config it scaffolds. An existing log's mode belongs to the operator.
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        log_options.mode(0o600);
    }
    let log_out = log_options.open(log_path).map_err(|error| {
        LaunchFailure::refusal(format!(
            "could not open the server log {}: {error}",
            log_path.display()
        ))
    })?;
    let log_err = log_out.try_clone().map_err(|error| {
        LaunchFailure::refusal(format!(
            "could not open the server log {}: {error}",
            log_path.display()
        ))
    })?;
    let executable = std::env::current_exe().map_err(|error| {
        LaunchFailure::refusal(format!("could not resolve the aion executable: {error}"))
    })?;
    let mut command = std::process::Command::new(executable);
    command
        .arg("server")
        .stdin(Stdio::null())
        .stdout(log_out)
        .stderr(log_err);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        // Its own process group: terminal signals aimed at the launcher (or
        // the shell that ran it) never reach the server.
        command.process_group(0);
    }
    command.spawn().map_err(|error| {
        LaunchFailure::refusal(format!("could not start the server process: {error}"))
    })
}

/// Provision the Aion home the way the server's own config scaffold does
/// (`ConfinedDir::open_or_create`): create it owner-only, refuse a symlink,
/// and tighten a permissive mode on a directory that already exists — all
/// BEFORE anything (the log file) is written beneath it.
fn provision_home(home: &Path) -> Result<(), LaunchFailure> {
    match std::fs::symlink_metadata(home) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(LaunchFailure::refusal(format!(
                "the Aion home {} is a symlink; the server refuses symlinked state roots — \
                 point AION_HOME at the real directory instead",
                home.display()
            )));
        }
        Ok(metadata) if !metadata.is_dir() => {
            return Err(LaunchFailure::refusal(format!(
                "the Aion home {} exists but is not a directory",
                home.display()
            )));
        }
        Ok(_) => {
            #[cfg(unix)]
            tighten_home(home)?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            std::fs::create_dir_all(home).map_err(|error| {
                LaunchFailure::refusal(format!(
                    "could not create the Aion home {}: {error}",
                    home.display()
                ))
            })?;
            #[cfg(unix)]
            tighten_home(home)?;
        }
        Err(error) => {
            return Err(LaunchFailure::refusal(format!(
                "could not inspect the Aion home {}: {error}",
                home.display()
            )));
        }
    }
    Ok(())
}

/// Owner-only, matching the server's own sensitive-root provisioning (which
/// tightens a permissive owned directory rather than refusing it).
#[cfg(unix)]
fn tighten_home(home: &Path) -> Result<(), LaunchFailure> {
    use std::os::unix::fs::PermissionsExt as _;
    let mode = std::fs::metadata(home)
        .map_err(|error| {
            LaunchFailure::refusal(format!(
                "could not inspect the Aion home {}: {error}",
                home.display()
            ))
        })?
        .permissions()
        .mode();
    if mode & 0o077 != 0 {
        std::fs::set_permissions(home, std::fs::Permissions::from_mode(0o700)).map_err(
            |error| {
                LaunchFailure::refusal(format!(
                    "could not set owner-only permissions on {}: {error}",
                    home.display()
                ))
            },
        )?;
    }
    Ok(())
}

/// The config line of the ready report. "Created" is claimed only when the
/// file did not exist before the spawn AND its bytes now equal the server's
/// embedded first-run template — anything else is reported as what was found.
fn config_report_line(
    pre_existing: Option<&Path>,
    project_config: &Path,
    home_config: &Path,
) -> String {
    if let Some(path) = pre_existing {
        return format!("config: {}", path.display());
    }
    match std::fs::read(home_config) {
        Ok(bytes) if bytes == FIRST_RUN_CONFIG.as_bytes() => format!(
            "config: created {} on first start — comments inside explain every surface it enables",
            home_config.display()
        ),
        Ok(_) => format!(
            "config: {} (appeared during startup; not the stock first-run template)",
            home_config.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => format!(
            "config: built-in defaults (no {} or {})",
            project_config.display(),
            home_config.display()
        ),
        Err(error) => format!(
            "config: {} (could not read it: {error})",
            home_config.display()
        ),
    }
}

/// The pid line of the ready report. `aion server stop` addresses the
/// server the home's pid file RECORDS — which is only the server just
/// launched when the record names its pid. A launch into a home whose
/// record another live server holds boots UNCLAIMED, and promising the verb
/// then would point the operator's stop at the WRONG process; the line says
/// what actually addresses each server instead.
fn pid_report_line(home: &Path, launched_pid: u32) -> String {
    match aion_server::control::pid_file::read(home) {
        Ok(Some(record)) if record.pid == launched_pid => {
            format!("server process id: {launched_pid} (stop it with: aion server stop)")
        }
        Ok(Some(record)) => format!(
            "server process id: {launched_pid} — NOTE: this home's pid file is held by \
             another live server (pid {}), so `aion server stop` stops THAT server, \
             not this one. This launch runs unclaimed; stop it by pid (kill \
             {launched_pid}), or give each server its own AION_HOME",
            record.pid
        ),
        Ok(None) => format!(
            "server process id: {launched_pid} — NOTE: no pid record appeared under \
             this home, so `aion server stop` cannot address it; stop it by pid \
             (kill {launched_pid}) and check the server log for the claim failure"
        ),
        Err(error) => format!(
            "server process id: {launched_pid} — NOTE: the home's pid record could \
             not be read ({error}), so whether `aion server stop` addresses this \
             server is unverified"
        ),
    }
}

/// The plain-language success report Tom's ask names: where the config, data,
/// console, and log live, and how to stop the server. Ends with a newline.
fn ready_report(
    store: &StoreConfig,
    config_line: &str,
    url: &str,
    log_path: &Path,
    pid_line: &str,
) -> String {
    let data_line = match store.backend {
        StoreBackend::Haematite => match store.data_dir.as_deref() {
            Some(data_dir) => format!("data: {data_dir}"),
            None => "data: haematite store (directory resolved by the server)".to_owned(),
        },
        StoreBackend::Memory => "data: in-memory — state does not survive a stop".to_owned(),
    };
    format!(
        "aion is up.\n{config_line}\n{data_line}\nconsole: {url}\nlog: {log}\n{pid_line}\n",
        log = log_path.display()
    )
}

/// Open the console in the platform browser unless `--no-open` said not to.
fn open_console(no_open: bool, url: &str) {
    if no_open {
        return;
    }
    println!("opening your browser…");
    if let Err(error) = console::open_browser(url) {
        // Best-effort by design: the server is up either way.
        println!("could not open a browser ({error}); open {url} yourself");
    }
}

/// The refusal for a port that answered, but not as an Aion server.
fn not_aion_report(address: SocketAddr, answer: Option<&str>) -> String {
    let answered = match answer {
        Some(line) => format!("it answered `{line}`"),
        None => "it accepted the connection but did not answer the liveness probe".to_owned(),
    };
    format!(
        "port {port} at {address} is already in use by something that is not an Aion server \
         ({answered}). Stop that process, or point Aion elsewhere: set `[server] listen_address` \
         in the config, or AION_SERVER_LISTEN_ADDRESS.",
        port = address.port()
    )
}

/// The last [`LOG_TAIL_LINES`] lines of the server log for a failure report,
/// read from a bounded window at the END of the file — the log is
/// append-forever, so an unbounded read would grow with server age. A log
/// that cannot be read is itself reported, never swallowed.
fn log_tail(path: &Path) -> String {
    match read_tail_window(path) {
        Ok(text) => {
            let lines: Vec<&str> = text.lines().collect();
            let start = lines.len().saturating_sub(LOG_TAIL_LINES);
            let tail = lines[start..].join("\n");
            if tail.is_empty() {
                "(the log is empty)".to_owned()
            } else {
                tail
            }
        }
        Err(error) => format!("(could not read the log: {error})"),
    }
}

/// Read at most [`LOG_TAIL_WINDOW_BYTES`] from the end of the file.
fn read_tail_window(path: &Path) -> std::io::Result<String> {
    use std::io::{Read as _, Seek as _};
    let mut file = std::fs::File::open(path)?;
    let length = file.metadata()?.len();
    file.seek(std::io::SeekFrom::Start(
        length.saturating_sub(LOG_TAIL_WINDOW_BYTES),
    ))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestError = Box<dyn std::error::Error>;

    #[test]
    fn the_not_aion_refusal_names_port_and_answer() -> Result<(), std::net::AddrParseError> {
        let address: SocketAddr = "127.0.0.1:8080".parse()?;
        let quoted = not_aion_report(address, Some("HTTP/1.1 404 Not Found"));
        assert!(quoted.contains("port 8080"));
        assert!(quoted.contains("not an Aion server"));
        assert!(quoted.contains("`HTTP/1.1 404 Not Found`"));
        assert!(quoted.contains("AION_SERVER_LISTEN_ADDRESS"));

        let silent = not_aion_report(address, None);
        assert!(silent.contains("port 8080"));
        assert!(silent.contains("did not answer the liveness probe"));
        Ok(())
    }

    #[test]
    fn the_config_line_claims_created_only_for_the_template_bytes() -> Result<(), TestError> {
        let scratch = tempfile::tempdir()?;
        let project_config = scratch.path().join("aion.toml");
        let home_config = scratch.path().join("config.toml");

        // A pre-existing config is reported as found, whatever the home holds.
        let line = config_report_line(Some(&project_config), &project_config, &home_config);
        assert_eq!(line, format!("config: {}", project_config.display()));

        // Absent before AND after: built-in defaults.
        let line = config_report_line(None, &project_config, &home_config);
        assert!(line.starts_with("config: built-in defaults"));

        // Appeared with exactly the template bytes: created by the server.
        std::fs::write(&home_config, FIRST_RUN_CONFIG)?;
        let line = config_report_line(None, &project_config, &home_config);
        assert!(
            line.contains(&format!("created {}", home_config.display())),
            "template bytes must report as created, got: {line}"
        );

        // Appeared with OTHER bytes: found, never claimed as created.
        std::fs::write(&home_config, "# operator's own\n")?;
        let line = config_report_line(None, &project_config, &home_config);
        assert!(
            !line.contains("created") && line.contains("appeared during startup"),
            "foreign bytes must not claim creation, got: {line}"
        );
        Ok(())
    }

    #[test]
    fn the_ready_report_names_every_location_per_backend() {
        let log_path = Path::new("/tmp/aion-home/server.log");
        let mut store = StoreConfig::default();
        store.backend = StoreBackend::Haematite;
        store.data_dir = Some("/data/haematite".to_owned());
        let pid_line = "server process id: 42 (stop it with: aion server stop)";
        let report = ready_report(
            &store,
            "config: X",
            "http://127.0.0.1:8080/",
            log_path,
            pid_line,
        );
        assert!(report.starts_with("aion is up.\n"));
        assert!(report.contains("config: X\n"));
        assert!(report.contains("data: /data/haematite\n"));
        assert!(report.contains("console: http://127.0.0.1:8080/\n"));
        assert!(report.contains("log: /tmp/aion-home/server.log\n"));
        assert!(report.contains("server process id: 42 (stop it with: aion server stop)\n"));

        store.backend = StoreBackend::Memory;
        let report = ready_report(&store, "config: X", "u", log_path, pid_line);
        assert!(report.contains("data: in-memory — state does not survive a stop\n"));
    }

    /// The stop-verb promise is made only when the home's record names the
    /// launched pid: a record held by another live server (the unclaimed
    /// launch) or an absent record must NOT point the operator's stop at the
    /// wrong process.
    #[test]
    fn the_pid_line_promises_the_stop_verb_only_for_the_recorded_pid() -> Result<(), TestError> {
        let home = tempfile::tempdir()?;
        // No record at all: the verb is not promised.
        let line = pid_report_line(home.path(), 42);
        assert!(
            line.contains("no pid record appeared") && !line.contains("(stop it with:"),
            "an absent record must withhold the promise, got: {line}"
        );

        // The record names the launched pid: the promise stands.
        let me = aion_server::control::incarnation::self_identity()?;
        let record = aion_server::control::PidRecord {
            pid: me.pid,
            started_at_unix_secs: me.started_at_unix_secs,
            binary_sha256: me.binary_sha256,
            version: "0.0.0-test".to_owned(),
            commit: "test".to_owned(),
            http_address: "127.0.0.1:8080".parse()?,
            grpc_address: "127.0.0.1:50051".parse()?,
            drain_timeout_seconds: 30,
        };
        let run_dir = home.path().join("run");
        std::fs::create_dir_all(&run_dir)?;
        std::fs::write(
            run_dir.join("aion-server.pid"),
            serde_json::to_string(&record)?,
        )?;
        let line = pid_report_line(home.path(), me.pid);
        assert!(
            line.contains("(stop it with: aion server stop)"),
            "the recorded pid earns the promise, got: {line}"
        );

        // The record names a DIFFERENT pid: the promise is replaced by the
        // warning that stop addresses the other server.
        let line = pid_report_line(home.path(), me.pid.wrapping_add(1));
        assert!(
            line.contains("stops THAT server") && !line.contains("(stop it with:"),
            "a foreign record must redirect the operator, got: {line}"
        );
        Ok(())
    }

    #[test]
    fn the_log_tail_is_bounded_and_reports_unreadable_logs() -> Result<(), TestError> {
        let scratch = tempfile::tempdir()?;
        let log = scratch.path().join("server.log");

        let missing = log_tail(&log);
        assert!(
            missing.starts_with("(could not read the log:"),
            "a missing log must be reported, got: {missing}"
        );

        std::fs::write(&log, "")?;
        assert_eq!(log_tail(&log), "(the log is empty)");

        // More lines than the tail keeps: only the last LOG_TAIL_LINES stay,
        // even when the file is larger than the read window.
        let line = "x".repeat(1024);
        let mut many = String::new();
        for n in 0..100 {
            use std::fmt::Write as _;
            let _ = writeln!(many, "{n} {line}");
        }
        std::fs::write(&log, many)?;
        let tail = log_tail(&log);
        assert_eq!(tail.lines().count(), LOG_TAIL_LINES);
        assert!(tail.lines().last().is_some_and(|l| l.starts_with("99 ")));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn provisioning_tightens_a_permissive_home_and_refuses_a_symlink() -> Result<(), TestError> {
        use std::os::unix::fs::PermissionsExt as _;

        let scratch = tempfile::tempdir()?;
        let home = scratch.path().join("aion-home");
        std::fs::create_dir(&home)?;
        std::fs::set_permissions(&home, std::fs::Permissions::from_mode(0o755))?;
        provision_home(&home).map_err(|failure| failure.message)?;
        assert_eq!(
            std::fs::metadata(&home)?.permissions().mode() & 0o777,
            0o700,
            "a permissive existing home must be tightened before the log is written"
        );

        let linked = scratch.path().join("linked-home");
        std::os::unix::fs::symlink(&home, &linked)?;
        let Err(failure) = provision_home(&linked) else {
            return Err("a symlinked home must be refused".into());
        };
        assert_eq!(failure.code, 2);
        assert!(failure.message.contains("symlink"));
        Ok(())
    }
}
