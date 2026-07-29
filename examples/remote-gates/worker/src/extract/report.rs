//! What one leg's stream yielded, findings and denominator together.

use crate::types::Finding;

/// The result of decoding one leg's machine-readable stream.
///
/// `denominator` is the load-bearing half. A leg that reports no findings has
/// only proved something if its stream also proved it looked at anything, so the
/// count of well-formed "this unit was processed" records travels with the
/// findings and is checked before any green verdict is allowed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StreamReport {
    /// Every diagnostic decoded from the stream, in stream order.
    pub findings: Vec<Finding>,
    /// Well-formed processed-unit records seen on the same stream.
    pub denominator: i64,
    /// Lines the decoder could not read as records of the expected shape.
    pub unparsed: i64,
}

impl StreamReport {
    /// True when the stream yielded neither a finding nor a processed unit —
    /// the shape that must never be reported as a green gate.
    #[must_use]
    pub fn is_silent(&self) -> bool {
        self.denominator == 0 && self.findings.is_empty()
    }

    /// Appends a finding produced outside the stream decode itself.
    pub fn push(&mut self, finding: Finding) {
        self.findings.push(finding);
    }
}
