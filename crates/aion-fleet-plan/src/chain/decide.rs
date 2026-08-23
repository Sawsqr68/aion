//! `next_action` — the replay-deterministic chain decision function.
//!
//! **This function is the whole of R6.** Design §6's proof obligation is
//! that crash/replay at every boundary re-derives the same next action and
//! the same exact SHA. That holds here by construction and is proven by
//! kill-and-resume: `next_action` is a PURE, TOTAL function of
//! `(NormalizedGraph, ChainLedger)`. It takes no clock, no environment, no
//! filesystem, no network, no randomness, and no `&mut` state. Calling it
//! twice on the same inputs cannot produce two answers, and a process that
//! dies and restarts on the same recorded ledger re-enters at the same
//! decision.
//!
//! Design §6's verbatim prohibition — *"any implementation that resolves
//! that object with `git rev-parse`, reads a live branch ref or worktree,
//! greps prose for a conclusion, consults time or worker availability, or
//! reads the current deployed module to recover a prior decision violates
//! the design"* — is discharged by the signature: there is nothing in scope
//! to resolve, read, grep, or consult.
//!
//! # The fixed ordering (§2.6)
//!
//! > pushed receipt → verify → accept/hold → admit
//!
//! and *"No hold can approve a stage whose conclusion has not verified."*
//! Both are structural below: the acceptance arms are reachable only from a
//! recorded `ConclusionVerified`, and the hold arm is inside that same
//! branch.

use crate::chain::history::{ChainLedger, Recorded};
use crate::chain::records::SkipRecord;
use crate::error::ChainError;
use crate::graph::{Admit, Advance, Comparison, NormalizedGraph, Stage};

/// How the chain ended.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChainOutcome {
    /// Every admitted stage ran, verified, and was accepted.
    Completed,
    /// At least one stage was durably skipped by its admission condition,
    /// and the chain still succeeded. Design §9.4: *"the outcome says
    /// planned descope, not `stage_failed`, `stage_conclusion_refused`,
    /// `unplanned`, or missing work."*
    PlannedDescope,
    /// A stage failed. Nothing downstream ran.
    StageFailed,
    /// A conclusion was refused. No SHA was accepted and nothing
    /// downstream ran.
    StageConclusionRefused,
}

/// The next thing the coordinator must do.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ChainAction {
    /// Dispatch a stage from an EXACT base commit.
    DispatchStage {
        /// Which stage.
        stage_id: String,
        /// The exact base: the root's authored base for the root stage,
        /// otherwise the recorded ACCEPTED result SHA of the most recent
        /// accepted predecessor (§3 clauses 1, 4, 5).
        base_commit: String,
        /// The legs to cut, in declared order.
        legs: Vec<String>,
    },
    /// Invoke the sealed verifier on the recorded pushed SHA.
    VerifyConclusion {
        /// Which stage.
        stage_id: String,
        /// The child whose result is verified.
        child_workflow_id: String,
        /// The EXACT recorded pushed commit.
        result_commit: String,
        /// The canonical path to read.
        conclusion_path: String,
        /// The stage's declared closed vocabulary.
        vocabulary: Vec<String>,
    },
    /// Offer the `advance=hold` seam. Reachable ONLY after the conclusion
    /// has verified (§2.6).
    OfferHold {
        /// Which stage.
        stage_id: String,
        /// The result commit the hold is offered on.
        result_commit: String,
        /// The already-verified conclusion.
        verified_conclusion: String,
    },
    /// Record acceptance of an exact result SHA.
    RecordAcceptance {
        /// Which stage.
        stage_id: String,
        /// The EXACT accepted commit.
        accepted_commit: String,
        /// The advance policy that authorised it.
        advance: String,
        /// The already-verified conclusion.
        verified_conclusion: String,
    },
    /// Evaluate a successor's admission from RECORDED history.
    EvaluateAdmission {
        /// Which stage.
        stage_id: String,
        /// The canonical condition.
        condition: String,
    },
    /// Record a stage skipped. No worktree is cut and no builder seat is
    /// spent (§9.4).
    RecordSkip {
        /// The record, ready to append.
        record: SkipRecord,
    },
    /// The chain is finished.
    Finish {
        /// How it ended.
        outcome: ChainOutcome,
        /// The chain's result commit — the last ACCEPTED SHA — or `None`
        /// when nothing was ever accepted.
        result_commit: Option<String>,
    },
}

/// Decide the next action from the normalized graph and the recorded ledger.
///
/// # Errors
///
/// [`ChainError::UnknownStage`] when the ledger names a stage outside the
/// graph. The graph is the ordering authority (§5.3 final ¶), so a ledger
/// that disagrees with it is a defect, not an ordinary state.
pub fn next_action(
    graph: &NormalizedGraph,
    ledger: &ChainLedger,
    root_base_commit: &str,
) -> Result<ChainAction, ChainError> {
    for fact in &ledger.facts {
        if let Some(named) = names_stage(fact) {
            if graph.stage(named).is_none() {
                return Err(ChainError::UnknownStage {
                    stage_id: named.to_owned(),
                });
            }
        }
    }

    // Terminal arms first: a failing stage dispatches nothing downstream,
    // and a refused conclusion accepts no SHA and runs nothing downstream.
    if ledger.stage_failure().is_some() {
        return Ok(ChainAction::Finish {
            outcome: ChainOutcome::StageFailed,
            result_commit: ledger.last_accepted_commit().map(str::to_owned),
        });
    }
    if ledger.conclusion_refusal().is_some() {
        return Ok(ChainAction::Finish {
            outcome: ChainOutcome::StageConclusionRefused,
            result_commit: ledger.last_accepted_commit().map(str::to_owned),
        });
    }

    // Walk the chain in the graph's fixed order and act on the first stage
    // that is not yet finished with. The walk is total and order-fixed, so
    // the same ledger always selects the same stage.
    for stage in &graph.order {
        if ledger.skipped(&stage.id).is_some() {
            continue;
        }
        if ledger.accepted(&stage.id).is_some() {
            continue;
        }

        // §2.6 ordering: admission is evaluated BEFORE dispatch, because a
        // skipped stage must never cut a worktree or spend a seat (§9.4).
        if ledger.dispatch_of(&stage.id).is_none() && !ledger.admitted(&stage.id) {
            return Ok(admission_action(stage, ledger));
        }

        let Some((base_commit, child_workflow_id)) = ledger.dispatch_of(&stage.id) else {
            let base = base_for(graph, ledger, stage, root_base_commit);
            return Ok(ChainAction::DispatchStage {
                stage_id: stage.id.clone(),
                base_commit: base,
                legs: stage.legs.clone(),
            });
        };
        let _ = base_commit;

        let Some(receipt) = ledger.receipt_of(&stage.id) else {
            // Dispatched, not yet complete. The coordinator awaits the
            // recorded completion; it does not rerun git or the child
            // (§6 clause 3).
            return Ok(ChainAction::VerifyConclusion {
                stage_id: stage.id.clone(),
                child_workflow_id: child_workflow_id.to_owned(),
                result_commit: String::new(),
                conclusion_path: stage.conclusion_path(),
                vocabulary: stage.conclusions.clone(),
            });
        };

        // §2.6: pushed receipt → verify → accept/hold → admit.
        let Some(conclusion) = ledger.conclusion_of(&stage.id) else {
            return Ok(ChainAction::VerifyConclusion {
                stage_id: stage.id.clone(),
                child_workflow_id: receipt.child_workflow_id.clone(),
                result_commit: receipt.pushed_commit.clone(),
                conclusion_path: stage.conclusion_path(),
                vocabulary: stage.conclusions.clone(),
            });
        };

        // The conclusion HAS verified. Only now is accept-or-hold reachable.
        if stage.advance == Advance::Hold {
            match ledger.hold_of(&stage.id) {
                None => {
                    return Ok(ChainAction::OfferHold {
                        stage_id: stage.id.clone(),
                        result_commit: receipt.pushed_commit.clone(),
                        verified_conclusion: conclusion.to_owned(),
                    });
                }
                Some(hold) if !hold.approved => {
                    // A declined hold stops the chain without accepting a
                    // SHA. It is not a stage failure and not a refusal.
                    return Ok(ChainAction::Finish {
                        outcome: ChainOutcome::Completed,
                        result_commit: ledger.last_accepted_commit().map(str::to_owned),
                    });
                }
                Some(_) => {}
            }
        }

        return Ok(ChainAction::RecordAcceptance {
            stage_id: stage.id.clone(),
            accepted_commit: receipt.pushed_commit.clone(),
            advance: advance_text(stage.advance).to_owned(),
            verified_conclusion: conclusion.to_owned(),
        });
    }

    // Every stage is accepted or skipped.
    let any_skip = graph.order.iter().any(|s| ledger.skipped(&s.id).is_some());
    Ok(ChainAction::Finish {
        outcome: if any_skip {
            ChainOutcome::PlannedDescope
        } else {
            ChainOutcome::Completed
        },
        result_commit: ledger.last_accepted_commit().map(str::to_owned),
    })
}

/// Evaluate a stage's admission from RECORDED history alone (§6 clause 6).
fn admission_action(stage: &Stage, ledger: &ChainLedger) -> ChainAction {
    match &stage.admit {
        Admit::Always => ChainAction::EvaluateAdmission {
            stage_id: stage.id.clone(),
            condition: stage.admit.canonical(),
        },
        Admit::Conclusion {
            stage_id: source,
            comparison,
            value,
        } => {
            let observed = ledger.conclusion_of(source).unwrap_or_default();
            let holds = match comparison {
                Comparison::Equals => observed == value,
                Comparison::NotEquals => observed != value,
            };
            if holds {
                ChainAction::EvaluateAdmission {
                    stage_id: stage.id.clone(),
                    condition: stage.admit.canonical(),
                }
            } else {
                ChainAction::RecordSkip {
                    record: SkipRecord {
                        stage_id: stage.id.clone(),
                        source_stage_id: source.clone(),
                        observed_conclusion: observed.to_owned(),
                        condition: stage.admit.canonical(),
                    },
                }
            }
        }
    }
}

/// The exact base commit for a stage (design §3, all five clauses).
///
/// Clause 1: the root is cut from the chain's authored `base_ref` resolved
/// at chain start. Clause 4: a successor's base is the recorded ACCEPTED
/// result SHA. Clause 5: a skipped stage creates no base, so the successor
/// uses the most recent accepted predecessor — which is exactly what
/// `last_accepted_commit` returns, because a skip appends no acceptance.
fn base_for(
    graph: &NormalizedGraph,
    ledger: &ChainLedger,
    stage: &Stage,
    root_base_commit: &str,
) -> String {
    if graph.position(&stage.id) == Some(0) {
        return root_base_commit.to_owned();
    }
    ledger
        .last_accepted_commit()
        .unwrap_or(root_base_commit)
        .to_owned()
}

const fn advance_text(advance: Advance) -> &'static str {
    match advance {
        Advance::Auto => "auto",
        Advance::Hold => "hold",
    }
}

/// The stage a recorded fact is about, when it names one.
fn names_stage(fact: &Recorded) -> Option<&str> {
    match fact {
        Recorded::PlanNormalized { .. } => None,
        Recorded::StageDispatched { stage_id, .. } | Recorded::StageAdmitted { stage_id, .. } => {
            Some(stage_id)
        }
        Recorded::StageCompleted { receipt } => Some(&receipt.stage_id),
        Recorded::StageFailedRecord { record } => Some(&record.stage_id),
        Recorded::ConclusionVerified { record } => Some(&record.stage_id),
        Recorded::ConclusionRefusedRecord { record } => Some(&record.stage_id),
        Recorded::HoldDecided { decision } => Some(&decision.stage_id),
        Recorded::ResultAccepted { decision } => Some(&decision.stage_id),
        Recorded::StageSkipped { record } => Some(&record.stage_id),
    }
}
