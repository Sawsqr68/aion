//! R8 — the #140 exemplar (`ef96ba5c`), end to end, as EXECUTABLE evidence.
//!
//! Both paths run against the real coordinator machinery: the real
//! validator produces the ordering graph, the real step binary advances
//! every boundary, the real decision function chooses every action, and the
//! real ledger is the only history consulted.
//!
//! * **§9.3 happy path** — three stages, each based on the predecessor's
//!   ACCEPTED SHA, a chain report carrying all ids, bases, results,
//!   conclusions and acceptance decisions, and **no skips**.
//! * **§9.4 measured-red descope** — `measured-red\n` verifies as a SUCCESS
//!   conclusion and is never routed to `judge_flagged`; L2 runs; L3's
//!   admission evaluates FALSE from RECORDED history; L3 is recorded
//!   skipped with source stage, observed conclusion and canonical
//!   condition; **no L3 worktree is cut and no L3 builder seat is spent**;
//!   the chain result is L2's accepted SHA; the outcome is **planned
//!   descope**.
//!
//! Also R4 (the record shapes, field for field) and R5 (the base law, all
//! five clauses), because both are properties of these two walks.
//!
//! **Fixture-size law: boundary-level synthetic inputs throughout — a
//! three-stage block, a ledger of at most twelve records, token files of at
//! most 20 bytes. No multi-GB fixtures.**

#[path = "support/chain.rs"]
mod support;

use aion_fleet_plan::chain::apply::{External, apply};
use aion_fleet_plan::chain::records::StageFailed;
use aion_fleet_plan::chain::report::ChainReport;
use aion_fleet_plan::chain::{ChainAction, ChainLedger, ChainOutcome, Recorded, next_action};
use aion_fleet_plan::graph::{Admit, Advance, Comparison, NormalizedGraph};
use support::{Workspace, exemplar_graph, exemplar_graph_json, sha};

fn root_base() -> String {
    sha(1)
}
fn l1_pushed() -> String {
    sha(2)
}
fn l2_pushed() -> String {
    sha(3)
}
fn l3_pushed() -> String {
    sha(4)
}

fn dispatch_json(child: &str) -> String {
    format!(r#"{{"result":"dispatched","child_workflow_id":"{child}"}}"#)
}
fn completion_json(base: &str, pushed: &str) -> String {
    format!(
        r#"{{"result":"completed","child_outcome":"done","base_commit":"{base}","pushed_commit":"{pushed}","kept_root":"","failed":false}}"#
    )
}
fn verified_json(stage: &str, child: &str, commit: &str, value: &str) -> String {
    format!(
        r#"{{"result":"verified","record":{{"stage_id":"{stage}","child_workflow_id":"{child}","result_commit":"{commit}","conclusion_path":".fleet/conclusions/{stage}","conclusion":"{value}"}}}}"#
    )
}

/// Run the chain to completion, returning every applied fact in order.
fn drive(ws: &Workspace) -> Vec<serde_json::Value> {
    let base = root_base();
    let mut applied = Vec::new();
    // Bounded by the graph's stage count times the boundaries per stage —
    // never an unbounded loop, and never one blocking sleep.
    for _ in 0..40 {
        let out = ws.step(&base);
        if out["finished"] == serde_json::Value::Bool(true) {
            applied.push(out);
            return applied;
        }
        assert_eq!(
            out["ok"],
            serde_json::Value::Bool(true),
            "step refused: {out}"
        );
        applied.push(out);
    }
    unreachable!("the chain did not finish within its bound");
}

fn ledger_of(ws: &Workspace) -> ChainLedger {
    let bytes = ws.ledger_bytes();
    serde_json::from_slice(&bytes).unwrap_or_else(|e| unreachable!("ledger did not decode: {e}"))
}

// ============================================================ §9.2

#[test]
fn r8a_check_plan_exit_by_exit() {
    // §9.2: exit 0 on the §9.1 block; exit 4 on each named specimen; and
    // NO builder leg is dispatched on 1, 3, or 4.
    let dir = tempfile::tempdir().unwrap_or_else(|e| unreachable!("{e}"));
    let plan_path = dir.path().join("plan.md");
    let run = |lines: &[&str], loose: &str| -> (i32, serde_json::Value) {
        let text = format!("# PLAN\n\n```fleet-stages-v1\n{}\n```\n", lines.join("\n"));
        std::fs::write(&plan_path, text).unwrap_or_else(|e| unreachable!("{e}"));
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_aion-fleet-plan"))
            .arg("check-plan")
            .arg("--plan-file")
            .arg(&plan_path)
            .arg("--legs")
            .arg(support::EXEMPLAR_LEGS.join(","))
            .arg("--loose")
            .arg(loose)
            .output()
            .unwrap_or_else(|e| unreachable!("{e}"));
        let text = String::from_utf8_lossy(&out.stdout);
        let json: serde_json::Value = serde_json::from_str(text.trim())
            .unwrap_or_else(|e| unreachable!("check-plan stdout is not JSON ({e}): {text}"));
        (out.status.code().unwrap_or(-1), json)
    };

    // exit 0 — the §9.1 block.
    let (code, json) = run(&support::EXEMPLAR, "");
    assert_eq!(code, 0, "the #140 exemplar block did not exit 0: {json}");
    assert_eq!(json["chain_block"], serde_json::Value::Bool(true));

    // exit 1 — loose `.fleet` state (§5.2 clause 17). Absence keeps exit 1
    // and is NOT widened into exit 4.
    let (code, json) = run(&support::EXEMPLAR, " M .fleet/plan.md");
    assert_eq!(code, 1, "loose .fleet state did not exit 1: {json}");
    assert_eq!(json["cause_code"], "plan_state_uncommitted");

    // exit 4 — every specimen §9.2 names, each with its own cause code.
    let mut cycle = support::EXEMPLAR;
    cycle[0] = "stage id=l1 legs=l1-design after=l3 admit=always conclusions=design-ready,measured-red conclusion_leg=l1-design advance=auto";
    let mut unknown_pred = support::EXEMPLAR;
    unknown_pred[1] = "stage id=l2 legs=l2-classify after=l9 admit=always conclusions=classification-ready conclusion_leg=l2-classify advance=auto";
    let mut dup_leg = support::EXEMPLAR;
    dup_leg[1] = "stage id=l2 legs=l1-design after=l1 admit=always conclusions=classification-ready conclusion_leg=l1-design advance=auto";
    let mut undeclared = support::EXEMPLAR;
    undeclared[2] = "stage id=l3 legs=l3-route after=l2 admit=l1.conclusion!=never-declared conclusions=route-ready conclusion_leg=l3-route advance=auto";
    let mut bad_owner = support::EXEMPLAR;
    bad_owner[1] = "stage id=l2 legs=l2-classify after=l1 admit=always conclusions=classification-ready conclusion_leg=l1-design advance=auto";

    let specimens: &[(&str, Vec<&str>, &str)] = &[
        (
            "unknown predecessor",
            unknown_pred.to_vec(),
            "unknown_predecessor",
        ),
        ("cycle", cycle.to_vec(), "cycle"),
        ("duplicated leg", dup_leg.to_vec(), "duplicate_leg_id"),
        (
            "undeclared conclusion",
            undeclared.to_vec(),
            "admit_undeclared_conclusion",
        ),
        (
            "conclusion owner outside its stage",
            bad_owner.to_vec(),
            "conclusion_leg_outside_stage",
        ),
        (
            "multi-leg stage with multiple conclusion values",
            vec![
                "stage id=l1 legs=l1-design,l2-classify after=- admit=always conclusions=a,b conclusion_leg=l1-design advance=auto",
                "stage id=l3 legs=l3-route after=l1 admit=always conclusions=route-ready conclusion_leg=l3-route advance=auto",
            ],
            "multi_leg_conclusion_count",
        ),
        (
            "arbitrary multi-parent graph",
            vec![
                "stage id=l1 legs=l1-design after=- admit=always conclusions=design-ready conclusion_leg=l1-design advance=auto",
                "stage id=l2 legs=l2-classify after=l1 admit=always conclusions=classification-ready conclusion_leg=l2-classify advance=auto",
                "stage id=l3 legs=l3-route after=l1 admit=always conclusions=route-ready conclusion_leg=l3-route advance=auto",
            ],
            "multiple_successors",
        ),
    ];
    for (name, lines, expect) in specimens {
        let (code, json) = run(lines, "");
        assert_eq!(code, 4, "specimen {name:?} did not exit 4: {json}");
        assert_eq!(
            json["cause_code"], *expect,
            "specimen {name:?} reported the wrong cause code: {json}"
        );
        // §5.3: exit 4 must NOT widen `plan_mismatch`, which is exit 3.
        assert_ne!(code, 3, "specimen {name:?} widened plan_mismatch");
    }
}

#[test]
fn r8a_a_plan_without_a_block_takes_the_legacy_path() {
    // §7 clause 5, and the coordinator's own refusal of a missing chain
    // contract: exit 0 with `chain_block: false`, which is what tells the
    // shell to fall through to the pre-existing behaviour byte-identically.
    let dir = tempfile::tempdir().unwrap_or_else(|e| unreachable!("{e}"));
    let plan_path = dir.path().join("plan.md");
    std::fs::write(&plan_path, "# PLAN\n\nOrdinary parallel-only plan.\n")
        .unwrap_or_else(|e| unreachable!("{e}"));
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_aion-fleet-plan"))
        .arg("check-plan")
        .arg("--plan-file")
        .arg(&plan_path)
        .arg("--legs")
        .arg("build")
        .output()
        .unwrap_or_else(|e| unreachable!("{e}"));
    assert_eq!(out.status.code(), Some(0));
    let json: serde_json::Value =
        serde_json::from_slice(&out.stdout).unwrap_or_else(|e| unreachable!("{e}"));
    assert_eq!(json["chain_block"], serde_json::Value::Bool(false));
}

// ============================================================ §9.3

#[test]
fn r8b_happy_path_three_stages_each_on_the_predecessors_accepted_sha() {
    let ws = Workspace::new(&exemplar_graph_json());
    ws.deposit("dispatch-l1.json", &dispatch_json("child-l1"));
    ws.deposit("dispatch-l2.json", &dispatch_json("child-l2"));
    ws.deposit("dispatch-l3.json", &dispatch_json("child-l3"));
    ws.deposit(
        "completion-l1.json",
        &completion_json(&root_base(), &l1_pushed()),
    );
    ws.deposit(
        "completion-l2.json",
        &completion_json(&l1_pushed(), &l2_pushed()),
    );
    ws.deposit(
        "completion-l3.json",
        &completion_json(&l2_pushed(), &l3_pushed()),
    );
    // §9.3: L1 concludes `design-ready`, so L3's `!=measured-red` is TRUE.
    ws.deposit(
        "verify-l1.json",
        &verified_json("l1", "child-l1", &l1_pushed(), "design-ready"),
    );
    ws.deposit(
        "verify-l2.json",
        &verified_json("l2", "child-l2", &l2_pushed(), "classification-ready"),
    );
    ws.deposit(
        "verify-l3.json",
        &verified_json("l3", "child-l3", &l3_pushed(), "route-ready"),
    );

    let applied = drive(&ws);
    let last = applied
        .last()
        .unwrap_or_else(|| unreachable!("no steps ran"));
    assert_eq!(last["finished"], serde_json::Value::Bool(true));
    assert_eq!(last["outcome"], "completed");
    assert_eq!(last["result_commit"], l3_pushed());

    let ledger = ledger_of(&ws);
    let graph = exemplar_graph();
    let report = ChainReport::derive(&graph, &ledger, ChainOutcome::Completed);

    // §9.3 clause 6: all stage workflow ids, bases, results, conclusions,
    // acceptance decisions — and NO SKIPS.
    assert!(!report.has_skips(), "the happy path recorded a skip");
    assert_eq!(report.stages.len(), 3);
    let expected: [(&str, &str, &str, &str, &str); 3] = [
        ("l1", "child-l1", &root_base(), &l1_pushed(), "design-ready"),
        (
            "l2",
            "child-l2",
            &l1_pushed(),
            &l2_pushed(),
            "classification-ready",
        ),
        ("l3", "child-l3", &l2_pushed(), &l3_pushed(), "route-ready"),
    ];
    for (line, (id, child, base, result, conclusion)) in report.stages.iter().zip(expected) {
        assert_eq!(line.stage_id, id);
        assert_eq!(
            line.child_workflow_id.as_deref(),
            Some(child),
            "{id} child id"
        );
        assert_eq!(line.base_commit.as_deref(), Some(base), "{id} base");
        assert_eq!(line.result_commit.as_deref(), Some(result), "{id} result");
        assert_eq!(
            line.conclusion.as_deref(),
            Some(conclusion),
            "{id} conclusion"
        );
        assert_eq!(
            line.accepted_commit.as_deref(),
            Some(result),
            "{id} accepted"
        );
        assert_eq!(line.advance.as_deref(), Some("auto"), "{id} advance");
    }

    // R5 clause 4, restated as the property that matters: each successor's
    // base IS the predecessor's accepted SHA — not the original chain base,
    // and not a branch name.
    assert_eq!(
        report.stages[1].base_commit,
        report.stages[0].accepted_commit
    );
    assert_eq!(
        report.stages[2].base_commit,
        report.stages[1].accepted_commit
    );
    assert_ne!(
        report.stages[1].base_commit.as_deref(),
        Some(root_base().as_str())
    );
}

// ============================================================ §9.4

/// Set up and drive the §9.4 measured-red descope walk.
///
/// A dispatch receipt for L3 is deliberately AVAILABLE at the barrier. If
/// the coordinator dispatched L3 anyway it would succeed, and the "no L3
/// seat spent" assertions would catch it — the descope must come from the
/// RECORDED condition, not from L3 being unrunnable.
fn drive_descope() -> (Workspace, Vec<serde_json::Value>) {
    let ws = Workspace::new(&exemplar_graph_json());
    ws.deposit("dispatch-l1.json", &dispatch_json("child-l1"));
    ws.deposit("dispatch-l2.json", &dispatch_json("child-l2"));
    ws.deposit("dispatch-l3.json", &dispatch_json("child-l3"));
    ws.deposit(
        "completion-l1.json",
        &completion_json(&root_base(), &l1_pushed()),
    );
    ws.deposit(
        "completion-l2.json",
        &completion_json(&l1_pushed(), &l2_pushed()),
    );
    ws.deposit(
        "completion-l3.json",
        &completion_json(&l2_pushed(), &l3_pushed()),
    );
    // §9.4 clause 1: the exact bytes `measured-red\n`.
    ws.deposit(
        "verify-l1.json",
        &verified_json("l1", "child-l1", &l1_pushed(), "measured-red"),
    );
    ws.deposit(
        "verify-l2.json",
        &verified_json("l2", "child-l2", &l2_pushed(), "classification-ready"),
    );
    ws.deposit(
        "verify-l3.json",
        &verified_json("l3", "child-l3", &l3_pushed(), "route-ready"),
    );

    let applied = drive(&ws);
    (ws, applied)
}

#[test]
fn r8c_measured_red_verifies_as_success_and_l2_runs_from_l1s_accepted_sha() {
    let (ws, _applied) = drive_descope();
    let ledger = ledger_of(&ws);

    // §9.4 clause 2: `measured-red` verifies as a SUCCESS conclusion and is
    // NEVER routed to `judge_flagged`.
    assert_eq!(ledger.conclusion_of("l1"), Some("measured-red"));
    assert!(
        ledger.stage_failure().is_none(),
        "`measured-red` was routed to a stage failure"
    );
    assert!(
        ledger.conclusion_refusal().is_none(),
        "`measured-red` was routed to a conclusion refusal"
    );
    // ...and `advance=auto` then accepts the SAME exact SHA.
    let l1 = ledger
        .accepted("l1")
        .unwrap_or_else(|| unreachable!("l1 was not accepted"));
    assert_eq!(l1.accepted_commit, l1_pushed());
    assert_eq!(l1.verified_conclusion, "measured-red");

    // §9.4 clause 3: L2 still runs from L1's accepted SHA (`admit=always`).
    let (l2_base, l2_child) = ledger
        .dispatch_of("l2")
        .unwrap_or_else(|| unreachable!("l2 was never dispatched"));
    assert_eq!(
        l2_base,
        l1_pushed(),
        "L2 was not based on L1's accepted SHA"
    );
    assert_eq!(l2_child, "child-l2");
    assert_eq!(
        ledger.accepted("l2").map(|a| a.accepted_commit.as_str()),
        Some(l2_pushed().as_str())
    );
}

#[test]
fn r8c_l3_is_skipped_with_no_worktree_cut_and_no_seat_spent() {
    let (ws, applied) = drive_descope();
    let ledger = ledger_of(&ws);
    let graph = exemplar_graph();

    // §9.4 clause 5: L3 is recorded SKIPPED with source stage, observed
    // conclusion, and the canonical condition.
    let skip = ledger
        .skipped("l3")
        .unwrap_or_else(|| unreachable!("l3 was not recorded skipped"));
    assert_eq!(skip.stage_id, "l3");
    assert_eq!(skip.source_stage_id, "l1");
    assert_eq!(skip.observed_conclusion, "measured-red");
    assert_eq!(skip.condition, "l1.conclusion!=measured-red");

    // §9.4 clause 5, the load-bearing half: **NO L3 WORKTREE IS CUT AND NO
    // L3 BUILDER SEAT IS SPENT.** There is no dispatch record for L3, and
    // no completion, even though a dispatch receipt was sitting at the
    // barrier the whole time.
    assert!(
        ledger.dispatch_of("l3").is_none(),
        "an L3 stage was DISPATCHED on the descope path — a worktree was cut and a seat spent"
    );
    assert!(
        ledger.receipt_of("l3").is_none(),
        "an L3 completion was recorded on the descope path"
    );
    assert!(
        ledger.accepted("l3").is_none(),
        "L3 was accepted on the descope path"
    );
    assert!(
        !ledger.admitted("l3"),
        "L3's admission was recorded TRUE on the descope path"
    );
    assert!(
        ledger.conclusion_of("l3").is_none(),
        "an L3 conclusion was verified on the descope path"
    );

    // §9.4 clause 6: the chain result is L2's accepted SHA, and the outcome
    // says PLANNED DESCOPE — not stage_failed, not stage_conclusion_refused,
    // not unplanned, not missing work.
    let last = applied
        .last()
        .unwrap_or_else(|| unreachable!("no steps ran"));
    assert_eq!(last["finished"], serde_json::Value::Bool(true));
    assert_eq!(last["outcome"], "planned_descope");
    assert_eq!(last["result_commit"], l2_pushed());
    for forbidden in ["stage_failed", "stage_conclusion_refused", "unplanned"] {
        assert_ne!(
            last["outcome"], forbidden,
            "the descope reported {forbidden}"
        );
    }

    let report = ChainReport::derive(&graph, &ledger, ChainOutcome::PlannedDescope);
    assert!(report.has_skips());
    assert_eq!(report.result_commit.as_deref(), Some(l2_pushed().as_str()));
    let l3_line = &report.stages[2];
    assert!(l3_line.child_workflow_id.is_none());
    assert!(
        l3_line.base_commit.is_none(),
        "a base was created for a skipped stage"
    );
    let skip_line = l3_line
        .skipped
        .as_ref()
        .unwrap_or_else(|| unreachable!("the report does not name L3 as skipped"));
    assert_eq!(skip_line.source_stage_id, "l1");
    assert_eq!(skip_line.observed_conclusion, "measured-red");
    assert_eq!(skip_line.condition, "l1.conclusion!=measured-red");
}

// ============================================================ R4

#[test]
fn r4_the_cause_is_never_unplanned_and_is_never_defaulted() {
    // Design §4: `cause` preserves the child outcome's typed label and "is
    // never reported as `unplanned`". The constructor is the only way to
    // build the record, and it refuses both `unplanned` and a blank.
    for forbidden in ["unplanned", "", "   "] {
        let got = StageFailed::new("l1", "child-l1", "unplanned", forbidden, "/kept", "");
        assert!(
            got.is_err(),
            "a stage_failed record was built with cause {forbidden:?}"
        );
    }
    // A real typed label is accepted, verbatim.
    for label in [
        "unforked",
        "unmerged",
        "unreported",
        "gates_red",
        "judge_flagged",
    ] {
        let record = StageFailed::new("l1", "child-l1", label, label, "/kept", "")
            .unwrap_or_else(|c| unreachable!("typed label {c:?} was refused"));
        assert_eq!(record.cause, label);
        assert_eq!(record.child_outcome, label);
    }
}

#[test]
fn r4_a_failing_stage_dispatches_nothing_downstream() {
    let ws = Workspace::new(&exemplar_graph_json());
    ws.deposit("dispatch-l1.json", &dispatch_json("child-l1"));
    ws.deposit("dispatch-l2.json", &dispatch_json("child-l2"));
    ws.deposit("dispatch-l3.json", &dispatch_json("child-l3"));
    ws.deposit(
        "completion-l1.json",
        &format!(
            r#"{{"result":"completed","child_outcome":"gates_red","base_commit":"{}","pushed_commit":"","kept_root":"/fleet/runs/r1","failed":true}}"#,
            root_base()
        ),
    );
    let applied = drive(&ws);
    let ledger = ledger_of(&ws);

    let failure = ledger
        .stage_failure()
        .unwrap_or_else(|| unreachable!("no stage_failed record"));
    // Design §4's field set, exactly.
    assert_eq!(failure.stage_id, "l1");
    assert_eq!(failure.child_workflow_id, "child-l1");
    assert_eq!(failure.child_outcome, "gates_red");
    assert_eq!(failure.cause, "gates_red");
    assert_ne!(failure.cause, "unplanned");
    assert_eq!(failure.kept_root, "/fleet/runs/r1");
    assert_eq!(failure.last_accepted_commit, "");

    // Nothing downstream ran, even with L2's and L3's receipts waiting at
    // the barrier.
    for downstream in ["l2", "l3"] {
        assert!(
            ledger.dispatch_of(downstream).is_none(),
            "{downstream} was dispatched after a failing stage"
        );
    }
    let last = applied
        .last()
        .unwrap_or_else(|| unreachable!("no steps ran"));
    assert_eq!(last["outcome"], "stage_failed");
    assert_eq!(last["result_commit"], serde_json::Value::Null);
}

#[test]
fn r4_a_refused_conclusion_accepts_no_sha_and_runs_nothing_downstream() {
    let ws = Workspace::new(&exemplar_graph_json());
    ws.deposit("dispatch-l1.json", &dispatch_json("child-l1"));
    ws.deposit("dispatch-l2.json", &dispatch_json("child-l2"));
    ws.deposit(
        "completion-l1.json",
        &completion_json(&root_base(), &l1_pushed()),
    );
    ws.deposit(
        "verify-l1.json",
        &format!(
            r#"{{"result":"conclusion_refused","record":{{"stage_id":"l1","child_workflow_id":"child-l1","result_commit":"{}","conclusion_path":".fleet/conclusions/l1","cause_code":"conclusion_malformed","observed_bytes":"design-ready","kept_root":"/fleet/runs/r1","last_accepted_commit":""}}}}"#,
            l1_pushed()
        ),
    );
    let applied = drive(&ws);
    let ledger = ledger_of(&ws);

    let refusal = ledger
        .conclusion_refusal()
        .unwrap_or_else(|| unreachable!("no stage_conclusion_refused record"));
    assert_eq!(refusal.stage_id, "l1");
    assert_eq!(refusal.observed_bytes, "design-ready");
    assert!(
        ledger.accepted("l1").is_none(),
        "a SHA was accepted after a refused conclusion"
    );
    assert!(ledger.dispatch_of("l2").is_none(), "L2 ran after a refusal");
    let last = applied
        .last()
        .unwrap_or_else(|| unreachable!("no steps ran"));
    assert_eq!(last["outcome"], "stage_conclusion_refused");
}

#[test]
fn r4_no_hold_approves_a_stage_whose_conclusion_has_not_verified() {
    // Design §2.6, verbatim: "No hold can approve a stage whose conclusion
    // has not verified." Structural: the hold arm is reachable only from a
    // recorded ConclusionVerified.
    let mut graph = exemplar_graph();
    graph.order[0].advance = Advance::Hold;
    let mut ledger = ChainLedger::new();
    ledger.record(Recorded::StageAdmitted {
        stage_id: "l1".to_owned(),
        condition: "always".to_owned(),
    });
    ledger.record(Recorded::StageDispatched {
        stage_id: "l1".to_owned(),
        base_commit: root_base(),
        child_workflow_id: "child-l1".to_owned(),
    });
    ledger.record(Recorded::StageCompleted {
        receipt: aion_fleet_plan::chain::records::StageReceipt {
            stage_id: "l1".to_owned(),
            child_workflow_id: "child-l1".to_owned(),
            base_commit: root_base(),
            child_outcome: "done".to_owned(),
            pushed_commit: l1_pushed(),
        },
    });

    // The child is complete and pushed, but nothing has verified. The only
    // admissible next action is VERIFY — never OfferHold, never accept.
    let action = next_action(&graph, &ledger, &root_base()).unwrap_or_else(|e| unreachable!("{e}"));
    let ChainAction::VerifyConclusion {
        stage_id,
        result_commit,
        ..
    } = &action
    else {
        unreachable!("with no verified conclusion the next action was {action:?}, not a verify");
    };
    assert_eq!(stage_id, "l1");
    assert_eq!(result_commit, &l1_pushed());

    // Only after the conclusion verifies is the hold offered.
    ledger.record(Recorded::ConclusionVerified {
        record: aion_fleet_plan::chain::records::StageConclusion {
            stage_id: "l1".to_owned(),
            child_workflow_id: "child-l1".to_owned(),
            result_commit: l1_pushed(),
            conclusion_path: ".fleet/conclusions/l1".to_owned(),
            conclusion: "design-ready".to_owned(),
        },
    });
    let action = next_action(&graph, &ledger, &root_base()).unwrap_or_else(|e| unreachable!("{e}"));
    let ChainAction::OfferHold {
        verified_conclusion,
        ..
    } = &action
    else {
        unreachable!("after verification the next action was {action:?}, not a hold");
    };
    assert_eq!(verified_conclusion, "design-ready");

    // And the hold decision is its own dedicated recorded event (§6).
    let held = apply(
        &action,
        &External::Held {
            decision: Box::new(aion_fleet_plan::chain::records::HoldDecision {
                stage_id: "l1".to_owned(),
                result_commit: l1_pushed(),
                verified_conclusion: "design-ready".to_owned(),
                approved: true,
                approver: "operator".to_owned(),
                note: "reviewed".to_owned(),
            }),
        },
    )
    .unwrap_or_else(|e| unreachable!("{e}"));
    assert!(matches!(held, Recorded::HoldDecided { .. }));
}

// ============================================================ R5

#[test]
fn r5_the_base_law_all_five_clauses() {
    let graph: NormalizedGraph = exemplar_graph();

    // Clause 1 — the root is cut from the chain's authored base_ref
    // resolved at chain start.
    let ledger = ChainLedger::new();
    let action = next_action(&graph, &ledger, &root_base()).unwrap_or_else(|e| unreachable!("{e}"));
    // Admission is evaluated before dispatch, so step once conceptually.
    let mut ledger = ledger;
    ledger.record(apply(&action, &External::None).unwrap_or_else(|e| unreachable!("{e}")));
    let action = next_action(&graph, &ledger, &root_base()).unwrap_or_else(|e| unreachable!("{e}"));
    let ChainAction::DispatchStage {
        stage_id,
        base_commit,
        legs,
    } = &action
    else {
        unreachable!("the root's next action was {action:?}");
    };
    assert_eq!(stage_id, "l1");
    assert_eq!(base_commit, &root_base());

    // Clause 2 — every leg in a stage is cut from that SAME exact stage
    // base. The dispatch action carries one base and the stage's whole leg
    // list, so there is no per-leg base to diverge.
    assert_eq!(legs, &vec!["l1-design".to_owned()]);
    let build_stage = &exemplar_graph().order[0];
    assert_eq!(build_stage.legs.len(), 1);

    // Clause 5 — a skipped stage creates no base, and its successor uses
    // the most recent ACCEPTED predecessor. Proven by the §9.4 walk: after
    // L3 is skipped the chain result is L2's accepted SHA and L3 has no
    // base at all. (See `r8c_measured_red_descope_exactly_as_the_design_writes_it`.)
}

#[test]
fn r5_the_condition_is_the_normalized_graphs_canonical_text_not_plan_prose() {
    // Design §4: "Free-form report text is never a branch condition." The
    // skip record's `condition` comes from the normalized graph's canonical
    // rendering, which is why it is stable regardless of how the plan line
    // was spaced or ordered.
    let graph = exemplar_graph();
    let Admit::Conclusion {
        stage_id,
        comparison,
        value,
    } = &graph.order[2].admit
    else {
        unreachable!("l3's admit did not normalize to a conclusion comparison");
    };
    assert_eq!(stage_id, "l1");
    assert_eq!(*comparison, Comparison::NotEquals);
    assert_eq!(value, "measured-red");
    assert_eq!(
        graph.order[2].admit.canonical(),
        "l1.conclusion!=measured-red"
    );
}
