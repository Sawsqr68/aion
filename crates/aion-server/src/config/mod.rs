//! Runtime configuration loading and validation for `aion-server`.
//!
//! The config surface is split across cohesive submodules and re-exported here
//! in full, so every `crate::config::X` path resolves identically to the old
//! single-file layout:
//!
//! - [`defaults`] — default values and operator-facing validation messages.
//! - [`sections`] — the typed `[section]` sub-configurations and their defaults.
//! - [`runtime`] — the [`RuntimeConfig`] runtime view and [`CliOverrides`].
//! - [`load`] — [`ServerConfig`], its load/merge/validate pipeline, and the
//!   workflow-package discovery helpers.
//! - [`env`] / [`file`] — the environment-variable and TOML-file loaders.
//! - [`home`] — the single typed Aion-home resolver.

/// Environment variable configuration loader.
pub mod env;
/// File-based configuration loader.
pub mod file;

mod defaults;
mod error;
mod home;
mod load;
mod resolution;
mod runtime;
mod scaffold;
mod sections;
mod worker_supervision;

pub(crate) use defaults::{
    AUTHORING_GLEAM_PATH_EMPTY, AUTHORING_PROJECT_ROOT_REQUIRED,
    CLUSTER_BROADCAST_CAPACITY_REQUIRED, CORS_ALLOWED_ORIGIN_INVALID,
    DEPLOY_MAX_ARCHIVE_BYTES_REQUIRED, DEPLOY_MAX_INFLATED_BYTES_REQUIRED,
    EVENT_BROADCAST_CAPACITY_REQUIRED, LEGACY_AUTHORING_WORKSPACE_DIR, LEGACY_HAEMATITE_DATA_DIR,
    OUTBOX_BACKOFF_BASE_REQUIRED, OUTBOX_BACKOFF_MAX_REQUIRED, OUTBOX_BACKOFF_MULTIPLIER_REQUIRED,
    OUTBOX_BATCH_SIZE_REQUIRED, OUTBOX_MAX_ATTEMPTS_REQUIRED, OUTBOX_POLL_INTERVAL_REQUIRED,
    OUTBOX_RECONCILE_INTERVAL_REQUIRED, OUTBOX_RECONCILE_STALE_AFTER_REQUIRED,
    QUERY_TIMEOUT_REQUIRED,
};
pub use defaults::{
    DEFAULT_AUTHORING_WORKSPACE_DIR, DEFAULT_CLUSTER_BROADCAST_CAPACITY,
    DEFAULT_DEPLOY_MAX_ARCHIVE_BYTES, DEFAULT_DEPLOY_MAX_INFLATED_BYTES,
    DEFAULT_EVENT_BROADCAST_CAPACITY, DEFAULT_FAILOVER_CONFIRMATIONS,
    DEFAULT_FAILOVER_POLL_INTERVAL_MS, DEFAULT_HAEMATITE_DATA_DIR, DEFAULT_HTTP_ADDRESS,
    DEFAULT_MAX_IN_FLIGHT_ACTIVITIES, DEFAULT_MCP_AWAIT_POLL_INTERVAL_MS,
    DEFAULT_MCP_DISCOVER_TTL_MS, DEFAULT_MCP_TASK_POLL_INTERVAL_MS, DEFAULT_MCP_TASK_TTL_MS,
    DEFAULT_MCP_TOOLS_LIST_TTL_MS, DEFAULT_OBSERVABILITY_MAX_EVENT_BYTES,
    DEFAULT_OBSERVABILITY_MAX_STREAM_EVENTS, DEFAULT_OUTBOX_BACKOFF_BASE_MS,
    DEFAULT_OUTBOX_BACKOFF_MAX_MS, DEFAULT_OUTBOX_BACKOFF_MULTIPLIER, DEFAULT_OUTBOX_BATCH_SIZE,
    DEFAULT_OUTBOX_MAX_ATTEMPTS, DEFAULT_OUTBOX_POLL_INTERVAL_MS, DEFAULT_QUERY_TIMEOUT_MS,
    MAX_TRANSCRIPT_WINDOW_LIMIT, OBSERVABILITY_MAX_BATCH_EVENTS_REQUIRED,
    OBSERVABILITY_MAX_BATCH_HOLD_MS_REQUIRED, STORE_NODE_CACHE_BUDGET_REQUIRED,
};
pub(crate) use error::config_error;
pub use home::{AionHome, HomeSource, aion_home};
pub use load::ServerConfig;
pub(crate) use resolution::{ConfigResolution, ConfigSource};
pub use runtime::{CliOverrides, RuntimeConfig};
pub use scaffold::FIRST_RUN_CONFIG;
pub(crate) use scaffold::load_or_scaffold;
pub use sections::{
    AuthConfig, AuthoringConfig, AutoCreate, ClusterConfig, ClusterPeer, DeployConfig, DevConfig,
    DrainConfig, ListenConfig, McpConfig, MetricsConfig, NamespaceConfig, NamespaceMode,
    NamespacesConfig, ObservabilityConfig, OpsConsoleAssetSource, OpsConsoleConfig, OutboxConfig,
    OutboxTransport, ResolvedMcpConfig, RuntimeSection, ServerSection, StoreBackend, StoreConfig,
    TlsConfig, WebSocketConfig, WorkerConfig,
};
pub use worker_supervision::WorkerSupervisionConfig;
