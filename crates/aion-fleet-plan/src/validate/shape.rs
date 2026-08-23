//! §5.2 clauses 7, 8, 9, 13, 14 and 16 — the chain's shape.
//!
//! **Evaluation order note, and it is deliberate.** §5.2 lists acyclicity
//! (clause 9) after the cardinality rules (clause 8), and says the redundant
//! check exists so "the refusal makes the refusal named". This module
//! therefore runs [`check_acyclic`] BEFORE [`check_chain_cardinality`]: a
//! pure cycle has no stage with `after=-`, so a cardinality-first order
//! would report a cycle as [`CauseCode::NoRoot`] and the named refusal the
//! clause exists for would never fire. Design §9.2 names "`l1 after=l3`" as
//! an exit-4 specimen; under this order it refuses as `cycle`.

use crate::cause::CauseCode;
use crate::error::PlanRefusal;
use crate::grammar::RawStage;
use crate::graph::Admit;

/// §5.2 clause 7 — every `after` id names a known stage, and no stage
/// follows itself.
///
/// # Errors
///
/// [`CauseCode::UnknownPredecessor`], [`CauseCode::SelfPredecessor`].
pub fn check_predecessors(stages: &[RawStage]) -> Result<(), PlanRefusal> {
    let known: Vec<&str> = stages.iter().map(|s| s.id.as_str()).collect();
    for stage in stages {
        let Some(after) = &stage.after else { continue };
        if after == &stage.id {
            return Err(PlanRefusal::at(
                CauseCode::SelfPredecessor,
                stage.id.clone(),
                format!("stage {:?} names itself as its own predecessor", stage.id),
            ));
        }
        if !known.contains(&after.as_str()) {
            return Err(PlanRefusal::at(
                CauseCode::UnknownPredecessor,
                stage.id.clone(),
                format!(
                    "stage {:?} declares `after={after}`, which names no stage in the block",
                    stage.id
                ),
            ));
        }
    }
    Ok(())
}

/// §5.2 clause 9 — the graph is acyclic.
///
/// Validated explicitly even though the clause-8 cardinality rules should
/// imply it, because "the redundant check makes the refusal named and
/// prevents a future relaxation from silently accepting cycles" (§5.2).
///
/// Each stage has at most one `after`, so following predecessors from any
/// stage walks a single path; a cycle is a stage reachable from itself.
///
/// # Errors
///
/// [`CauseCode::Cycle`], naming the stages on the cycle in walk order.
pub fn check_acyclic(stages: &[RawStage]) -> Result<(), PlanRefusal> {
    for start in stages {
        let mut path: Vec<&str> = vec![start.id.as_str()];
        let mut cursor = start;
        // At most one `after` per stage bounds the walk by the stage count.
        for _ in 0..stages.len() {
            let Some(after) = &cursor.after else { break };
            if let Some(at) = path.iter().position(|s| s == after) {
                let mut cycle: Vec<&str> = path[at..].to_vec();
                cycle.push(after.as_str());
                return Err(PlanRefusal::at(
                    CauseCode::Cycle,
                    start.id.clone(),
                    format!(
                        "the stage graph contains a cycle: {}. The v1 graph is a single \
                         chain (§2.1).",
                        cycle.join(" <- ")
                    ),
                ));
            }
            let Some(next) = stages.iter().find(|s| &s.id == after) else {
                // Clause 7 refuses an unknown predecessor before this runs.
                break;
            };
            path.push(next.id.as_str());
            cursor = next;
        }
    }
    Ok(())
}

/// §5.2 clause 8 — exactly one root, at most one predecessor and successor
/// per stage, and exactly one terminal chain.
///
/// A stage's `after` holds exactly one id by grammar, so "at most one
/// predecessor" is structural. "At most one successor" is the clause that
/// refuses an arbitrary multi-parent graph: in a grammar whose `after`
/// holds one id, fan-out is the only shape multi-parent can take, and
/// design §3 refuses arbitrary multi-parent stage graphs in v1 because
/// "they would require a new canonical cross-stage merge order and conflict
/// policy".
///
/// # Errors
///
/// [`CauseCode::NoRoot`], [`CauseCode::MultipleRoots`],
/// [`CauseCode::MultipleSuccessors`], [`CauseCode::MultipleTerminals`].
pub fn check_chain_cardinality(stages: &[RawStage]) -> Result<(), PlanRefusal> {
    let roots: Vec<&str> = stages
        .iter()
        .filter(|s| s.after.is_none())
        .map(|s| s.id.as_str())
        .collect();
    match roots.len() {
        1 => {}
        0 => {
            return Err(PlanRefusal::whole(
                CauseCode::NoRoot,
                "no stage declares `after=-`; the v1 graph has exactly one root (§2.1)",
            ));
        }
        _ => {
            return Err(PlanRefusal::whole(
                CauseCode::MultipleRoots,
                format!(
                    "stages {} all declare `after=-`; the v1 graph has exactly one root (§2.1)",
                    roots.join(", ")
                ),
            ));
        }
    }

    for stage in stages {
        let successors: Vec<&str> = stages
            .iter()
            .filter(|s| s.after.as_deref() == Some(stage.id.as_str()))
            .map(|s| s.id.as_str())
            .collect();
        if successors.len() > 1 {
            return Err(PlanRefusal::at(
                CauseCode::MultipleSuccessors,
                stage.id.clone(),
                format!(
                    "stages {} all declare `after={}`; every stage has at most one successor. \
                     Arbitrary multi-parent stage graphs are refused in v1 — they would need a \
                     canonical cross-stage merge order and conflict policy the design \
                     deliberately does not choose (§3, §10).",
                    successors.join(", "),
                    stage.id
                ),
            ));
        }
    }

    let terminals: Vec<&str> = stages
        .iter()
        .filter(|s| {
            !stages
                .iter()
                .any(|o| o.after.as_deref() == Some(s.id.as_str()))
        })
        .map(|s| s.id.as_str())
        .collect();
    if terminals.len() != 1 {
        return Err(PlanRefusal::whole(
            CauseCode::MultipleTerminals,
            format!(
                "stages {} each have no successor; the v1 graph has exactly one terminal \
                 chain (§2.1)",
                terminals.join(", ")
            ),
        ));
    }

    Ok(())
}

/// The single chain in execution order, root first.
///
/// Callable only after [`check_chain_cardinality`] has passed, which is what
/// makes the order total.
#[must_use]
pub fn chain_order(stages: &[RawStage]) -> Vec<usize> {
    let mut order = Vec::with_capacity(stages.len());
    let Some(root) = stages.iter().position(|s| s.after.is_none()) else {
        return order;
    };
    order.push(root);
    let mut current = root;
    while let Some(next) = stages
        .iter()
        .position(|s| s.after.as_deref() == Some(stages[current].id.as_str()))
    {
        if order.contains(&next) {
            break;
        }
        order.push(next);
        current = next;
    }
    order
}

/// §5.2 clauses 13 and 14 — every `admit` reference names an EARLIER
/// reachable stage, and every compared conclusion belongs to that stage's
/// declared closed vocabulary.
///
/// `order` is the chain order from [`chain_order`]; "earlier reachable"
/// means strictly earlier on that single chain, which is the only sense in
/// which a stage's conclusion can already be recorded when the admission is
/// evaluated (§6 clause 6).
///
/// # Errors
///
/// [`CauseCode::AdmitUnknownStage`], [`CauseCode::AdmitUnreachableStage`],
/// [`CauseCode::AdmitUndeclaredConclusion`].
pub fn check_admit_references(
    stages: &[RawStage],
    order: &[usize],
    parsed: &[Admit],
) -> Result<(), PlanRefusal> {
    for (position, &index) in order.iter().enumerate() {
        let stage = &stages[index];
        let Some(Admit::Conclusion {
            stage_id: referenced,
            value,
            ..
        }) = parsed.get(index)
        else {
            continue;
        };

        let Some(target_index) = stages.iter().position(|s| &s.id == referenced) else {
            return Err(PlanRefusal::at(
                CauseCode::AdmitUnknownStage,
                stage.id.clone(),
                format!(
                    "stage {:?} admits on `{referenced}.conclusion`, which names no stage in \
                     the block",
                    stage.id
                ),
            ));
        };

        let Some(target_position) = order.iter().position(|&i| i == target_index) else {
            return Err(PlanRefusal::at(
                CauseCode::AdmitUnreachableStage,
                stage.id.clone(),
                format!(
                    "stage {:?} admits on `{referenced}.conclusion`, but {referenced} is not on \
                     the chain",
                    stage.id
                ),
            ));
        };
        if target_position >= position {
            return Err(PlanRefusal::at(
                CauseCode::AdmitUnreachableStage,
                stage.id.clone(),
                format!(
                    "stage {:?} admits on `{referenced}.conclusion`, but {referenced} runs at or \
                     after it on the chain; an admission derives only from an EARLIER stage's \
                     already-recorded conclusion (§5.2 clause 13, §6 clause 6)",
                    stage.id
                ),
            ));
        }

        let target = &stages[target_index];
        if !target.conclusions.iter().any(|v| v == value) {
            return Err(PlanRefusal::at(
                CauseCode::AdmitUndeclaredConclusion,
                stage.id.clone(),
                format!(
                    "stage {:?} compares against conclusion {value:?}, which is outside stage \
                     {referenced}'s declared closed vocabulary ({})",
                    stage.id,
                    target.conclusions.join(",")
                ),
            ));
        }
    }
    Ok(())
}

/// §5.2 clause 16 — `advance` is exactly `auto` or `hold`.
///
/// # Errors
///
/// [`CauseCode::InvalidAdvance`].
pub fn check_advance(stages: &[RawStage]) -> Result<(), PlanRefusal> {
    for stage in stages {
        if stage.advance != "auto" && stage.advance != "hold" {
            return Err(PlanRefusal::at(
                CauseCode::InvalidAdvance,
                stage.id.clone(),
                format!(
                    "stage {:?} declares `advance={}`; §5.2 clause 16 admits exactly `auto` or \
                     `hold`",
                    stage.id, stage.advance
                ),
            ));
        }
    }
    Ok(())
}
