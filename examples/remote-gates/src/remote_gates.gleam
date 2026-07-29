//// remote_gates: hand a repository and a branch to a machine that is not yours,
//// and get the whole gate battery back as data. The clone is resolved to a real
//// tip commit, the four Rust legs fan out over that one working tree at once, and
//// every leg returns its real exit code with the diagnostics that produced it —
//// file, line, column, lint. A red gate names itself; nothing here claims a pass.

import aion/activity
import aion/awl/codec as awlc
import aion/awl/error as awl_error
import aion/awl/runtime
import aion/codec.{type Codec}
import aion/duration
import aion/error
import aion/signal
import aion/workflow
import gleam/dynamic.{type Dynamic}
import gleam/dynamic/decode
import gleam/json
import gleam/list
import gleam/option.{type Option, None, Some}
import gleam/result

/// One diagnostic lifted from a leg's machine-readable stream: a compiler
/// message's primary span, or a failed test's name and captured output.
pub type Finding {
  Finding(
    leg: String,
    level: String,
    file: String,
    line: Option(Int),
    column: Option(Int),
    code: String,
    message: String,
  )
}

/// One leg's actual result. `denominator` is how many well-formed records the
/// leg's stream yielded: zero findings with a zero denominator is a broken
/// stream, not a green gate, and the leg fails on that ground alone.
pub type LegResult {
  LegResult(
    leg: String,
    exit_code: Int,
    passed: Bool,
    findings: List(Finding),
    denominator: Int,
    log_path: String,
    output_tail: String,
  )
}

/// The working tree the legs actually ran over, resolved to its tip commit so
/// every finding is bound to a commit rather than to a branch name.
pub type Checkout {
  Checkout(
    path: String,
    commit: String,
    branch: String,
    fetched_fresh: Bool,
    free_disk_bytes: Int,
  )
}

/// What the side channel did with the summary — including an honest skip.
pub type NotifyResult {
  NotifyResult(
    delivered: Bool,
    detail: String,
  )
}

pub type Clean {
  Clean(
    commit: String,
    legs: List(LegResult),
  )
}

/// Carries ALL leg results, passed flags included — the VM cannot build a
/// failed-only sublist from named-fork bindings (cargo-gates workbench finding).
pub type Failing {
  Failing(
    commit: String,
    legs: List(LegResult),
  )
}

pub type RemoteGatesInput {
  RemoteGatesInput(
    repo: String,
    branch: String,
  )
}

pub type RemoteGatesOutcome {
  CleanOutcome(Clean)
}

/// Typed definition binding the codecs to the execute function.
pub fn definition() -> workflow.WorkflowDefinition(RemoteGatesInput, RemoteGatesOutcome, awl_error.AwlError) {
  workflow.define(
    "remote_gates",
    remote_gates_input_codec(),
    remote_gates_outcome_codec(),
    awl_error.codec(),
    execute,
  )
}

/// Engine entry point.
pub fn run(raw_input: Dynamic) -> Result(String, String) {
  runtime.run(raw_input, remote_gates_input_codec(), remote_gates_outcome_codec(), execute)
}

/// Workflow body generated from the AWL steps.
pub fn execute(input: RemoteGatesInput) -> Result(RemoteGatesOutcome, awl_error.AwlError) {
  let repo = input.repo
  let branch = input.branch
  step_fetch_source(branch, repo)
}

fn step_fetch_source(branch: String, repo: String) -> Result(RemoteGatesOutcome, awl_error.AwlError) {
  use checkout <- result.try(clone_repo_activity(repo, branch) |> activity.timeout(duration.milliseconds(600000)) |> activity.task_queue("remote_gates") |> activity.node("shell") |> workflow.run |> awl_error.map_activity_error)
  use awl_branches <- result.try(workflow.all([run_check_activity_raw(checkout.path) |> activity.timeout(duration.milliseconds(900000)) |> activity.task_queue("remote_gates") |> activity.node("shell"), run_clippy_activity_raw(checkout.path) |> activity.timeout(duration.milliseconds(900000)) |> activity.task_queue("remote_gates") |> activity.node("shell"), run_nextest_activity_raw(checkout.path) |> activity.timeout(duration.milliseconds(2700000)) |> activity.task_queue("remote_gates") |> activity.node("shell"), run_fmt_check_activity_raw(checkout.path) |> activity.timeout(duration.milliseconds(300000)) |> activity.task_queue("remote_gates") |> activity.node("shell")]) |> awl_error.map_activity_error)
  let assert [awl_raw_0, awl_raw_1, awl_raw_2, awl_raw_3] = awl_branches
  use check_leg <- result.try(awlc.decoded(leg_result_codec(), awl_raw_0, "run_check"))
  use clippy_leg <- result.try(awlc.decoded(leg_result_codec(), awl_raw_1, "run_clippy"))
  use nextest_leg <- result.try(awlc.decoded(leg_result_codec(), awl_raw_2, "run_nextest"))
  use fmt_leg <- result.try(awlc.decoded(leg_result_codec(), awl_raw_3, "run_fmt_check"))
  case { { check_leg.passed && clippy_leg.passed } && nextest_leg.passed } && fmt_leg.passed {
    True -> {
      step_announce_clean(branch, check_leg, checkout, clippy_leg, fmt_leg, nextest_leg, repo)
    }
    False -> {
      step_announce_failing(branch, check_leg, checkout, clippy_leg, fmt_leg, nextest_leg, repo)
    }
  }
}

fn step_announce_clean(branch: String, check_leg: LegResult, checkout: Checkout, clippy_leg: LegResult, fmt_leg: LegResult, nextest_leg: LegResult, repo: String) -> Result(RemoteGatesOutcome, awl_error.AwlError) {
  let _ = notify_result_activity({ { { { "remote_gates CLEAN " <> repo } <> "@" } <> branch } <> " " } <> checkout.commit) |> activity.timeout(duration.milliseconds(120000)) |> activity.task_queue("remote_gates") |> activity.node("shell") |> workflow.run
  Ok(CleanOutcome(Clean(commit: checkout.commit, legs: [check_leg, clippy_leg, nextest_leg, fmt_leg])))
}

fn step_announce_failing(branch: String, check_leg: LegResult, checkout: Checkout, clippy_leg: LegResult, fmt_leg: LegResult, nextest_leg: LegResult, repo: String) -> Result(RemoteGatesOutcome, awl_error.AwlError) {
  let _ = notify_result_activity({ { { { "remote_gates FAILING " <> repo } <> "@" } <> branch } <> " " } <> checkout.commit) |> activity.timeout(duration.milliseconds(120000)) |> activity.task_queue("remote_gates") |> activity.node("shell") |> workflow.run
  Error(awl_error.AwlOutcomeFailure("failing", json.to_string(failing_to_json(Failing(commit: checkout.commit, legs: [check_leg, clippy_leg, nextest_leg, fmt_leg])))))
}

pub type CloneRepoInput {
  CloneRepoInput(
    repo: String,
    branch: String,
  )
}

fn clone_repo_activity(
  repo: String,
  branch: String,
) -> activity.Activity(CloneRepoInput, Checkout) {
  activity.new(
    "clone_repo",
    CloneRepoInput(
      repo: repo,
      branch: branch,
    ),
    clone_repo_input_codec(),
    checkout_codec(),
    fn(_) { Error(error.terminal("activity body is provided by a worker")) },
  )
}

pub type RunCheckInput {
  RunCheckInput(
    path: String,
  )
}

fn run_check_activity(
  path: String,
) -> activity.Activity(RunCheckInput, LegResult) {
  activity.new(
    "run_check",
    RunCheckInput(
      path: path,
    ),
    run_check_input_codec(),
    leg_result_codec(),
    fn(_) { Error(error.terminal("activity body is provided by a worker")) },
  )
}

fn run_check_activity_raw(
  path: String,
) -> activity.Activity(String, String) {
  let awl_input_codec = run_check_input_codec()
  activity.new(
    "run_check",
    awl_input_codec.encode(RunCheckInput(
      path: path,
    )),
    awlc.raw(),
    awlc.raw(),
    fn(_) { Error(error.terminal("activity body is provided by a worker")) },
  )
}

pub type RunClippyInput {
  RunClippyInput(
    path: String,
  )
}

fn run_clippy_activity(
  path: String,
) -> activity.Activity(RunClippyInput, LegResult) {
  activity.new(
    "run_clippy",
    RunClippyInput(
      path: path,
    ),
    run_clippy_input_codec(),
    leg_result_codec(),
    fn(_) { Error(error.terminal("activity body is provided by a worker")) },
  )
}

fn run_clippy_activity_raw(
  path: String,
) -> activity.Activity(String, String) {
  let awl_input_codec_2 = run_clippy_input_codec()
  activity.new(
    "run_clippy",
    awl_input_codec_2.encode(RunClippyInput(
      path: path,
    )),
    awlc.raw(),
    awlc.raw(),
    fn(_) { Error(error.terminal("activity body is provided by a worker")) },
  )
}

pub type RunNextestInput {
  RunNextestInput(
    path: String,
  )
}

fn run_nextest_activity(
  path: String,
) -> activity.Activity(RunNextestInput, LegResult) {
  activity.new(
    "run_nextest",
    RunNextestInput(
      path: path,
    ),
    run_nextest_input_codec(),
    leg_result_codec(),
    fn(_) { Error(error.terminal("activity body is provided by a worker")) },
  )
}

fn run_nextest_activity_raw(
  path: String,
) -> activity.Activity(String, String) {
  let awl_input_codec_3 = run_nextest_input_codec()
  activity.new(
    "run_nextest",
    awl_input_codec_3.encode(RunNextestInput(
      path: path,
    )),
    awlc.raw(),
    awlc.raw(),
    fn(_) { Error(error.terminal("activity body is provided by a worker")) },
  )
}

pub type RunFmtCheckInput {
  RunFmtCheckInput(
    path: String,
  )
}

fn run_fmt_check_activity(
  path: String,
) -> activity.Activity(RunFmtCheckInput, LegResult) {
  activity.new(
    "run_fmt_check",
    RunFmtCheckInput(
      path: path,
    ),
    run_fmt_check_input_codec(),
    leg_result_codec(),
    fn(_) { Error(error.terminal("activity body is provided by a worker")) },
  )
}

fn run_fmt_check_activity_raw(
  path: String,
) -> activity.Activity(String, String) {
  let awl_input_codec_4 = run_fmt_check_input_codec()
  activity.new(
    "run_fmt_check",
    awl_input_codec_4.encode(RunFmtCheckInput(
      path: path,
    )),
    awlc.raw(),
    awlc.raw(),
    fn(_) { Error(error.terminal("activity body is provided by a worker")) },
  )
}

pub type NotifyResultInput {
  NotifyResultInput(
    summary: String,
  )
}

/// A side channel: an unreachable notifier must never sink a gate run.
fn notify_result_activity(
  summary: String,
) -> activity.Activity(NotifyResultInput, NotifyResult) {
  activity.new(
    "notify_result",
    NotifyResultInput(
      summary: summary,
    ),
    notify_result_input_codec(),
    notify_result_codec(),
    fn(_) { Error(error.terminal("activity body is provided by a worker")) },
  )
}

fn remote_gates_input_codec() -> Codec(RemoteGatesInput) {
  codec.json_codec(remote_gates_input_to_json, remote_gates_input_decoder())
}

fn remote_gates_input_to_json(value: RemoteGatesInput) -> json.Json {
  json.object([
    #("repo", awlc.string_to_json(value.repo)),
    #("branch", awlc.string_to_json(value.branch)),
  ])
}

fn remote_gates_input_decoder() -> decode.Decoder(RemoteGatesInput) {
  use repo <- decode.field("repo", awlc.string_decoder())
  use branch <- decode.field("branch", awlc.string_decoder())
  decode.success(RemoteGatesInput(
    repo: repo,
    branch: branch,
  ))
}

fn remote_gates_outcome_codec() -> Codec(RemoteGatesOutcome) {
  codec.json_codec(remote_gates_outcome_to_json, remote_gates_outcome_decoder())
}

fn remote_gates_outcome_to_json(value: RemoteGatesOutcome) -> json.Json {
  case value {
    CleanOutcome(payload) -> json.object([#("outcome", json.string("clean")), #("payload", clean_to_json(payload))])
  }
}

fn remote_gates_outcome_decoder() -> decode.Decoder(RemoteGatesOutcome) {
  use outcome <- decode.field("outcome", decode.string)
  case outcome {
    "clean" -> {
      use payload <- decode.field("payload", clean_decoder())
      decode.success(CleanOutcome(payload))
    }
    _ -> decode.failure(CleanOutcome(Clean(commit: "", legs: [])), "RemoteGatesOutcome")
  }
}

fn finding_codec() -> Codec(Finding) {
  codec.json_codec(finding_to_json, finding_decoder())
}

fn finding_to_json(value: Finding) -> json.Json {
  json.object(list.flatten([
    [#("leg", awlc.string_to_json(value.leg))],
    [#("level", awlc.string_to_json(value.level))],
    [#("file", awlc.string_to_json(value.file))],
    case value.line {
      Some(inner) -> [#("line", awlc.int_to_json(inner))]
      None -> []
    },
    case value.column {
      Some(inner) -> [#("column", awlc.int_to_json(inner))]
      None -> []
    },
    [#("code", awlc.string_to_json(value.code))],
    [#("message", awlc.string_to_json(value.message))],
  ]))
}

fn finding_decoder() -> decode.Decoder(Finding) {
  use leg <- decode.field("leg", awlc.string_decoder())
  use level <- decode.field("level", awlc.string_decoder())
  use file <- decode.field("file", awlc.string_decoder())
  use line <- decode.optional_field("line", None, decode.map(awlc.int_decoder(), Some))
  use column <- decode.optional_field("column", None, decode.map(awlc.int_decoder(), Some))
  use code <- decode.field("code", awlc.string_decoder())
  use message <- decode.field("message", awlc.string_decoder())
  decode.success(Finding(
    leg: leg,
    level: level,
    file: file,
    line: line,
    column: column,
    code: code,
    message: message,
  ))
}

fn leg_result_codec() -> Codec(LegResult) {
  codec.json_codec(leg_result_to_json, leg_result_decoder())
}

fn leg_result_to_json(value: LegResult) -> json.Json {
  json.object([
    #("leg", awlc.string_to_json(value.leg)),
    #("exit_code", awlc.int_to_json(value.exit_code)),
    #("passed", awlc.bool_to_json(value.passed)),
    #("findings", list_finding_to_json(value.findings)),
    #("denominator", awlc.int_to_json(value.denominator)),
    #("log_path", awlc.string_to_json(value.log_path)),
    #("output_tail", awlc.string_to_json(value.output_tail)),
  ])
}

fn leg_result_decoder() -> decode.Decoder(LegResult) {
  use leg <- decode.field("leg", awlc.string_decoder())
  use exit_code <- decode.field("exit_code", awlc.int_decoder())
  use passed <- decode.field("passed", awlc.bool_decoder())
  use findings <- decode.field("findings", list_finding_decoder())
  use denominator <- decode.field("denominator", awlc.int_decoder())
  use log_path <- decode.field("log_path", awlc.string_decoder())
  use output_tail <- decode.field("output_tail", awlc.string_decoder())
  decode.success(LegResult(
    leg: leg,
    exit_code: exit_code,
    passed: passed,
    findings: findings,
    denominator: denominator,
    log_path: log_path,
    output_tail: output_tail,
  ))
}

fn checkout_codec() -> Codec(Checkout) {
  codec.json_codec(checkout_to_json, checkout_decoder())
}

fn checkout_to_json(value: Checkout) -> json.Json {
  json.object([
    #("path", awlc.string_to_json(value.path)),
    #("commit", awlc.string_to_json(value.commit)),
    #("branch", awlc.string_to_json(value.branch)),
    #("fetched_fresh", awlc.bool_to_json(value.fetched_fresh)),
    #("free_disk_bytes", awlc.int_to_json(value.free_disk_bytes)),
  ])
}

fn checkout_decoder() -> decode.Decoder(Checkout) {
  use path <- decode.field("path", awlc.string_decoder())
  use commit <- decode.field("commit", awlc.string_decoder())
  use branch <- decode.field("branch", awlc.string_decoder())
  use fetched_fresh <- decode.field("fetched_fresh", awlc.bool_decoder())
  use free_disk_bytes <- decode.field("free_disk_bytes", awlc.int_decoder())
  decode.success(Checkout(
    path: path,
    commit: commit,
    branch: branch,
    fetched_fresh: fetched_fresh,
    free_disk_bytes: free_disk_bytes,
  ))
}

fn notify_result_codec() -> Codec(NotifyResult) {
  codec.json_codec(notify_result_to_json, notify_result_decoder())
}

fn notify_result_to_json(value: NotifyResult) -> json.Json {
  json.object([
    #("delivered", awlc.bool_to_json(value.delivered)),
    #("detail", awlc.string_to_json(value.detail)),
  ])
}

fn notify_result_decoder() -> decode.Decoder(NotifyResult) {
  use delivered <- decode.field("delivered", awlc.bool_decoder())
  use detail <- decode.field("detail", awlc.string_decoder())
  decode.success(NotifyResult(
    delivered: delivered,
    detail: detail,
  ))
}

fn clean_codec() -> Codec(Clean) {
  codec.json_codec(clean_to_json, clean_decoder())
}

fn clean_to_json(value: Clean) -> json.Json {
  json.object([
    #("commit", awlc.string_to_json(value.commit)),
    #("legs", list_leg_result_to_json(value.legs)),
  ])
}

fn clean_decoder() -> decode.Decoder(Clean) {
  use commit <- decode.field("commit", awlc.string_decoder())
  use legs <- decode.field("legs", list_leg_result_decoder())
  decode.success(Clean(
    commit: commit,
    legs: legs,
  ))
}

fn failing_codec() -> Codec(Failing) {
  codec.json_codec(failing_to_json, failing_decoder())
}

fn failing_to_json(value: Failing) -> json.Json {
  json.object([
    #("commit", awlc.string_to_json(value.commit)),
    #("legs", list_leg_result_to_json(value.legs)),
  ])
}

fn failing_decoder() -> decode.Decoder(Failing) {
  use commit <- decode.field("commit", awlc.string_decoder())
  use legs <- decode.field("legs", list_leg_result_decoder())
  decode.success(Failing(
    commit: commit,
    legs: legs,
  ))
}

fn clone_repo_input_codec() -> Codec(CloneRepoInput) {
  codec.json_codec(clone_repo_input_to_json, clone_repo_input_decoder())
}

fn clone_repo_input_to_json(value: CloneRepoInput) -> json.Json {
  json.object([
    #("repo", awlc.string_to_json(value.repo)),
    #("branch", awlc.string_to_json(value.branch)),
  ])
}

fn clone_repo_input_decoder() -> decode.Decoder(CloneRepoInput) {
  use repo <- decode.field("repo", awlc.string_decoder())
  use branch <- decode.field("branch", awlc.string_decoder())
  decode.success(CloneRepoInput(
    repo: repo,
    branch: branch,
  ))
}

fn run_check_input_codec() -> Codec(RunCheckInput) {
  codec.json_codec(run_check_input_to_json, run_check_input_decoder())
}

fn run_check_input_to_json(value: RunCheckInput) -> json.Json {
  json.object([
    #("path", awlc.string_to_json(value.path)),
  ])
}

fn run_check_input_decoder() -> decode.Decoder(RunCheckInput) {
  use path <- decode.field("path", awlc.string_decoder())
  decode.success(RunCheckInput(
    path: path,
  ))
}

fn run_clippy_input_codec() -> Codec(RunClippyInput) {
  codec.json_codec(run_clippy_input_to_json, run_clippy_input_decoder())
}

fn run_clippy_input_to_json(value: RunClippyInput) -> json.Json {
  json.object([
    #("path", awlc.string_to_json(value.path)),
  ])
}

fn run_clippy_input_decoder() -> decode.Decoder(RunClippyInput) {
  use path <- decode.field("path", awlc.string_decoder())
  decode.success(RunClippyInput(
    path: path,
  ))
}

fn run_nextest_input_codec() -> Codec(RunNextestInput) {
  codec.json_codec(run_nextest_input_to_json, run_nextest_input_decoder())
}

fn run_nextest_input_to_json(value: RunNextestInput) -> json.Json {
  json.object([
    #("path", awlc.string_to_json(value.path)),
  ])
}

fn run_nextest_input_decoder() -> decode.Decoder(RunNextestInput) {
  use path <- decode.field("path", awlc.string_decoder())
  decode.success(RunNextestInput(
    path: path,
  ))
}

fn run_fmt_check_input_codec() -> Codec(RunFmtCheckInput) {
  codec.json_codec(run_fmt_check_input_to_json, run_fmt_check_input_decoder())
}

fn run_fmt_check_input_to_json(value: RunFmtCheckInput) -> json.Json {
  json.object([
    #("path", awlc.string_to_json(value.path)),
  ])
}

fn run_fmt_check_input_decoder() -> decode.Decoder(RunFmtCheckInput) {
  use path <- decode.field("path", awlc.string_decoder())
  decode.success(RunFmtCheckInput(
    path: path,
  ))
}

fn notify_result_input_codec() -> Codec(NotifyResultInput) {
  codec.json_codec(notify_result_input_to_json, notify_result_input_decoder())
}

fn notify_result_input_to_json(value: NotifyResultInput) -> json.Json {
  json.object([
    #("summary", awlc.string_to_json(value.summary)),
  ])
}

fn notify_result_input_decoder() -> decode.Decoder(NotifyResultInput) {
  use summary <- decode.field("summary", awlc.string_decoder())
  decode.success(NotifyResultInput(
    summary: summary,
  ))
}

fn list_finding_codec() -> Codec(List(Finding)) {
  codec.json_codec(list_finding_to_json, list_finding_decoder())
}
fn list_finding_to_json(values: List(Finding)) -> json.Json { json.array(values, finding_to_json) }
fn list_finding_decoder() -> decode.Decoder(List(Finding)) { decode.list(finding_decoder()) }

fn list_leg_result_codec() -> Codec(List(LegResult)) {
  codec.json_codec(list_leg_result_to_json, list_leg_result_decoder())
}
fn list_leg_result_to_json(values: List(LegResult)) -> json.Json { json.array(values, leg_result_to_json) }
fn list_leg_result_decoder() -> decode.Decoder(List(LegResult)) { decode.list(leg_result_decoder()) }

fn option_int_codec() -> Codec(Option(Int)) {
  codec.json_codec(option_int_to_json, option_int_decoder())
}
fn option_int_to_json(value: Option(Int)) -> json.Json { json.nullable(value, awlc.int_to_json) }
fn option_int_decoder() -> decode.Decoder(Option(Int)) { decode.optional(awlc.int_decoder()) }

