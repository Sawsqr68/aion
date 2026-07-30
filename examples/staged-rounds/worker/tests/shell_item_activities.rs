//! Hermetic shell-activity tests for the per-item side: `provision_item`
//! against REAL git in scratch repositories, the pure `plan_reviews` and
//! `fold_phase` boundaries (the battery-and-panel acceptance rule), and the
//! destructive-path guard. Runtime behavior is proven by execution — worktrees
//! are read back from disk, never assumed.

use staged_rounds_worker::handlers::{fold_phase, plan_reviews, provision_item};
use staged_rounds_worker::paths::guard_destructive_path;
use staged_rounds_worker::shell::Shell;
use staged_rounds_worker::types::{
    Finding, FoldPhaseInput, GateOutcome, GateRun, GatedItem, ItemReport, ItemVerdict, Lens,
    Overall, PhaseState, PlanReviewsInput, Severity, WorkItem,
};

mod fixtures;
use fixtures::{commit_file_in, git, item, provision_input, scratch_repo};

// --- local fixtures -------------------------------------------------------

/// The two-lens panel most fold tests configure.
fn panel() -> Vec<Lens> {
    vec![lens("correctness"), lens("regressions")]
}

fn lens(name: &str) -> Lens {
    Lens {
        name: name.to_owned(),
        charter: format!("hunt {name} problems"),
    }
}

fn item_with_deps(id: &str, phase: i64, depends_on: &[&str]) -> WorkItem {
    WorkItem {
        phase,
        depends_on: depends_on.iter().map(|dep| (*dep).to_owned()).collect(),
        ..item(id)
    }
}

fn empty_state() -> PhaseState {
    PhaseState {
        ready: vec![],
        blocked: vec![],
        done: vec![],
        evidence: String::new(),
    }
}

/// A green battery outcome: nothing ran red, the tree stayed clean.
fn green_gate() -> GateOutcome {
    GateOutcome {
        pass: true,
        runs: vec![GateRun {
            name: "test".to_owned(),
            argv: vec!["cargo".to_owned(), "test".to_owned()],
            exit_code: 0,
            passed: true,
            output_tail: "ok".to_owned(),
        }],
        diff: "diff --git a/x b/x".to_owned(),
        diagnostics: "all 1 gate command(s) green".to_owned(),
    }
}

/// A RED battery outcome carrying the failing command's real evidence.
fn red_gate() -> GateOutcome {
    GateOutcome {
        pass: false,
        runs: vec![GateRun {
            name: "clippy".to_owned(),
            argv: vec!["cargo".to_owned(), "clippy".to_owned()],
            exit_code: 101,
            passed: false,
            output_tail: "error: MARKER_CLIPPY_WALL".to_owned(),
        }],
        diff: "diff --git a/x b/x".to_owned(),
        diagnostics: "gate `clippy` exited 101:\nerror: MARKER_CLIPPY_WALL".to_owned(),
    }
}

fn gated(work: &WorkItem, branch: &str, base: &str, gate: GateOutcome) -> GatedItem {
    GatedItem {
        item: work.clone(),
        workspace_path: format!("/ws/{}", work.id),
        branch: branch.to_owned(),
        base_commit: base.to_owned(),
        report: ItemReport {
            item_id: work.id.clone(),
            summary: format!("did {}", work.id),
            commits: vec!["head".to_owned()],
            claims: vec![],
        },
        gate,
    }
}

fn accept_verdict(id: &str, lens_name: &str) -> ItemVerdict {
    ItemVerdict {
        item_id: id.to_owned(),
        lens: lens_name.to_owned(),
        overall: Overall::Accept,
        findings: vec![],
        reject_reason: None,
    }
}

/// Every configured lens accepting one item — the only verdict set that can
/// accept it.
fn whole_panel_accepts(id: &str) -> Vec<ItemVerdict> {
    panel()
        .iter()
        .map(|lens| accept_verdict(id, &lens.name))
        .collect()
}

fn reject_verdict(id: &str, lens_name: &str, reason: &str, blocking_title: &str) -> ItemVerdict {
    ItemVerdict {
        item_id: id.to_owned(),
        lens: lens_name.to_owned(),
        overall: Overall::Reject,
        findings: vec![Finding {
            severity: Severity::Blocking,
            title: blocking_title.to_owned(),
            evidence: "evidence".to_owned(),
        }],
        reject_reason: Some(reason.to_owned()),
    }
}

/// The seed fold: partition `incoming` with nothing judged.
fn seed(prior: PhaseState, incoming: Vec<WorkItem>) -> FoldPhaseInput {
    FoldPhaseInput {
        prior,
        incoming,
        gated: vec![],
        verdicts: vec![],
        lenses: panel(),
    }
}

/// A round fold: judge `gated` against `verdicts` under the configured panel.
fn round(prior: PhaseState, gated: Vec<GatedItem>, verdicts: Vec<ItemVerdict>) -> FoldPhaseInput {
    FoldPhaseInput {
        prior,
        incoming: vec![],
        gated,
        verdicts,
        lenses: panel(),
    }
}

// --- provision_item -------------------------------------------------------

#[test]
fn provision_item_creates_a_worktree_on_the_item_branch_and_reports_the_base() -> anyhow::Result<()>
{
    let (_dir, repo, run_root) = scratch_repo()?;
    let provisioned = provision_item(
        &Shell::inherited(),
        provision_input(&repo, &run_root, item("it-a"), vec![]),
    )
    .map_err(|error| anyhow::anyhow!(error.message().to_owned()))?;

    assert_eq!(provisioned.workspace_path, format!("{run_root}/items/it-a"));
    assert_eq!(provisioned.branch, "staged/wf-1/it-a");
    let head = git(&provisioned.workspace_path, &["rev-parse", "HEAD"])?;
    assert_eq!(provisioned.base_commit, head);
    let branch = git(
        &provisioned.workspace_path,
        &["rev-parse", "--abbrev-ref", "HEAD"],
    )?;
    assert_eq!(branch, "staged/wf-1/it-a");
    Ok(())
}

#[test]
fn provision_item_reuses_a_clean_existing_worktree_preserving_commits() -> anyhow::Result<()> {
    let (_dir, repo, run_root) = scratch_repo()?;
    let shell = Shell::inherited();
    let first = provision_item(
        &shell,
        provision_input(&repo, &run_root, item("it-a"), vec![]),
    )
    .map_err(|error| anyhow::anyhow!(error.message().to_owned()))?;
    commit_file_in(&first.workspace_path, "it-a.txt", "round 1\n", "round 1")?;
    let round1_head = git(&first.workspace_path, &["rev-parse", "HEAD"])?;

    let second = provision_item(
        &shell,
        provision_input(&repo, &run_root, item("it-a"), vec![]),
    )
    .map_err(|error| anyhow::anyhow!(error.message().to_owned()))?;

    assert_eq!(second.workspace_path, first.workspace_path);
    assert_eq!(second.branch, first.branch);
    // The committed round-1 work SURVIVES re-provisioning.
    let head_after = git(&second.workspace_path, &["rev-parse", "HEAD"])?;
    assert_eq!(head_after, round1_head, "round-1 commit must be preserved");
    // And the base is the merge base with the base branch, not the new head.
    let merge_base = git(&repo, &["merge-base", &second.branch, "main"])?;
    assert_eq!(second.base_commit, merge_base);
    assert_ne!(second.base_commit, round1_head);
    Ok(())
}

#[test]
fn provision_item_refuses_a_dirty_existing_worktree() -> anyhow::Result<()> {
    let (_dir, repo, run_root) = scratch_repo()?;
    let shell = Shell::inherited();
    let first = provision_item(
        &shell,
        provision_input(&repo, &run_root, item("it-a"), vec![]),
    )
    .map_err(|error| anyhow::anyhow!(error.message().to_owned()))?;
    std::fs::write(
        std::path::Path::new(&first.workspace_path).join("uncommitted.txt"),
        "dirty\n",
    )?;

    let Err(error) = provision_item(
        &shell,
        provision_input(&repo, &run_root, item("it-a"), vec![]),
    ) else {
        anyhow::bail!("a dirty existing worktree was silently re-provisioned");
    };
    assert!(
        error.message().contains("UNCOMMITTED"),
        "{}",
        error.message()
    );
    Ok(())
}

#[test]
fn provision_item_refuses_a_dirty_stale_holder_by_name() -> anyhow::Result<()> {
    let (_dir, repo, run_root) = scratch_repo()?;
    let shell = Shell::inherited();
    // A stale worktree elsewhere holds the item branch, dirty.
    let stale = format!("{repo}/.staged-rounds/stale-holder");
    git(
        &repo,
        &["worktree", "add", "-B", "staged/wf-1/it-a", &stale, "main"],
    )?;
    std::fs::write(std::path::Path::new(&stale).join("dirty.txt"), "x\n")?;

    let Err(error) = provision_item(
        &shell,
        provision_input(&repo, &run_root, item("it-a"), vec![]),
    ) else {
        anyhow::bail!("a dirty stale holder was silently destroyed");
    };
    assert!(
        error.message().contains("stale-holder"),
        "{}",
        error.message()
    );
    assert!(
        error.message().contains("UNCOMMITTED"),
        "{}",
        error.message()
    );
    Ok(())
}

#[test]
fn provision_item_reclaims_a_clean_stale_holder() -> anyhow::Result<()> {
    let (_dir, repo, run_root) = scratch_repo()?;
    let shell = Shell::inherited();
    let stale = format!("{repo}/.staged-rounds/stale-holder");
    git(
        &repo,
        &["worktree", "add", "-B", "staged/wf-1/it-a", &stale, "main"],
    )?;

    let provisioned = provision_item(
        &shell,
        provision_input(&repo, &run_root, item("it-a"), vec![]),
    )
    .map_err(|error| anyhow::anyhow!(error.message().to_owned()))?;
    assert_eq!(provisioned.branch, "staged/wf-1/it-a");
    assert!(!std::path::Path::new(&stale).exists());
    Ok(())
}

#[test]
fn provision_item_refuses_a_non_slug_item_id() -> anyhow::Result<()> {
    let (_dir, repo, run_root) = scratch_repo()?;
    for bad in ["It-A", "it_a", "it a", "", "-a", "a-"] {
        let Err(error) = provision_item(
            &Shell::inherited(),
            provision_input(&repo, &run_root, item(bad), vec![]),
        ) else {
            anyhow::bail!("non-slug id {bad:?} was accepted");
        };
        assert!(
            error.message().contains("git-ref-safe slug"),
            "{}",
            error.message()
        );
    }
    Ok(())
}

// --- fold_phase: the battery-and-panel acceptance rule ---------------------

#[test]
fn fold_phase_seeds_ready_and_blocked_from_incoming_by_phase_and_deps() -> anyhow::Result<()> {
    let state = fold_phase(seed(
        empty_state(),
        vec![
            item_with_deps("it-a", 1, &[]),
            item_with_deps("it-b", 1, &[]),
            item_with_deps("it-c", 2, &["it-a"]),
        ],
    ))
    .map_err(|error| anyhow::anyhow!(error.message().to_owned()))?;

    let ready: Vec<&str> = state.ready.iter().map(|entry| entry.id.as_str()).collect();
    let blocked: Vec<&str> = state
        .blocked
        .iter()
        .map(|entry| entry.id.as_str())
        .collect();
    assert_eq!(ready, vec!["it-a", "it-b"]);
    assert_eq!(blocked, vec!["it-c"]);
    assert!(state.done.is_empty());
    Ok(())
}

#[test]
fn fold_phase_accepts_a_green_battery_and_whole_panel_and_releases_dependents() -> anyhow::Result<()>
{
    let seeded = fold_phase(seed(
        empty_state(),
        vec![
            item_with_deps("it-a", 1, &[]),
            item_with_deps("it-c", 2, &["it-a"]),
        ],
    ))
    .map_err(|error| anyhow::anyhow!(error.message().to_owned()))?;

    let work = seeded.ready[0].clone();
    let state = fold_phase(round(
        seeded,
        vec![gated(&work, "staged/wf-1/it-a", "base", green_gate())],
        whole_panel_accepts("it-a"),
    ))
    .map_err(|error| anyhow::anyhow!(error.message().to_owned()))?;

    assert_eq!(state.done.len(), 1);
    assert_eq!(state.done[0].item_id, "it-a");
    assert_eq!(state.done[0].branch, "staged/wf-1/it-a");
    assert_eq!(state.done[0].summary, "did it-a");
    let ready: Vec<&str> = state.ready.iter().map(|entry| entry.id.as_str()).collect();
    assert_eq!(ready, vec!["it-c"], "the dependent must be released");
    assert!(state.blocked.is_empty());
    Ok(())
}

/// THE fix for the pipeline's worst gap: a RED battery keeps the item out of
/// `done` even when every configured lens accepted it. A reviewer agent's
/// opinion cannot overrule a recorded non-zero exit code.
#[test]
fn a_red_gate_keeps_an_item_out_of_done_despite_a_unanimous_accept() -> anyhow::Result<()> {
    let work = item("it-a");
    let state = fold_phase(round(
        PhaseState {
            ready: vec![work.clone()],
            ..empty_state()
        },
        vec![gated(&work, "staged/wf-1/it-a", "base", red_gate())],
        whole_panel_accepts("it-a"),
    ))
    .map_err(|error| anyhow::anyhow!(error.message().to_owned()))?;

    assert!(
        state.done.is_empty(),
        "a red battery must make the item ineligible whatever the panel said"
    );
    assert_eq!(state.ready.len(), 1);
    assert_eq!(state.ready[0].id, "it-a");
    // The battery's real failing output rides into the next dev turn.
    assert!(
        state.ready[0].feedback.contains("GATE BATTERY IS RED"),
        "{}",
        state.ready[0].feedback
    );
    assert!(
        state.ready[0].feedback.contains("MARKER_CLIPPY_WALL"),
        "the failing gate output must reach the next round's dev turn: {}",
        state.ready[0].feedback
    );
    assert!(
        state.ready[0].feedback.contains("cargo clippy"),
        "the resolved argv must reach the next round's dev turn: {}",
        state.ready[0].feedback
    );
    assert!(
        state.evidence.contains("gate battery is RED"),
        "{}",
        state.evidence
    );
    Ok(())
}

/// A missing lens verdict is a recorded VIOLATION and does not accept: an item
/// accepted because one lens's activity produced nothing would be accepted on
/// strictly less evidence than the operator configured.
#[test]
fn a_missing_lens_verdict_is_a_violation_and_does_not_accept() -> anyhow::Result<()> {
    let work = item("it-a");
    let state = fold_phase(round(
        PhaseState {
            ready: vec![work.clone()],
            ..empty_state()
        },
        vec![gated(&work, "staged/wf-1/it-a", "base", green_gate())],
        // Only ONE of the two configured lenses reported.
        vec![accept_verdict("it-a", "correctness")],
    ))
    .map_err(|error| anyhow::anyhow!(error.message().to_owned()))?;

    assert!(
        state.done.is_empty(),
        "an item must not be accepted when a configured lens never reported"
    );
    assert_eq!(state.ready[0].id, "it-a");
    assert!(
        state.evidence.contains("never reported"),
        "the missing lens must be recorded as a violation: {}",
        state.evidence
    );
    assert!(
        state.ready[0].feedback.contains("regressions"),
        "the feedback must name the lens that did not report: {}",
        state.ready[0].feedback
    );
    Ok(())
}

/// An EMPTY verdict set never accepts — every configured lens is missing.
#[test]
fn an_empty_verdict_set_never_accepts() -> anyhow::Result<()> {
    let work = item("it-a");
    let state = fold_phase(round(
        PhaseState {
            ready: vec![work.clone()],
            ..empty_state()
        },
        vec![gated(&work, "staged/wf-1/it-a", "base", green_gate())],
        vec![],
    ))
    .map_err(|error| anyhow::anyhow!(error.message().to_owned()))?;
    assert!(state.done.is_empty(), "no verdict at all can never accept");
    assert_eq!(state.ready[0].id, "it-a");
    Ok(())
}

/// An EMPTY panel is a terminal error on EVERY call, the plan seed included:
/// a run configured with no lenses could never accept anything, so it fails
/// before a single worktree is provisioned.
#[test]
fn an_empty_lens_list_fails_loudly_even_on_the_seed() -> anyhow::Result<()> {
    let Err(error) = fold_phase(FoldPhaseInput {
        prior: empty_state(),
        incoming: vec![item("it-a")],
        gated: vec![],
        verdicts: vec![],
        lenses: vec![],
    }) else {
        anyhow::bail!("an empty review panel was accepted");
    };
    assert!(error.message().contains("EMPTY"), "{}", error.message());
    assert!(
        error.message().contains("config.lenses"),
        "the refusal must name what to configure: {}",
        error.message()
    );
    Ok(())
}

#[test]
fn a_duplicated_or_unusable_lens_name_fails_loudly() -> anyhow::Result<()> {
    let Err(error) = fold_phase(FoldPhaseInput {
        prior: empty_state(),
        incoming: vec![],
        gated: vec![],
        verdicts: vec![],
        lenses: vec![lens("correctness"), lens("correctness")],
    }) else {
        anyhow::bail!("a duplicated lens name was accepted");
    };
    assert!(error.message().contains("twice"), "{}", error.message());

    let Err(error) = fold_phase(FoldPhaseInput {
        prior: empty_state(),
        incoming: vec![],
        gated: vec![],
        verdicts: vec![],
        lenses: vec![lens("a/b")],
    }) else {
        anyhow::bail!("an unusable lens name was accepted");
    };
    assert!(error.message().contains("a/b"), "{}", error.message());
    Ok(())
}

#[test]
fn fold_phase_keeps_a_rejected_item_pending_with_every_rejection_attached() -> anyhow::Result<()> {
    let work = item("it-a");
    let state = fold_phase(round(
        PhaseState {
            ready: vec![work.clone()],
            ..empty_state()
        },
        vec![gated(&work, "staged/wf-1/it-a", "base", green_gate())],
        vec![
            accept_verdict("it-a", "correctness"),
            reject_verdict(
                "it-a",
                "regressions",
                "wrong seam",
                "Splits the wrong module",
            ),
        ],
    ))
    .map_err(|error| anyhow::anyhow!(error.message().to_owned()))?;

    assert!(state.done.is_empty());
    assert_eq!(state.ready.len(), 1);
    assert_eq!(state.ready[0].id, "it-a");
    let feedback = &state.ready[0].feedback;
    assert!(feedback.contains("REVIEW PANEL REJECTED"), "{feedback}");
    assert!(
        feedback.contains("regressions: wrong seam [blocking: Splits the wrong module]"),
        "the rejecting LENS must be named with its reason: {feedback}"
    );
    assert!(
        !feedback.contains("GATE BATTERY IS RED"),
        "a green battery must not be reported as red: {feedback}"
    );
    assert!(
        state.evidence.contains("it-a not accepted"),
        "{}",
        state.evidence
    );
    Ok(())
}

/// BOTH the gate failures and the rejecting verdicts reach the next round.
#[test]
fn a_red_gate_and_a_rejection_both_reach_the_next_rounds_feedback() -> anyhow::Result<()> {
    let work = item("it-a");
    let state = fold_phase(round(
        PhaseState {
            ready: vec![work.clone()],
            ..empty_state()
        },
        vec![gated(&work, "staged/wf-1/it-a", "base", red_gate())],
        vec![
            accept_verdict("it-a", "correctness"),
            reject_verdict(
                "it-a",
                "regressions",
                "breaks the caller",
                "Caller regressed",
            ),
        ],
    ))
    .map_err(|error| anyhow::anyhow!(error.message().to_owned()))?;

    let feedback = &state.ready[0].feedback;
    assert!(feedback.contains("MARKER_CLIPPY_WALL"), "{feedback}");
    assert!(feedback.contains("Caller regressed"), "{feedback}");
    // The mechanical failure comes FIRST: it is objective, and it is usually
    // the cause of the findings.
    let gate_at = feedback
        .find("GATE BATTERY IS RED")
        .ok_or_else(|| anyhow::anyhow!("no gate section in: {feedback}"))?;
    let panel_at = feedback
        .find("REVIEW PANEL REJECTED")
        .ok_or_else(|| anyhow::anyhow!("no panel section in: {feedback}"))?;
    assert!(gate_at < panel_at, "gate failures must lead: {feedback}");
    Ok(())
}

#[test]
fn fold_phase_derives_reject_from_a_blocking_finding_despite_asserted_accept() -> anyhow::Result<()>
{
    let work = item("it-a");
    let mut lying = reject_verdict("it-a", "regressions", "real problem", "Broken invariant");
    lying.overall = Overall::Accept; // asserted accept, blocking finding
    let state = fold_phase(round(
        PhaseState {
            ready: vec![work.clone()],
            ..empty_state()
        },
        vec![gated(&work, "b", "c", green_gate())],
        vec![accept_verdict("it-a", "correctness"), lying],
    ))
    .map_err(|error| anyhow::anyhow!(error.message().to_owned()))?;

    assert!(state.done.is_empty(), "derive-and-check must reject");
    assert_eq!(state.ready[0].id, "it-a");
    assert!(
        state.evidence.contains("disagrees with the derived"),
        "{}",
        state.evidence
    );
    // The DERIVATION must also drive the feedback, not just the evidence. The
    // two are separately computed, and `accepted` already refuses on the
    // recorded violation alone — so without this the derivation's effect on
    // the rejection line is unpinned, and a lying verdict cycles the item with
    // "the panel produced no usable rejection reason" while a perfectly usable
    // one existed. That silently spends a fix cycle out of a bounded budget.
    let feedback = &state.ready[0].feedback;
    assert!(
        feedback.contains("regressions") && feedback.contains("Broken invariant"),
        "the derived rejection must name its lens and blocking finding even when \
         the reviewer asserted accept: {feedback}"
    );
    Ok(())
}

#[test]
fn fold_phase_fails_loudly_on_a_verdict_naming_an_ungated_item_or_lens() -> anyhow::Result<()> {
    let work = item("it-a");
    let mut verdicts = whole_panel_accepts("it-a");
    verdicts.push(accept_verdict("it-ghost", "correctness"));
    let Err(error) = fold_phase(round(
        PhaseState {
            ready: vec![work.clone()],
            ..empty_state()
        },
        vec![gated(&work, "b", "c", green_gate())],
        verdicts,
    )) else {
        anyhow::bail!("a verdict for an ungated item was silently ignored");
    };
    assert!(error.message().contains("it-ghost"), "{}", error.message());

    let Err(error) = fold_phase(round(
        PhaseState {
            ready: vec![item("it-a")],
            ..empty_state()
        },
        vec![gated(&item("it-a"), "b", "c", green_gate())],
        vec![accept_verdict("it-a", "unconfigured_lens")],
    )) else {
        anyhow::bail!("a verdict from an unconfigured lens was silently accepted");
    };
    assert!(
        error.message().contains("unconfigured_lens"),
        "{}",
        error.message()
    );

    let Err(error) = fold_phase(round(
        PhaseState {
            ready: vec![item("it-a")],
            ..empty_state()
        },
        vec![gated(&item("it-a"), "b", "c", green_gate())],
        vec![
            accept_verdict("it-a", "correctness"),
            accept_verdict("it-a", "correctness"),
        ],
    )) else {
        anyhow::bail!("two verdicts from one panel seat were silently accepted");
    };
    assert!(
        error.message().contains("two verdicts"),
        "{}",
        error.message()
    );

    // A verdict that somehow arrived UNSTAMPED (no lens at all) matches no
    // configured lens, so it is refused rather than counted as a report.
    let Err(error) = fold_phase(round(
        PhaseState {
            ready: vec![item("it-a")],
            ..empty_state()
        },
        vec![gated(&item("it-a"), "b", "c", green_gate())],
        vec![accept_verdict("it-a", "")],
    )) else {
        anyhow::bail!("an unstamped verdict was silently counted as a lens report");
    };
    assert!(
        error.message().contains("did \nnot configure")
            || error.message().contains("not configure"),
        "{}",
        error.message()
    );
    Ok(())
}

#[test]
fn fold_phase_fails_loudly_on_a_dangling_dependency() -> anyhow::Result<()> {
    let Err(error) = fold_phase(seed(
        empty_state(),
        vec![item_with_deps("it-a", 1, &["it-nowhere"])],
    )) else {
        anyhow::bail!("a dangling depends_on was accepted");
    };
    assert!(
        error.message().contains("it-nowhere"),
        "{}",
        error.message()
    );
    Ok(())
}

#[test]
fn fold_phase_never_leaves_blocked_nonempty_with_ready_empty() -> anyhow::Result<()> {
    // A dependency cycle can never release: the fold must fail loudly
    // rather than return a state the loop would spin on forever.
    let Err(error) = fold_phase(seed(
        empty_state(),
        vec![
            item_with_deps("it-a", 1, &["it-b"]),
            item_with_deps("it-b", 1, &["it-a"]),
        ],
    )) else {
        anyhow::bail!("a dependency cycle produced a spinnable state");
    };
    assert!(error.message().contains("deadlock"), "{}", error.message());
    Ok(())
}

// --- plan_reviews ---------------------------------------------------------

#[test]
fn plan_reviews_flattens_item_times_lens_in_deterministic_order() -> anyhow::Result<()> {
    let gated_items = vec![
        gated(&item("it-a"), "b-a", "base", green_gate()),
        gated(&item("it-b"), "b-b", "base", red_gate()),
    ];
    let panel_tasks = plan_reviews(PlanReviewsInput {
        gated: gated_items,
        lenses: panel(),
    })
    .map_err(|error| anyhow::anyhow!(error.message().to_owned()))?;

    let seats: Vec<(&str, &str)> = panel_tasks
        .iter()
        .map(|task| (task.work.item.id.as_str(), task.lens.name.as_str()))
        .collect();
    assert_eq!(
        seats,
        vec![
            ("it-a", "correctness"),
            ("it-a", "regressions"),
            ("it-b", "correctness"),
            ("it-b", "regressions"),
        ],
        "item order outer, configured lens order inner — a replay must produce \
         byte-identical seats"
    );
    // Each seat carries the lens CHARTER (it travels in the prompt) and the
    // item's recorded battery outcome (it grounds findings).
    assert_eq!(panel_tasks[0].lens.charter, "hunt correctness problems");
    assert!(panel_tasks[0].work.gate.pass);
    assert!(!panel_tasks[2].work.gate.pass);
    Ok(())
}

#[test]
fn plan_reviews_over_no_items_is_an_empty_panel_not_an_error() -> anyhow::Result<()> {
    let panel_tasks = plan_reviews(PlanReviewsInput {
        gated: vec![],
        lenses: panel(),
    })
    .map_err(|error| anyhow::anyhow!(error.message().to_owned()))?;
    assert!(panel_tasks.is_empty());
    Ok(())
}

#[test]
fn plan_reviews_refuses_an_empty_panel_loudly() -> anyhow::Result<()> {
    let Err(error) = plan_reviews(PlanReviewsInput {
        gated: vec![gated(&item("it-a"), "b", "base", green_gate())],
        lenses: vec![],
    }) else {
        anyhow::bail!("an empty review panel produced review tasks");
    };
    assert!(error.message().contains("EMPTY"), "{}", error.message());
    assert!(
        error.message().contains("config.lenses"),
        "{}",
        error.message()
    );
    Ok(())
}

#[test]
fn plan_reviews_refuses_two_gated_results_for_one_item() -> anyhow::Result<()> {
    let Err(error) = plan_reviews(PlanReviewsInput {
        gated: vec![
            gated(&item("it-a"), "b", "base", green_gate()),
            gated(&item("it-a"), "b", "base", green_gate()),
        ],
        lenses: panel(),
    }) else {
        anyhow::bail!("a duplicated gated item produced an ambiguous panel");
    };
    assert!(error.message().contains("it-a"), "{}", error.message());
    Ok(())
}

// --- the destructive-path guard ---------------------------------------------

#[test]
fn destructive_paths_outside_staged_rounds_root_are_refused() -> anyhow::Result<()> {
    let (_dir, repo, run_root) = scratch_repo()?;
    std::fs::create_dir_all(&run_root)?;
    // The repo root itself and the staged-rounds root are refused; a real
    // run workspace under it is allowed.
    assert!(guard_destructive_path(&repo, &repo).is_err());
    let staged_root = format!("{repo}/.staged-rounds");
    assert!(guard_destructive_path(&repo, &staged_root).is_err());
    guard_destructive_path(&repo, &run_root)
        .map_err(|error| anyhow::anyhow!(error.message().to_owned()))?;
    Ok(())
}
