//! Turning a decided [`ChainAction`] plus its external result into the one
//! [`Recorded`] fact that advances the ledger.
//!
//! Every boundary design §6 names is one call to [`apply`]. The split
//! matters for replay: the DECISION ([`crate::chain::next_action`]) is pure
//! and re-derivable, and the EXTERNAL RESULT (a dispatch receipt, a child
//! completion, a verifier verdict, a hold decision) is recorded exactly
//! once. On replay the recorded result is consumed and the external work is
//! not repeated — design §6 clauses 3 and 4, verbatim.

use crate::chain::decide::ChainAction;
use crate::chain::history::Recorded;
use crate::chain::records::{
    AcceptanceDecision, HoldDecision, StageConclusion, StageConclusionRefused, StageFailed,
    StageReceipt,
};
use crate::error::ChainError;

/// The external result a boundary needs, when it needs one.
///
/// The decision function never produces these; a sealed activity does, and
/// its result is what gets recorded.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum External {
    /// A stage flight was started and returned its dispatch receipt.
    Dispatched {
        /// The child flight's workflow id, as the dispatcher assigned it.
        child_workflow_id: String,
    },
    /// A stage flight reached a terminal state and returned its receipts.
    Completed {
        /// The child's terminal outcome label, verbatim.
        child_outcome: String,
        /// The EXACT base commit the child reported.
        base_commit: String,
        /// The EXACT pushed commit the child reported.
        pushed_commit: String,
        /// The kept run root, when the child failed.
        kept_root: String,
        /// Whether the terminal outcome is a failure. When true,
        /// `child_outcome` IS the typed label design §4 requires `cause` to
        /// preserve, and it is never `unplanned`.
        failed: bool,
    },
    /// The sealed verifier returned a typed conclusion.
    Verified {
        /// The record, verbatim.
        record: Box<StageConclusion>,
    },
    /// The sealed verifier returned a typed refusal.
    ConclusionRefused {
        /// The record, verbatim.
        record: Box<StageConclusionRefused>,
    },
    /// A held stage was approved or declined at the human seam.
    Held {
        /// The decision, verbatim.
        decision: Box<HoldDecision>,
    },
    /// The action needed nothing from outside.
    None,
}

impl External {
    /// The inbox file name a sealed script deposits this result at, for the
    /// action it answers.
    #[must_use]
    pub fn inbox_name(action: &ChainAction) -> Option<String> {
        match action {
            ChainAction::DispatchStage { stage_id, .. } => {
                Some(format!("dispatch-{stage_id}.json"))
            }
            ChainAction::VerifyConclusion {
                stage_id,
                result_commit,
                ..
            } => Some(if result_commit.is_empty() {
                format!("completion-{stage_id}.json")
            } else {
                format!("verify-{stage_id}.json")
            }),
            ChainAction::OfferHold { stage_id, .. } => Some(format!("hold-{stage_id}.json")),
            ChainAction::RecordAcceptance { .. }
            | ChainAction::EvaluateAdmission { .. }
            | ChainAction::RecordSkip { .. }
            | ChainAction::Finish { .. } => None,
        }
    }
}

/// Turn a decided action plus its external result into one recorded fact.
///
/// # Errors
///
/// [`ChainError::OutOfOrder`] when the external result does not answer the
/// action that was decided — a wiring defect, never an ordinary state.
/// Also when a failing child's typed label is `unplanned` or empty, which
/// design §4 forbids outright: the coordinator STOPS rather than
/// fabricating a cause.
pub fn apply(action: &ChainAction, external: &External) -> Result<Recorded, ChainError> {
    match (action, external) {
        (
            ChainAction::DispatchStage {
                stage_id,
                base_commit,
                ..
            },
            External::Dispatched { child_workflow_id },
        ) => Ok(Recorded::StageDispatched {
            stage_id: stage_id.clone(),
            base_commit: base_commit.clone(),
            child_workflow_id: child_workflow_id.clone(),
        }),

        (
            ChainAction::VerifyConclusion {
                stage_id,
                child_workflow_id,
                ..
            },
            completed @ External::Completed { .. },
        ) => completion_fact(stage_id, child_workflow_id, completed),

        (ChainAction::VerifyConclusion { .. }, External::Verified { record }) => {
            Ok(Recorded::ConclusionVerified {
                record: (**record).clone(),
            })
        }
        (ChainAction::VerifyConclusion { .. }, External::ConclusionRefused { record }) => {
            Ok(Recorded::ConclusionRefusedRecord {
                record: (**record).clone(),
            })
        }

        (ChainAction::OfferHold { .. }, External::Held { decision }) => Ok(Recorded::HoldDecided {
            decision: (**decision).clone(),
        }),

        (
            ChainAction::RecordAcceptance {
                stage_id,
                accepted_commit,
                advance,
                verified_conclusion,
            },
            External::None,
        ) => Ok(Recorded::ResultAccepted {
            decision: AcceptanceDecision {
                stage_id: stage_id.clone(),
                accepted_commit: accepted_commit.clone(),
                advance: advance.clone(),
                verified_conclusion: verified_conclusion.clone(),
            },
        }),

        (
            ChainAction::EvaluateAdmission {
                stage_id,
                condition,
            },
            External::None,
        ) => Ok(Recorded::StageAdmitted {
            stage_id: stage_id.clone(),
            condition: condition.clone(),
        }),

        (ChainAction::RecordSkip { record }, External::None) => Ok(Recorded::StageSkipped {
            record: record.clone(),
        }),

        (action, external) => Err(ChainError::OutOfOrder {
            detail: format!(
                "external result {external:?} does not answer decided action {action:?}"
            ),
        }),
    }
}

/// The completion boundary (§6 clause 3), split out so [`apply`] stays one
/// readable dispatch table.
fn completion_fact(
    stage_id: &str,
    child_workflow_id: &str,
    completed: &External,
) -> Result<Recorded, ChainError> {
    let External::Completed {
        child_outcome,
        base_commit,
        pushed_commit,
        kept_root,
        failed,
    } = completed
    else {
        return Err(ChainError::OutOfOrder {
            detail: format!("completion_fact called with {completed:?}"),
        });
    };
    if *failed {
        // Design §4: `cause` preserves the child outcome's typed label and
        // "is never reported as `unplanned`". The constructor refuses that
        // value, so a coordinator that could not recover the label STOPS
        // here rather than defaulting.
        let record = StageFailed::new(
            stage_id,
            child_workflow_id,
            child_outcome.clone(),
            child_outcome.clone(),
            kept_root.clone(),
            String::new(),
        )
        .map_err(|cause| ChainError::OutOfOrder {
            detail: format!(
                "stage `{stage_id}` failed but its typed cause is {cause:?}; design §4 forbids \
                 `unplanned` and forbids a defaulted cause. The child's typed label must be \
                 recovered from its own recorded terminal outcome, or the coordinator STOPS \
                 and reports."
            ),
        })?;
        return Ok(Recorded::StageFailedRecord { record });
    }
    Ok(Recorded::StageCompleted {
        receipt: StageReceipt {
            stage_id: stage_id.to_owned(),
            child_workflow_id: child_workflow_id.to_owned(),
            base_commit: base_commit.clone(),
            child_outcome: child_outcome.clone(),
            pushed_commit: pushed_commit.clone(),
        },
    })
}
