//! The `aion server` subcommand: argument surface for running the full Aion
//! workflow server in-process via [`aion_server::run()`], plus the control
//! verbs (`stop`, `status`) that act on a server already running under this
//! machine's Aion home.

use std::net::SocketAddr;
use std::num::{NonZeroU32, NonZeroU64, NonZeroUsize};
use std::path::PathBuf;

use aion_server::config::{CliOverrides, ServerConfig};

use crate::console;
use clap::{Args, Subcommand};

/// The whole `aion server` surface: bare `aion server` runs the server with
/// the flattened [`ServerArgs`]; `aion server stop` / `aion server status`
/// are control verbs against the recorded running server. Run flags and a
/// control verb are mutually exclusive by construction
/// (`args_conflicts_with_subcommands`), so `aion server stop --config x.toml`
/// parses the verb's own `--config`, never the run surface's.
#[derive(Args, Clone, Debug)]
#[command(args_conflicts_with_subcommands = true)]
pub struct ServerCommand {
    /// Control verb, when one was given.
    #[command(subcommand)]
    pub command: Option<ServerSubcommand>,
    /// The run-the-server argument surface (the default with no verb).
    #[command(flatten)]
    pub run: ServerArgs,
}

/// Control verbs for a server already running under this Aion home.
#[derive(Subcommand, Clone, Debug)]
pub enum ServerSubcommand {
    /// Stop the running server: verify the recorded incarnation, send
    /// SIGTERM, wait for the drain, and report the drain outcome by name.
    Stop(crate::server_stop::StopArgs),
    /// Report which server is actually running: the pid-file record, the
    /// live build identity from its own listener, and readiness — each
    /// layer's absence reported as its own distinct state.
    Status(crate::server_status::StatusArgs),
}

/// Arguments for `aion server`, identical to the surface of the former
/// standalone `aion-server` binary.
#[derive(Args, Clone, Debug)]
pub struct ServerArgs {
    /// Path to a TOML server configuration file. Optional when using local defaults.
    #[arg(long)]
    config: Option<PathBuf>,
    /// Override the HTTP/JSON and ops-console listener address.
    #[arg(long)]
    listen_address: Option<SocketAddr>,
    /// Override the gRPC listener address this server binds.
    ///
    /// This is the server's own bind address. The global `--endpoint` is the
    /// address a *client* dials and has no effect here; `aion server` refuses
    /// rather than start on a different port than the operator named.
    #[arg(long)]
    grpc_address: Option<SocketAddr>,
    /// RETIRED. Selected the libSQL backend, which no longer exists. Kept only so
    /// that passing it is refused by name with the haematite remedy rather than
    /// rejected as an unknown flag, which would name nothing.
    #[arg(long)]
    store_url: Option<String>,
    /// Number of engine scheduler worker threads.
    #[arg(long)]
    scheduler_threads: Option<NonZeroUsize>,
    /// Recorded calls to one `(module, function, arity)` before the embedded VM
    /// JIT-compiles it. Omitted, the VM applies its own default.
    ///
    /// This moves the moment of compilation; it does not switch compilation
    /// off. A very large value defers it past any realistic workload, but the
    /// call counter saturates rather than stopping, so compilation still occurs
    /// on the far side of that horizon.
    #[arg(long = "jit-threshold")]
    jit_threshold: Option<NonZeroU32>,
    /// Maximum graceful drain duration in seconds.
    #[arg(long = "drain-timeout")]
    drain_timeout_seconds: Option<NonZeroU64>,
    /// Workflow package archive to load at startup. Repeat to load multiple packages.
    #[arg(long = "workflow-package")]
    workflow_packages: Vec<PathBuf>,
    /// Path to the external `gleam` binary that commissions the server-side
    /// authoring loop. Setting it mounts the `/authoring/*` endpoints (and
    /// requires `--authoring-project-root`); absent, the server compiles no
    /// Gleam and deploys pre-built `.aion` files only.
    #[arg(long = "gleam-path")]
    gleam_path: Option<PathBuf>,
    /// Built Gleam workflow project root that submitted authoring source is
    /// written into and packaged from. Required when `--gleam-path` is set.
    #[arg(long = "authoring-project-root")]
    authoring_project_root: Option<PathBuf>,
    /// Open the ops console in the default browser once the HTTP listener is up.
    /// Resolves the served URL from the effective config (the same
    /// `--listen-address`/config/default precedence the server uses).
    #[arg(long)]
    open: bool,
}

impl ServerArgs {
    /// Whether `--open` was passed.
    #[must_use]
    pub fn open(&self) -> bool {
        self.open
    }
}

/// Resolve the served HTTP URL from the effective config and, once the listener
/// answers its own liveness probe, open it in the default browser. Best-effort:
/// any failure (config load, listener never live, no opener) is logged and
/// dropped so it never affects the server run.
///
/// The URL, the probe, and the opener are the shared [`crate::console`]
/// plumbing the bare `aion` launcher also uses, so "up" means the same thing
/// on both paths: HTTP 200 from `/health/live`, not a bare TCP accept.
pub fn spawn_browser_open(overrides: &CliOverrides) {
    let address = match ServerConfig::load(overrides) {
        Ok(config) => {
            let (_store, runtime) = config.into_parts();
            runtime.listen.http
        }
        Err(error) => {
            eprintln!("aion server --open: could not resolve listen address: {error}");
            return;
        }
    };
    tokio::spawn(async move {
        if console::wait_until_live(address, console::LIVE_BUDGET).await {
            let url = console::served_url(address);
            match console::open_browser(&url) {
                Ok(()) => eprintln!("aion server --open: opening {url}"),
                Err(error) => {
                    eprintln!("aion server --open: could not open {url}: {error}");
                }
            }
        } else {
            eprintln!(
                "aion server --open: listener at {address} did not come up; not opening browser"
            );
        }
    });
}

impl From<ServerArgs> for CliOverrides {
    fn from(args: ServerArgs) -> Self {
        Self {
            config_path: args.config,
            listen_address: args.listen_address,
            grpc_address: args.grpc_address,
            store_url: args.store_url,
            scheduler_threads: args.scheduler_threads.map(NonZeroUsize::get),
            jit_threshold: args.jit_threshold.map(NonZeroU32::get),
            drain_timeout_seconds: args.drain_timeout_seconds.map(NonZeroU64::get),
            workflow_packages: args.workflow_packages,
            gleam_path: args.gleam_path,
            authoring_project_root: args.authoring_project_root,
        }
    }
}
