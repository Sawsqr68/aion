//! axum HTTP/JSON workflow facade.
//!
//! Module layout:
//! - [`router`] — public router construction (the only public surface).
//! - `workflows` — workflow management handlers.
//! - `schedules` — schedule management handlers.
//! - `events` — websocket event-subscription handlers.
//! - `deploy` — operator deploy handlers (mounted only when `[deploy].enabled`).
//! - `awl_deployed` — the read-only deployed-AWL studio surface (mounted only
//!   when `[deploy].enabled`, alongside the deploy read model it reflects).
//! - `authoring` — server-side Gleam authoring (mounted only when `[authoring].gleam_path` is set).
//! - `dev_ui` — local dev-server surface (mounted only when `[dev].enabled`).
//! - `auth` — caller-identity extraction from request headers.
//! - `routing` — shard-owner forwarding for HTTP writes (clustered builds).
//! - `visibility` — visibility query-string parsing and namespace scoping.
//! - `payload` — HTTP body/payload encode-decode shapes and conversions.
//! - `build` — `GET /build`, the server's own revision.
//! - `update_status` — `GET /update-status`, the installed version joined
//!   with the last completed manual update check.
//! - `describe_live` — the mid-step join: current step + attempt liveness +
//!   progress note + transcript tail, in one read.
//! - `assistant` — `GET /assistant`, the built-in assistant this binary carries.
//! - `queues` — the fleet-wide unserved-queue read.
//! - `managed_workers` — `GET /workers/managed`, the supervision status join,
//!   and the `POST /workers/managed/{name}/{start,stop,restart}` lifecycle
//!   commands (mounted only when `[deploy].enabled`, like the gRPC
//!   `DeployService` counterparts they mirror).
//! - `unrecoverable` — the fleet-wide degraded-residency read.
//! - `clean_dtos` — clean JSON request/response DTOs for the workflow POST endpoints.
//! - `error` — wire-error-to-HTTP response mapping.

mod assistant;
pub(crate) mod auth;
mod authoring;
mod awl;
mod awl_deployed;
mod build;
mod changelog;
mod children;
mod clean_dtos;
mod cluster_command;
mod deploy;
mod describe_live;
mod dev_ui;
mod error;
mod events;
mod history;
mod intervene;
mod managed_workers;
mod outbox;
mod payload;
mod queues;
mod router;
#[cfg(feature = "haematite-backend")]
mod routing;
mod schedules;
mod transcripts;
mod unrecoverable;
mod update_status;
mod visibility;
mod whoami;
mod worker_deployments;
mod workers;
mod workflows;

#[cfg(test)]
mod children_tests;
#[cfg(test)]
mod events_filter_integration_tests;
#[cfg(test)]
mod history_tests;
#[cfg(test)]
pub(crate) mod test_support;
#[cfg(test)]
mod transcripts_tests;

pub use router::{http_router, workflow_router};
