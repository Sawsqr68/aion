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
//!
//! # Nothing moves quietly between queues
//!
//! The embedded assistant serves a private queue derived from its own identity
//! ([`super::document::private_task_queue`]), and older versions of the
//! document served `default` — the queue every out-of-box worker comes up on
//! (#200). A catalog carried across that change therefore has a routed
//! assistant on one queue and an embedded assistant on another.
//!
//! The install does not resolve that, because resolving it would mean
//! re-pointing a route the operator placed. What it does instead is SAY it: a
//! deferred install whose routed version declares a different queue logs at
//! WARN naming the queue the routed version serves, the queue the embedded
//! document would claim, why the two differ, and that routing was not touched.
//! The move happens when the operator deploys, and it is announced before,
//! not discovered after.

use aion::{Engine, WorkflowVersionInfo};
use aion_package::ContentHash;

use super::document::{EmbeddedAssistant, embedded_assistant};

/// What the routed resident version declares as its task queue.
///
/// Three distinguishable answers, because "the routed assistant is not on the
/// embedded queue" and "nobody could read what queue it is on" must never be
/// reported as the same thing: one is a migration to announce, the other is an
/// unreadable catalog entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoutedQueues {
    /// No resident version holds the route, so there is no queue to name.
    NoRoutedVersion,
    /// The routed version's contract declares these queues, in contract order.
    Declared(Vec<String>),
    /// The routed version's queues could not be read. The reason, verbatim.
    Unreadable(String),
}

impl RoutedQueues {
    /// Whether a deploy of the embedded document would move the assistant onto
    /// a queue the routed version does not already serve.
    ///
    /// Only [`Self::Declared`] can answer: an unreadable entry and an absent
    /// route both mean "unknown", and an unknown is never reported as a move.
    #[must_use]
    pub fn moves_to(&self, embedded_queue: &str) -> bool {
        match self {
            Self::Declared(queues) => !queues.iter().any(|queue| queue == embedded_queue),
            Self::NoRoutedVersion | Self::Unreadable(_) => false,
        }
    }

    /// A log-ready rendering: the queues, the absence, or the failure.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::Declared(queues) if queues.is_empty() => String::from("none declared"),
            Self::Declared(queues) => queues.join(", "),
            Self::NoRoutedVersion => String::from("no routed version"),
            Self::Unreadable(reason) => format!("unreadable: {reason}"),
        }
    }
}

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
        /// The private queue this install just claimed. Never `default`.
        task_queue: String,
    },
    /// A version was already resident AND routed, and it is the embedded one.
    AlreadyCurrent {
        /// The workflow type.
        workflow_type: String,
        /// The routed content hash, equal to the embedded one.
        content_hash: String,
        /// The private queue the routed version serves. Never `default`.
        task_queue: String,
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
        /// The queue the embedded document would claim on deploy.
        embedded_queue: String,
        /// The queue(s) the routed version actually serves right now.
        routed_queues: RoutedQueues,
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

    /// Whether this outcome is a deferred install that ALSO holds the assistant
    /// on a different queue from the one the embedded document declares.
    ///
    /// The queue migration #200 refuses to perform silently: this is the
    /// condition the boot log announces at WARN, and the condition a test can
    /// pin without reading log text.
    #[must_use]
    pub fn defers_a_queue_move(&self) -> bool {
        match self {
            Self::Deferred {
                embedded_queue,
                routed_queues,
                ..
            } => routed_queues.moves_to(embedded_queue),
            Self::Installed { .. } | Self::AlreadyCurrent { .. } | Self::Failed { .. } => false,
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
                task_queue: embedded.task_queue().to_owned(),
            },
            Err(error) => AssistantInstall::Failed {
                reason: format!(
                    "the embedded assistant package `{embedded_hash}` did not load: {error}"
                ),
            },
        };
    }

    let routed = resident.iter().find(|version| version.route_active);
    let routed_hash = routed.map(|version| version.content_hash.to_string());
    if routed_hash.as_deref() == Some(embedded_hash.as_str()) {
        return AssistantInstall::AlreadyCurrent {
            workflow_type,
            content_hash: embedded_hash,
            task_queue: embedded.task_queue().to_owned(),
        };
    }
    let routed_queues = routed.map_or(RoutedQueues::NoRoutedVersion, |version| {
        routed_queues(engine, version)
    });
    AssistantInstall::Deferred {
        workflow_type,
        embedded_hash,
        routed_hash,
        embedded_queue: embedded.task_queue().to_owned(),
        routed_queues,
    }
}

/// Reads the queues the routed version's own contract declares.
///
/// Read from the CATALOG entry for that exact `(workflow_type, content_hash)`,
/// never from the embedded document: the whole point is to say what the version
/// an operator placed is serving, which the binary's own document cannot know.
/// Every failure to read is reported as [`RoutedQueues::Unreadable`] carrying
/// the reason — an unknown queue is never rendered as an empty one, because
/// "declares nothing" and "could not be read" lead an operator to opposite acts.
fn routed_queues(engine: &Engine, version: &WorkflowVersionInfo) -> RoutedQueues {
    let hash: &ContentHash = &version.content_hash;
    let loaded = match engine.workflow_catalog().get(&version.workflow_type, hash) {
        Ok(Some(loaded)) => loaded,
        Ok(None) => {
            return RoutedQueues::Unreadable(format!(
                "the catalog lists `{}` at `{hash}` but holds no entry for it",
                version.workflow_type
            ));
        }
        Err(error) => {
            return RoutedQueues::Unreadable(format!("the catalog could not be read: {error}"));
        }
    };
    match loaded.contract() {
        Ok(contract) => RoutedQueues::Declared(
            contract
                .workers
                .iter()
                .map(|worker| worker.task_queue.clone())
                .collect(),
        ),
        Err(error) => RoutedQueues::Unreadable(format!(
            "the routed version's contract could not be read: {error}"
        )),
    }
}

/// One structured line per boot stating what the install did, and — when it
/// stood down — what the operator would run to complete the cut.
fn log_outcome(outcome: &AssistantInstall) {
    match outcome {
        AssistantInstall::Installed {
            workflow_type,
            content_hash,
            task_queue,
        } => tracing::info!(
            operation = "assistant.install",
            outcome = outcome.outcome(),
            %workflow_type,
            %content_hash,
            %task_queue,
            "the built-in assistant was installed and routed on a catalog that held no version \
             of it; it serves its own private queue, so `default` is free for the workers an \
             operator brings up"
        ),
        AssistantInstall::AlreadyCurrent {
            workflow_type,
            content_hash,
            task_queue,
        } => tracing::info!(
            operation = "assistant.install",
            outcome = outcome.outcome(),
            %workflow_type,
            %content_hash,
            %task_queue,
            "the built-in assistant is already the routed version"
        ),
        AssistantInstall::Deferred {
            workflow_type,
            embedded_hash,
            routed_hash,
            embedded_queue,
            routed_queues,
        } => {
            // `none` means no resident version holds the route at all — not
            // that the embedded one does. Both cases stand down for the same
            // reason: this catalog already holds versions nobody here placed.
            let routed_hash = routed_hash.as_deref().unwrap_or("none");
            let routed_queue = routed_queues.describe();
            if outcome.defers_a_queue_move() {
                // THE LOUD LINE. This catalog's assistant serves a queue the
                // embedded document no longer declares, and the deploy that
                // fixes that is a QUEUE MOVE: the worker serving the old queue
                // stops receiving the assistant's dispatches the moment the
                // new version routes. Nothing here performs it. It is named in
                // full — from, to, and why — so the move is a decision the
                // operator makes with the facts, not one they reconstruct from
                // a stalled session afterwards.
                tracing::warn!(
                    operation = "assistant.install",
                    outcome = outcome.outcome(),
                    %workflow_type,
                    %embedded_hash,
                    routed_hash,
                    from_task_queue = %routed_queue,
                    to_task_queue = %embedded_queue,
                    "QUEUE MOVE PENDING: the assistant routed on this catalog serves task queue \
                     `{routed_queue}`, and the assistant embedded in this binary declares the \
                     private queue `{embedded_queue}` instead — the built-in assistant no longer \
                     claims `default`, because that is the queue every out-of-box worker comes up \
                     on and a worker refused there is a worker that never starts (#200). NOTHING \
                     WAS MOVED: routing is untouched and the sessions on the routed version keep \
                     running on `{routed_queue}`. To make the move deliberately: `aion assistant \
                     document --output assistant.awl`, then `aion deploy assistant.awl` (which \
                     loads AND routes it), and then start a worker on `{embedded_queue}` — a \
                     worker still serving `{routed_queue}` will not receive the new version's \
                     dispatches"
                );
            } else {
                tracing::warn!(
                    operation = "assistant.install",
                    outcome = outcome.outcome(),
                    %workflow_type,
                    %embedded_hash,
                    routed_hash,
                    routed_task_queue = %routed_queue,
                    embedded_task_queue = %embedded_queue,
                    "the embedded assistant document is not the routed version on this catalog, \
                     and routing was NOT changed — a restart must never move a route an operator \
                     chose. To cut over deliberately: `aion assistant document --output \
                     assistant.awl`, then `aion deploy assistant.awl` (which loads AND routes \
                     it), and only then restart the worker serving its queue"
                );
            }
        }
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
