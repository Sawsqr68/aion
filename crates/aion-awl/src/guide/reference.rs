//! The full language reference, embedded so the installed binary carries it.
//!
//! The glossary's citations name sections of `AWL-REFERENCE.md`. An author
//! working from a crates.io install has no repository to resolve that path
//! against, so the document itself ships inside the crate: `aion awl guide
//! --reference` prints exactly this text.
//!
//! `guide-embed/AWL-REFERENCE.md` is git-tracked under the crate root —
//! exactly as `assistant-embed/` and `ops-console-embed/` are under
//! `aion-server` — so `cargo package` carries it and an installed build
//! holds the same bytes this repository does. The authored document lives at
//! [`REFERENCE_PATH`](super::REFERENCE_PATH), the copy assistant sessions
//! read from their repository clone; `tests/guide_reference_gate.rs` pins
//! the two byte-identical and holds every citation honest, so a divergence
//! is a failing gate, never a quiet fork.

/// The complete text of the AWL language reference.
///
/// Embedded at compile time from the crate-local `guide-embed/` copy, which
/// the reference gate pins byte-identical to the authored document at
/// [`REFERENCE_PATH`](super::REFERENCE_PATH). A section renamed in the
/// document either updates the glossary or fails the gate, and either way
/// this embed follows on the next sync.
#[must_use]
pub fn reference_text() -> &'static str {
    // Relative to this source file: up out of `src/guide/` and `src/` to the
    // crate root, where the packaged copy lives.
    include_str!("../../guide-embed/AWL-REFERENCE.md")
}
