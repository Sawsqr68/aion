//! Pure phase-state folding boundary for the AWL-authored round loop.
//!
//! AWL loops thread exactly one value, so this activity partitions the plan's
//! items into ready/blocked/done, applies the round's BATTERY OUTCOMES and
//! PANEL VERDICTS through [`super::judge`], attaches the resulting feedback to
//! rejected items for their resumed dev session, and releases newly-unblocked
//! items. It performs no shell I/O and owns no workflow disposition — the
//! loop's `until` reads the returned partitions.
//!
//! The acceptance rule itself (red gate ⇒ ineligible; every configured lens
//! must have reported; derive-and-check per verdict) lives in
//! [`super::judge`], next to the feedback rendering it feeds.

use std::collections::BTreeSet;

use aion_worker::ActivityFailure;

use crate::types::{DoneItem, FoldPhaseInput, PhaseState, WorkItem};

use super::judge::judge_round;
use super::panel::require_panel;

/// Fold one completed round (or the plan seed) into the loop-carried
/// [`PhaseState`].
///
/// # Errors
///
/// Terminal on protocol violations that must never pass silently: an unusable
/// review panel (empty, duplicated, or an unusable lens name — checked on the
/// SEED call too, so a run that could never accept anything fails before its
/// first agent turn), duplicate item ids, a verdict naming an ungated item or
/// an unconfigured lens, a gated result matching no ready item, a dangling
/// `depends_on` id naming no known item, or a dependency deadlock that would
/// leave `blocked` non-empty with `ready` empty.
pub fn fold_phase(input: FoldPhaseInput) -> Result<PhaseState, ActivityFailure> {
    let FoldPhaseInput {
        prior,
        incoming,
        gated,
        verdicts,
        lenses,
    } = input;

    // The panel is checked on EVERY call, seed included: a run configured with
    // no lenses can never accept an item, and the honest place to say so is
    // before any worktree is provisioned.
    require_panel(&lenses)?;

    // The working pool: prior ready items plus every newly planned item.
    let mut pool: Vec<WorkItem> = prior.ready;
    pool.extend(incoming);
    let mut blocked = prior.blocked;
    let mut done = prior.done;

    require_unique_ids(&pool, &blocked, &done)?;
    require_known_dependencies(&pool, &blocked, &done)?;

    let mut round_lines: Vec<String> = Vec::new();
    for judgement in judge_round(gated, &verdicts, &lenses)? {
        let Some(position) = pool
            .iter()
            .position(|item| item.id == judgement.result.item.id)
        else {
            return Err(ActivityFailure::terminal(format!(
                "gated result for item {} does not correspond to any ready item",
                judgement.result.item.id
            )));
        };
        let mut item = pool.remove(position);
        if judgement.accepted {
            done.push(DoneItem {
                item_id: item.id,
                branch: judgement.result.branch.clone(),
                base_commit: judgement.result.base_commit.clone(),
                summary: judgement.result.report.summary.clone(),
            });
        } else {
            round_lines.push(format!(
                "item {} not accepted: {}",
                item.id,
                one_line(&judgement.feedback)
            ));
            item.feedback = judgement.feedback;
            pool.push(item);
        }
        round_lines.extend(judgement.violations);
    }

    // Release: every blocked item whose dependencies are all done moves to
    // ready. This preserves the invariant `blocked non-empty ⇒ ready
    // non-empty` for any acyclic dependency graph.
    let done_ids: BTreeSet<&str> = done.iter().map(|entry| entry.item_id.as_str()).collect();
    let (released, still_blocked): (Vec<WorkItem>, Vec<WorkItem>) = blocked
        .drain(..)
        .partition(|item| dependencies_satisfied(item, &done_ids));
    pool.extend(released);
    let (ready, newly_blocked): (Vec<WorkItem>, Vec<WorkItem>) = pool
        .drain(..)
        .partition(|item| dependencies_satisfied(item, &done_ids));
    let mut blocked = still_blocked;
    blocked.extend(newly_blocked);

    if ready.is_empty() && !blocked.is_empty() {
        let stuck: Vec<&str> = blocked.iter().map(|item| item.id.as_str()).collect();
        return Err(ActivityFailure::terminal(format!(
            "dependency deadlock: no item is ready but {stuck:?} remain \
             blocked — the plan's depends_on graph cannot make progress"
        )));
    }

    Ok(PhaseState {
        ready,
        blocked,
        done,
        evidence: smart_join(&prior.evidence, &round_lines.join("; ")),
    })
}

/// The evidence string is a single `; `-joined line, so a multi-paragraph
/// feedback body is collapsed to its first line there. The FULL text still
/// rides on the item's `feedback` for the next dev turn — this is the index,
/// not the record.
fn one_line(feedback: &str) -> String {
    feedback
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("(no feedback rendered)")
        .to_owned()
}

fn dependencies_satisfied(item: &WorkItem, done_ids: &BTreeSet<&str>) -> bool {
    item.depends_on
        .iter()
        .all(|dependency| done_ids.contains(dependency.as_str()))
}

/// Every item id across the partitions must be unique — a duplicate would
/// make verdict matching and dependency release ambiguous.
fn require_unique_ids(
    pool: &[WorkItem],
    blocked: &[WorkItem],
    done: &[DoneItem],
) -> Result<(), ActivityFailure> {
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let ids = pool
        .iter()
        .map(|item| item.id.as_str())
        .chain(blocked.iter().map(|item| item.id.as_str()))
        .chain(done.iter().map(|entry| entry.item_id.as_str()));
    for id in ids {
        if !seen.insert(id) {
            return Err(ActivityFailure::terminal(format!(
                "duplicate work item id {id} across the phase partitions"
            )));
        }
    }
    Ok(())
}

/// Every `depends_on` id must name a known item (pool, blocked, or done) —
/// a dangling dependency could never release and would deadlock the plan.
fn require_known_dependencies(
    pool: &[WorkItem],
    blocked: &[WorkItem],
    done: &[DoneItem],
) -> Result<(), ActivityFailure> {
    let known: BTreeSet<&str> = pool
        .iter()
        .map(|item| item.id.as_str())
        .chain(blocked.iter().map(|item| item.id.as_str()))
        .chain(done.iter().map(|entry| entry.item_id.as_str()))
        .collect();
    for item in pool.iter().chain(blocked.iter()) {
        for dependency in &item.depends_on {
            if !known.contains(dependency.as_str()) {
                return Err(ActivityFailure::terminal(format!(
                    "item {} depends on {dependency:?}, which names no \
                     planned item",
                    item.id
                )));
            }
        }
    }
    Ok(())
}

fn smart_join(prior_evidence: &str, round_evidence: &str) -> String {
    match (prior_evidence.is_empty(), round_evidence.is_empty()) {
        (_, true) => prior_evidence.to_owned(),
        (true, false) => round_evidence.to_owned(),
        (false, false) => format!("{prior_evidence}; {round_evidence}"),
    }
}
