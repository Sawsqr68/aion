//! Review-lens identity: the one validator and the one session-suffix
//! derivation every consumer of a lens name reads.
//!
//! A lens name is not decoration. It KEYS the lens's verdict in `fold_phase`
//! (which requires every configured lens to have reported before an item can
//! reach `done`) and it rides in the reviewer's per-run `--session-id`
//! (`<workflow id>-review-<item id>-<lens name>`). Both uses fail badly on a
//! name carrying whitespace, a path separator, or nothing at all: two lenses
//! whose names differ only by surrounding space would silently be two
//! different keys, and a name with a `/` would smear a session id across a
//! directory boundary. So the name is validated ONCE, here, and every
//! consumer calls the same function.
//!
//! This is not an arbitrary limit — the character set is exactly what a
//! session-id token and a verdict key can carry without ambiguity, and the
//! refusal names it.

/// The characters a lens name may carry beyond ASCII alphanumerics. `_` is
/// deliberate: the shipped panels use `brief_compliance` and `spec_fidelity`.
const EXTRA_NAME_CHARS: [char; 3] = ['-', '_', '.'];

/// Require a lens name that can key a verdict and name a session.
///
/// # Errors
///
/// A message naming the offending name and the accepted set when the name is
/// blank, has surrounding whitespace, or carries a character outside
/// `[A-Za-z0-9]` plus `-`, `_`, `.`.
pub fn require_lens_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err(
            "a review lens carries an EMPTY name — the name keys the lens's \
             verdict and names its session, so it is required"
                .to_owned(),
        );
    }
    if name.trim() != name {
        return Err(format!(
            "review lens name {name:?} carries surrounding whitespace — two \
             lens names differing only by space would silently key two \
             different verdicts; trim it"
        ));
    }
    if !name
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || EXTRA_NAME_CHARS.contains(&character))
    {
        return Err(format!(
            "review lens name {name:?} carries a character outside \
             [A-Za-z0-9] and -_. — the name rides in the reviewer's session \
             id and keys its verdict, so it is refused"
        ));
    }
    Ok(())
}

/// The reviewer's per-run session suffix for one panel seat.
///
/// It MUST distinguish the lens as well as the item: concurrent lenses over
/// ONE item are separate reviewer runs, and a suffix naming only the item
/// would collide them onto a single Norn session — one lens resuming (and
/// seeing) another lens's transcript, which is exactly the blindness the
/// panel depends on.
#[must_use]
pub fn review_session_suffix(item_id: &str, lens_name: &str) -> String {
    format!("review-{item_id}-{lens_name}")
}

#[cfg(test)]
mod tests {
    use super::{require_lens_name, review_session_suffix};

    #[test]
    fn the_shipped_panel_names_are_accepted() -> anyhow::Result<()> {
        for name in [
            "correctness",
            "regressions",
            "brief_compliance",
            "spec_fidelity",
            "scope-fence",
            "v1.2",
        ] {
            require_lens_name(name).map_err(anyhow::Error::msg)?;
        }
        Ok(())
    }

    #[test]
    fn a_blank_or_unsafe_lens_name_is_refused_by_name() {
        for bad in [
            "",
            " correctness",
            "correctness ",
            "a/b",
            "a b",
            "a\tb",
            "é",
        ] {
            assert!(
                require_lens_name(bad).is_err(),
                "lens name {bad:?} should be refused"
            );
        }
    }

    #[test]
    fn the_session_suffix_distinguishes_the_lens_as_well_as_the_item() {
        assert_eq!(
            review_session_suffix("it-1", "correctness"),
            "review-it-1-correctness"
        );
        assert_ne!(
            review_session_suffix("it-1", "correctness"),
            review_session_suffix("it-1", "regressions"),
            "two lenses over one item must not share a session id"
        );
    }
}
