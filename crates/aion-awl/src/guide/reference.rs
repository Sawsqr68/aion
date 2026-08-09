//! The full language reference, embedded so the installed binary carries it.
//!
//! The glossary's citations name sections of `AWL-REFERENCE.md`. An author
//! working from a crates.io install has no repository to resolve that path
//! against, so the document itself ships inside the crate: `aion awl guide
//! --reference` prints exactly this text.

/// The complete text of the AWL language reference.
///
/// Embedded at compile time from [`REFERENCE_PATH`](super::REFERENCE_PATH)
/// (`examples/assistant/resources/AWL-REFERENCE.md`) — the very file
/// `tests/guide_reference_gate.rs` reads at test time to hold every citation
/// honest. Because the embedded copy and the gate read the SAME file, the
/// text a surface prints and the sections the gate verifies cannot drift
/// apart: a section renamed in the document either updates the glossary or
/// fails the gate, and either way this embed follows on the next build.
#[must_use]
pub fn reference_text() -> &'static str {
    // Relative to this source file: up out of `src/guide/`, `src/`,
    // `aion-awl/`, and `crates/` to the repository root.
    include_str!("../../../../examples/assistant/resources/AWL-REFERENCE.md")
}
