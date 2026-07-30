//! `plan_reviews`: flatten gated items × lenses into ONE panel list, and the
//! shared panel validation both `plan_reviews` and `fold_phase` read.
//!
//! WHY FLATTEN: the review panel is item × lens, but AWL's `fork` is
//! single-level. Flattening the product into one list means the existing
//! single-level `fork` covers the whole panel with no new vocabulary and no
//! unproven nesting semantics, and it gives the console ONE flat set of
//! concurrent reviewers — which is also what an operator wants to watch on a
//! fleet run.
//!
//! PURE: no shell, no I/O, no clock. The order is deterministic (item order
//! outer, configured lens order inner) so a replay produces byte-identical
//! panel tasks.
//!
//! AN EMPTY PANEL IS A TERMINAL ERROR, not an empty list. A run configured
//! with no lenses could never accept anything — `fold_phase` requires every
//! configured lens to have reported — so it would burn every round and end
//! `exhausted` with no explanation. Failing loudly at the first fold, before
//! any agent turn, is the honest behaviour.

use std::collections::BTreeSet;

use aion_worker::ActivityFailure;

use crate::lens::require_lens_name;
use crate::types::{GatedItem, Lens, PlanReviewsInput, ReviewTask};

/// Validate the run's configured review panel: non-empty, every name usable
/// as a verdict key and a session-id token, and no duplicates.
///
/// Both `plan_reviews` (which builds the seats) and `fold_phase` (which
/// requires every seat to have reported) call this, so the two can never
/// disagree about what the panel IS.
///
/// # Errors
///
/// Terminal when the panel is empty, when a lens name is unusable, or when two
/// lenses share a name (their verdicts would silently collide onto one key and
/// their reviewer sessions onto one session id).
pub(super) fn require_panel(lenses: &[Lens]) -> Result<(), ActivityFailure> {
    if lenses.is_empty() {
        return Err(ActivityFailure::terminal(
            "the run's review panel (config.lenses) is EMPTY — an empty panel \
             could never accept any item, because every configured lens must \
             report before an item reaches done. Configure at least one lens; \
             this run cannot produce a result."
                .to_owned(),
        ));
    }
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for lens in lenses {
        require_lens_name(&lens.name).map_err(ActivityFailure::terminal)?;
        if !seen.insert(lens.name.as_str()) {
            return Err(ActivityFailure::terminal(format!(
                "the review panel configures the lens name {:?} twice — two \
                 lenses sharing a name would collide onto one verdict key and \
                 one reviewer session id",
                lens.name
            )));
        }
    }
    Ok(())
}

/// Every gated item id must be distinct: the id keys the item's verdicts, so a
/// duplicate would make the panel's verdict matching ambiguous.
fn require_distinct_items(gated: &[GatedItem]) -> Result<(), ActivityFailure> {
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for entry in gated {
        if !seen.insert(entry.item.id.as_str()) {
            return Err(ActivityFailure::terminal(format!(
                "the round produced two gated results for item {} — the item id \
                 keys its panel verdicts, so a duplicate is refused",
                entry.item.id
            )));
        }
    }
    Ok(())
}

/// `plan_reviews`: one [`ReviewTask`] per (gated item, configured lens) pair,
/// in deterministic order.
///
/// # Errors
///
/// Terminal when the configured panel is unusable (see [`require_panel`]) or
/// when the round carries two gated results for one item.
pub fn plan_reviews(input: PlanReviewsInput) -> Result<Vec<ReviewTask>, ActivityFailure> {
    let PlanReviewsInput { gated, lenses } = input;
    require_panel(&lenses)?;
    require_distinct_items(&gated)?;
    let mut panel: Vec<ReviewTask> = Vec::with_capacity(gated.len().saturating_mul(lenses.len()));
    for work in gated {
        for lens in &lenses {
            panel.push(ReviewTask {
                lens: lens.clone(),
                work: work.clone(),
            });
        }
    }
    Ok(panel)
}
