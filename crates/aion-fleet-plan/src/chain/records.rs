//! The chain-level records, with EXACTLY the fields design §4 shows.
//!
//! Design §2.6's lesson, verbatim: *"a stage-scoped fact that the existing
//! child contract cannot carry belongs in a chain-scoped record, not in a
//! compatibility-breaking extension of the child."* Nothing here is a
//! `FleetDevReport` field, and nothing here changes the child contract.
//!
//! Field sets are pinned by test against the design's own block quotations.
//! "Exactly the fields the design shows, no more, no fewer" is a contract,
//! so an added convenience field is a defect, not an improvement.

use serde::{Deserialize, Serialize};

use crate::error::ConclusionCause;

/// Design §4, verbatim:
///
/// ```text
/// stage_failed {
///   stage_id,
///   child_workflow_id,
///   child_outcome,
///   cause,
///   kept_root,
///   last_accepted_commit
/// }
/// ```
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StageFailed {
    /// Which stage failed.
    pub stage_id: String,
    /// The child flight's workflow id.
    pub child_workflow_id: String,
    /// The child's terminal outcome.
    pub child_outcome: String,
    /// The child outcome's TYPED label — `unforked`, `unmerged`,
    /// `unreported`, `gates_red`, `judge_flagged`, and so on.
    ///
    /// Design §4, verbatim: *"It is never reported as `unplanned`."*
    /// [`StageFailed::new`] is the only constructor and it refuses that
    /// value, so the invariant cannot be violated by a caller's default.
    pub cause: String,
    /// The kept run root — the evidence and the salvage.
    pub kept_root: String,
    /// The last accepted commit before this stage.
    pub last_accepted_commit: String,
}

/// A `cause` that design §4 forbids.
pub const FORBIDDEN_CAUSE: &str = "unplanned";

impl StageFailed {
    /// Build a `stage_failed` record, refusing a `cause` the design forbids.
    ///
    /// # Errors
    ///
    /// Returns the offending cause when it is `unplanned` or empty. Design
    /// §4 forbids `unplanned` outright; an empty cause is a silently
    /// defaulted one, which the same clause exists to prevent. A caller
    /// that cannot recover the child's typed label must STOP and report,
    /// never fabricate.
    pub fn new(
        stage_id: impl Into<String>,
        child_workflow_id: impl Into<String>,
        child_outcome: impl Into<String>,
        cause: impl Into<String>,
        kept_root: impl Into<String>,
        last_accepted_commit: impl Into<String>,
    ) -> Result<Self, String> {
        let cause = cause.into();
        if cause == FORBIDDEN_CAUSE || cause.trim().is_empty() {
            return Err(cause);
        }
        Ok(Self {
            stage_id: stage_id.into(),
            child_workflow_id: child_workflow_id.into(),
            child_outcome: child_outcome.into(),
            cause,
            kept_root: kept_root.into(),
            last_accepted_commit: last_accepted_commit.into(),
        })
    }
}

/// Design §4, verbatim:
///
/// ```text
/// stage_conclusion {
///   stage_id,
///   child_workflow_id,
///   result_commit,
///   conclusion_path,
///   conclusion
/// }
/// ```
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StageConclusion {
    /// Which stage concluded.
    pub stage_id: String,
    /// The child flight's workflow id.
    pub child_workflow_id: String,
    /// The EXACT pushed commit the token was read from.
    pub result_commit: String,
    /// The canonical `.fleet/conclusions/<stage-id>` path.
    pub conclusion_path: String,
    /// The verified declared value.
    pub conclusion: String,
}

/// Design §4, verbatim:
///
/// ```text
/// stage_conclusion_refused {
///   stage_id,
///   child_workflow_id,
///   result_commit,
///   conclusion_path,
///   cause_code,
///   observed_bytes,
///   kept_root,
///   last_accepted_commit
/// }
/// ```
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StageConclusionRefused {
    /// Which stage's conclusion was refused.
    pub stage_id: String,
    /// The child flight's workflow id.
    pub child_workflow_id: String,
    /// The EXACT pushed commit that was read.
    pub result_commit: String,
    /// The canonical path that was read.
    pub conclusion_path: String,
    /// Exactly one of `conclusion_missing`, `conclusion_malformed`,
    /// `conclusion_undeclared`.
    pub cause_code: ConclusionCause,
    /// The bytes actually found, every one of them rendered.
    pub observed_bytes: String,
    /// The kept run root.
    pub kept_root: String,
    /// The last accepted commit before this stage.
    pub last_accepted_commit: String,
}

/// The child's dispatch and completion receipts (design §3 final ¶, §6
/// clauses 2–3).
///
/// The design's instruction is to CONSUME these shapes rather than re-read
/// refs: `CloneProbe.head_commit`, `PushProbe.pushed_commit`, and
/// `FleetDevReport.base_commit`/`.pushed_commit`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StageReceipt {
    /// Which stage this receipt is for.
    pub stage_id: String,
    /// The child flight's workflow id, recorded at dispatch.
    pub child_workflow_id: String,
    /// The EXACT base commit the stage was cut from.
    pub base_commit: String,
    /// The child's terminal outcome label.
    pub child_outcome: String,
    /// The EXACT pushed commit the child returned.
    pub pushed_commit: String,
}

/// The acceptance decision, recorded before any successor dispatch.
///
/// Design §6 clause 5: *"Only after conclusion verification, automatic
/// acceptance or a held approval records the decision and exact accepted
/// commit SHA before any successor dispatch."*
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AcceptanceDecision {
    /// Which stage was accepted.
    pub stage_id: String,
    /// The EXACT accepted result commit SHA. Design §3 clause 4: *"That
    /// SHA, not a mutable branch name and not a fresh `git rev-parse`, is
    /// the next stage's `base_ref` value."*
    pub accepted_commit: String,
    /// The advance policy that authorised it, as authored.
    pub advance: String,
    /// The conclusion that had already verified. No hold can approve a
    /// stage whose conclusion has not verified (§2.6), and acceptance
    /// carries the verified value so the ordering is legible in the record.
    pub verified_conclusion: String,
}

/// An `advance=hold` decision — a dedicated history event, because design
/// §6 requires one "for any decision not otherwise durably represented
/// before the next dispatch, especially `advance=hold` approval and
/// conditional skip".
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct HoldDecision {
    /// Which stage was held.
    pub stage_id: String,
    /// The result commit the hold was offered on.
    pub result_commit: String,
    /// The conclusion verified BEFORE the hold was offered.
    pub verified_conclusion: String,
    /// Whether the hold was approved.
    pub approved: bool,
    /// Who approved or declined, verbatim from the approval seam.
    pub approver: String,
    /// The approver's note.
    pub note: String,
}

/// A conditional skip — the other decision design §6 names explicitly.
///
/// Design §4: *"A skip record names the stage, referenced conclusion, and
/// condition."* All three are here, and the condition is the normalized
/// graph's canonical rendering, never quoted plan prose.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SkipRecord {
    /// The stage that was skipped.
    pub stage_id: String,
    /// The stage whose recorded conclusion the condition read.
    pub source_stage_id: String,
    /// The conclusion observed on that source stage.
    pub observed_conclusion: String,
    /// The canonical condition, as normalized.
    pub condition: String,
}
