//! The release changelog, embedded so the installed binary carries it.
//!
//! The console's "What's new" panel answers "what changed in the version I am
//! running" — a question about the BINARY, not about whatever tree happens to
//! sit on the operator's disk. A crates.io install has no repository at all,
//! and a repository checkout may be newer or older than the running server.
//! So the changelog ships inside the binary, embedded at compile time from
//! the crate-local `changelog-embed/` copy — exactly as `assistant-embed/`,
//! `ops-console-embed/`, and aion-awl's `guide-embed/` do — and is never read
//! from disk at runtime.
//!
//! The authored document lives at the repository root (`CHANGELOG.md`);
//! `changelog_embed_matches_the_authored_document` pins the two
//! byte-identical, so a release that edits one without the other fails its
//! gates instead of shipping a stale panel.

/// The complete changelog text, newest release first, as authored at the
/// repository root and embedded at compile time.
#[must_use]
pub fn changelog_text() -> &'static str {
    // Relative to this source file: up out of `src/` to the crate root, where
    // the packaged copy lives.
    include_str!("../changelog-embed/CHANGELOG.md")
}

#[cfg(test)]
mod tests {
    use super::changelog_text;

    #[test]
    fn the_embedded_changelog_is_nonempty_and_leads_with_the_title() {
        let text = changelog_text();
        assert!(
            text.starts_with("# Changelog"),
            "the embedded changelog must lead with its title heading; got: {:?}",
            text.lines().next()
        );
        assert!(
            text.contains("\n## "),
            "the embedded changelog must contain at least one release section"
        );
    }

    #[test]
    fn changelog_embed_matches_the_authored_document() -> Result<(), std::io::Error> {
        let authored_path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../CHANGELOG.md");
        let authored = std::fs::read_to_string(&authored_path)?;
        assert_eq!(
            changelog_text(),
            authored,
            "changelog-embed/CHANGELOG.md has drifted from the authored \
             CHANGELOG.md at the repository root; sync it with: \
             cp CHANGELOG.md crates/aion-server/changelog-embed/CHANGELOG.md"
        );
        Ok(())
    }
}
