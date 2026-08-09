//! Boot-time installation of the embedded assistant into the engine catalog.
//!
//! # The rule, in one sentence
//!
//! The embedded document is loaded (and therefore routed) ONLY when the engine
//! holds no version of the assistant workflow type at all.
//!
//! # Why it is that narrow
//!
//! [`aion::Engine::load_package`] re-points routing for a package's workflow
//! type on every load, idempotent hash or not — that is correct for an operator
//! deploy (re-deploying a rolled-back version must take effect) and wrong for a
//! restart. A server that loaded the embedded document unconditionally at boot
//! would undo an operator's rollback every time the process restarted, without
//! anyone asking it to, and the session parked on the previous version would
//! find its type pointing somewhere else. So the install claims only the empty
//! case, which no operator decision can be sitting in.
//!
//! When a version IS resident the install stands down and reports which — an
//! [`AssistantInstall::AlreadyCurrent`] when the routed hash is the embedded
//! one, an [`AssistantInstall::Deferred`] naming both hashes otherwise. A
//! deferred install is not a failure: it is the live cut waiting for the
//! operator, and the log line carries the verbs that perform it.

use aion::Engine;

use super::document::{EmbeddedAssistant, embedded_assistant};

/// What the boot install did, and to what.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssistantInstall {
    /// The engine held no version of the assistant type, so the embedded
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
        /// The routed hash, or `None` when no resident version holds the route.
        routed_hash: Option<String>,
    },
    /// The install could not be attempted or did not complete. The server keeps
    /// serving; the assistant is simply not installed by this boot.
    Failed {
        /// What refused, verbatim.
        reason: String,
    },
}

impl AssistantInstall {
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

/// Installs the embedded assistant into `engine` under the rule above, logging
/// exactly what happened.
///
/// Never returns an error: a server whose assistant could not be installed is a
/// server with no assistant, not a server that must refuse to boot. The outcome
/// is returned (and logged) so the caller and `/assistant` can both say so.
pub async fn install_embedded_assistant(engine: &Engine) -> AssistantInstall {
    let embedded = match embedded_assistant() {
        Ok(embedded) => embedded,
        Err(error) => {
            let reason = error.to_string();
            tracing::error!(
                operation = "assistant.install",
                outcome = "failed",
                %reason,
                "the embedded assistant document could not be prepared; this server has no \
                 built-in assistant"
            );
            return AssistantInstall::Failed { reason };
        }
    };
    let outcome = install_verified(engine, embedded).await;
    log_outcome(&outcome);
    outcome
}

/// Installs the embedded assistant into the engine `state` is serving.
///
/// The server's own boot path. An engine that cannot be resolved is reported as
/// a failed install rather than a failed boot: the transports, the console, and
/// every other surface stay up.
pub async fn install_embedded_assistant_for_server(state: &crate::ServerState) -> AssistantInstall {
    match state.engine() {
        Ok(engine) => install_embedded_assistant(engine.as_ref()).await,
        Err(error) => {
            let reason = format!("the engine is not available: {error}");
            let outcome = AssistantInstall::Failed { reason };
            log_outcome(&outcome);
            outcome
        }
    }
}

/// The install decision itself, over an already-prepared embedded assistant.
async fn install_verified(engine: &Engine, embedded: &EmbeddedAssistant) -> AssistantInstall {
    let workflow_type = embedded.workflow_type().to_owned();
    let embedded_hash = embedded.content_hash().to_string();

    let versions = match engine.list_workflow_versions() {
        Ok(versions) => versions,
        Err(error) => {
            return AssistantInstall::Failed {
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
            Ok(_) => AssistantInstall::Installed {
                workflow_type,
                content_hash: embedded_hash,
            },
            Err(error) => AssistantInstall::Failed {
                reason: format!(
                    "the embedded assistant package `{embedded_hash}` did not load: {error}"
                ),
            },
        };
    }

    let routed_hash = resident
        .iter()
        .find(|version| version.route_active)
        .map(|version| version.content_hash.to_string());
    if routed_hash.as_deref() == Some(embedded_hash.as_str()) {
        return AssistantInstall::AlreadyCurrent {
            workflow_type,
            content_hash: embedded_hash,
        };
    }
    AssistantInstall::Deferred {
        workflow_type,
        embedded_hash,
        routed_hash,
    }
}

/// One structured line per boot stating what the install did, and — when it
/// stood down — what the operator would run to complete the cut.
fn log_outcome(outcome: &AssistantInstall) {
    match outcome {
        AssistantInstall::Installed {
            workflow_type,
            content_hash,
        } => tracing::info!(
            operation = "assistant.install",
            outcome = outcome.outcome(),
            %workflow_type,
            %content_hash,
            "the built-in assistant was installed and routed on a catalog that held no version \
             of it"
        ),
        AssistantInstall::AlreadyCurrent {
            workflow_type,
            content_hash,
        } => tracing::info!(
            operation = "assistant.install",
            outcome = outcome.outcome(),
            %workflow_type,
            %content_hash,
            "the built-in assistant is already the routed version"
        ),
        AssistantInstall::Deferred {
            workflow_type,
            embedded_hash,
            routed_hash,
        } => tracing::warn!(
            operation = "assistant.install",
            outcome = outcome.outcome(),
            %workflow_type,
            %embedded_hash,
            // `none` means no resident version holds the route at all — not
            // that the embedded one does. Both cases stand down for the same
            // reason: this catalog already holds versions nobody here placed.
            routed_hash = routed_hash.as_deref().unwrap_or("none"),
            "the embedded assistant document is not the routed version on this catalog, and \
             routing was NOT changed — a restart must never move a route an operator chose. \
             To cut over deliberately: `aion assistant document --output assistant.awl`, then \
             `aion deploy assistant.awl` (which loads AND routes it), and only then restart \
             the worker serving its queue"
        ),
        AssistantInstall::Failed { reason } => tracing::error!(
            operation = "assistant.install",
            outcome = outcome.outcome(),
            %reason,
            "the built-in assistant was not installed"
        ),
    }
}

#[cfg(test)]
#[path = "install_tests.rs"]
mod install_tests;
