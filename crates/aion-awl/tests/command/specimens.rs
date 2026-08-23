//! The specimen corpus: one refusing document per cause code, and the two
//! canonical accepting shapes every other family reads.
//!
//! RED FIRST is a property of this file's existence, not of its comments: the
//! refusal specimens below were written before the passing paths, and
//! `every_cause_has_a_refusing_specimen` in `command_refusals.rs` fails the
//! build if any cause code in the closed vocabulary lacks one.

/// The narration every specimen opens with (the parser requires one).
pub const NARRATION: &str = "//! Command-surface specimen.\n";

/// A whole workflow document wrapping `command_text`.
///
/// Whole documents rather than fragments, because a command declaration is a
/// TOP-LEVEL declaration and a fragment would not exercise the dispatch that
/// reaches it.
#[must_use]
pub fn document(command_text: &str) -> String {
    format!(
        "{NARRATION}workflow specimen\n  outcome done: type String, route success\n\n\
         {command_text}\n\nstep go\n  route done(\"ok\")\n"
    )
}

/// THE CANONICAL ADMITTING FIXTURE — the wall-flight2 cure.
///
/// `git tag -- <name> <sha>`: a tag name beginning with `-` is read as an
/// option by `git tag` unless the end-of-options marker stands in front of
/// it. The declaration states that `git tag` admits the marker, and the
/// emitter then places it unconditionally.
pub const GIT_TAG: &str = "command tag_release(tag_name: String, commit: String)\n  \
                           program \"git\" \"tag\"\n  admits \"--\"\n  \
                           operand name: String = \"{{tag_name}}\"\n  \
                           operand sha: String = \"{{commit}}\"";

/// THE CANONICAL REFUSING FIXTURE — the diff census.
///
/// `git diff --no-ext-diff --name-only <BASE>..HEAD`. This invocation must
/// NOT get a `--`: git would read what follows as a PATHSPEC, silently
/// turning a revision range into a path and changing what the census
/// measures. So it declares that it refuses the marker, and carries the
/// mandatory specimen that proves the leading-dash refusal fires.
pub const DIFF_CENSUS: &str = "command diff_census(base_pin: String)\n  \
                               program \"git\" \"diff\"\n  refuses \"--\"\n  \
                               specimen \"-x\"\n  option \"--no-ext-diff\"\n  \
                               option \"--name-only\"\n  \
                               operand range: String = \"{{base_pin}}..HEAD\"";
