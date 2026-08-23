//! The §5.2 plan-time validation: all seventeen clauses, plus the brief's
//! derived stage-id charset tightening.

pub mod clauses;
pub mod plan;
pub mod shape;

pub use clauses::{
    PathClaim, check_conclusion_leg_membership, check_conclusion_path_distinctness,
    check_dispatch_leg_cover, check_id_charset, check_leg_uniqueness,
    check_multi_leg_conclusion_count, check_multi_value_leg_count, check_stage_has_legs,
    check_stage_id_nonempty, check_stage_id_uniqueness,
};
pub use plan::{CheckOutcome, PlanInputs, check_plan};
pub use shape::{
    chain_order, check_acyclic, check_admit_references, check_advance, check_chain_cardinality,
    check_predecessors,
};
