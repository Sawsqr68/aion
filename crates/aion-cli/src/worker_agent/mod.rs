//! `aion worker agent` — serve the compiled-in agent harness on the queue an
//! `.awl` document declares.
//!
//! Before this verb existed, an agent worker meant a bespoke Rust binary: eleven
//! of them under `examples/`, each hand-rolling the same composition. Even the
//! harness compiled into the shipped binary by default could not be SERVED without
//! writing Rust — the composition root only announced that it composed. This verb
//! is the out-of-box agent worker, and it writes no crate.
//!
//! * [`args`] is the flag/environment vocabulary and its resolution. Nothing is
//!   invented: every connection setting comes from a flag or a named variable, the
//!   flag wins, and every missing one is reported together, by name.
//! * [`surface`] derives the typed action surface from the document (through
//!   [`crate::worker_surface`], shared with `aion worker shell`) and renders it for
//!   `--check`.
//! * [`serve`] resolves WHICH harness to compose, honours `--check`, and runs the
//!   liminal serve loop.
//!
//! The verb is harness-NEUTRAL: `--harness` and the per-harness configuration flags are
//! flattened in from [`crate::harness`], the composition root, so nothing here names a
//! concrete adapter. A build carrying one harness derives it, a build carrying several
//! refuses until one is selected, and a build carrying none still reports the surface and
//! then refuses to serve.

mod args;
pub(super) mod serve;
mod surface;

#[cfg(test)]
mod tests;

pub(crate) use args::AgentArgs;
pub(crate) use serve::run;
