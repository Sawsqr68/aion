//! The stage- and leg-identifier charset, and why it is closed.
//!
//! **This is a derived tightening, not a contradiction, and it is flagged to
//! the design's owner as an addition the design should absorb.**
//!
//! Measured against the settled design at `e17f7484d`, §5.2 constrains stage
//! ids only to "unique, nonempty" — there is NO charset clause. But
//! `<stage-id>` interpolates directly into the canonical conclusion path
//! `.fleet/conclusions/<stage-id>` (§2.1, §5.1): author-controlled text
//! flowing into a path. §2.1's canonical authored form does say "ASCII
//! identifiers", and every exemplar in the design matches `[a-z0-9-]`.
//!
//! This module therefore closes the set to `^[a-z0-9][a-z0-9_-]*$`:
//!
//! * no `.`   — no `.`, no `..`, no trailing-dot form;
//! * no `/`   — no nesting, no absolute path;
//! * no leading separator — no `-lead` (option injection) and no `_lead`;
//! * lowercase ASCII only, and nonempty by the leading-class requirement.
//!
//! **Traversal is closed by construction, not by filtering.** The set is
//! defined by what it admits; there is no denylist to outrun. The boundary
//! is verified by test against the full nine-vector specimen set, because
//! one specimen does not cover a vector class and a synthetic arm only holds
//! the shapes its author imagined.
//!
//! Composition with the existing dispatch guard: `csv_guard`
//! (`workflows/fleet-dev/bin/fleet-lib.sh`, fn `csv_guard`) already refuses
//! any `leg_ids_csv` outside `[A-Za-z0-9,-]`. The two guards compose and the
//! stricter wins per id — `_` is refused EARLIER by `csv_guard`, uppercase
//! and a leading `-` are refused HERE. `fleet-lib.sh` is not touched.

use crate::cause::CauseCode;

/// The canonical prefix every stage's conclusion artifact lives under
/// (design §2.1, §5.1).
pub const CONCLUSION_PREFIX: &str = ".fleet/conclusions/";

/// Which kind of identifier is being checked, so the refusal names it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdKind {
    /// A stage id, which interpolates into the conclusion path.
    Stage,
    /// A dispatch-fixed leg id.
    Leg,
}

impl IdKind {
    /// The charset cause code for this kind.
    #[must_use]
    pub const fn charset_cause(self) -> CauseCode {
        match self {
            Self::Stage => CauseCode::InvalidStageIdCharset,
            Self::Leg => CauseCode::InvalidLegIdCharset,
        }
    }

    /// The human-readable noun used in refusal detail.
    #[must_use]
    pub const fn noun(self) -> &'static str {
        match self {
            Self::Stage => "stage id",
            Self::Leg => "leg id",
        }
    }
}

/// Does `id` match `^[a-z0-9][a-z0-9_-]*$`?
///
/// Hand-written rather than pulling in `regex`, which the workspace
/// dependency table does not carry. The two clauses below ARE the regex:
/// a leading `[a-z0-9]`, then any tail of `[a-z0-9_-]`.
#[must_use]
pub fn is_valid_id(id: &str) -> bool {
    let mut chars = id.chars();
    let Some(first) = chars.next() else {
        // The empty id has no leading character, so it cannot match. This
        // is charset specimen #8.
        return false;
    };
    if !first.is_ascii_lowercase() && !first.is_ascii_digit() {
        return false;
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

/// The canonical conclusion path for a stage id (design §2.1, §5.1).
///
/// The path is DERIVED from the stage id rather than authored as another
/// free-form key, which is the design's own reason for choosing it.
#[must_use]
pub fn conclusion_path(stage_id: &str) -> String {
    format!("{CONCLUSION_PREFIX}{stage_id}")
}

/// Normalize a conclusion path for the §5.2 clause-15 distinctness check.
///
/// Normalization is NFKC-free ASCII case folding: inside the accepted
/// charset it is the identity, which is exactly why clause 15 is
/// unreachable at whole-plan level while the charset clause holds. The
/// clause is still implemented and still tested directly — the same
/// defence-in-depth the design itself blesses for acyclicity ("the
/// redundant check makes the refusal named and prevents a future relaxation
/// from silently accepting cycles").
#[must_use]
pub fn normalize_conclusion_path(path: &str) -> String {
    path.to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The brief's full nine-vector specimen set, each naming its class.
    /// One specimen does not cover a vector class.
    const REFUSED_VECTORS: &[(&str, &str)] = &[
        ("../x", "parent-directory traversal"),
        ("a/b", "path separator / nesting"),
        (".", "current-directory selector"),
        ("..", "bare parent selector"),
        ("-lead", "leading `-` (option injection)"),
        ("_lead", "leading `_` (leading separator)"),
        ("Upper", "uppercase outside the charset"),
        ("", "empty id"),
        ("x.", "trailing dot"),
    ];

    #[test]
    fn all_nine_charset_vectors_are_refused() {
        for (specimen, class) in REFUSED_VECTORS {
            assert!(
                !is_valid_id(specimen),
                "charset vector {class:?} specimen {specimen:?} was ACCEPTED; \
                 the boundary is verified by test, not trusted to care"
            );
        }
    }

    #[test]
    fn no_refused_vector_can_escape_the_conclusion_prefix() {
        // The security property the charset exists for: no accepted id can
        // make the derived path leave `.fleet/conclusions/`.
        for (specimen, class) in REFUSED_VECTORS {
            if is_valid_id(specimen) {
                let path = conclusion_path(specimen);
                assert!(
                    path.starts_with(CONCLUSION_PREFIX) && !path.contains(".."),
                    "vector {class:?} escaped the prefix: {path}"
                );
            }
        }
    }

    #[test]
    fn the_design_exemplar_ids_are_accepted() {
        // Every stage id the design writes, from §2.1 and §9.1.
        for id in ["build", "finish", "l1", "l2", "l3", "a", "b", "c"] {
            assert!(is_valid_id(id), "design exemplar id {id:?} was refused");
        }
        // Every leg id the #140 exemplar writes.
        for id in ["l1-design", "l2-classify", "l3-route"] {
            assert!(is_valid_id(id), "exemplar leg id {id:?} was refused");
        }
    }

    #[test]
    fn underscore_is_accepted_in_the_tail_only() {
        assert!(is_valid_id("a_b"));
        assert!(!is_valid_id("_ab"));
    }

    #[test]
    fn digits_may_lead() {
        assert!(is_valid_id("1st"));
    }

    #[test]
    fn conclusion_path_is_the_designs_canonical_shape() {
        assert_eq!(conclusion_path("l1"), ".fleet/conclusions/l1");
    }
}
