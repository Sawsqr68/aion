//! R6 — replay determinism at all SEVEN boundaries, **by kill-and-resume,
//! not by argument**.
//!
//! Design §6's proof obligation, verbatim: *"crash/replay at every boundary
//! (parse complete, child dispatched, child complete, conclusion verified,
//! acceptance recorded, skip recorded, next child dispatched) must
//! re-derive the same next action and exact SHA."*
//!
//! Each boundary is walked to as a real ledger state; a REAL process this
//! test started is then `SIGKILL`ed by the exact pid it wrote into its own
//! receipt; and the resumed process is asserted to re-derive the same next
//! action and the same exact SHA.
//!
//! # ⛔ KILL SCOPE
//!
//! See `support/harness.rs`. Only processes this test spawned; pid
//! re-derived AT FIRE TIME from that process's own receipt; exact numeric
//! pid; **no `pkill`, no `killall`, no pattern kill in any spelling** — and
//! [`no_pattern_kill_anywhere`] asserts that over the crate's own bytes.
//!
//! # Fixture-size law
//!
//! Every input is boundary-level and synthetic: a three-stage graph, a
//! ledger of at most eleven JSON records, and inbox files of a few hundred
//! bytes. No multi-GB fixtures.

#[path = "support/support_r6.rs"]
mod support;

use support::chain::{Workspace, exemplar_graph_json, sha};

/// The chain's root base, resolved once at chain start (design §3 clause 1).
fn root_base() -> String {
    sha(1)
}

/// L1's pushed result commit.
fn l1_pushed() -> String {
    sha(2)
}

/// L2's pushed result commit.
fn l2_pushed() -> String {
    sha(3)
}

/// The seven recorded boundaries design §6 names, in order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Boundary {
    ParseComplete,
    ChildDispatched,
    ChildComplete,
    ConclusionVerified,
    AcceptanceRecorded,
    SkipRecorded,
    NextChildDispatched,
}

const ALL_BOUNDARIES: [Boundary; 7] = [
    Boundary::ParseComplete,
    Boundary::ChildDispatched,
    Boundary::ChildComplete,
    Boundary::ConclusionVerified,
    Boundary::AcceptanceRecorded,
    Boundary::SkipRecorded,
    Boundary::NextChildDispatched,
];

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

/// Advance the workspace to exactly `target`, on the §9.4 measured-red
/// descope path — the path that exercises the skip boundary.
///
/// Returns the number of steps taken.
fn walk_to(ws: &Workspace, target: Boundary) -> usize {
    let base = root_base();
    let mut steps = 0usize;
    // Boundary 1 — parse complete. The graph is written at workspace
    // construction and the ledger is empty; nothing to step.
    if target == Boundary::ParseComplete {
        return steps;
    }

    // admit l1 (always), then dispatch l1.
    ws.step(&base);
    steps += 1;
    ws.deposit("dispatch-l1.json", &dispatch_json("child-l1"));
    ws.step(&base);
    steps += 1;
    if target == Boundary::ChildDispatched {
        return steps;
    }

    ws.deposit("completion-l1.json", &completion_json(&base, &l1_pushed()));
    ws.step(&base);
    steps += 1;
    if target == Boundary::ChildComplete {
        return steps;
    }

    // §9.4: `measured-red` verifies as a SUCCESS conclusion.
    ws.deposit(
        "verify-l1.json",
        &verified_json("l1", "child-l1", &l1_pushed(), "measured-red"),
    );
    ws.step(&base);
    steps += 1;
    if target == Boundary::ConclusionVerified {
        return steps;
    }

    ws.step(&base); // record acceptance of l1
    steps += 1;
    if target == Boundary::AcceptanceRecorded {
        return steps;
    }

    // l2 admit=always → admitted, dispatched, completed, verified, accepted.
    ws.step(&base);
    steps += 1;
    ws.deposit("dispatch-l2.json", &dispatch_json("child-l2"));
    ws.step(&base);
    steps += 1;
    if target == Boundary::NextChildDispatched {
        return steps;
    }
    ws.deposit(
        "completion-l2.json",
        &completion_json(&l1_pushed(), &l2_pushed()),
    );
    ws.step(&base);
    steps += 1;
    ws.deposit(
        "verify-l2.json",
        &verified_json("l2", "child-l2", &l2_pushed(), "classification-ready"),
    );
    ws.step(&base);
    steps += 1;
    ws.step(&base); // accept l2
    steps += 1;

    // l3 admit=l1.conclusion!=measured-red → FALSE from recorded history.
    ws.step(&base); // record the skip
    steps += 1;
    if target == Boundary::SkipRecorded {
        return steps;
    }
    steps
}

#[test]
fn crash_and_resume_at_every_one_of_the_seven_boundaries() {
    for boundary in ALL_BOUNDARIES {
        let ws = Workspace::new(&exemplar_graph_json());
        walk_to(&ws, boundary);

        let base = root_base();
        let before_bytes = ws.ledger_bytes();
        let control_next = ws.peek_next(&base);
        assert!(
            !control_next.is_null(),
            "{boundary:?}: no control next action"
        );

        // ---- THE CRASH. A real process this test started, killed by the
        // exact pid it wrote into its own receipt, read AT FIRE TIME.
        ws.clear_inbox();
        let mut own = ws.spawn_step(&base, 30);
        let killed_pid = own
            .kill_by_own_receipt()
            .unwrap_or_else(|why| unreachable!("{boundary:?}: {why}"));
        assert!(killed_pid > 0, "{boundary:?}: killed a non-pid");

        // ---- The ledger is never torn. It is either exactly what it was
        // (the kill landed before the atomic rename) or exactly one fact
        // further on (the process won the race). Both are valid crash
        // observations, and BOTH must decode.
        let after_bytes = ws.ledger_bytes();
        if !after_bytes.is_empty() {
            let decoded: Result<aion_fleet_plan::chain::ChainLedger, _> =
                serde_json::from_slice(&after_bytes);
            assert!(
                decoded.is_ok(),
                "{boundary:?}: the ledger was TORN by the kill — atomic rename failed"
            );
        }
        let unchanged = after_bytes == before_bytes;

        // ---- THE RESUME. Re-deposit what the step needs and run again.
        redeposit_for(&ws, boundary);
        let resumed_next = ws.peek_next(&base);

        if unchanged {
            assert_eq!(
                resumed_next, control_next,
                "{boundary:?}: the resumed run derived a DIFFERENT next action after the crash"
            );
            assert_eq!(
                exact_sha(&resumed_next),
                exact_sha(&control_next),
                "{boundary:?}: the resumed run derived a different exact SHA"
            );
        }

        // ---- And two processes resuming from BYTE-IDENTICAL recorded
        // state apply the same fact. The twin is seeded from the exact
        // post-crash bytes, so this compares like for like whichever side
        // of the atomic rename the kill landed on.
        let twin = Workspace::from_ledger_bytes(&exemplar_graph_json(), &after_bytes);
        redeposit_for(&twin, boundary);
        let first = ws.step(&base);
        let second = twin.step(&base);
        assert_eq!(
            first["applied"], second["applied"],
            "{boundary:?}: two runs from byte-identical recorded state applied DIFFERENT facts"
        );
        assert_eq!(
            exact_sha(&first["next"]),
            exact_sha(&second["next"]),
            "{boundary:?}: two runs from byte-identical recorded state derived different SHAs"
        );
    }
}

/// Re-deposit the external result the boundary's next step needs, after the
/// inbox was cleared for the crash.
fn redeposit_for(ws: &Workspace, boundary: Boundary) {
    let base = root_base();
    let (name, body) = match boundary {
        Boundary::ChildDispatched => ("completion-l1.json", completion_json(&base, &l1_pushed())),
        Boundary::ChildComplete => (
            "verify-l1.json",
            verified_json("l1", "child-l1", &l1_pushed(), "measured-red"),
        ),
        Boundary::NextChildDispatched => (
            "completion-l2.json",
            completion_json(&l1_pushed(), &l2_pushed()),
        ),
        Boundary::ParseComplete
        | Boundary::ConclusionVerified
        | Boundary::AcceptanceRecorded
        | Boundary::SkipRecorded => ("", String::new()),
    };
    if !name.is_empty() {
        ws.deposit(name, &body);
    }
    // The dispatch barrier for whichever stage comes next.
    ws.deposit("dispatch-l1.json", &dispatch_json("child-l1"));
    ws.deposit("dispatch-l2.json", &dispatch_json("child-l2"));
    ws.deposit("dispatch-l3.json", &dispatch_json("child-l3"));
}

/// Every 40-hex SHA an action mentions, so "the same exact SHA" is checked
/// rather than assumed from a whole-value compare.
fn exact_sha(action: &serde_json::Value) -> Vec<String> {
    let mut found = Vec::new();
    collect_shas(action, &mut found);
    found.sort();
    found
}

fn collect_shas(value: &serde_json::Value, out: &mut Vec<String>) {
    match value {
        serde_json::Value::String(s) => {
            if s.len() == 40 && s.bytes().all(|b| b.is_ascii_hexdigit()) {
                out.push(s.clone());
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_shas(item, out);
            }
        }
        serde_json::Value::Object(map) => {
            for item in map.values() {
                collect_shas(item, out);
            }
        }
        _ => {}
    }
}

#[test]
fn the_decision_function_is_pure_over_the_recorded_ledger() {
    // Ten evaluations of the same recorded state produce one answer. If
    // anything in the decision path consulted a clock, the environment, a
    // ref, or entropy, this is where it would show.
    let ws = Workspace::new(&exemplar_graph_json());
    walk_to(&ws, Boundary::AcceptanceRecorded);
    let base = root_base();
    let first = ws.peek_next(&base);
    for round in 1..10 {
        assert_eq!(
            ws.peek_next(&base),
            first,
            "evaluation {round} disagreed with the first — the decision is not pure"
        );
    }
}

#[test]
fn a_successor_uses_the_recorded_accepted_sha_never_a_ref() {
    // Design §6 clause 7 and §3 clause 4. L2's dispatch base must be
    // exactly L1's ACCEPTED result SHA.
    let ws = Workspace::new(&exemplar_graph_json());
    walk_to(&ws, Boundary::AcceptanceRecorded);
    let base = root_base();
    ws.step(&base); // admit l2
    let next = ws.peek_next(&base);
    assert_eq!(next["action"], "dispatch_stage");
    assert_eq!(next["stage_id"], "l2");
    assert_eq!(
        next["base_commit"],
        l1_pushed(),
        "L2 was not based on L1's recorded ACCEPTED result SHA"
    );
    assert_ne!(
        next["base_commit"], base,
        "L2 fell back to the original chain base instead of L1's accepted SHA"
    );
}

#[test]
fn a_skipped_stage_creates_no_base() {
    // Design §3 clause 5. After L3 is skipped, the chain result is L2's
    // accepted SHA — the skip appended no acceptance.
    let ws = Workspace::new(&exemplar_graph_json());
    walk_to(&ws, Boundary::SkipRecorded);
    let next = ws.peek_next(&root_base());
    assert_eq!(next["action"], "finish");
    assert_eq!(next["outcome"], "planned_descope");
    assert_eq!(
        next["result_commit"],
        l2_pushed(),
        "the chain result is not L2's accepted SHA"
    );
}

#[test]
fn no_pattern_kill_anywhere() {
    // ⛔ KILL SCOPE, asserted over this crate's own bytes rather than
    // promised in prose. A broad pattern kill on this box takes out another
    // lane while every board stays green.
    // The needles are ASSEMBLED rather than written literally: a sweep that
    // spelled them out would match its own source and could only be made
    // green by exempting itself, which is the bypass this wall exists to
    // prevent.
    let k = "kill";
    let forbidden: Vec<String> = vec![
        format!("p{k}"),
        format!("{k}all"),
        format!("{k}pg"),
        format!("{k} -9 -"),
        format!("{k} $("),
        format!("{k} `"),
        format!("xargs {k}"),
        format!("p{}", "grep"),
        format!("pid{}", "of"),
    ];
    let mut checked = 0usize;
    for entry in walk_rs(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) {
        let Ok(text) = std::fs::read_to_string(&entry) else {
            continue;
        };
        checked += 1;
        // Comment lines are prose ABOUT the wall — the wall itself is about
        // what executes, so the sweep reads code lines only. Skipping
        // comments is what lets the prohibition be written down at all.
        for (number, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") {
                continue;
            }
            for needle in &forbidden {
                assert!(
                    !line.contains(needle.as_str()),
                    "{}:{} executes the forbidden pattern-kill spelling {needle:?}",
                    entry.display(),
                    number + 1
                );
            }
        }
    }
    assert!(
        checked > 5,
        "the kill-scope sweep read only {checked} files"
    );
}

fn walk_rs(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.file_name().is_some_and(|n| n == "target") {
                    continue;
                }
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }
    out
}
