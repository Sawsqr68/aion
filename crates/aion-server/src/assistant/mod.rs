//! The built-in assistant: the aion authoring assistant ships INSIDE the
//! server binary, present the moment `aion server` runs.
//!
//! This is the ops-console doctrine applied to a workflow document. The
//! console is compiled into the binary and served at `/` with no operator
//! step; the assistant document
//! (`crates/aion-server/assistant-embed/assistant.awl`) is compiled into the
//! binary the same way, installed into the engine at boot, and described over
//! `/assistant`. A stock `cargo install aion-cli` therefore carries the
//! assistant — no repository checkout, no example crate, no hand deploy.
//!
//! # The brain is a seam, not a dependency
//!
//! Nothing here names an agent harness. The document declares one `agent`
//! action on the assistant's own private queue; WHICH agent serves that
//! queue, and how it is launched, is declared in the DOCUMENT a worker is
//! started from — its `harness` section names the kind (norn today, ACP for
//! external agents) and every launch setting. This module installs the
//! embedded document and describes it; it neither spawns nor selects a brain.
//!
//! # The queue is the assistant's own name, never `default`
//!
//! [`document::private_task_queue`] IS the rule: the built-in assistant serves
//! the queue named after the workflow type it exports, and the embedded
//! document is refused at load if it declares anything else. `default` is
//! where an out-of-box worker comes up with no configuration, and contract
//! admission holds a registering worker against every reachable contract on
//! its queue — so an assistant sitting on `default` refused the first worker a
//! newcomer started (#200). The queue belongs to those workers.
//!
//! A catalog that predates that change still has its assistant on the old
//! queue. Nothing here moves it: [`install`] reports the divergence at WARN,
//! naming both queues and the verbs that perform the move, and leaves routing
//! exactly as the operator left it.
//!
//! # What the install will and will not do
//!
//! [`install::install_embedded_assistant`] loads the embedded package into
//! the engine ONLY when the catalog holds no version of the assistant
//! workflow type at all — a fresh home. Loading a package re-points routing
//! for its type ([`aion::Engine::load_package`]), so an unconditional boot
//! install would silently steal the route from an operator's deliberate
//! rollback, on every restart. When any version is already resident the
//! install stands down and says so, naming both hashes and the operator's own
//! cut verbs. The live cut is the operator's act, never a side effect of a
//! restart.

/// The served description of the embedded assistant: identity, contract, and
/// current residency in the engine catalog.
pub mod descriptor;
/// The embedded AWL document, its compiled identity, and its verified session
/// contract.
pub mod document;
/// Boot-time installation of the embedded document into the engine catalog.
pub mod install;

pub use descriptor::{
    AssistantDescriptor, AssistantResidency, AssistantSessionContract, AssistantSignal, describe,
};
pub use document::{
    CONTINUE_END_FIELD, CONTINUE_MESSAGE_FIELD, CONTINUE_SIGNAL, EMBEDDED_ASSISTANT_DOCUMENT,
    EMBEDDED_ASSISTANT_FILENAME, EmbeddedAssistant, EmbeddedAssistantError, OBJECTIVE_INPUT,
    REPO_PATH_INPUT, STATUS_QUERY, embedded_assistant, private_task_queue,
};
pub use install::{
    AssistantInstall, RoutedQueues, install_embedded_assistant,
    install_embedded_assistant_for_server,
};
