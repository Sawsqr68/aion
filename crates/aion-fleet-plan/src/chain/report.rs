//! The chain report (design §9.3 clause 6).
//!
//! > emit a chain report containing all stage workflow ids, bases, results,
//! > verdicts, conclusions, acceptance decisions, and no skips.
//!
//! The report is DERIVED from the recorded ledger and the normalized graph
//! — it is not a second source of truth, and building it re-reads nothing.
//! An optional chain summary "may aggregate receipts, but every stage must
//! already have passed its own gates and judge" (§4): this report
//! aggregates and judges nothing.

use serde::{Deserialize, Serialize};

use crate::chain::decide::ChainOutcome;
use crate::chain::history::ChainLedger;
use crate::graph::NormalizedGraph;

/// One stage's line in the chain report.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StageLine {
    /// The stage id.
    pub stage_id: String,
    /// The child flight's workflow id, when the stage was dispatched.
    pub child_workflow_id: Option<String>,
    /// The exact base commit the stage was cut from.
    pub base_commit: Option<String>,
    /// The child's terminal outcome label.
    pub child_outcome: Option<String>,
    /// The exact pushed result commit.
    pub result_commit: Option<String>,
    /// The verified conclusion.
    pub conclusion: Option<String>,
    /// The exact accepted result SHA.
    pub accepted_commit: Option<String>,
    /// The advance policy that authorised acceptance.
    pub advance: Option<String>,
    /// When the stage was skipped: the source stage, observed conclusion,
    /// and canonical condition, exactly as design §4 requires a skip record
    /// to name them.
    pub skipped: Option<SkipLine>,
}

/// A skipped stage's line.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SkipLine {
    /// The stage whose recorded conclusion the condition read.
    pub source_stage_id: String,
    /// The conclusion observed on that source stage.
    pub observed_conclusion: String,
    /// The canonical condition.
    pub condition: String,
}

/// The whole chain report.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ChainReport {
    /// How the chain ended.
    pub outcome: ChainOutcome,
    /// The chain's result commit — the last ACCEPTED SHA.
    pub result_commit: Option<String>,
    /// One line per stage, in chain order.
    pub stages: Vec<StageLine>,
}

impl ChainReport {
    /// Derive the report from the graph and the recorded ledger.
    #[must_use]
    pub fn derive(graph: &NormalizedGraph, ledger: &ChainLedger, outcome: ChainOutcome) -> Self {
        let stages = graph
            .order
            .iter()
            .map(|stage| {
                let dispatch = ledger.dispatch_of(&stage.id);
                let receipt = ledger.receipt_of(&stage.id);
                let accepted = ledger.accepted(&stage.id);
                StageLine {
                    stage_id: stage.id.clone(),
                    child_workflow_id: dispatch
                        .map(|(_, child)| child.to_owned())
                        .or_else(|| receipt.map(|r| r.child_workflow_id.clone())),
                    base_commit: dispatch
                        .map(|(base, _)| base.to_owned())
                        .or_else(|| receipt.map(|r| r.base_commit.clone())),
                    child_outcome: receipt.map(|r| r.child_outcome.clone()),
                    result_commit: receipt.map(|r| r.pushed_commit.clone()),
                    conclusion: ledger.conclusion_of(&stage.id).map(str::to_owned),
                    accepted_commit: accepted.map(|a| a.accepted_commit.clone()),
                    advance: accepted.map(|a| a.advance.clone()),
                    skipped: ledger.skipped(&stage.id).map(|s| SkipLine {
                        source_stage_id: s.source_stage_id.clone(),
                        observed_conclusion: s.observed_conclusion.clone(),
                        condition: s.condition.clone(),
                    }),
                }
            })
            .collect();
        Self {
            outcome,
            result_commit: ledger.last_accepted_commit().map(str::to_owned),
            stages,
        }
    }

    /// Were any stages skipped?
    #[must_use]
    pub fn has_skips(&self) -> bool {
        self.stages.iter().any(|s| s.skipped.is_some())
    }
}
