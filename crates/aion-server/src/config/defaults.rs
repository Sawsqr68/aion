//! Default values and operator-facing validation messages for the server config.
//!
//! This module holds the pure constants that back the config surface: the
//! compile-time default listener addresses, every `DEFAULT_*` tuning-knob
//! default, and the `*_REQUIRED` / `*_EMPTY` / `*_INVALID` operator-facing
//! validation messages. They are re-exported from the `config` module so every
//! existing `crate::config::X` path resolves identically.

use std::net::SocketAddr;

/// Shipping HTTP/WebSocket listener used when no server override is supplied.
pub const DEFAULT_HTTP_ADDRESS: SocketAddr =
    SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), 8080);
pub(super) const DEFAULT_GRPC_ADDRESS: SocketAddr =
    SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), 50051);
/// Maximum bounded transcript page accepted by the HTTP transcript endpoint.
pub const MAX_TRANSCRIPT_WINDOW_LIMIT: u32 = 2_000;

/// Default SS-5b failover poll interval (milliseconds) when `[store.cluster]`
/// does not set `failover_poll_interval_ms`.
pub const DEFAULT_FAILOVER_POLL_INTERVAL_MS: u64 = 500;

/// Default SS-5b debounce count when `[store.cluster]` does not set
/// `failover_confirmations`.
pub const DEFAULT_FAILOVER_CONFIRMATIONS: u32 = 3;

/// Generous platform default for `[namespaces] max_in_flight_activities`: the
/// cluster-wide concurrent in-flight-activity ceiling applied to a namespace
/// that sets no explicit override. A generous power-of-two (Control-Plane
/// Phase 2 §6.1 / Open Decision 4) so the default is HEADROOM, not a low hard
/// cap — a tenant only ever hits a ceiling it (or the operator) raised. Nothing
/// enforces it yet (P2-Q1 is config + record field only).
pub const DEFAULT_MAX_IN_FLIGHT_ACTIVITIES: u32 = 1024;

/// Default `event_broadcast_capacity` applied when omitted, so a minimal/empty
/// config boots without forcing the operator to size a tuning knob. Sized for
/// global event volume across namespaces; override for high-throughput fleets.
pub const DEFAULT_EVENT_BROADCAST_CAPACITY: usize = 256;

/// Default `cluster_broadcast_capacity` applied when omitted. Cluster topology
/// events are low-rate, so a small lag buffer is ample.
pub const DEFAULT_CLUSTER_BROADCAST_CAPACITY: usize = 64;

/// Operator-facing message for an explicitly zero `event_broadcast_capacity`
/// (omitting the key uses [`DEFAULT_EVENT_BROADCAST_CAPACITY`]; an explicit zero
/// is a genuine misconfiguration — a zero-capacity channel streams nothing).
pub(crate) const EVENT_BROADCAST_CAPACITY_REQUIRED: &str = "websocket.event_broadcast_capacity must be a positive integer when set (the server always mounts /events/stream, so a zero-capacity channel would stream nothing); omit websocket.event_broadcast_capacity (or AION_WEBSOCKET_EVENT_BROADCAST_CAPACITY) to use the default, or set it to a positive integer";

/// Operator-facing message for an explicitly zero `cluster_broadcast_capacity`
/// (omitting the key uses [`DEFAULT_CLUSTER_BROADCAST_CAPACITY`]).
pub(crate) const CLUSTER_BROADCAST_CAPACITY_REQUIRED: &str = "websocket.cluster_broadcast_capacity must be a positive integer when set (the server always mounts the WS3 cluster subscription on /events/stream); omit websocket.cluster_broadcast_capacity (or AION_WEBSOCKET_CLUSTER_BROADCAST_CAPACITY) to use the default, or set it to a positive integer";

/// Default `deploy.max_archive_bytes` applied when omitted and deploy is
/// enabled. A conservative 64 MiB upload ceiling: large enough for real
/// workflow packages, small enough to bound a single upload. Overridable.
pub const DEFAULT_DEPLOY_MAX_ARCHIVE_BYTES: u64 = 64 * 1024 * 1024;

/// Default `deploy.max_inflated_bytes` applied when omitted and deploy is
/// enabled. A conservative 256 MiB decompressed-contents ceiling (4x the
/// archive ceiling) that still hard-caps DEFLATE-bomb inflation. Overridable.
pub const DEFAULT_DEPLOY_MAX_INFLATED_BYTES: u64 = 256 * 1024 * 1024;

/// Operator-facing message for an explicitly zero `deploy.max_archive_bytes`
/// (omitting the key uses [`DEFAULT_DEPLOY_MAX_ARCHIVE_BYTES`]; an explicit zero
/// is a genuine misconfiguration — a zero ceiling refuses every upload).
pub(crate) const DEPLOY_MAX_ARCHIVE_BYTES_REQUIRED: &str = "deploy.max_archive_bytes must be a positive number of bytes when set (a zero archive ceiling would refuse every upload); omit deploy.max_archive_bytes (or AION_DEPLOY_MAX_ARCHIVE_BYTES) to use the conservative default, or set it to a positive number of bytes sized for the deployment's packages";

/// Operator-facing message for an explicitly zero `deploy.max_inflated_bytes`
/// (omitting the key uses [`DEFAULT_DEPLOY_MAX_INFLATED_BYTES`]; an explicit
/// zero is a genuine misconfiguration — a compressed upload under
/// `deploy.max_archive_bytes` can inflate ~1000:1).
pub(crate) const DEPLOY_MAX_INFLATED_BYTES_REQUIRED: &str = "deploy.max_inflated_bytes must be a positive number of bytes when set (a compressed upload under deploy.max_archive_bytes can inflate ~1000:1, so a zero inflate ceiling is incoherent); omit deploy.max_inflated_bytes (or AION_DEPLOY_MAX_INFLATED_BYTES) to use the conservative default, or set it to a positive number of bytes no smaller than deploy.max_archive_bytes";

/// Default `query_timeout_ms` applied when omitted, so a minimal/empty config
/// boots with a sane workflow-query reply deadline instead of failing startup.
pub const DEFAULT_QUERY_TIMEOUT_MS: u64 = 10_000;

/// Operator-facing message for an explicitly zero `query_timeout_ms` (omitting
/// the key uses [`DEFAULT_QUERY_TIMEOUT_MS`]; an explicit zero is a genuine
/// misconfiguration — a zero deadline would fail every query immediately).
pub(crate) const QUERY_TIMEOUT_REQUIRED: &str = "runtime.query_timeout_ms must be a positive integer when set (the server always mounts /workflows/query, so a zero deadline would fail every query immediately); omit runtime.query_timeout_ms (or AION_RUNTIME_QUERY_TIMEOUT_MS) to use the default, or set it to a positive number of milliseconds";

/// Default `outbox.poll_interval_ms` applied when omitted and the dispatcher is
/// enabled. A tight 20ms claim cadence keeps fan-out latency low; raise it for
/// lower-rate, larger-batch sweeps.
pub const DEFAULT_OUTBOX_POLL_INTERVAL_MS: u64 = 20;

/// Default `outbox.batch_size` applied when omitted and the dispatcher is
/// enabled: rows claimed per sweep.
pub const DEFAULT_OUTBOX_BATCH_SIZE: u32 = 16;

/// Default `outbox.max_attempts` applied when omitted and the dispatcher is
/// enabled: dispatch attempts before a row is dead-lettered to `failed`.
pub const DEFAULT_OUTBOX_MAX_ATTEMPTS: u32 = 5;

/// Default `outbox.backoff_base_ms` applied when omitted and the dispatcher is
/// enabled: the first retry's backoff, in milliseconds.
pub const DEFAULT_OUTBOX_BACKOFF_BASE_MS: u64 = 50;

/// Default `outbox.backoff_multiplier` applied when omitted and the dispatcher
/// is enabled: geometric growth factor per prior attempt (must be >= 1).
pub const DEFAULT_OUTBOX_BACKOFF_MULTIPLIER: u32 = 2;

/// Default `outbox.backoff_max_ms` applied when omitted and the dispatcher is
/// enabled: the per-retry backoff ceiling, in milliseconds.
pub const DEFAULT_OUTBOX_BACKOFF_MAX_MS: u64 = 1_000;

/// Operator-facing message for an explicitly zero `outbox.poll_interval_ms`
/// (omitting the key uses [`DEFAULT_OUTBOX_POLL_INTERVAL_MS`]).
pub(crate) const OUTBOX_POLL_INTERVAL_REQUIRED: &str = "outbox.poll_interval_ms must be a positive number of milliseconds when set (a zero claim cadence is invalid); omit outbox.poll_interval_ms (or AION_OUTBOX_POLL_INTERVAL_MS) to use the default, or set it to a positive number of milliseconds sized for fan-out volume and latency";

/// Operator-facing message for an explicitly zero `outbox.batch_size`
/// (omitting the key uses [`DEFAULT_OUTBOX_BATCH_SIZE`]).
pub(crate) const OUTBOX_BATCH_SIZE_REQUIRED: &str = "outbox.batch_size must be a positive integer when set (a zero per-sweep claim ceiling claims nothing); omit outbox.batch_size (or AION_OUTBOX_BATCH_SIZE) to use the default, or set it to a positive integer";

/// Operator-facing message for an explicitly zero `outbox.max_attempts`
/// (omitting the key uses [`DEFAULT_OUTBOX_MAX_ATTEMPTS`]).
pub(crate) const OUTBOX_MAX_ATTEMPTS_REQUIRED: &str = "outbox.max_attempts must be a positive integer when set (a zero retry budget dead-letters before the first attempt); omit outbox.max_attempts (or AION_OUTBOX_MAX_ATTEMPTS) to use the default, or set it to a positive integer";

/// Operator-facing message for an explicitly zero `outbox.backoff_base_ms`
/// (omitting the key uses [`DEFAULT_OUTBOX_BACKOFF_BASE_MS`]).
pub(crate) const OUTBOX_BACKOFF_BASE_REQUIRED: &str = "outbox.backoff_base_ms must be a positive number of milliseconds when set (a zero first-retry backoff is invalid); omit outbox.backoff_base_ms (or AION_OUTBOX_BACKOFF_BASE_MS) to use the default, or set it to a positive number of milliseconds";

/// Operator-facing message for an explicitly zero `outbox.backoff_multiplier`
/// (omitting the key uses [`DEFAULT_OUTBOX_BACKOFF_MULTIPLIER`]).
pub(crate) const OUTBOX_BACKOFF_MULTIPLIER_REQUIRED: &str = "outbox.backoff_multiplier must be at least one when set so backoff never shrinks (a zero multiplier collapses the backoff curve); omit outbox.backoff_multiplier (or AION_OUTBOX_BACKOFF_MULTIPLIER) to use the default, or set it to a positive integer";

/// Operator-facing message for an explicitly undersized `outbox.backoff_max_ms`
/// (omitting the key uses [`DEFAULT_OUTBOX_BACKOFF_MAX_MS`]; an explicit value
/// must be at least `outbox.backoff_base_ms`).
pub(crate) const OUTBOX_BACKOFF_MAX_REQUIRED: &str = "outbox.backoff_max_ms must be at least outbox.backoff_base_ms when set (a ceiling below the base would cap the very first retry below its own backoff); omit outbox.backoff_max_ms (or AION_OUTBOX_BACKOFF_MAX_MS) to use the default, or set it to a positive number of milliseconds no smaller than outbox.backoff_base_ms";

/// Operator-facing message for an absent or zero `outbox.reconcile_interval_ms`.
pub(crate) const OUTBOX_RECONCILE_INTERVAL_REQUIRED: &str = "outbox.reconcile_interval_ms is required and has no default when live outbox reconciliation is enabled: set both outbox.reconcile_interval_ms and outbox.reconcile_stale_after_ms (or AION_OUTBOX_RECONCILE_INTERVAL_MS / AION_OUTBOX_RECONCILE_STALE_AFTER_MS) to positive millisecond values, or omit both to leave reconciliation disabled";

/// Operator-facing message for an absent or zero `outbox.reconcile_stale_after_ms`.
pub(crate) const OUTBOX_RECONCILE_STALE_AFTER_REQUIRED: &str = "outbox.reconcile_stale_after_ms is required and has no default when live outbox reconciliation is enabled: set both outbox.reconcile_interval_ms and outbox.reconcile_stale_after_ms (or AION_OUTBOX_RECONCILE_INTERVAL_MS / AION_OUTBOX_RECONCILE_STALE_AFTER_MS) to positive millisecond values, or omit both to leave reconciliation disabled";

/// Operator-facing message for an absent or empty `authoring.gleam_path` value.
pub(crate) const AUTHORING_GLEAM_PATH_EMPTY: &str = "authoring.gleam_path must not be empty when set: it names the external gleam binary the authoring loop spawns; set authoring.gleam_path (or AION_AUTHORING_GLEAM_PATH) to the path of a runnable gleam binary, or remove it to leave the authoring surface dark";

/// Operator-facing message for an absent `authoring.project_root` when the
/// authoring surface is commissioned.
pub(crate) const AUTHORING_PROJECT_ROOT_REQUIRED: &str = "authoring.project_root is required and has no default when authoring.gleam_path is set: submitted Gleam source is written into and packaged from a built project, so the operator must provision and name the project root (a directory with gleam.toml, the aion_flow dependency, workflow.toml, and schemas/); set authoring.project_root (or AION_AUTHORING_PROJECT_ROOT)";

/// Default `observability.max_event_bytes`: the ceiling on one persisted
/// transcript event's serialized size. 256 KiB comfortably holds real
/// tool-result payloads while bounding what one hostile/verbose harness line
/// can write to the durable `O` keyspace. Overridable per deployment.
pub const DEFAULT_OBSERVABILITY_MAX_EVENT_BYTES: usize = 256 * 1024;

/// Default `observability.max_stream_events`: the ceiling on retained events
/// per `(workflow, activity, attempt)` transcript stream. Past it one marker
/// record is retained and further events stay live-only. Overridable.
pub const DEFAULT_OBSERVABILITY_MAX_STREAM_EVENTS: u64 = 20_000;

/// Operator-facing message for an explicitly zero `observability.max_event_bytes`
/// (omitting the key uses [`DEFAULT_OBSERVABILITY_MAX_EVENT_BYTES`]; a zero
/// ceiling would truncate every transcript event to nothing).
pub(crate) const OBSERVABILITY_MAX_EVENT_BYTES_REQUIRED: &str = "observability.max_event_bytes must be a positive number of bytes when set (a zero ceiling would truncate every retained transcript event to nothing); omit observability.max_event_bytes (or AION_OBSERVABILITY_MAX_EVENT_BYTES) to use the default, or set it to a positive number of bytes";

/// Operator-facing message for an explicitly zero `observability.max_stream_events`
/// (omitting the key uses [`DEFAULT_OBSERVABILITY_MAX_STREAM_EVENTS`]; a zero
/// cap would retain no transcript at all).
pub(crate) const OBSERVABILITY_MAX_STREAM_EVENTS_REQUIRED: &str = "observability.max_stream_events must be a positive integer when set (a zero per-stream cap would retain no transcript at all); omit observability.max_stream_events (or AION_OBSERVABILITY_MAX_STREAM_EVENTS) to use the default, or set it to a positive integer";

/// Operator-facing message for an absent or zero `observability.max_batch_events`.
///
/// There is NO default for this knob: every durable transcript commit
/// re-persists its whole containing storage leaf, so the number of events one
/// commit carries is a deployment trade (store cost against how much transcript
/// a single refused commit can lose) that only the operator can make.
pub const OBSERVABILITY_MAX_BATCH_EVENTS_REQUIRED: &str = "observability.max_batch_events is required and has no default: it is the maximum number of transcript events the drain commits in one durable append, and one durable commit re-persists the whole containing storage leaf, so committing one event at a time is the write amplification measured on 2026-08-17 (a 136 KB status sweep left 73.7 MB of permanent store). Set observability.max_batch_events (or AION_OBSERVABILITY_MAX_BATCH_EVENTS) to a positive integer, together with observability.max_batch_hold_ms";

/// Operator-facing message for an absent `observability.max_batch_hold_ms`.
///
/// Zero is a valid setting (never wait), so absence — not zero — is the error:
/// the operator must state how long a partial batch may be held, because that
/// window bounds how much not-yet-durable transcript a crash can cost.
pub const OBSERVABILITY_MAX_BATCH_HOLD_MS_REQUIRED: &str = "observability.max_batch_hold_ms is required and has no default: it is how long, in milliseconds, the transcript drain may hold a partial batch open waiting for it to fill, which bounds how much not-yet-durable transcript a crash can cost. Set observability.max_batch_hold_ms (or AION_OBSERVABILITY_MAX_BATCH_HOLD_MS) to a non-negative integer — 0 means never wait, and commit whatever is already queued — together with observability.max_batch_events";

/// Child directory below Aion home used by the default haematite store.
///
/// The complete default is `<AION_HOME>/data`; it is resolved dynamically after
/// file and environment overlays so an explicit `store.data_dir` always wins.
pub const DEFAULT_HAEMATITE_DATA_DIR: &str = "data";

/// Child directory below Aion home used by the out-of-box AWL studio.
///
/// The complete default is `<AION_HOME>/authoring`; it is resolved dynamically
/// after file and environment overlays so an explicit
/// `authoring.workspace_dir` always wins. This does not commission the separate
/// Gleam authoring loop, which still requires an explicit
/// `authoring.gleam_path`.
pub const DEFAULT_AUTHORING_WORKSPACE_DIR: &str = "authoring";

/// CWD-relative store default used before Aion home existed.
pub(crate) const LEGACY_HAEMATITE_DATA_DIR: &str = "aion-data";

/// CWD-relative studio default shipped by the first out-of-box authoring round.
pub(crate) const LEGACY_AUTHORING_WORKSPACE_DIR: &str = "aion-authoring";

/// Operator-facing message for an empty or malformed `cors_allowed_origins`
/// entry.
pub(crate) const CORS_ALLOWED_ORIGIN_INVALID: &str = "server.cors_allowed_origins entries must each be a valid HTTP origin (scheme://host[:port], e.g. http://localhost:5173) with no path or trailing slash";

/// Default `mcp.discover_ttl_ms`: how long a client may treat a
/// `server/discover` result as fresh. Sixty seconds — the discover result names
/// the protocol revisions and the extension set, which change only on a deploy,
/// but a minute is short enough that a rolling upgrade is visible to a client
/// promptly. Overridable per deployment.
pub const DEFAULT_MCP_DISCOVER_TTL_MS: u64 = 60_000;

/// Default `mcp.tools_list_ttl_ms`: how long a client may treat a `tools/list`
/// result as fresh. Five minutes — the catalog is compiled into the binary and
/// cannot change while the process runs, so the only event that invalidates it
/// is a restart. Overridable per deployment.
pub const DEFAULT_MCP_TOOLS_LIST_TTL_MS: u64 = 300_000;

/// Default `mcp.task_ttl_ms`: how long a created task lives before the server
/// may fail and delete it. One hour, which bounds what a client that walked
/// away can hold. A run that legitimately takes longer is not lost by this: the
/// task is only the WAIT, and the run remains reachable through the handles the
/// task's creation returned. Set `0` for an unlimited lifetime.
pub const DEFAULT_MCP_TASK_TTL_MS: u64 = 3_600_000;

/// Default `mcp.task_poll_interval_ms`: the interval a created task asks its
/// client to poll at. One second. Overridable per deployment.
pub const DEFAULT_MCP_TASK_POLL_INTERVAL_MS: u64 = 1_000;

/// Default `mcp.await_poll_interval_ms`: how often an awaited `start_run`
/// re-reads the run's projected status server-side. Five hundred milliseconds —
/// fast enough that a short run's completion is not noticeably delayed, slow
/// enough that a long run costs one history read every half second.
pub const DEFAULT_MCP_AWAIT_POLL_INTERVAL_MS: u64 = 500;

/// Operator-facing message for an explicitly zero `mcp.task_poll_interval_ms`.
pub(crate) const MCP_TASK_POLL_INTERVAL_REQUIRED: &str = "mcp.task_poll_interval_ms must be a positive number of milliseconds when set (zero would ask every client to poll in a tight loop); omit mcp.task_poll_interval_ms to use the default, or set it to a positive value";

/// Operator-facing message for an explicitly zero `mcp.await_poll_interval_ms`.
pub(crate) const MCP_AWAIT_POLL_INTERVAL_REQUIRED: &str = "mcp.await_poll_interval_ms must be a positive number of milliseconds when set (zero would spin the server re-reading history without pause); omit mcp.await_poll_interval_ms to use the default, or set it to a positive value";
