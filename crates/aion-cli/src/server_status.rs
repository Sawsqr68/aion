//! `aion server status`: which layers am I actually running?
//!
//! Three layers, each observed independently and each absence its own
//! honest state (no pid file ≠ server down ≠ HTTP unreachable ≠ not ready
//! — no layer's silence is ever reported as another layer's health):
//!
//! 1. **The record**: the pid file's incarnation, verified against the live
//!    process table.
//! 2. **The build**: `GET /build` on the address the record says the server
//!    bound (or the configured address when no record exists, with the
//!    source named) — asserted on the BODY, never the status code, because
//!    the ops-console SPA catch-all answers `200 text/html` for any unknown
//!    route on an image without the endpoint.
//! 3. **Readiness**: `GET /health/ready`, which answers by status code with
//!    an empty body — so a 200 carrying a body is the SPA shell on an image
//!    without the route. Whether that shell is a FOREIGN service or this
//!    server without the route mounted is decided by the build layer's own
//!    evidence: if `/build` just answered with a valid identity, the address
//!    is this aion server and the honest line is "does not mount
//!    /health/ready" — never an accusation that someone else took the port.
//!
//! Exit codes, so a script can ask "is the server up?": 0 when the RECORDED
//! incarnation is RUNNING and verified; 1 when it is not (gone, mismatched,
//! or no record — each face printed); 2 when the verb itself could not do
//! its job (unresolvable home or config, unreadable pid file). The verdict
//! is about the recorded incarnation — the server the control verbs can
//! address — not about whether ANY server answers on some address: a server
//! that booted unclaimed answers HTTP but exits 1 here, with the finding
//! printed. `aion server stop` codes the no-record case differently (0,
//! idempotent success) because stopping is an action whose goal state
//! already holds, while status is a predicate whose answer is "no".

use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::ExitCode;

use aion_server::config::{CliOverrides, ServerConfig, aion_home};
use aion_server::control::{IncarnationProbe, incarnation, outcome, pid_file};
use clap::Args;
use serde::Deserialize;

use crate::console::PROBE_STEP_TIMEOUT;

/// Arguments for `aion server status`.
#[derive(Args, Clone, Debug)]
pub struct StatusArgs {
    /// Path to the TOML server configuration file. Used to resolve the store
    /// path and, when no pid file exists, the address to probe.
    #[arg(long)]
    config: Option<PathBuf>,
}

/// What `GET /build` answered when the body parsed as a build identity.
#[derive(Debug, Deserialize)]
struct BuildBody {
    version: String,
    commit: String,
    dirty: String,
    built_at: String,
}

/// Run `aion server status`.
pub async fn run(args: &StatusArgs) -> ExitCode {
    let home = match aion_home() {
        Ok(home) => home.path,
        Err(error) => {
            eprintln!("aion server status: could not resolve the Aion home: {error}");
            return ExitCode::from(2);
        }
    };
    let overrides = CliOverrides {
        config_path: args.config.clone(),
        ..CliOverrides::default()
    };
    let config = match ServerConfig::load(&overrides) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("aion server status: could not load configuration: {error}");
            return ExitCode::from(2);
        }
    };
    println!("home: {}", home.display());
    // The store line is always stated, and the BACKEND decides what it says:
    // config resolution fills `data_dir` unconditionally (`<AION_HOME>/data`
    // when the file names none), so the directory's presence proves nothing
    // about durability — a memory-backend server dies with its state no
    // matter what path the config carries. Same truth `aion launch` prints.
    match config.store.backend {
        aion_server::config::StoreBackend::Haematite => match &config.store.data_dir {
            Some(data_dir) => println!("store: {data_dir}"),
            None => println!("store: haematite (data directory resolved by the server)"),
        },
        aion_server::config::StoreBackend::Memory => {
            println!("store: in-memory — state does not survive a stop");
        }
    }

    // Layer 1: the record. An unreadable file is a verb failure, not a
    // reportable server state — but the HTTP layers still answer from the
    // configured address so the operator gets every observable fact.
    let mut verb_failed = false;
    let record = match pid_file::read(&home) {
        Ok(record) => record,
        Err(error) => {
            eprintln!("pid file: {error}");
            verb_failed = true;
            None
        }
    };
    let mut running_verified = false;
    let http_address = if let Some(record) = &record {
        running_verified = render_record_layer(&home, record);
        println!(
            "probing the address the record says the server bound: {}",
            record.http_address
        );
        record.http_address
    } else {
        if !verb_failed {
            println!(
                "pid file: none at {} — no server has claimed this home (or the \
                 running server predates the pid file)",
                pid_file::pid_file_path(&home).display()
            );
        }
        let address = config.server.listen_address;
        println!("probing the CONFIGURED address (no recorded bind): {address}");
        address
    };

    // Layers 2 and 3: the server's own listener. The build layer's verdict
    // — "this address answered with an aion build identity" — feeds the
    // readiness layer's diagnosis of a 200-with-body.
    let identity_confirmed = render_build_layer(http_address).await;
    render_ready_layer(http_address, identity_confirmed).await;
    if identity_confirmed && !running_verified {
        // The two layers disagree in the one direction worth a warning: an
        // aion server IS answering on this address, but it is not the
        // verified recorded incarnation — a server that booted UNCLAIMED
        // over another claimant's record, or one predating the pid file.
        // The exit code stays honest to the verb's question (the RECORDED
        // server), and this line stops the operator reading "not running"
        // as "nothing is listening".
        println!(
            "note: an aion server IS answering on {http_address}, but it is not a \
             verified recorded incarnation of this home — a server booted unclaimed \
             (another claimant held the record), or one predating the pid file. The \
             control verbs address only the recorded server"
        );
    }
    if verb_failed {
        ExitCode::from(2)
    } else if running_verified {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Render the pid-file record and its incarnation verdict, including the
/// death note's account when the recorded process is gone. Returns whether
/// the recorded incarnation is RUNNING and verified — the fact the verb's
/// exit code answers with.
fn render_record_layer(home: &std::path::Path, record: &pid_file::PidRecord) -> bool {
    println!(
        "pid file: pid {} — version {}, commit {}, bound http {} / grpc {}",
        record.pid, record.version, record.commit, record.http_address, record.grpc_address
    );
    match incarnation::probe(record) {
        IncarnationProbe::Verified { .. } => {
            let now_unix = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(record.started_at_unix_secs, |elapsed| elapsed.as_secs());
            let uptime_secs = now_unix.saturating_sub(record.started_at_unix_secs);
            println!("process: RUNNING, incarnation verified (started {uptime_secs}s ago)");
            return true;
        }
        IncarnationProbe::ProcessGone => {
            println!(
                "process: GONE — the recorded pid is not in the process table; the \
                 server died without removing its record"
            );
            match outcome::read_fate(home, record.pid) {
                Ok(fate) => crate::server_stop::render_fate(record.pid, &fate),
                Err(error) => println!("death note: {error}"),
            }
        }
        IncarnationProbe::DifferentIncarnation {
            live_started_at_unix_secs,
            exe,
        } => {
            let exe_note = exe.map_or_else(String::new, |path| {
                format!(" (running `{}`)", path.display())
            });
            println!(
                "process: MISMATCH — pid {} is alive but started at unix second {}, \
                 not the recorded {}{exe_note}; the pid was recycled onto a different \
                 process after the recorded server died",
                record.pid, live_started_at_unix_secs, record.started_at_unix_secs
            );
        }
    }
    false
}

/// Render the `/build` layer, asserted on the body. Returns whether the
/// address answered with a valid aion build identity — the evidence the
/// readiness layer uses to tell "this server without the route" apart from
/// "someone else's service on this address".
async fn render_build_layer(address: SocketAddr) -> bool {
    let url = format!("http://{address}/build");
    match probe(&url).await {
        Err(error) => {
            println!("build: unreachable — {error}");
            false
        }
        Ok((status, body)) => {
            if status == reqwest::StatusCode::UNAUTHORIZED
                || status == reqwest::StatusCode::FORBIDDEN
            {
                println!(
                    "build: the endpoint refused this unauthenticated probe ({status}); \
                     the listener is up but auth is enabled"
                );
                return false;
            }
            if let Ok(build) = serde_json::from_str::<BuildBody>(&body) {
                let dirty = if build.dirty == "true" { "-dirty" } else { "" };
                println!(
                    "build: {}+{}{} built {}",
                    build.version, build.commit, dirty, build.built_at
                );
                true
            } else {
                println!(
                    "build: the listener answered {status} but the body is not a build \
                     identity — an app shell or a different service is on this address \
                     (an image without /build answers 200 here; the body is the truth)"
                );
                false
            }
        }
    }
}

/// Render the `/health/ready` layer. The endpoint answers by status code
/// with an EMPTY body, so a 200 carrying a body is the SPA shell answering
/// for an unmounted route — and whether that shell belongs to THIS server or
/// to a foreign service is decided by `identity_confirmed`, the build
/// layer's own verdict for the same address moments earlier.
async fn render_ready_layer(address: SocketAddr, identity_confirmed: bool) {
    let url = format!("http://{address}/health/ready");
    match probe(&url).await {
        Err(error) => println!("ready: unreachable — {error}"),
        Ok((status, body)) => {
            if status == reqwest::StatusCode::OK && body.is_empty() {
                println!("ready: YES — the store answered the readiness probe");
            } else if status == reqwest::StatusCode::SERVICE_UNAVAILABLE {
                println!(
                    "ready: NO — the server is up but its runtime or store did not \
                     answer the readiness probe"
                );
            } else if status == reqwest::StatusCode::OK && identity_confirmed {
                println!(
                    "ready: this server does not mount /health/ready (the SPA shell \
                     answered for the route); the address IS this aion server — its \
                     /build just answered with a valid identity — but no health state \
                     was commissioned"
                );
            } else if status == reqwest::StatusCode::OK {
                println!(
                    "ready: the listener answered 200 with a body, but /health/ready \
                     answers with an empty one — an app shell or a different service \
                     is on this address"
                );
            } else if status == reqwest::StatusCode::UNAUTHORIZED
                || status == reqwest::StatusCode::FORBIDDEN
            {
                // The same face the build layer names for these codes: the
                // listener is up, auth is on, and this probe is
                // unauthenticated — not an unexpected answer, and not a
                // readiness verdict either way.
                println!(
                    "ready: the endpoint refused this unauthenticated probe ({status}); \
                     the listener is up but auth is enabled, so readiness cannot be \
                     probed unauthenticated"
                );
            } else {
                println!("ready: unexpected answer {status}");
            }
        }
    }
}

/// One bounded GET, returning status and body text.
async fn probe(url: &str) -> Result<(reqwest::StatusCode, String), String> {
    let client = reqwest::Client::builder()
        .timeout(PROBE_STEP_TIMEOUT)
        .build()
        .map_err(|error| error.to_string())?;
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    let status = response.status();
    let body = response.text().await.map_err(|error| error.to_string())?;
    Ok((status, body))
}
