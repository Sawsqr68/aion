//! `aion worker awl` — serve the queue a WORKER DOCUMENT declares.
//!
//! The third serving path, and the only one whose wiring is typed by the
//! language. `aion worker shell` wires a queue's actions to host commands
//! through a strict TOML manifest that carries no types; an SDK worker writes
//! a crate; this verb reads a `.awl` document whose opening declaration is
//! `worker <queue>`, and serves exactly what that document says.
//!
//! # What it registers, and how it is admitted
//!
//! The advertised surface is derived through `aion_awl::compile_worker_document`,
//! which shares its contract derivation with the deploy path — so this worker's
//! advertisement and the server's stored contract for the queue are two
//! INDEPENDENT derivations of two documents, and the server's existing
//! worker-contract admission is what holds them equal. A worker document that
//! has drifted from the workflow that calls it is refused at registration, by
//! field, exactly as a stale SDK worker build is. That is the whole point of
//! splitting the declaration across two files with a gate in between.
//!
//! # What executes
//!
//! Every action runs through `aion_worker::shell::ShellAction` — the SAME
//! executor the server uses for a workflow document's declared bodies. Argv
//! parsing, parameter substitution, the cleared environment, process-group
//! containment and the failure classification are therefore one
//! implementation, not two that happen to agree today. The document's capture
//! selector decides only what a SUCCESSFUL command's result is: the outcome
//! record, its trimmed stdout as a string, or its stdout decoded as JSON.

mod args;
mod serve;

#[cfg(test)]
mod tests;

pub(crate) use args::AwlWorkerArgs;
pub(crate) use serve::run;
