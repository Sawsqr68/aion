//! `aion worker agent` — serve the compiled-in agent harness on the queue an
//! `.awl` document declares.
//!
//! Before this verb existed, an agent worker meant a bespoke Rust binary: eleven
//! of them under `examples/`, each hand-rolling the same composition. Even the
//! harness compiled into the shipped binary by default could not be SERVED without
//! writing Rust — the composition root only announced that it composed. This verb
//! is the out-of-box agent worker, and it writes no crate.
//!
//! * [`args`] is the CONNECTION vocabulary and its resolution — where this worker
//!   dials, who it says it is, where it sits. Nothing is invented: each setting comes
//!   from a flag or a named variable, the flag wins, and every missing one is reported
//!   together, by name. The LAUNCH settings are not here; they are declared in the
//!   document's `harness` section and the flags that carried them were deleted.
//! * [`surface`] derives the typed action surface from the document (through
//!   [`crate::worker_surface`], shared with `aion worker shell`) and renders it for
//!   `--check`.
//! * [`serve`] reads the document's declared harness, resolves it against the adapters
//!   this build links, honours `--check`, and runs the liminal serve loop.
//!
//! The verb is harness-NEUTRAL: the document names the kind and [`crate::harness`], the
//! composition root, is the only code that turns that name into an adapter — so nothing
//! here names a concrete one. A build that carries the declared kind resolves it, a
//! build that does not refuses naming what it carries, and a build carrying none still
//! reports the surface and then refuses to serve.

mod args;
pub(super) mod serve;
mod surface;

#[cfg(test)]
mod tests;

pub(crate) use args::AgentArgs;
pub(crate) use serve::run;
