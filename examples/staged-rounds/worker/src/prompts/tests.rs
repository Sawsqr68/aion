//! Prompt-assembly tests: each role's prompt carries the fenced context, the
//! dev prompt carries the verbatim gate argv AND an accurate account of what
//! the pipeline actually does after the turn, the reviewer prompt carries its
//! LENS CHARTER and the RECORDED battery outcome, and the integration-fix
//! prompt carries the failing battery.

use super::{dev_item, fix_integration, planner, remediate, review_item};

const DEV_CONTEXT: &str = r#"{
    "work": {
        "item": {
            "id": "it-1",
            "title": "t",
            "goal": "g",
            "scope_in": ["src/"],
            "scope_out": ["docs/"],
            "phase": 1,
            "depends_on": [],
            "feedback": ""
        },
        "workspace_path": "/repo/.staged-rounds/wf/items/it-1",
        "branch": "staged/wf/it-1",
        "base_commit": "abc123"
    },
    "gates": [
        {"name": "fmt", "argv": ["cargo", "fmt", "--all"]},
        {"name": "test", "argv": ["cargo", "test", "--workspace"]}
    ]
}"#;

/// A panel seat over an item whose battery went RED.
const REVIEW_CONTEXT: &str = r#"{
    "task": {
        "lens": {
            "name": "regressions",
            "charter": "MARKER_CHARTER: hunt collateral damage in the callers."
        },
        "work": {
            "item": {
                "id": "it-1", "title": "t", "goal": "g",
                "scope_in": ["src/"], "scope_out": ["docs/"],
                "phase": 1, "depends_on": [], "feedback": ""
            },
            "workspace_path": "/repo/.staged-rounds/wf/items/it-1",
            "branch": "staged/wf/it-1",
            "base_commit": "abc123",
            "report": {"item_id": "it-1", "summary": "s", "commits": [], "claims": []},
            "gate": {
                "pass": false,
                "runs": [
                    {"name": "fmt", "argv": ["cargo", "fmt", "--all"],
                     "exit_code": 0, "passed": true, "output_tail": ""},
                    {"name": "test", "argv": ["cargo", "test", "--workspace"],
                     "exit_code": 101, "passed": false,
                     "output_tail": "MARKER_TEST_FAILURE"}
                ],
                "diff": "",
                "diagnostics": "gate `test` exited 101"
            }
        }
    }
}"#;

/// A failing integration verification.
const FIX_CONTEXT: &str = r#"{
    "verify": {
        "pass": false,
        "runs": [
            {"name": "build", "argv": ["cargo", "build", "--workspace"],
             "exit_code": 101, "passed": false, "output_tail": "MARKER_UNION_BREAK"}
        ],
        "diff": "",
        "diagnostics": "gate `build` exited 101"
    },
    "plan": {"summary": "two disjoint items", "items": []},
    "integration_branch": "staged/wf/integration",
    "workspace_path": "/repo/.staged-rounds/wf/integration"
}"#;

#[test]
fn every_role_prompt_carries_the_fenced_context() {
    for assemble in [planner, dev_item, review_item, remediate, fix_integration] {
        let prompt = assemble("{\"marker\":\"CONTEXT_MARKER\"}");
        assert!(prompt.contains("```json"), "missing fence:\n{prompt}");
        assert!(
            prompt.contains("CONTEXT_MARKER"),
            "missing context:\n{prompt}"
        );
    }
}

#[test]
fn the_dev_prompt_lists_the_exact_gate_argv() {
    let prompt = dev_item(DEV_CONTEXT);
    assert!(
        prompt.contains("`cargo fmt --all`"),
        "missing fmt argv:\n{prompt}"
    );
    assert!(
        prompt.contains("`cargo test --workspace`"),
        "missing test argv:\n{prompt}"
    );
    assert!(prompt.contains("GATE DISCIPLINE"), "{prompt}");
}

/// The doctrine the dev agent is told must be TRUE. Before `gate_item` existed
/// the prompt claimed the pipeline's review "confirms" the gates are clean,
/// while nothing ran them at all. It now states what actually happens: the
/// battery is re-run authoritatively, a red battery costs a round, and a panel
/// of named lenses reviews the diff.
#[test]
fn the_dev_prompt_states_what_the_pipeline_really_does_after_the_turn() {
    let prompt = dev_item(DEV_CONTEXT);
    for claim in [
        "the pipeline runs this same battery itself",
        "RECORDS every exit code as data",
        "costs one of the run's bounded rounds",
        "panel of named review lenses",
    ] {
        assert!(
            prompt.contains(claim),
            "the dev prompt must state {claim:?}:\n{prompt}"
        );
    }
    // And it must NOT reprise the old false claim that the pipeline's review is
    // mere confirmation of gates nothing ran.
    assert!(
        !prompt.contains("Its review is CONFIRMATION"),
        "the false doctrine survived:\n{prompt}"
    );
}

#[test]
fn an_empty_gate_battery_is_named_explicitly() {
    let prompt = dev_item("{\"work\":{},\"gates\":[]}");
    assert!(
        prompt.contains("configured gate battery is empty"),
        "{prompt}"
    );
}

#[test]
fn the_reviewer_prompt_names_the_exact_diff_command() {
    let prompt = review_item(REVIEW_CONTEXT);
    assert!(
        prompt.contains("git diff abc123"),
        "missing base-commit diff command:\n{prompt}"
    );
    assert!(prompt.contains("READ-ONLY"), "{prompt}");
}

/// The lens's charter travels in the PROMPT, not the profile: one reviewer
/// connection serves every lens, so the charter is the only thing that makes
/// this seat different from the seat running beside it.
#[test]
fn the_reviewer_prompt_carries_its_lens_name_and_charter() {
    let prompt = review_item(REVIEW_CONTEXT);
    assert!(
        prompt.contains("YOUR LENS: regressions"),
        "the seat must be told which lens it is:\n{prompt}"
    );
    assert!(
        prompt.contains("MARKER_CHARTER: hunt collateral damage in the callers."),
        "the charter must ride in the prompt verbatim:\n{prompt}"
    );
    assert!(
        prompt.contains("cannot see the other lenses' verdicts"),
        "{prompt}"
    );
}

/// The reviewer receives the GATE OUTCOME: it is real data now, and it grounds
/// findings.
#[test]
fn the_reviewer_prompt_carries_the_recorded_gate_outcome() {
    let prompt = review_item(REVIEW_CONTEXT);
    assert!(
        prompt.contains("THE MECHANICAL BATTERY ALREADY RAN"),
        "{prompt}"
    );
    assert!(
        prompt.contains("- test — exit 101 (RED): `cargo test --workspace`"),
        "each command's recorded status and resolved argv must be shown:\n{prompt}"
    );
    assert!(
        prompt.contains("- fmt — exit 0 (pass): `cargo fmt --all`"),
        "{prompt}"
    );
    assert!(
        prompt.contains("The battery is RED"),
        "the verdict must be stated plainly:\n{prompt}"
    );
    assert!(prompt.contains("MARKER_TEST_FAILURE"), "{prompt}");
}

#[test]
fn a_green_battery_is_framed_as_the_floor_not_as_evidence() {
    let green = REVIEW_CONTEXT
        .replace("\"pass\": false", "\"pass\": true")
        .replace("\"passed\": false", "\"passed\": true");
    let prompt = review_item(&green);
    assert!(prompt.contains("The battery PASSED"), "{prompt}");
    assert!(
        prompt.contains("That is the floor"),
        "a green battery must never read as evidence in the change's favour:\n{prompt}"
    );
}

#[test]
fn an_empty_battery_is_named_to_the_reviewer_too() {
    let prompt = review_item(
        "{\"task\":{\"lens\":{\"name\":\"correctness\",\"charter\":\"c\"},\
         \"work\":{\"gate\":{\"pass\":true,\"runs\":[]}}}}",
    );
    assert!(
        prompt.contains("NOTHING was verified mechanically"),
        "{prompt}"
    );
}

#[test]
fn a_reviewer_context_missing_the_base_renders_the_placeholder() {
    let prompt = review_item("{}");
    assert!(prompt.contains("git diff <base_commit>"), "{prompt}");
    assert!(
        prompt.contains("no lens name and no"),
        "a seat with no charter must be told to report it, not guess:\n{prompt}"
    );
}

#[test]
fn the_remediate_prompt_frames_the_resumed_planner() {
    let prompt = remediate("{\"merge\":{},\"plan\":{},\"workspace_path\":\"/x\"}");
    assert!(prompt.contains("planner, resumed"), "{prompt}");
    assert!(prompt.contains("IN"), "{prompt}");
    assert!(prompt.contains("Do NOT run `git commit`"), "{prompt}");
}

/// The integration fixer must receive the FAILING BATTERY OUTPUT — the measured
/// failure, not a summary of it.
#[test]
fn the_fix_integration_prompt_carries_the_failing_battery() {
    let prompt = fix_integration(FIX_CONTEXT);
    assert!(
        prompt.contains("THE FAILING BATTERY, AS RECORDED"),
        "{prompt}"
    );
    assert!(
        prompt.contains("gate `build` exited 101 running `cargo build --workspace`"),
        "the resolved argv and exit code must be shown:\n{prompt}"
    );
    assert!(prompt.contains("MARKER_UNION_BREAK"), "{prompt}");
    assert!(
        prompt.contains("union of your items is red"),
        "the fixer must be told the merge was clean and the union is not:\n{prompt}"
    );
    assert!(
        prompt.contains("You run NO git"),
        "the machinery commits the repair:\n{prompt}"
    );
}

/// A verification that failed on the TREE assertion (every command exited 0,
/// but a gate dirtied the tree) still tells the fixer what happened.
#[test]
fn a_tree_assertion_failure_is_explained_to_the_fixer() {
    let prompt = fix_integration(
        "{\"verify\":{\"pass\":false,\"runs\":[{\"name\":\"fmt\",\"argv\":[\"fmt\"],\
         \"exit_code\":0,\"passed\":true,\"output_tail\":\"\"}],\
         \"diagnostics\":\"MARKER_DIRTIED the integration worktree\"},\"plan\":{}}",
    );
    assert!(
        prompt.contains("failed on the tree assertion itself"),
        "{prompt}"
    );
    assert!(prompt.contains("MARKER_DIRTIED"), "{prompt}");
}

#[test]
fn the_planner_prompt_states_the_parallel_safety_rule() {
    let prompt = planner("{\"material\":{},\"repo_root\":\"/r\",\"workspace_path\":\"/r\"}");
    assert!(prompt.contains("IN PARALLEL"), "{prompt}");
    assert!(prompt.contains("git-ref-safe slug"), "{prompt}");
}
