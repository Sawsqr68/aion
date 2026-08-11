//! Boot-time installation of the embedded update-check document into the
//! engine catalog.
//!
//! # The same rule as the assistant, for the same reason
//!
//! The embedded document is loaded (and therefore routed) ONLY when the
//! engine holds no version of the update-check workflow type at all.
//! [`aion::Engine::load_package`] re-points routing for a package's workflow
//! type on every load — correct for an operator deploy, wrong for a restart:
//! a server that loaded the embedded document unconditionally at boot would
//! undo an operator's deliberate deploy every time the process restarted. See
//! [`crate::assistant::install`], whose policy this mirrors verbatim.
//!
//! # Installing is not checking
//!
//! This makes the check STARTABLE, nothing more. No workflow is started, no
//! network is touched, nothing is fetched: the check is manual-only, and every
//! run of it is an explicit operator act. [`install_tests`] pins that a boot
//! install leaves zero `update_check` executions behind.

use aion::Engine;

use super::document::{EmbeddedUpdateCheck, embedded_update_check};

/// What the boot install did, and to what.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateCheckInstall {
    /// The engine held no version of the update-check type, so the embedded
    /// document was loaded and routed.
    Installed {
        /// The workflow type now routed.
        workflow_type: String,
        /// The embedded content hash that took the route.
        content_hash: String,
    },
    /// A version was already resident AND routed, and it is the embedded one.
    AlreadyCurrent {
        /// The workflow type.
        workflow_type: String,
        /// The routed content hash, equal to the embedded one.
        content_hash: String,
    },
    /// Versions are resident but the embedded one does not hold the route.
    /// The cut is the operator's to make.
    Deferred {
        /// The workflow type.
        workflow_type: String,
        /// The embedded document's content hash.
        embedded_hash: String,
        /// The routed hash, or `None` when no resident version holds the
        /// route.
        routed_hash: Option<String>,
    },
    /// The install could not be attempted or did not complete. The server
    /// keeps serving; the update check is simply not installed by this boot.
    Failed {
        /// What refused, verbatim.
        reason: String,
    },
}

impl UpdateCheckInstall {
    /// A stable label for logs and metrics.
    #[must_use]
    pub const fn outcome(&self) -> &'static str {
        match self {
            Self::Installed { .. } => "installed",
            Self::AlreadyCurrent { .. } => "already_current",
            Self::Deferred { .. } => "deferred",
            Self::Failed { .. } => "failed",
        }
    }
}

/// Installs the embedded update-check document into `engine` under the rule
/// above, logging exactly what happened.
///
/// Never returns an error: a server whose update check could not be installed
/// is a server that cannot check for updates, not a server that must refuse
/// to boot.
pub async fn install_embedded_update_check(engine: &Engine) -> UpdateCheckInstall {
    let embedded = match embedded_update_check() {
        Ok(embedded) => embedded,
        Err(error) => {
            let reason = error.to_string();
            tracing::error!(
                operation = "update_check.install",
                outcome = "failed",
                %reason,
                "the embedded update-check document could not be prepared; this server cannot \
                 run the built-in update check"
            );
            return UpdateCheckInstall::Failed { reason };
        }
    };
    let outcome = install_verified(engine, embedded).await;
    log_outcome(&outcome);
    outcome
}

/// Installs the embedded update-check document into the engine `state` is
/// serving — the server's own boot path. An engine that cannot be resolved is
/// reported as a failed install rather than a failed boot.
pub async fn install_embedded_update_check_for_server(
    state: &crate::ServerState,
) -> UpdateCheckInstall {
    match state.engine() {
        Ok(engine) => install_embedded_update_check(engine.as_ref()).await,
        Err(error) => {
            let reason = format!("the engine is not available: {error}");
            let outcome = UpdateCheckInstall::Failed { reason };
            log_outcome(&outcome);
            outcome
        }
    }
}

/// The install decision itself, over an already-prepared embedded document.
async fn install_verified(engine: &Engine, embedded: &EmbeddedUpdateCheck) -> UpdateCheckInstall {
    let workflow_type = embedded.workflow_type().to_owned();
    let embedded_hash = embedded.content_hash().to_string();

    let versions = match engine.list_workflow_versions() {
        Ok(versions) => versions,
        Err(error) => {
            return UpdateCheckInstall::Failed {
                reason: format!("the engine catalog could not be read: {error}"),
            };
        }
    };
    let resident: Vec<_> = versions
        .into_iter()
        .filter(|version| version.workflow_type == workflow_type)
        .collect();

    if resident.is_empty() {
        return match engine.load_package(embedded.package().clone()).await {
            Ok(_) => UpdateCheckInstall::Installed {
                workflow_type,
                content_hash: embedded_hash,
            },
            Err(error) => UpdateCheckInstall::Failed {
                reason: format!(
                    "the embedded update-check package `{embedded_hash}` did not load: {error}"
                ),
            },
        };
    }

    let routed_hash = resident
        .iter()
        .find(|version| version.route_active)
        .map(|version| version.content_hash.to_string());
    if routed_hash.as_deref() == Some(embedded_hash.as_str()) {
        return UpdateCheckInstall::AlreadyCurrent {
            workflow_type,
            content_hash: embedded_hash,
        };
    }
    UpdateCheckInstall::Deferred {
        workflow_type,
        embedded_hash,
        routed_hash,
    }
}

/// One structured line per boot stating what the install did, and — when it
/// stood down — what the operator would do about it.
fn log_outcome(outcome: &UpdateCheckInstall) {
    match outcome {
        UpdateCheckInstall::Installed {
            workflow_type,
            content_hash,
        } => tracing::info!(
            operation = "update_check.install",
            outcome = outcome.outcome(),
            %workflow_type,
            %content_hash,
            "the built-in update check was installed and routed on a catalog that held no \
             version of it; nothing was started — every check is an explicit operator act"
        ),
        UpdateCheckInstall::AlreadyCurrent {
            workflow_type,
            content_hash,
        } => tracing::info!(
            operation = "update_check.install",
            outcome = outcome.outcome(),
            %workflow_type,
            %content_hash,
            "the built-in update check is already the routed version"
        ),
        UpdateCheckInstall::Deferred {
            workflow_type,
            embedded_hash,
            routed_hash,
        } => tracing::warn!(
            operation = "update_check.install",
            outcome = outcome.outcome(),
            %workflow_type,
            %embedded_hash,
            routed_hash = routed_hash.as_deref().unwrap_or("none"),
            "a version of the update-check workflow type is already resident on this catalog, \
             and routing was NOT changed — a restart must never move a route an operator \
             chose. To return to the embedded document deliberately, retire the resident \
             versions (`aion unload update_check <hash>`) and restart"
        ),
        UpdateCheckInstall::Failed { reason } => tracing::error!(
            operation = "update_check.install",
            outcome = outcome.outcome(),
            %reason,
            "the built-in update check was not installed"
        ),
    }
}

#[cfg(test)]
#[path = "install_tests.rs"]
mod install_tests;
