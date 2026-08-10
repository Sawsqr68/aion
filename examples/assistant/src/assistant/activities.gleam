//// Typed activity constructors for the assistant session.
////
//// Both activities are served by the agent-dev worker in production. The
//// `assistant` agent activity speaks the canonical agent-outcome contract:
//// ONE prompt string in, the `aion/agent.AgentOutcome` record out (`text` +
//// `stop_reason`) — the workflow composes every prompt
//// (`assistant/prompts`). The worker pins one norn session per
//// activity TYPE per run (`{workflow_id}-assistant` + `--resume-if-exists`),
//// so every round of this session — each a fresh dispatch of the same
//// `assistant` activity type — resumes the SAME conversation.
////
//// The local implementations carried by each `activity.new` are terminal
//// errors on purpose: this package ships no second, in-process
//// implementation of the worker. The `aion/testing` suites register their
//// own scenario handlers with `testing.mock_activity`; a dispatch that
//// reaches one of these stubs is a test that forgot to register a handler,
//// and it fails loudly saying so.

import aion/activity
import aion/agent.{type AgentOutcome}
import aion/codec
import aion/duration
import aion/error
import assistant_codecs as codecs
import assistant_io as io
import gleam/dynamic/decode
import gleam/json

/// The retry policy both steps carry for RETRYABLE failures (worker
/// death/restart mid-step, a lost connection): a few fixed re-deliveries.
/// Safe for the agent step because the worker pins one norn session per
/// activity type per run (`--resume-if-exists`): a retried round resumes the
/// SAME session and its accumulated conversation, never starting over.
/// Terminal failures (a real harness error) are not retried. Neither step
/// carries a timeout by design: an assistant round runs as long as it needs,
/// and a wedged round is cancelled/intervened, never timed out.
fn survives_worker_restart(
  step: activity.Activity(i, o),
) -> activity.Activity(i, o) {
  activity.retry(
    step,
    activity.RetryPolicy(
      max_attempts: 3,
      backoff: activity.Fixed(delay: duration.seconds(5)),
    ),
  )
}

/// Materialise the session workspace at `<root>/<run_id>/repo`: a clone of
/// `repo_path` when given, a fresh scratch git workspace when empty.
pub fn provision(
  input: io.ProvisionInput,
) -> activity.Activity(io.ProvisionInput, io.Workspace) {
  activity.new(
    "assistant_provision",
    input,
    codecs.provision_input_codec(),
    codecs.workspace_codec(),
    unserved("assistant_provision"),
  )
  |> survives_worker_restart
}

/// One assistant round under the canonical agent-outcome contract: prompt in
/// (a bare JSON string on the wire), the `AgentOutcome` record out — the
/// shape every harness emits from its `wait_result` boundary since #181.
pub fn assistant(prompt: String) -> activity.Activity(String, AgentOutcome) {
  activity.new(
    "assistant",
    prompt,
    text_codec(),
    agent.outcome_codec(),
    unserved("assistant"),
  )
  |> survives_worker_restart
}

/// The wire codec for agent prompts: a bare JSON string.
pub fn text_codec() -> codec.Codec(String) {
  codec.json_codec(json.string, decode.string)
}

/// The deliberate local stub: production serves this name from the agent-dev
/// worker, and tests must register a scenario handler for it.
fn unserved(name: String) -> fn(input) -> Result(output, error.ActivityError) {
  fn(_input) {
    Error(error.terminal(
      "the `"
      <> name
      <> "` activity is served by the agent-dev worker; register a "
      <> "testing.mock_activity handler for it in tests",
    ))
  }
}
