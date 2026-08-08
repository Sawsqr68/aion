//! Worker-scaffold generation for an AWL document (`aion awl scaffold`).
//!
//! Given ONE compiled `worker` contract — the same `PackageContract` the
//! server deploys and admits against — this module renders a standalone Cargo
//! crate that serves the queue: a connection per node, a wire descriptor for
//! every registered action taken from the document itself, no handler for any
//! server-executed bodied action, and a loud terminal failure in every
//! un-implemented handler.
//!
//! Nothing here touches the filesystem; the caller writes the rendered files.

mod declaration_rs;
mod error;
mod handlers_rs;
mod main_rs;
mod manifest;
mod plan;
mod scaffold;
mod shell_manifest;
mod text;

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

pub use error::AwlScaffoldError;
pub use plan::{Connection, ConnectionPlan, ServableAction};
pub use scaffold::{
    AionDependency, AwlWorkerScaffold, DocumentRoot, FileOwnership, ScaffoldedFile, WorkerScaffold,
    scaffold_awl_worker,
};
pub use shell_manifest::emit_shell_manifest;
