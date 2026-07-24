//// Unit tests for prompt composition: the round-one prompt carries the full
//// working contract (ground truth, authoring method, honesty rules, the
//// workspace and its confinement note); continuation prompts are the
//// operator's message verbatim (the pinned norn session already holds the
//// contract from round one).

import assistant/prompts
import assistant_io as io
import gleam/string
import gleeunit/should
import support/harness

fn contains(haystack: String, needle: String) {
  haystack |> string.contains(needle) |> should.be_true
}

fn workspace() -> io.Workspace {
  io.Workspace(path: "/clones/run-1/repo")
}

pub fn first_round_carries_the_objective_and_workspace_test() {
  let input = harness.base_input()
  let prompt = prompts.first_round(input, workspace())
  contains(prompt, input.objective)
  contains(prompt, "/clones/run-1/repo")
  contains(prompt, input.repo_path)
}

pub fn first_round_teaches_ground_truth_over_memory_test() {
  let prompt = prompts.first_round(harness.base_input(), workspace())
  contains(prompt, "from memory")
  contains(prompt, ".assistant/resources/")
  contains(prompt, "ENVIRONMENT.md")
  contains(prompt, "AWL-AUTHORING.md")
  contains(prompt, "AWL-REFERENCE.md")
  contains(prompt, "WORKERS.md")
  contains(prompt, "examples/awl-hello/awl_hello.awl")
  contains(prompt, "crates/aion-awl/tests/fixtures/rev2/**/valid/*.awl")
}

pub fn first_round_teaches_the_awl_authoring_canon_test() {
  let prompt = prompts.first_round(harness.base_input(), workspace())
  contains(prompt, "AWL source (`.awl`) is the authoring truth")
  contains(prompt, "aion awl check FILE.awl")
  contains(prompt, "children/subflows")
  contains(prompt, "shell workers are host-native")
  prompt |> string.contains("aion_flow") |> should.be_false
  prompt |> string.contains("gleam.toml") |> should.be_false
}

pub fn first_round_teaches_direct_awl_deploy_and_run_test() {
  let prompt = prompts.first_round(harness.base_input(), workspace())
  contains(prompt, "aion deploy FILE.awl")
  contains(prompt, "aion run FILE.awl --input 'JSON'")
  contains(prompt, "observe the durable run")
  prompt |> string.contains("aion package") |> should.be_false
}

pub fn first_round_carries_the_honesty_rules_test() {
  let prompt = prompts.first_round(harness.base_input(), workspace())
  contains(prompt, "I have not verified this")
  contains(prompt, "Never invent AWL syntax or worker capability")
}

pub fn first_round_states_the_file_tool_confinement_test() {
  // The norn file tools are confined to the workspace; shell commands start
  // there but can read elsewhere — the prompt must say so.
  let prompt = prompts.first_round(harness.base_input(), workspace())
  contains(prompt, "confined")
  contains(prompt, "shell")
}

pub fn first_round_without_a_repo_self_serves_a_clone_test() {
  let input = io.Input(objective: "explain signals", repo_path: "")
  let prompt = prompts.first_round(input, workspace())
  contains(prompt, "scratch git workspace")
  contains(prompt, "No repository was attached")
  contains(
    prompt,
    "git clone --depth 1 https://github.com/ablative-io/aion.git",
  )
  contains(prompt, "private")
  contains(prompt, ".assistant/resources/")
}

pub fn continuation_is_the_operator_message_verbatim_test() {
  prompts.continuation("Make the timeout configurable, please")
  |> should.equal("Make the timeout configurable, please")
}
