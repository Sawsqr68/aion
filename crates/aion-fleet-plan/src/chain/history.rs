//! The recorded chain ledger — the durable home of every chain decision.
//!
//! Design §6's law: *"every workflow-visible choice is a function of
//! recorded history plus immutable bound configuration, never wall clock,
//! live fleet membership, mutable branch refs, or live git state."*
//!
//! [`ChainLedger`] IS that recorded history. It is append-only, it is
//! serializable, and it is the ONLY thing [`crate::chain::next_action`]
//! reads besides the normalized graph. Nothing in this module resolves a
//! ref, reads a clock, or asks whether a worker is up.
//!
//! Design §6's final ¶ requires a `ContinuedAsNew` to carry the normalized
//! graph, recorded stage receipts and conclusions, acceptance/skip
//! decisions, and last accepted SHA forward "as explicit input/history
//! rather than rediscover them from git". [`ChainLedger`] plus the
//! [`crate::graph::NormalizedGraph`] is exactly that payload, and
//! [`ChainLedger::carry_forward`] is the accessor that names it.

use serde::{Deserialize, Serialize};

use crate::chain::records::{
    AcceptanceDecision, HoldDecision, SkipRecord, StageConclusion, StageConclusionRefused,
    StageFailed, StageReceipt,
};
use crate::graph::NormalizedGraph;

/// One recorded fact. The variant set is closed and each variant
/// corresponds to one of design §6's recorded boundaries.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "fact", rename_all = "snake_case")]
pub enum Recorded {
    /// Boundary 1 — `check_plan` parsed and normalized the block once, and
    /// its typed success result is authoritative for the run (§6 clause 1).
    PlanNormalized {
        /// The block version normalized.
        version: String,
        /// The stage ids in chain order, so the ledger alone shows the
        /// order the graph fixed.
        order: Vec<String>,
    },
    /// Boundary 2 — before each child stage dispatch, history records the
    /// stage id, exact base commit SHA, and child workflow id (§6 clause 2).
    StageDispatched {
        /// The stage dispatched.
        stage_id: String,
        /// The EXACT base commit it was cut from.
        base_commit: String,
        /// The child flight's workflow id.
        child_workflow_id: String,
    },
    /// Boundary 3 — child completion returns a typed child outcome plus
    /// exact receipts; replay consumes the recorded completion (§6 clause 3).
    StageCompleted {
        /// The receipt, verbatim.
        receipt: StageReceipt,
    },
    /// The child ended on a failure outcome. Terminal: a failing stage
    /// dispatches nothing downstream (§4).
    StageFailedRecord {
        /// The record, verbatim.
        record: StageFailed,
    },
    /// Boundary 4 — the sealed verifier's recorded typed result
    /// (§6 clause 4).
    ConclusionVerified {
        /// The record, verbatim.
        record: StageConclusion,
    },
    /// Boundary 4, refusal arm. Terminal: no SHA is accepted and nothing
    /// downstream runs (§4).
    ConclusionRefusedRecord {
        /// The record, verbatim.
        record: StageConclusionRefused,
    },
    /// The `advance=hold` seam, recorded as its own event (§6 final ¶).
    HoldDecided {
        /// The decision, verbatim.
        decision: HoldDecision,
    },
    /// Boundary 5 — acceptance and the exact accepted commit SHA, recorded
    /// before any successor dispatch (§6 clause 5).
    ResultAccepted {
        /// The decision, verbatim.
        decision: AcceptanceDecision,
    },
    /// Boundary 6 — an admission evaluated true, recorded before
    /// proceeding (§6 clause 6).
    StageAdmitted {
        /// The stage admitted.
        stage_id: String,
        /// The canonical condition that was evaluated.
        condition: String,
    },
    /// Boundary 6 — a conditional skip, recorded durably (§4, §6).
    StageSkipped {
        /// The record, verbatim.
        record: SkipRecord,
    },
}

/// The append-only recorded ledger.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ChainLedger {
    /// The facts, in the order they were recorded.
    pub facts: Vec<Recorded>,
}

impl ChainLedger {
    /// An empty ledger.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Append one recorded fact.
    pub fn record(&mut self, fact: Recorded) {
        self.facts.push(fact);
    }

    /// The last accepted result commit SHA, or `None` before the first
    /// acceptance.
    ///
    /// Design §3 clause 4: this SHA — never a mutable branch name, never a
    /// fresh `git rev-parse` — is the next stage's base.
    #[must_use]
    pub fn last_accepted_commit(&self) -> Option<&str> {
        self.facts.iter().rev().find_map(|f| match f {
            Recorded::ResultAccepted { decision } => Some(decision.accepted_commit.as_str()),
            _ => None,
        })
    }

    /// The recorded conclusion of `stage_id`, if one has verified.
    ///
    /// This is the ONLY source an admission condition reads (§6 clause 6):
    /// a recorded `stage_conclusion`, never a report, never prose, never
    /// the tree.
    #[must_use]
    pub fn conclusion_of(&self, stage_id: &str) -> Option<&str> {
        self.facts.iter().find_map(|f| match f {
            Recorded::ConclusionVerified { record } if record.stage_id == stage_id => {
                Some(record.conclusion.as_str())
            }
            _ => None,
        })
    }

    /// The dispatch record for `stage_id`, if it was dispatched.
    #[must_use]
    pub fn dispatch_of(&self, stage_id: &str) -> Option<(&str, &str)> {
        self.facts.iter().find_map(|f| match f {
            Recorded::StageDispatched {
                stage_id: id,
                base_commit,
                child_workflow_id,
            } if id == stage_id => Some((base_commit.as_str(), child_workflow_id.as_str())),
            _ => None,
        })
    }

    /// The completion receipt for `stage_id`, if it completed.
    #[must_use]
    pub fn receipt_of(&self, stage_id: &str) -> Option<&StageReceipt> {
        self.facts.iter().find_map(|f| match f {
            Recorded::StageCompleted { receipt } if receipt.stage_id == stage_id => Some(receipt),
            _ => None,
        })
    }

    /// The hold decision for `stage_id`, if one was recorded.
    #[must_use]
    pub fn hold_of(&self, stage_id: &str) -> Option<&HoldDecision> {
        self.facts.iter().find_map(|f| match f {
            Recorded::HoldDecided { decision } if decision.stage_id == stage_id => Some(decision),
            _ => None,
        })
    }

    /// Has `stage_id` been accepted?
    #[must_use]
    pub fn accepted(&self, stage_id: &str) -> Option<&AcceptanceDecision> {
        self.facts.iter().find_map(|f| match f {
            Recorded::ResultAccepted { decision } if decision.stage_id == stage_id => {
                Some(decision)
            }
            _ => None,
        })
    }

    /// Was `stage_id` recorded skipped?
    #[must_use]
    pub fn skipped(&self, stage_id: &str) -> Option<&SkipRecord> {
        self.facts.iter().find_map(|f| match f {
            Recorded::StageSkipped { record } if record.stage_id == stage_id => Some(record),
            _ => None,
        })
    }

    /// Was `stage_id`'s admission recorded true?
    #[must_use]
    pub fn admitted(&self, stage_id: &str) -> bool {
        self.facts
            .iter()
            .any(|f| matches!(f, Recorded::StageAdmitted { stage_id: id, .. } if id == stage_id))
    }

    /// The terminal stage-failure record, if the chain stopped on one.
    #[must_use]
    pub fn stage_failure(&self) -> Option<&StageFailed> {
        self.facts.iter().find_map(|f| match f {
            Recorded::StageFailedRecord { record } => Some(record),
            _ => None,
        })
    }

    /// The terminal conclusion-refusal record, if the chain stopped on one.
    #[must_use]
    pub fn conclusion_refusal(&self) -> Option<&StageConclusionRefused> {
        self.facts.iter().find_map(|f| match f {
            Recorded::ConclusionRefusedRecord { record } => Some(record),
            _ => None,
        })
    }

    /// The explicit `ContinuedAsNew` payload (§6 final ¶).
    ///
    /// The graph, the receipts, the conclusions, the decisions, and the
    /// last accepted SHA — carried forward as data, never rediscovered
    /// from git.
    #[must_use]
    pub fn carry_forward(&self, graph: &NormalizedGraph) -> CarryForward {
        CarryForward {
            graph: graph.clone(),
            ledger: self.clone(),
            last_accepted_commit: self.last_accepted_commit().map(str::to_owned),
        }
    }
}

/// The explicit payload a `ContinuedAsNew` carries forward (§6 final ¶).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CarryForward {
    /// The normalized graph — the run's ordering authority, carried as
    /// data rather than re-derived by a later parse.
    pub graph: NormalizedGraph,
    /// Every recorded receipt, conclusion, and decision.
    pub ledger: ChainLedger,
    /// The last accepted SHA.
    pub last_accepted_commit: Option<String>,
}
