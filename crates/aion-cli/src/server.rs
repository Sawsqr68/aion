//! The `aion server` subcommand: argument surface for running the full Aion
//! workflow server in-process via [`aion_server::run()`].

use std::net::SocketAddr;
use std::num::{NonZeroU64, NonZeroUsize};
use std::path::PathBuf;

use aion_server::config::{CliOverrides, ServerConfig};

use crate::console;
use clap::Args;

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
    /// Retired event-store URL flag; configuration loading refuses it with the haematite remedy.
    #[arg(long)]
    store_url: Option<String>,
    /// Number of engine scheduler worker threads.
    #[arg(long)]
    scheduler_threads: Option<NonZeroUsize>,
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
            drain_timeout_seconds: args.drain_timeout_seconds.map(NonZeroU64::get),
            workflow_packages: args.workflow_packages,
            gleam_path: args.gleam_path,
            authoring_project_root: args.authoring_project_root,
        }
    }
}
