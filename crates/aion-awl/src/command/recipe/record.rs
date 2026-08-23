//! The one-way door from a builder refusal into a verdict record.
//!
//! It lives here, and not beside [`super::compare`], for one mechanical
//! reason: the compare surface's own source is READ by a test that refuses
//! any mention of a cause in it. Putting the conversion in that file would
//! make the genericity assertion unprovable, and an assertion that cannot
//! run is not an assertion.
//!
//! The direction is one-way by design. A refusal this builder produced
//! becomes data; data does not become a refusal. Cut-1's
//! `CommandCause::from_code` answers `None` for any string outside its
//! closed set, so a reverse conversion could only ever throw foreign records
//! away — and throwing away exactly the records the assembly layer exists to
//! carry is the failure the one-vocabulary construction was ruled to avoid.

use super::super::error::CommandRefusal;
use super::compare::{VerdictRecord, VerdictRecordError};

/// The record a builder refusal is, as data.
///
/// # Errors
///
/// Returns [`VerdictRecordError`] only if the vocabulary ever grew an empty
/// wire string, which `cause_tests.rs` pins impossible. The result is
/// returned rather than unwrapped because library code in this estate does
/// not `unwrap`, and a pin in another file is not a proof in this one.
pub fn record_of(refusal: &CommandRefusal) -> Result<VerdictRecord, VerdictRecordError> {
    VerdictRecord::new(
        refusal.cause().as_str(),
        refusal.layer().as_str(),
        refusal.observed_bytes(),
    )
}
