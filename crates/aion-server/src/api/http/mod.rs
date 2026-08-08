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
//! - `visibility` — visibility query-string parsing and namespace scoping.
//! - `payload` — HTTP body/payload encode-decode shapes and conversions.
//! - `build` — `GET /build`, the server's own revision.
//! - `queues` — the fleet-wide unserved-queue read.
//! - `unrecoverable` — the fleet-wide degraded-residency read.
//! - `clean_dtos` — clean JSON request/response DTOs for the workflow POST endpoints.
//! - `error` — wire-error-to-HTTP response mapping.

mod auth;
mod authoring;
mod awl;
mod awl_deployed;
mod build;
mod clean_dtos;
mod cluster_command;
mod deploy;
mod dev_ui;
mod error;
mod events;
mod history;
mod intervene;
mod outbox;
mod payload;
mod queues;
mod router;
mod schedules;
mod transcripts;
mod unrecoverable;
mod visibility;
mod whoami;
mod worker_deployments;
mod workers;
mod workflows;

#[cfg(test)]
mod events_filter_integration_tests;
#[cfg(test)]
mod history_tests;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod transcripts_tests;

pub use router::{http_router, workflow_router};
