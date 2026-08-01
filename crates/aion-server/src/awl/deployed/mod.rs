//! The read-only deployed-AWL surface: what a DEPLOYED package version
//! carries, never what the operator's workspace holds.
//!
//! Everything here reads. Nothing in this module writes to the workspace, to
//! the package store, or to any durable location; the single filesystem use is
//! the process-private staging directory in [`projection`], which exists for
//! the length of one call and holds only bytes that are being returned in the
//! same response. See that module for the invariant it states and pins.
//!
//! Source of truth is [`aion_package::Package::awl`] read back from the
//! archive persisted by the deploy, so a version that was deployed before AWL
//! source archiving existed, or was authored in Gleam, reports its absence as
//! a first-class state rather than an error or a silence.

mod document;
#[cfg(test)]
pub(crate) mod fixtures;
mod list;
mod projection;
mod service;
#[cfg(test)]
mod tests;
mod types;

pub use service::{document as read_document, versions as list_versions};
pub use types::{DeployedDocument, DeployedError, DeployedVersion};
