//! What the server says about the assistant it carries.
//!
//! The description is DERIVED on every read: the identity and contract come
//! from the embedded document's compiled form, and residency comes from the
//! engine catalog at request time. Nothing is remembered from boot, so a
//! description cannot go stale behind an operator's `aion deploy` or
//! `aion route` between restarts.

use aion::Engine;
use serde::Serialize;
use serde_json::Value;

use super::document::{
    CONTINUE_END_FIELD, CONTINUE_MESSAGE_FIELD, CONTINUE_SIGNAL, EMBEDDED_ASSISTANT_FILENAME,
    EmbeddedAssistant, OBJECTIVE_INPUT, REPO_PATH_INPUT, STATUS_QUERY,
};

/// Where the embedded document stands in the engine catalog right now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum AssistantResidency {
    /// The embedded version is loaded and holds the route: starting the
    /// assistant runs this document.
    Routed,
    /// The embedded version is loaded but another version holds the route (or
    /// nothing does). Starting the assistant does not run this document.
    LoadedNotRouted {
        /// The routed hash, when some resident version holds the route.
        routed_hash: Option<String>,
    },
    /// The embedded version is not in the catalog at all. `routed_hash` names
    /// whatever version of the type does hold the route; `None` means the type
    /// is unstartable on this server.
    NotLoaded {
        /// The routed hash, when some other version holds the route.
        routed_hash: Option<String>,
    },
    /// The catalog could not be read, so residency is unknown — never guessed.
    Unknown {
        /// Why the catalog could not be read.
        reason: String,
    },
}

/// The names the operator surfaces bind to, verified against the document at
/// load time. Published so a client drives the session by reading rather than
/// by restating them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssistantSessionContract {
    /// Start input carrying the operator's opening ask.
    pub objective_input: &'static str,
    /// Start input carrying the repository path (empty = scratch mode).
    pub repo_path_input: &'static str,
    /// The signal a parked session listens on.
    pub continue_signal: &'static str,
    /// Continuation field carrying the next prompt.
    pub message_field: &'static str,
    /// Continuation field that ends the session.
    pub end_field: &'static str,
    /// The read-only status query.
    pub status_query: &'static str,
}

impl AssistantSessionContract {
    /// The contract this binary was built with.
    #[must_use]
    pub const fn current() -> Self {
        Self {
            objective_input: OBJECTIVE_INPUT,
            repo_path_input: REPO_PATH_INPUT,
            continue_signal: CONTINUE_SIGNAL,
            message_field: CONTINUE_MESSAGE_FIELD,
            end_field: CONTINUE_END_FIELD,
            status_query: STATUS_QUERY,
        }
    }
}

/// One declared signal and its payload schema.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssistantSignal {
    /// Signal name.
    pub name: String,
    /// Payload schema, draft 2020-12, derived from the document.
    pub input_schema: Value,
}

/// The served description of the built-in assistant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssistantDescriptor {
    /// The workflow type an operator starts.
    pub workflow_type: String,
    /// The private task queue a worker must serve for a session to run.
    ///
    /// Published because an operator with no checkout has no other way to
    /// learn it, and because getting it wrong is silent: a worker on the wrong
    /// queue connects, registers, and never receives a dispatch. Derived from
    /// the document's own identity and never `default` — see
    /// [`super::document::private_task_queue`].
    pub task_queue: String,
    /// The embedded document's content hash — its version identity.
    pub content_hash: String,
    /// The document's filename inside the binary.
    pub document_filename: &'static str,
    /// Length of the embedded document in bytes, so a client can tell a
    /// truncated fetch of `/assistant/document` from a complete one.
    pub document_bytes: usize,
    /// Start input schema, draft 2020-12, derived from the document.
    pub input_schema: Value,
    /// Every declared signal with its payload schema.
    pub signals: Vec<AssistantSignal>,
    /// Every declared query name, in document order.
    pub queries: Vec<String>,
    /// The verified session contract.
    pub session: AssistantSessionContract,
    /// Where the embedded version stands in the catalog right now.
    pub residency: AssistantResidency,
}

/// Describes `embedded` against `engine`'s current catalog.
#[must_use]
pub fn describe(embedded: &EmbeddedAssistant, engine: &Engine) -> AssistantDescriptor {
    AssistantDescriptor {
        workflow_type: embedded.workflow_type().to_owned(),
        task_queue: embedded.task_queue().to_owned(),
        content_hash: embedded.content_hash().to_string(),
        document_filename: EMBEDDED_ASSISTANT_FILENAME,
        document_bytes: embedded.source().len(),
        input_schema: embedded.input_schema().clone(),
        signals: embedded
            .signals()
            .iter()
            .map(|signal| AssistantSignal {
                name: signal.name.clone(),
                input_schema: signal.input_schema.clone(),
            })
            .collect(),
        queries: embedded.queries().to_vec(),
        session: AssistantSessionContract::current(),
        residency: residency(embedded, engine),
    }
}

/// Reads the catalog and classifies the embedded version's standing.
fn residency(embedded: &EmbeddedAssistant, engine: &Engine) -> AssistantResidency {
    let versions = match engine.list_workflow_versions() {
        Ok(versions) => versions,
        Err(error) => {
            return AssistantResidency::Unknown {
                reason: format!("the engine catalog could not be read: {error}"),
            };
        }
    };
    let embedded_hash = embedded.content_hash().to_string();
    let resident: Vec<_> = versions
        .into_iter()
        .filter(|version| version.workflow_type == embedded.workflow_type())
        .collect();
    let loaded = resident
        .iter()
        .any(|version| version.content_hash.to_string() == embedded_hash);
    let routed_hash = resident
        .iter()
        .find(|version| version.route_active)
        .map(|version| version.content_hash.to_string());
    if !loaded {
        return AssistantResidency::NotLoaded { routed_hash };
    }
    if routed_hash.as_deref() == Some(embedded_hash.as_str()) {
        AssistantResidency::Routed
    } else {
        AssistantResidency::LoadedNotRouted { routed_hash }
    }
}
