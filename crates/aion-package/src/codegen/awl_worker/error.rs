//! Refusals from the AWL worker scaffold generator.
//!
//! Every one of these is a case where a crate COULD be emitted and would then
//! fail to compile, fail admission, or serve nothing. A generated skeleton
//! that does not compile is worse than no skeleton, so each is a refusal at
//! generation time carrying what the author must change.

/// Why a worker scaffold could not be generated.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AwlScaffoldError {
    /// Every action the queue declares carries a declarative body, so the
    /// server executes all of them and there is no worker to write.
    #[error(
        "the document's `worker {task_queue}` block declares no action for an out-of-band \
         worker: every action it declares carries a `run` body, which the server executes \
         itself. There is nothing for a worker to serve."
    )]
    NoServableAction {
        /// The queue (worker block) that declares only bodied actions.
        task_queue: String,
    },
    /// The queue has node-pinned worker work whose topology the shell runtime
    /// cannot express.
    #[error(
        "the document's `worker {task_queue}` block has node-pinned worker-owed actions, but \
         `aion worker shell` cannot express node topology: it has no node flag, registers with \
         the machine hostname as its node, and its startup check demands the whole worker-owed \
         action set regardless of pins. Use `aion awl scaffold`'s Rust crate, which opens one \
         connection per node, for this queue."
    )]
    NodePinnedQueue {
        /// The queue containing at least one worker-owed node pin.
        task_queue: String,
    },
    /// An action name that cannot name a Rust function.
    #[error(
        "action `{action}` of worker `{task_queue}` is not a Rust identifier, so the generated \
         crate could not name a handler function after it"
    )]
    ActionNameNotAnIdentifier {
        /// The queue declaring the action.
        task_queue: String,
        /// The offending action name.
        action: String,
    },
    /// An action named after one of the three keywords with no raw form.
    #[error(
        "action `{action}` of worker `{task_queue}` is named after the Rust keyword `{action}`, \
         which has no raw-identifier form, so the generated crate could not name a handler \
         function after it"
    )]
    ActionNameUnnameable {
        /// The queue declaring the action.
        task_queue: String,
        /// The offending action name.
        action: String,
    },
    /// A crate name Cargo would refuse.
    #[error(
        "`{crate_name}` is not a usable Cargo package name; it must be non-empty and contain \
         only ASCII letters, digits, `-`, and `_`"
    )]
    CrateNameInvalid {
        /// The rejected package name.
        crate_name: String,
    },
    /// An empty path where the generated crate needs one to reach the
    /// document.
    #[error(
        "the generated crate needs a {role} path to reach the AWL document, and an empty one \
         was supplied"
    )]
    DocumentPathEmpty {
        /// Which path was empty: `include` or `directory`.
        role: &'static str,
    },
    /// A schema that could not be rendered into the handler documentation.
    #[error("the declared {role} schema of action `{action}` could not be rendered: {reason}")]
    SchemaUnrenderable {
        /// The action whose schema failed to render.
        action: String,
        /// `input` or `output`.
        role: &'static str,
        /// The serialization failure.
        reason: String,
    },
}
