//! `literal_where_derived`: the shapes that are always derived, never typed.
//!
//! # The rule, stated exactly
//!
//! Some values are facts a machine re-derives every time it needs them. A git
//! object name is one: it is what the repository says, at a commit, and a
//! human who writes forty hex digits into a document has recorded what the
//! repository said ONCE, on a day, and frozen it. A content digest is
//! another. A fleet control token is a third.
//!
//! Writing one of those by hand into a fill is the defect this refuses. It is
//! not a style preference: a hand-copied base pin is a claim about a commit
//! that nothing re-checks, and the estate has already paid for the class
//! twice. The cure is that the value arrives as a declared parameter, so the
//! thing that derives it is the thing that supplies it.
//!
//! # Why the shape set is CLOSED and small
//!
//! A rule that guessed would be worse than no rule: refusing `"HEAD"` or
//! `"main"` because they "look derived" would make the surface unusable and
//! teach authors to work around it. So exactly three shapes are named, each
//! one unambiguous — a 40-hex git object name, a 64-hex content digest, and
//! the fleet control-token shape. Nothing else is refused, and widening the
//! set is a design change, not an implementation decision.

/// What kind of derived value a literal was found to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DerivedShape {
    /// Forty lowercase hex digits: a git object name.
    GitObjectName,
    /// Sixty-four lowercase hex digits: a content digest.
    ContentDigest,
    /// The fleet control-token shape, `<run>-ctl-<8 hex>-<who>-<8 digits>`.
    ControlToken,
}

impl DerivedShape {
    /// What derives this value, said to the author who wrote it by hand.
    #[must_use]
    pub const fn derivation(self) -> &'static str {
        match self {
            Self::GitObjectName => {
                "a git object name is what the repository answers at a commit, so it is \
                 re-derived, never transcribed"
            }
            Self::ContentDigest => {
                "a content digest is what the bytes hash to, so it is re-derived from the bytes, \
                 never transcribed"
            }
            Self::ControlToken => {
                "a control token is minted by the run that owns it, so it is supplied, never \
                 transcribed"
            }
        }
    }
}

/// The derived shape a literal run carries, with the offending bytes, or
/// `None` when the literal is ordinary text.
///
/// The literal is scanned word by word, split on the characters that separate
/// argv-ish text (whitespace, `/`, `:`, `=`, `,`, `..`), so a pin embedded in
/// a longer path is found rather than hidden by its surroundings.
#[must_use]
pub fn derived_shape_in(literal: &str) -> Option<(DerivedShape, &str)> {
    literal
        .split(|character: char| {
            character.is_whitespace()
                || matches!(character, '/' | ':' | '=' | ',' | '.' | '"' | '\'' | '@')
        })
        .filter(|run| !run.is_empty())
        .find_map(|run| shape_of(run).map(|shape| (shape, run)))
}

/// The shape one whole run is, if any.
fn shape_of(run: &str) -> Option<DerivedShape> {
    if is_lower_hex(run) {
        return match run.len() {
            40 => Some(DerivedShape::GitObjectName),
            64 => Some(DerivedShape::ContentDigest),
            _ => None,
        };
    }
    is_control_token(run).then_some(DerivedShape::ControlToken)
}

/// Whether every character is a lowercase hex digit.
fn is_lower_hex(run: &str) -> bool {
    !run.is_empty()
        && run
            .chars()
            .all(|character| character.is_ascii_digit() || ('a'..='f').contains(&character))
}

/// Whether `run` is the fleet control-token shape:
/// `<run-name>-ctl-<8 lowercase hex>-<who>-<8 digits>`.
///
/// Matched structurally rather than by regex so the shape is readable and so
/// each part's own rule is visible: the `ctl` marker is what makes the shape
/// unambiguous, and without it no amount of dashes would be a token.
fn is_control_token(run: &str) -> bool {
    let parts: Vec<&str> = run.split('-').collect();
    let [head, marker, digest, who, stamp] = parts.as_slice() else {
        return false;
    };
    *marker == "ctl"
        && !head.is_empty()
        && !who.is_empty()
        && digest.len() == 8
        && is_lower_hex(digest)
        && stamp.len() == 8
        && stamp.chars().all(|character| character.is_ascii_digit())
}

#[cfg(test)]
#[path = "derived_tests.rs"]
mod derived_tests;
