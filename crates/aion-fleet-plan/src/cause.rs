//! Stable cause codes for every refusal this crate can produce.
//!
//! A cause code is a wire contract: it is the stable, machine-readable name
//! design §5.3 requires exit 4 to carry, and design §2.6/§4 require the
//! conclusion verifier to carry. The `&'static str` rendering of each
//! variant is the contract; the variant name is not. Renaming a rendering
//! breaks a consumer, so the renderings are pinned by test.

use serde::{Deserialize, Serialize};

/// Which of `check_plan`'s four exit codes a refusal belongs to.
///
/// Design §5.3 preserves the existing meanings and precedence: `0` valid,
/// `1` plan or required committed artifact absent/incomplete, `3` committed
/// briefs disagree with dispatch-fixed leg ids, `4` `PLAN GRAPH INVALID`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ExitClass {
    /// Exit 1 — absent or incomplete committed plan state.
    AbsentOrIncomplete,
    /// Exit 4 — `PLAN GRAPH INVALID`.
    PlanGraphInvalid,
}

impl ExitClass {
    /// The process exit code this class maps to.
    #[must_use]
    pub const fn code(self) -> i32 {
        match self {
            Self::AbsentOrIncomplete => 1,
            Self::PlanGraphInvalid => 4,
        }
    }
}

/// Every stable cause code the plan validator can refuse with.
///
/// The `§5.2 clause` column in each doc comment is the design clause the
/// code discharges. Codes without a clause number belong to §2.1's
/// canonical authored form, which §5.2 clause 1 ("exactly one supported
/// block and version") presupposes: a line that is not in canonical form
/// has no parsed stage to validate.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CauseCode {
    /// §5.2 clause 1 — more than one `fleet-stages-v1` block in the plan.
    MultipleBlocks,
    /// §5.2 clause 1 — a `fleet-stages-*` fence naming a version this
    /// validator does not support.
    UnsupportedBlockVersion,
    /// §5.2 clause 1 — the block is present but declares no stage.
    EmptyBlock,
    /// §2.1 — a stage line that is not `stage` followed by the seven
    /// canonical `key=value` pairs.
    MalformedStageLine,
    /// §2.1 — a key outside the canonical seven.
    UnknownKey,
    /// §2.1 — the canonical seven keys are not in the displayed order.
    KeyOutOfOrder,
    /// §2.1 — the same key twice on one stage line.
    DuplicateKey,
    /// §2.1 — one of the canonical seven keys is absent.
    MissingKey,
    /// §2.1 — "no quoting": a value wrapped in quotes.
    QuotedValue,
    /// §2.1 — an empty value, or an empty element inside a comma-separated
    /// value.
    EmptyValue,
    /// §2.1 — `admit` is not `always`, `<stage>.conclusion==<value>`, or
    /// `<stage>.conclusion!=<value>`.
    MalformedAdmit,
    /// §5.2 clause 2 — two stages share an id.
    DuplicateStageId,
    /// §5.2 clause 2 — an empty stage id. Subsumed at whole-plan level by
    /// [`Self::InvalidStageIdCharset`], which the empty string also fails;
    /// implemented and tested separately so the refusal stays named.
    EmptyStageId,
    /// §5.2 clause 3 — a leg id appears in more than one place in the graph.
    DuplicateLegId,
    /// §5.2 clause 4 — a stage names a leg id that is not dispatch-declared.
    UnknownLegId,
    /// §5.2 clause 5 — a dispatch-declared leg id appears in no stage.
    LegNotPlaced,
    /// §5.2 clause 6 — a stage carries no leg.
    StageWithoutLegs,
    /// §5.2 clause 7 — `after` names a stage that does not exist.
    UnknownPredecessor,
    /// §5.2 clause 7 — a stage follows itself.
    SelfPredecessor,
    /// §5.2 clause 9 — the stage graph contains a cycle. Validated
    /// explicitly, and BEFORE the clause-8 cardinality checks, so that a
    /// cycle is refused by name rather than as a missing root.
    Cycle,
    /// §5.2 clause 8 — no stage declares `after=-`.
    NoRoot,
    /// §5.2 clause 8 — more than one stage declares `after=-`.
    MultipleRoots,
    /// §5.2 clause 8 — a stage is named as predecessor by more than one
    /// stage. This is the shape an arbitrary multi-parent graph takes in a
    /// grammar whose `after` holds exactly one id.
    MultipleSuccessors,
    /// §5.2 clause 8 — more than one stage has no successor.
    MultipleTerminals,
    /// §5.2 clause 10 — `conclusion_leg` names a leg outside its own stage.
    ConclusionLegOutsideStage,
    /// §5.2 clause 11 — a multi-leg stage does not declare exactly one
    /// conclusion value.
    MultiLegConclusionCount,
    /// §5.2 clause 12 — a stage with multiple declared conclusion values
    /// does not have exactly one leg. Contrapositive of clause 11; at
    /// whole-plan level clause 11 fires first. Implemented and tested
    /// separately so the refusal stays named.
    MultiValueLegCount,
    /// §5.2 clause 13 — an `admit` condition names a stage that does not
    /// exist.
    AdmitUnknownStage,
    /// §5.2 clause 13 — an `admit` condition names a stage that is not an
    /// EARLIER reachable stage on the chain.
    AdmitUnreachableStage,
    /// §5.2 clause 14 — an `admit` condition compares against a value
    /// outside the referenced stage's declared closed vocabulary.
    AdmitUndeclaredConclusion,
    /// §5.2 clause 15 — two stages derive the same canonical conclusion
    /// path. Subsumed at whole-plan level by [`Self::DuplicateStageId`] once
    /// the charset clause holds; implemented and tested separately so the
    /// refusal stays named.
    DuplicateConclusionPath,
    /// §5.2 clause 16 — `advance` is neither `auto` nor `hold`.
    InvalidAdvance,
    /// §5.2 clause 17 — the block, plan, or briefs are not committed at
    /// HEAD, or `.fleet` state is loose in the working tree. This is the
    /// one cause code in [`ExitClass::AbsentOrIncomplete`].
    PlanStateUncommitted,
    /// The brief's derived stage-id charset tightening: a stage id outside
    /// `^[a-z0-9][a-z0-9_-]*$`.
    InvalidStageIdCharset,
    /// The brief's derived charset tightening, applied to leg ids.
    InvalidLegIdCharset,
    /// The plan declares more stages than the chain's dispatch-authored
    /// `max_stages` ceiling admits. Refused here, before any stage leg is
    /// dispatched, rather than silently truncated.
    StageCountExceedsCeiling,
}

impl CauseCode {
    /// The stable wire rendering. This string is the contract.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MultipleBlocks => "multiple_blocks",
            Self::UnsupportedBlockVersion => "unsupported_block_version",
            Self::EmptyBlock => "empty_block",
            Self::MalformedStageLine => "malformed_stage_line",
            Self::UnknownKey => "unknown_key",
            Self::KeyOutOfOrder => "key_out_of_order",
            Self::DuplicateKey => "duplicate_key",
            Self::MissingKey => "missing_key",
            Self::QuotedValue => "quoted_value",
            Self::EmptyValue => "empty_value",
            Self::MalformedAdmit => "malformed_admit",
            Self::DuplicateStageId => "duplicate_stage_id",
            Self::EmptyStageId => "empty_stage_id",
            Self::DuplicateLegId => "duplicate_leg_id",
            Self::UnknownLegId => "unknown_leg_id",
            Self::LegNotPlaced => "leg_not_placed",
            Self::StageWithoutLegs => "stage_without_legs",
            Self::UnknownPredecessor => "unknown_predecessor",
            Self::SelfPredecessor => "self_predecessor",
            Self::Cycle => "cycle",
            Self::NoRoot => "no_root",
            Self::MultipleRoots => "multiple_roots",
            Self::MultipleSuccessors => "multiple_successors",
            Self::MultipleTerminals => "multiple_terminals",
            Self::ConclusionLegOutsideStage => "conclusion_leg_outside_stage",
            Self::MultiLegConclusionCount => "multi_leg_conclusion_count",
            Self::MultiValueLegCount => "multi_value_leg_count",
            Self::AdmitUnknownStage => "admit_unknown_stage",
            Self::AdmitUnreachableStage => "admit_unreachable_stage",
            Self::AdmitUndeclaredConclusion => "admit_undeclared_conclusion",
            Self::DuplicateConclusionPath => "duplicate_conclusion_path",
            Self::InvalidAdvance => "invalid_advance",
            Self::PlanStateUncommitted => "plan_state_uncommitted",
            Self::InvalidStageIdCharset => "invalid_stage_id_charset",
            Self::InvalidLegIdCharset => "invalid_leg_id_charset",
            Self::StageCountExceedsCeiling => "stage_count_exceeds_ceiling",
        }
    }

    /// Which exit code this refusal belongs to.
    ///
    /// Design §5.3: exit 4 "must not widen `plan_mismatch`" — an unknown
    /// predecessor, cycle, duplicate leg, invalid conclusion owner or
    /// vocabulary shape, invalid condition, or unsupported graph shape "is
    /// neither absence nor an id naming mismatch". Only clause 17's
    /// committed-state refusal is absence, and it keeps exit 1.
    #[must_use]
    pub const fn exit_class(self) -> ExitClass {
        match self {
            Self::PlanStateUncommitted => ExitClass::AbsentOrIncomplete,
            _ => ExitClass::PlanGraphInvalid,
        }
    }
}

impl std::fmt::Display for CauseCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
