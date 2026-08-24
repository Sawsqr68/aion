//! The run loop's doors: bind both transport listeners, claim the home's
//! pid file on the proof the binds provide, and hold the serve-until-shutdown
//! select that decides how the run ends.
//!
//! Split from `run.rs` as one coherent unit — the stop-verb work introduced
//! it whole (`OpenedDoors`, [`bind_and_claim`], [`serve_until_shutdown`],
//! [`transport_exit_report`]), and it owns exactly one concern: the doors and
//! the claim their successful binds prove safe.

use tokio::net::TcpListener;

use super::{ShutdownOutcome, shutdown_signal, transport_bind, transport_result};
use crate::error::ServerError;
use crate::shutdown;
use crate::state::ServerState;

/// Serve until a transport exits or the first termination signal arrives,
/// returning the drain's report. A transport exit shuts the engine down and
/// reports the nothing-was-draining shape; a signal runs the full drain and
/// then reaps both transport tasks unless the drain was forced.
pub(super) async fn serve_until_shutdown(
    state: &ServerState,
    shutdown_tx: &tokio::sync::watch::Sender<bool>,
    grpc: &mut tokio::task::JoinHandle<Result<(), ServerError>>,
    http: &mut tokio::task::JoinHandle<Result<(), ServerError>>,
) -> Result<shutdown::ShutdownReport, ServerError> {
    Ok(tokio::select! {
        result = &mut *grpc => {
            transport_result("gRPC", result)?;
            state.shutdown()?;
            transport_exit_report(state)
        },
        result = &mut *http => {
            transport_result("HTTP", result)?;
            state.shutdown()?;
            transport_exit_report(state)
        },
        result = shutdown_signal() => {
            result?;
            let _receiver_count = shutdown_tx.send(true);
            let report = shutdown::drain_after_first_signal(state.clone(), async {
                let _ = shutdown_signal().await;
            }).await?;
            if !matches!(report.outcome, ShutdownOutcome::Forced) {
                transport_result("gRPC", grpc.await)?;
                transport_result("HTTP", http.await)?;
            }
            report
        },
    })
}

/// The opened doors: both bound transport listeners, and the pid-file claim
/// their successful binds prove safe.
pub(super) struct OpenedDoors {
    pub(super) grpc_listener: TcpListener,
    pub(super) http_listener: TcpListener,
    pub(super) bound_grpc: std::net::SocketAddr,
    pub(super) bound_http: std::net::SocketAddr,
    /// Pid of this incarnation, for the outcome record at exit.
    pub(super) identity_pid: u32,
    /// Held for the whole run scope: its Drop removes the file on any
    /// ordinary exit, and removes it ONLY while it still holds this
    /// incarnation's record.
    pub(super) pid_file_guard: crate::control::pid_file::PidFileGuard,
}

/// Bind BOTH transport listeners, then claim the home's pid file (the stop
/// verb's R1: the file appears ON BIND). A successful pair of binds is the
/// proof that no live server is serving these addresses, so the claim can
/// never overwrite a running server's record from a boot that would only
/// lose the port race anyway.
pub(super) async fn bind_and_claim(
    home: &std::path::Path,
    grpc_address: std::net::SocketAddr,
    http_address: std::net::SocketAddr,
    commit: &str,
    drain_timeout: std::time::Duration,
) -> Result<OpenedDoors, ServerError> {
    let grpc_listener = TcpListener::bind(grpc_address)
        .await
        .map_err(|source| transport_bind("grpc", grpc_address, source))?;
    let http_listener = TcpListener::bind(http_address)
        .await
        .map_err(|source| transport_bind("http", http_address, source))?;
    let bound_grpc = grpc_listener
        .local_addr()
        .map_err(|source| transport_bind("grpc", grpc_address, source))?;
    let bound_http = http_listener
        .local_addr()
        .map_err(|source| transport_bind("http", http_address, source))?;
    let identity = crate::control::incarnation::self_identity()?;
    let pid_record = crate::control::pid_file::PidRecord {
        pid: identity.pid,
        started_at_unix_secs: identity.started_at_unix_secs,
        binary_sha256: identity.binary_sha256,
        version: env!("CARGO_PKG_VERSION").to_owned(),
        commit: commit.to_owned(),
        http_address: bound_http,
        grpc_address: bound_grpc,
        drain_timeout_seconds: drain_timeout.as_secs(),
    };
    let pid_file_guard = crate::control::pid_file::claim(home, &pid_record)?;
    // Self-verify the instrument the stop/status verbs will trust: probe the
    // record just written as those verbs would. On a box where the
    // process-table start-instant is not stable (e.g. a Linux fallback
    // reading a drifting clock), every later incarnation check fails and
    // `stop` refuses to signal this healthy server while accusing the
    // operator of pid recycling — an unfixable dead-end at stop time, but a
    // plain warning at boot time.
    match crate::control::incarnation::probe(&pid_record) {
        crate::control::incarnation::IncarnationProbe::Verified { .. } => {}
        other => {
            tracing::error!(
                pid = pid_record.pid,
                probe = ?other,
                "the pid record this server just wrote does not verify against its \
                 own live process: the incarnation instrument is unstable on this \
                 host, and `aion server stop`/`status` will refuse to trust the \
                 record. The server runs; the control verbs will not address it"
            );
        }
    }
    // The claim's outcome, banner-grade: one stable line an operator (or a
    // harness) can grep for, whichever face the claim's own reporting took.
    // An unclaimed boot serves but cannot be addressed by the control verbs;
    // that fact must not live only in a warning above the fold.
    tracing::info!(
        pid = identity.pid,
        home_claimed = pid_file_guard.holds_claim(),
        "home claim resolved"
    );
    Ok(OpenedDoors {
        grpc_listener,
        http_listener,
        bound_grpc,
        bound_http,
        identity_pid: identity.pid,
        pid_file_guard,
    })
}

/// The report for a run that ended because a transport task returned rather
/// than through the signal-driven drain: nothing was drained and nothing
/// parked, and the record says exactly that.
fn transport_exit_report(state: &ServerState) -> shutdown::ShutdownReport {
    shutdown::ShutdownReport {
        outcome: ShutdownOutcome::Clean,
        drain_timeout: state.runtime_config().drain_timeout,
        delivered_drain_requests: 0,
        parked: Vec::new(),
        parked_declared: Vec::new(),
        managed_workers_stopped: Vec::new(),
        managed_workers_unstopped: Vec::new(),
    }
}
