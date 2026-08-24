//! `check_plan` — the whole §5.2 validation, in one documented order.

use crate::cause::CauseCode;
use crate::error::PlanRefusal;
use crate::grammar::{BlockScan, RawStage, extract_block, parse_admit, parse_stage_line};
use crate::graph::{Admit, Advance, NormalizedGraph, Stage};
use crate::ids::conclusion_path;
use crate::validate::clauses::{
    PathClaim, check_conclusion_leg_membership, check_conclusion_path_distinctness,
    check_dispatch_leg_cover, check_id_charset, check_leg_uniqueness,
    check_multi_leg_conclusion_count, check_multi_value_leg_count, check_stage_has_legs,
    check_stage_id_nonempty, check_stage_id_uniqueness,
};
use crate::validate::shape::{
    chain_order, check_acyclic, check_admit_references, check_advance, check_chain_cardinality,
    check_predecessors,
};

/// Everything `check_plan` reads. All of it is committed state the caller
/// already resolved at HEAD; this function performs no I/O and resolves no
/// ref, so it is a pure function of its inputs and therefore replayable.
#[derive(Clone, Debug)]
pub struct PlanInputs {
    /// The verbatim text of `.fleet/plan.md` as committed at HEAD.
    pub plan_text: String,
    /// The dispatch-fixed leg ids, in dispatch order.
    pub dispatch_leg_ids: Vec<String>,
    /// Whether the working tree carries loose `.fleet` state (§5.2 clause
    /// 17). The blackboard is committed.
    pub fleet_state_loose: bool,
    /// Detail for a loose-state refusal, verbatim from `git status`.
    pub loose_detail: String,
    /// The chain's dispatch-authored `max_stages` ceiling, when the caller
    /// binds one. A plan declaring more stages than this refuses HERE,
    /// before any stage leg is dispatched, rather than being truncated.
    pub max_stages: Option<usize>,
}

/// What a `check_plan` call concluded.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckOutcome {
    /// No `fleet-stages-v1` block. Design §7 clause 5: the plan "continues
    /// to run only through the existing `fleet_dev` entry point". The
    /// caller must fall through to the pre-existing behaviour
    /// BYTE-IDENTICALLY; this is not a refusal.
    NoChainBlock,
    /// A valid chain. The normalized typed graph IS the run's ordering
    /// authority from here on (§5.3 final ¶).
    Valid(Box<NormalizedGraph>),
}

/// Validate the committed plan's staged-chain block.
///
/// # Errors
///
/// A [`PlanRefusal`] carrying a stable cause code. Its
/// [`crate::cause::ExitClass`] decides whether the caller exits 1 or 4;
/// exits 0 and 3 are the pre-existing script's and are untouched here.
///
/// ## Evaluation order, and why
///
/// 1. **charset** — first, because it is the boundary that closes traversal
///    for `.fleet/conclusions/<stage-id>`. Nothing reasons about an id that
///    might still contain `../`.
/// 2. clause 2 (nonempty, then unique)
/// 3. clause 6 (stage has legs) — before the leg clauses, so they reason
///    about a nonempty set
/// 4. clause 3 (leg uniqueness within the graph)
/// 5. clauses 4 and 5 (dispatch cover)
/// 6. clause 10 (conclusion owner), 11, 12 (vocabulary cardinality)
/// 7. clause 16 (advance)
/// 8. clause 7 (predecessors known, no self-follow)
/// 9. **clause 9 (acyclic) — BEFORE clause 8**, so a cycle is refused by
///    name rather than as a missing root. See [`check_acyclic`].
/// 10. clause 8 (chain cardinality)
/// 11. clauses 13 and 14 (admit references and vocabulary)
/// 12. clause 15 (distinct conclusion paths)
/// 13. the `max_stages` ceiling
///
/// Clause 1 is discharged by [`extract_block`] and clause 17 by the
/// committed-state check below, which is the one refusal in
/// [`crate::cause::ExitClass::AbsentOrIncomplete`].
pub fn check_plan(inputs: &PlanInputs) -> Result<CheckOutcome, PlanRefusal> {
    // §5.2 clause 17 — the block, plan, and briefs are committed at HEAD
    // with no loose `.fleet` state. This is the exit-1 class, and it is
    // checked before the graph so that absence keeps its existing
    // precedence over graph invalidity (§5.3, "preserve existing meanings
    // and precedence").
    if inputs.fleet_state_loose {
        return Err(PlanRefusal::whole(
            CauseCode::PlanStateUncommitted,
            format!(
                "uncommitted .fleet state (the blackboard is committed): {}",
                inputs.loose_detail
            ),
        ));
    }

    // §5.2 clause 1.
    let lines = match extract_block(&inputs.plan_text)? {
        BlockScan::Absent => return Ok(CheckOutcome::NoChainBlock),
        BlockScan::Present { lines } => lines,
    };

    // §2.1 canonical authored form.
    let stages: Vec<RawStage> = lines
        .iter()
        .map(|l| parse_stage_line(l))
        .collect::<Result<_, _>>()?;

    // 1. The charset boundary, first.
    check_id_charset(&stages)?;

    let stage_ids: Vec<String> = stages.iter().map(|s| s.id.clone()).collect();
    check_stage_id_nonempty(&stage_ids)?;
    check_stage_id_uniqueness(&stage_ids)?;
    check_stage_has_legs(&stages)?;
    check_leg_uniqueness(&stages)?;
    check_dispatch_leg_cover(&stages, &inputs.dispatch_leg_ids)?;
    check_conclusion_leg_membership(&stages)?;
    check_multi_leg_conclusion_count(&stages)?;
    check_multi_value_leg_count(&stages)?;
    check_advance(&stages)?;
    check_predecessors(&stages)?;
    check_acyclic(&stages)?;
    check_chain_cardinality(&stages)?;

    let parsed_admits: Vec<Admit> = stages
        .iter()
        .map(|s| parse_admit(&s.admit, &s.id))
        .collect::<Result<_, _>>()?;

    let order = chain_order(&stages);
    check_admit_references(&stages, &order, &parsed_admits)?;

    let claims: Vec<PathClaim> = stages
        .iter()
        .map(|s| PathClaim {
            stage_id: s.id.clone(),
            path: conclusion_path(&s.id),
        })
        .collect();
    check_conclusion_path_distinctness(&claims)?;

    if let Some(ceiling) = inputs.max_stages
        && stages.len() > ceiling
    {
        return Err(PlanRefusal::whole(
            CauseCode::StageCountExceedsCeiling,
            format!(
                "the plan declares {} stages; the chain's dispatch-authored `max_stages` \
                 ceiling is {ceiling}. Refused here, before any stage leg is dispatched — \
                 never silently truncated.",
                stages.len()
            ),
        ));
    }

    let ordered: Vec<Stage> = order
        .iter()
        .map(|&i| {
            let raw = &stages[i];
            Stage {
                id: raw.id.clone(),
                legs: raw.legs.clone(),
                after: raw.after.clone(),
                admit: parsed_admits[i].clone(),
                conclusions: raw.conclusions.clone(),
                conclusion_leg: raw.conclusion_leg.clone(),
                advance: if raw.advance == "hold" {
                    Advance::Hold
                } else {
                    Advance::Auto
                },
            }
        })
        .collect();

    Ok(CheckOutcome::Valid(Box::new(NormalizedGraph {
        version: crate::grammar::SUPPORTED_VERSION.to_owned(),
        order: ordered,
    })))
}
