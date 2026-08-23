//! The specimen-firing compare surface, typed over ONE record shape.
//!
//! # The invariant, stated exactly
//!
//! This surface consumes a RECORD SHAPE — a stable code, the layer that
//! produced it, and the bytes it observed — and never an enumeration of
//! causes. That is a load-bearing choice, not a stylistic one, and it is
//! forced by a mechanical fact: `CommandCause::from_code` answers `None` for
//! any string outside its own closed set, deliberately, so that an unknown
//! code is never a nearest match. A compare surface typed over that enum
//! could therefore not carry a foreign verdict AT ALL — a diff-census
//! verdict, a gate-entry named-hole verdict, a sealed verifier's
//! absent-versus-malformed pair — and the assembly layer's whole job is to
//! move such records around as data.
//!
//! So the conversion runs one way only: a builder refusal converts INTO a
//! record, and nothing converts out. The builder keeps its own
//! unrepresentable-forbidden-cause discipline where refusals are BUILT; what
//! travels through here is data, and data from a producer this cut never
//! absorbed travels exactly as well as data from one it did.
//!
//! # What this file must never contain
//!
//! A match on cause identity. Not a `match`, not an `if`, not a lookup
//! table. `the_compare_surface_matches_on_no_cause_identity` reads this
//! file's own source and refuses one — because a compare that special-cased
//! the exemplar fixture's members would pass that fixture while being
//! exactly the enumeration this surface exists not to be.

/// A record could not be built from what a producer said.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum VerdictRecordError {
    /// The code was empty or whitespace. A record that names nothing is a
    /// catch-all with no name at all.
    #[error(
        "a verdict record was built with an empty code; a record that names nothing cannot be \
         compared against anything"
    )]
    CodeEmpty,
    /// The producing layer was empty or whitespace.
    #[error(
        "a verdict record naming `{code}` was built with no producing layer; a record that cannot \
         say which discipline produced it cannot be read by one"
    )]
    LayerEmpty {
        /// The code the caller supplied.
        code: String,
    },
}

/// One verdict record, whoever produced it.
///
/// The fields are exactly what a comparison needs and nothing more: what was
/// named, who named it, and the bytes it was named over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerdictRecord {
    code: String,
    layer: String,
    observed_bytes: String,
}

impl VerdictRecord {
    /// Build a record, or say why the producer's answer is not one.
    ///
    /// # Errors
    ///
    /// Returns [`VerdictRecordError`] for an empty code or an empty layer.
    pub fn new(
        code: impl Into<String>,
        layer: impl Into<String>,
        observed_bytes: impl Into<String>,
    ) -> Result<Self, VerdictRecordError> {
        let code = code.into();
        if code.trim().is_empty() {
            return Err(VerdictRecordError::CodeEmpty);
        }
        let layer = layer.into();
        if layer.trim().is_empty() {
            return Err(VerdictRecordError::LayerEmpty { code });
        }
        Ok(Self {
            code,
            layer,
            observed_bytes: observed_bytes.into(),
        })
    }

    /// The stable code the producer named.
    #[must_use]
    pub fn code(&self) -> &str {
        &self.code
    }

    /// The layer — the discipline — that produced the record.
    #[must_use]
    pub fn layer(&self) -> &str {
        &self.layer
    }

    /// The bytes the producer observed.
    #[must_use]
    pub fn observed_bytes(&self) -> &str {
        &self.observed_bytes
    }

    /// The record as one line, for a receipt or a diagnostic.
    #[must_use]
    pub fn rendered(&self) -> String {
        format!(
            "{} at the {} layer (observed: {:?})",
            self.code, self.layer, self.observed_bytes
        )
    }
}

/// Which parts of two records disagree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComparisonParts {
    /// Whether the codes are the same string.
    pub code_agrees: bool,
    /// Whether the producing layers are the same string.
    pub layer_agrees: bool,
    /// Whether the observed bytes are the same bytes.
    pub bytes_agree: bool,
}

/// One typed comparison: what a stage declared it would measure, what it
/// measured, and exactly which parts disagreed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageComparison {
    /// The record the stage declared it would meet.
    pub expected: VerdictRecord,
    /// The record it actually met.
    pub observed: VerdictRecord,
    /// Which parts agreed.
    pub parts: ComparisonParts,
}

impl StageComparison {
    /// Whether every part agreed.
    #[must_use]
    pub const fn matched(&self) -> bool {
        self.parts.code_agrees && self.parts.layer_agrees && self.parts.bytes_agree
    }

    /// Both sides' bytes, rendered — what a refusal carries so a reader
    /// never has to re-run the stage to see what happened.
    #[must_use]
    pub fn rendered(&self) -> String {
        format!(
            "expected {}; observed {}",
            self.expected.rendered(),
            self.observed.rendered()
        )
    }
}

/// Compare two records as DATA.
///
/// Every record is consumed the same way: three string comparisons, no
/// knowledge of who produced either side, no privileged member.
#[must_use]
pub fn compare(expected: &VerdictRecord, observed: &VerdictRecord) -> StageComparison {
    StageComparison {
        expected: expected.clone(),
        observed: observed.clone(),
        parts: ComparisonParts {
            code_agrees: expected.code() == observed.code(),
            layer_agrees: expected.layer() == observed.layer(),
            bytes_agree: expected.observed_bytes() == observed.observed_bytes(),
        },
    }
}

/// This module's own source, so a test can assert what it does NOT contain.
///
/// The genericity duty is falsifiable only if something reads the mechanism
/// rather than the exemplar. This is that door.
#[must_use]
pub fn source_text() -> &'static str {
    include_str!("compare.rs")
}
