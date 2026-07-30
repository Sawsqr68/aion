//! Ported from `examples/assistant/test/prompts_test.gleam` — the same
//! assertions, now over the Rust composer that replaced it.
//!
//! Every assertion here is about CONTENT the assistant depends on: the
//! objective it was started for, the workspace it was given, the skill
//! documents it must read before answering, the authoring method, and the
//! honesty rules. The two repository branches (attached / absent) are each
//! covered in full: a composer only ever seen to fire on one branch is
//! untested on the other, and the branch the operator hits most is the one
//! nobody looks at.

use super::opening_prompt;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// The session workspace path used throughout, matching the Gleam fixture.
const WORKSPACE: &str = "/clones/run-1/repo";

/// The attached-repository path used throughout, matching the Gleam harness's
/// `base_input()`.
const REPO: &str = "/Users/tom/Developer/ablative/aion";

/// The objective used throughout, matching the Gleam harness's `base_input()`.
const OBJECTIVE: &str = "explain how aion signals work";

/// Assert `needle` appears in `haystack`, naming both on failure.
fn contains(haystack: &str, needle: &str) {
    assert!(
        haystack.contains(needle),
        "prompt is missing {needle:?}; prompt was:\n{haystack}"
    );
}

/// Assert `needle` does NOT appear in `haystack`.
fn excludes(haystack: &str, needle: &str) {
    assert!(
        !haystack.contains(needle),
        "prompt must not mention {needle:?}; prompt was:\n{haystack}"
    );
}

/// The prompt for a session WITH a repository attached.
fn with_repo() -> String {
    opening_prompt(OBJECTIVE, REPO, WORKSPACE)
}

/// The prompt for a session with NO repository attached.
fn without_repo() -> String {
    opening_prompt(OBJECTIVE, "", WORKSPACE)
}

#[test]
fn first_round_carries_the_objective_and_workspace() {
    let prompt = with_repo();
    contains(&prompt, OBJECTIVE);
    contains(&prompt, WORKSPACE);
    contains(&prompt, REPO);
    contains(&prompt, "## The operator's objective");
}

#[test]
fn first_round_teaches_ground_truth_over_memory() {
    for prompt in [with_repo(), without_repo()] {
        contains(&prompt, "from memory");
        contains(&prompt, ".assistant/resources/");
        contains(&prompt, "ENVIRONMENT.md");
        contains(&prompt, "AWL-AUTHORING.md");
        contains(&prompt, "COMMANDS.md");
        contains(&prompt, "AWL-REFERENCE.md");
        contains(&prompt, "WORKERS.md");
        contains(&prompt, "examples/awl-hello/awl_hello.awl");
        contains(
            &prompt,
            "crates/aion-awl/tests/fixtures/rev2/**/valid/*.awl",
        );
    }
}

#[test]
fn first_round_teaches_the_awl_authoring_canon() {
    for prompt in [with_repo(), without_repo()] {
        contains(&prompt, "AWL source (`.awl`) is the authoring truth");
        contains(&prompt, "aion awl check FILE.awl");
        contains(&prompt, "children/subflows");
        contains(&prompt, "shell workers are host-native");
        excludes(&prompt, "aion_flow");
        excludes(&prompt, "gleam.toml");
    }
}

#[test]
fn first_round_teaches_direct_awl_deploy_and_run() {
    for prompt in [with_repo(), without_repo()] {
        contains(&prompt, "aion deploy FILE.awl");
        contains(&prompt, "aion run FILE.awl --input 'JSON'");
        contains(&prompt, "observe the durable run");
        excludes(&prompt, "aion package");
    }
}

#[test]
fn first_round_carries_the_honesty_rules() {
    for prompt in [with_repo(), without_repo()] {
        contains(&prompt, "I have not verified this");
        contains(&prompt, "Never invent AWL syntax or worker capability");
    }
}

#[test]
fn first_round_states_the_file_tool_confinement() {
    // The norn file tools are confined to the workspace; shell commands start
    // there but can read elsewhere — the prompt must say so, on both branches.
    for prompt in [with_repo(), without_repo()] {
        contains(&prompt, "confined");
        contains(&prompt, "shell");
    }
}

#[test]
fn with_a_repo_the_workspace_is_named_as_a_clone_of_it() {
    let prompt = with_repo();
    contains(&prompt, "It is a clone of the aion repository at");
    contains(&prompt, REPO);
    contains(&prompt, "Your workspace IS a clone of the aion repository");
    contains(&prompt, "author `.awl` workflows inside the clone");
    // The self-clone instruction belongs to the OTHER branch only: telling an
    // assistant whose workspace already IS the repository to clone it would
    // send it off to fetch a second copy.
    excludes(&prompt, "No repository was attached");
    excludes(&prompt, "git clone --depth 1");
    excludes(&prompt, "scratch git workspace");
}

#[test]
fn without_a_repo_the_session_self_serves_a_clone() {
    let prompt = without_repo();
    contains(&prompt, "scratch git workspace");
    contains(&prompt, "No repository was attached");
    contains(
        &prompt,
        "git clone --depth 1 https://github.com/ablative-io/aion.git",
    );
    contains(&prompt, "private");
    contains(&prompt, ".assistant/resources/");
    // The attached-repository claims must not leak onto this branch: an
    // assistant told its workspace IS the repository would answer repo
    // questions from an empty scratch directory and call it verified.
    excludes(&prompt, "Your workspace IS a clone");
    excludes(&prompt, "It is a clone of the aion repository at");
}

#[test]
fn a_whitespace_only_repo_path_reads_as_no_repository() {
    // Ported from the Gleam original's `string.trim` on `repo_path`: the
    // branch is decided on the trimmed value, so a path of blanks never
    // produces a prompt claiming a clone that is not there.
    let prompt = opening_prompt(OBJECTIVE, "   \t ", WORKSPACE);
    contains(&prompt, "scratch git workspace");
    excludes(&prompt, "It is a clone of the aion repository at");
}

#[test]
fn the_embedded_repo_path_is_trimmed() {
    // Also ported from the Gleam `string.trim`: the path shown to the
    // assistant is the trimmed one, so it can be used as a path verbatim.
    let prompt = opening_prompt(OBJECTIVE, "  /repos/aion  ", WORKSPACE);
    contains(&prompt, "at `/repos/aion`");
}

#[test]
fn the_objective_is_last_so_it_is_never_buried_in_the_contract() -> TestResult {
    let prompt = with_repo();
    let heading = prompt
        .find("## The operator's objective")
        .ok_or("the objective heading is missing")?;
    let objective = prompt
        .rfind(OBJECTIVE)
        .ok_or("the objective text is missing")?;
    assert!(
        objective > heading,
        "the objective must follow its heading, not precede it"
    );
    assert!(
        prompt.trim_end().ends_with(OBJECTIVE),
        "the prompt must END with the operator's objective; got tail: {:?}",
        prompt.get(prompt.len().saturating_sub(120)..)
    );
    Ok(())
}

#[test]
fn the_two_branches_differ_only_where_the_repository_matters() {
    // Guards against a future edit that improves one branch and forgets the
    // other: everything except the repository-conditional sentences is shared.
    let attached = with_repo();
    let absent = without_repo();
    assert_ne!(
        attached, absent,
        "the branches must differ — otherwise the repository is not being reported"
    );
    for shared in [
        "## How to author an AWL workflow",
        "## Honesty rules",
        "## Ground truth",
        "## Your workspace",
        "## The operator's objective",
    ] {
        contains(&attached, shared);
        contains(&absent, shared);
    }
}
