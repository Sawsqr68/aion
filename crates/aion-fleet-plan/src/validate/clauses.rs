//! §5.2 clauses 2–6, 10–12, 15, and the derived charset clause.
//!
//! Each clause is its own function so that each has its own name, its own
//! cause code, and its own directly-testable refusing input. Three of them
//! are UNREACHABLE at whole-plan level because an earlier clause subsumes
//! them:
//!
//! | Clause | Subsumed by | Still implemented because |
//! |---|---|---|
//! | 2, "nonempty" | the charset clause (the empty string has no leading `[a-z0-9]`) | the refusal stays named |
//! | 12 | clause 11, its contrapositive | the refusal stays named |
//! | 15 | clause 2's uniqueness, once the charset holds | the refusal stays named |
//!
//! That is the pattern the design itself blesses for acyclicity: *"the
//! redundant check makes the refusal named and prevents a future relaxation
//! from silently accepting cycles"* (§5.2). Each of the three is tested
//! DIRECTLY, with a real refusing input handed to the real function.

use crate::cause::CauseCode;
use crate::error::PlanRefusal;
use crate::grammar::RawStage;
use crate::ids::{IdKind, is_valid_id, normalize_conclusion_path};

/// A stage's claim on a canonical conclusion path, for clause 15.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PathClaim {
    /// The claiming stage.
    pub stage_id: String,
    /// The canonical path it derives.
    pub path: String,
}

/// **Derived charset clause.** Every stage and leg id matches
/// `^[a-z0-9][a-z0-9_-]*$`.
///
/// Runs FIRST, before every other clause, because it is the boundary that
/// closes traversal for the `.fleet/conclusions/<stage-id>` path. A clause
/// that ran before it would be reasoning about an id that may still contain
/// `../`.
///
/// # Errors
///
/// [`CauseCode::InvalidStageIdCharset`] or [`CauseCode::InvalidLegIdCharset`].
pub fn check_id_charset(stages: &[RawStage]) -> Result<(), PlanRefusal> {
    for stage in stages {
        if !is_valid_id(&stage.id) {
            return Err(charset_refusal(IdKind::Stage, &stage.id, &stage.id));
        }
        for leg in &stage.legs {
            if !is_valid_id(leg) {
                return Err(charset_refusal(IdKind::Leg, leg, &stage.id));
            }
        }
        if !is_valid_id(&stage.conclusion_leg) {
            return Err(charset_refusal(
                IdKind::Leg,
                &stage.conclusion_leg,
                &stage.id,
            ));
        }
    }
    Ok(())
}

fn charset_refusal(kind: IdKind, offender: &str, stage_id: &str) -> PlanRefusal {
    PlanRefusal::at(
        kind.charset_cause(),
        stage_id,
        format!(
            "{} {offender:?} is outside `^[a-z0-9][a-z0-9_-]*$` — no dot, no slash, \
             no leading separator, lowercase ASCII only. `<stage-id>` interpolates into \
             `.fleet/conclusions/<stage-id>`, so the set is closed by construction rather \
             than filtered.",
            kind.noun()
        ),
    )
}

/// §5.2 clause 2, "nonempty" half.
///
/// Subsumed by the charset clause at whole-plan level; tested directly.
///
/// # Errors
///
/// [`CauseCode::EmptyStageId`].
pub fn check_stage_id_nonempty(stage_ids: &[String]) -> Result<(), PlanRefusal> {
    if stage_ids.iter().any(String::is_empty) {
        return Err(PlanRefusal::whole(
            CauseCode::EmptyStageId,
            "a stage declares an empty id",
        ));
    }
    Ok(())
}

/// §5.2 clause 2, "unique" half.
///
/// # Errors
///
/// [`CauseCode::DuplicateStageId`].
pub fn check_stage_id_uniqueness(stage_ids: &[String]) -> Result<(), PlanRefusal> {
    let mut seen: Vec<&String> = Vec::new();
    for id in stage_ids {
        if seen.contains(&id) {
            return Err(PlanRefusal::at(
                CauseCode::DuplicateStageId,
                id.clone(),
                format!("stage id {id:?} is declared more than once"),
            ));
        }
        seen.push(id);
    }
    Ok(())
}

/// §5.2 clause 3 — unique dispatch-fixed leg ids WITHIN the graph.
///
/// A leg belongs to exactly one stage: a leg in two stages would be cut
/// from two different stage bases, which the base law (§3 clause 2) forbids.
///
/// # Errors
///
/// [`CauseCode::DuplicateLegId`].
pub fn check_leg_uniqueness(stages: &[RawStage]) -> Result<(), PlanRefusal> {
    let mut seen: Vec<(&str, &str)> = Vec::new();
    for stage in stages {
        for leg in &stage.legs {
            if let Some((other, _)) = seen.iter().find(|(_, l)| *l == leg.as_str()) {
                return Err(PlanRefusal::at(
                    CauseCode::DuplicateLegId,
                    stage.id.clone(),
                    format!(
                        "leg id {leg:?} appears in stage {other:?} and stage {:?}; a leg \
                         belongs to exactly one stage, because every leg in a stage is cut \
                         from that stage's own base commit (§3 clause 2)",
                        stage.id
                    ),
                ));
            }
            seen.push((&stage.id, leg));
        }
    }
    Ok(())
}

/// §5.2 clauses 4 and 5 — every referenced leg id is dispatch-declared, and
/// every dispatch-declared leg id appears exactly once.
///
/// # Errors
///
/// [`CauseCode::UnknownLegId`] when the graph names a leg dispatch did not
/// declare; [`CauseCode::LegNotPlaced`] when dispatch declared a leg the
/// graph places in no stage.
pub fn check_dispatch_leg_cover(
    stages: &[RawStage],
    dispatch_legs: &[String],
) -> Result<(), PlanRefusal> {
    for stage in stages {
        for leg in &stage.legs {
            if !dispatch_legs.contains(leg) {
                return Err(PlanRefusal::at(
                    CauseCode::UnknownLegId,
                    stage.id.clone(),
                    format!(
                        "stage {:?} names leg {leg:?}, which is not dispatch-declared. Leg ids \
                         are FIXED AT DISPATCH; the planner orders those ids rather than \
                         inventing new ones (§5.1).",
                        stage.id
                    ),
                ));
            }
        }
    }
    for declared in dispatch_legs {
        let placed = stages.iter().filter(|s| s.legs.contains(declared)).count();
        if placed == 0 {
            return Err(PlanRefusal::whole(
                CauseCode::LegNotPlaced,
                format!(
                    "dispatch-declared leg {declared:?} appears in no stage; every \
                     dispatch-declared leg id appears exactly once (§5.2 clause 5)"
                ),
            ));
        }
    }
    Ok(())
}

/// §5.2 clause 6 — every stage has at least one leg.
///
/// # Errors
///
/// [`CauseCode::StageWithoutLegs`].
pub fn check_stage_has_legs(stages: &[RawStage]) -> Result<(), PlanRefusal> {
    for stage in stages {
        if stage.legs.is_empty() {
            return Err(PlanRefusal::at(
                CauseCode::StageWithoutLegs,
                stage.id.clone(),
                format!("stage {:?} declares no leg", stage.id),
            ));
        }
    }
    Ok(())
}

/// §5.2 clause 10 — every `conclusion_leg` belongs to its own stage.
///
/// `conclusion_leg` answers ownership for multi-leg stages (§2.6). An owner
/// outside the stage would designate a leg that cannot see the stage it is
/// concluding about.
///
/// # Errors
///
/// [`CauseCode::ConclusionLegOutsideStage`].
pub fn check_conclusion_leg_membership(stages: &[RawStage]) -> Result<(), PlanRefusal> {
    for stage in stages {
        if !stage.legs.contains(&stage.conclusion_leg) {
            return Err(PlanRefusal::at(
                CauseCode::ConclusionLegOutsideStage,
                stage.id.clone(),
                format!(
                    "stage {:?} designates conclusion_leg {:?}, which is not one of its own \
                     legs ({})",
                    stage.id,
                    stage.conclusion_leg,
                    stage.legs.join(",")
                ),
            ));
        }
    }
    Ok(())
}

/// §5.2 clause 11 — a multi-leg stage declares exactly one conclusion value.
///
/// Design §2.1's reason: *"no parallel leg can honestly choose among
/// alternatives based on the not-yet-produced stage fan-in."*
///
/// # Errors
///
/// [`CauseCode::MultiLegConclusionCount`].
pub fn check_multi_leg_conclusion_count(stages: &[RawStage]) -> Result<(), PlanRefusal> {
    for stage in stages {
        if stage.legs.len() > 1 && stage.conclusions.len() != 1 {
            return Err(PlanRefusal::at(
                CauseCode::MultiLegConclusionCount,
                stage.id.clone(),
                format!(
                    "stage {:?} has {} legs and declares {} conclusion values; a multi-leg \
                     stage declares exactly one, because no parallel leg can honestly choose \
                     among alternatives based on the not-yet-produced stage fan-in (§2.1)",
                    stage.id,
                    stage.legs.len(),
                    stage.conclusions.len()
                ),
            ));
        }
    }
    Ok(())
}

/// §5.2 clause 12 — a stage with multiple declared conclusion values has
/// exactly one leg.
///
/// The contrapositive of clause 11. At whole-plan level clause 11 fires
/// first on every input that reaches both; this clause is implemented and
/// tested directly so its refusal stays named.
///
/// # Errors
///
/// [`CauseCode::MultiValueLegCount`].
pub fn check_multi_value_leg_count(stages: &[RawStage]) -> Result<(), PlanRefusal> {
    for stage in stages {
        if stage.conclusions.len() > 1 && stage.legs.len() != 1 {
            return Err(PlanRefusal::at(
                CauseCode::MultiValueLegCount,
                stage.id.clone(),
                format!(
                    "stage {:?} declares {} conclusion values and has {} legs; a multi-value \
                     vocabulary is restricted to a single-leg stage in v1 (§2.1)",
                    stage.id,
                    stage.conclusions.len(),
                    stage.legs.len()
                ),
            ));
        }
    }
    Ok(())
}

/// §5.2 clause 15 — canonical conclusion paths are distinct after stage-id
/// normalization.
///
/// Takes the derived claims rather than the stages, so the clause can be
/// handed a real colliding pair directly. Unreachable at whole-plan level
/// while the charset clause holds (normalization is the identity inside the
/// accepted charset, so a path collision implies a stage-id collision,
/// which clause 2 refuses first).
///
/// # Errors
///
/// [`CauseCode::DuplicateConclusionPath`].
pub fn check_conclusion_path_distinctness(claims: &[PathClaim]) -> Result<(), PlanRefusal> {
    let mut seen: Vec<(String, &str)> = Vec::new();
    for claim in claims {
        let normalized = normalize_conclusion_path(&claim.path);
        if let Some((_, other)) = seen.iter().find(|(p, _)| *p == normalized) {
            return Err(PlanRefusal::at(
                CauseCode::DuplicateConclusionPath,
                claim.stage_id.clone(),
                format!(
                    "stage {:?} and stage {other:?} both derive the canonical conclusion path \
                     {:?} after normalization; two stages cannot own one artifact",
                    claim.stage_id, claim.path
                ),
            ));
        }
        seen.push((normalized, &claim.stage_id));
    }
    Ok(())
}
