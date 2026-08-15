//! Examples for the worker vocabulary: contracts, config, bodies, children,
//! and subflows.

/// `worker`: a workflow document's requirement, and a worker DOCUMENT.
pub(crate) const WORKER: &[&str] = &[
    r"//! The worker block is a typed requirement on the `greeter` task queue.
workflow greet_politely
  input name: String
  outcome done: type Greeting, route success

type Greeting { text: String }

worker greeter
  action greet(name: String) -> Greeting

step greet
  greet(name: name) -> reply
  route done(reply)",
    r#"//! A worker DOCUMENT: the implementation of one task queue.
worker greeter
  action greet(name: String) -> Greeting
    run json "greeter-tool --json --name $name"

type Greeting { message: String }"#,
];

/// `action`: a requirement in a workflow document, and an implementation in a
/// worker document.
pub(crate) const ACTION: &[&str] = &[
    r"//! Each action line is one wire contract: named parameters, one return.
workflow inspect_repo
  input repo: String
  outcome done: type Report, route success

type Report { summary: String }

worker inspectors
  action inspect(repo: String) -> Report

step inspect
  inspect(repo: repo) -> report
  route done(report)",
    r#"//! In a worker document every action carries the command that runs it.
worker archiver
  action archive(path: String) -> Archived
    run "tar -cf archive.tar $path"

type Archived { exit_code: Int, stderr: String }"#,
];

/// `node`: pinned on the declaration, and overridden at one call site.
pub(crate) const NODE: &[&str] = &[
    r"//! Pin an action's dispatch to the named worker node.
workflow build_release
  input tag: String
  outcome done: type Build, route success

type Build { artifact: String }

worker builders
  action build(tag: String) -> Build
    node linux_large

step build
  build(tag: tag) -> built
  route done(built)",
    r"//! Override the declared node at one urgent call site.
workflow hotfix_build
  input tag: String
  outcome done: type Build, route success

type Build { artifact: String }

worker builders
  action build(tag: String) -> Build
    node standard, timeout 30m

step build
  build(tag: tag) -> built
    node urgent, timeout 5m
  route done(built)",
];

/// `retry`: the fixed-interval form, and the growing backoff form.
pub(crate) const RETRY: &[&str] = &[
    r"//! Retry the flaky pull three times before the step faults.
workflow sync_mirror
  input remote: String
  outcome done: type Synced, route success

type Synced { revision: String }

worker mirrors
  action pull(remote: String) -> Synced
    retry 3 every 30s

step pull
  pull(remote: remote) -> synced
  route done(synced)",
    r"//! The growing form: intervals stretch from five seconds toward four minutes.
workflow crawl_source
  input source: String
  outcome done: type Page, route success

type Page { body: String }

worker crawlers
  action crawl(source: String) -> Page
    retry 6 backoff 5s..4m

step crawl
  crawl(source: source) -> page
  route done(page)",
];

/// `every`: the fixed retry interval, its one form.
pub(crate) const EVERY: &[&str] = &[
    r"//! `every` fixes the interval between retries: ten seconds, each time.
workflow ping_service
  input url: String
  outcome done: type Pong, route success

type Pong { latency_ms: Int }

worker probes
  action ping(url: String) -> Pong
    retry 5 every 10s

step ping
  ping(url: url) -> pong
  route done(pong)",
];

/// `backoff`: the growing retry interval, its one form.
pub(crate) const BACKOFF: &[&str] = &[
    r"//! `backoff` grows the retry interval from ten seconds toward two minutes.
workflow fetch_upstream
  input source: String
  outcome done: type Fetched, route success

type Fetched { bytes: Int }

worker fetchers
  action fetch(source: String) -> Fetched
    retry 4 backoff 10s..2m

step fetch
  fetch(source: source) -> fetched
  route done(fetched)",
];

/// `advisory`: the declaration marker, its one form.
pub(crate) const ADVISORY: &[&str] = &[
    r"//! An advisory announcement can fail without faulting the run.
workflow ship_build
  input version: String
  outcome done: type String, route success

worker notifier
  action announce(version: String) -> Nil
    advisory, timeout 30s

step ship
  announce(version: version)
  route done(version)",
];

/// `agent`: the declaration marker, its one form.
pub(crate) const AGENT: &[&str] = &[
    r"//! An agent action hands its prompt to a harness and binds the outcome
//! record: accumulated `text`, the last answer as `final_message`, the canonical
//! `stop_reason`, and the `session_id` handle of the conversation that produced it.
workflow ask_reviewer
  input question: String
  outcome done: type String, route success

type Reply { text: String, final_message: String, stop_reason: String, session_id: String }

worker assistants
  action ask(prompt: String) -> Reply
    agent, timeout 10m

step ask
  ask(prompt: question) -> reply
  route done(reply.text)",
];

/// `run`: a workflow document's declared body, and a worker document's two
/// capture selectors.
pub(crate) const RUN: &[&str] = &[
    r#"//! The document is the implementation: the server runs the command itself.
workflow greet_by_hand
  input name: String
  outcome done: type Stdout, route success

type Stdout { stdout: String }

worker shell
  action greet(name: String) -> Stdout
    run "echo hello $name"

step greet
  greet(name: name) -> out
  route done(out)"#,
    r#"//! `run text` returns the command's trimmed stdout as one String.
worker shell_tools
  action checksum(path: String) -> String
    run text "shasum $path""#,
    r#"//! `run json` parses the command's stdout into the declared type.
worker probes
  action disk_report(mount: String) -> Report
    run json "disk-tool --json $mount"

type Report { free_bytes: Int, mount: String }"#,
];

/// `child`: one awaited child, and an awaited child beside a detached one.
pub(crate) const CHILD: &[&str] = &[
    r"//! Call a separate durable workflow and await its result.
workflow grade_essay
  input essay: String
  outcome done: type Grade, route success

type Grade { score: Int }

child score_essay(essay: String) -> Grade

step grade
  score_essay(essay: essay) -> grade
  route done(grade)",
    r"//! Await one child; detach another this run never waits for.
workflow grade_and_notify
  input essay: String
  outcome done: type Grade, route success

type Grade { score: Int }

child score_essay(essay: String) -> Grade
child notify_graded(score: Int) -> Nil

step grade
  score_essay(essay: essay) -> grade
  spawn notify_graded(score: grade.score)
  route done(grade)",
];

/// `spawn`: fire-and-forget, its one form.
pub(crate) const SPAWN: &[&str] = &[r"//! Start a child without waiting; it can bind no result.
workflow archive_report
  input report_id: String
  outcome done: type String, route success

child notify_archived(report_id: String) -> Nil

step archive
  spawn notify_archived(report_id: report_id)
  route done(report_id)"];

/// `subflow`: one invocation, and one run per item inside a region.
pub(crate) const SUBFLOW: &[&str] = &[
    r"//! An inline reusable graph with typed parameters and one outcome.
workflow review_repo
  input repo: String
  outcome done: type Report, route success

type Report { summary: String }

worker reviewers
  action inspect(target: String) -> Report

subflow review_once(target: String)
  outcome out: type Report

  step look
    inspect(target: target) -> report
    route out(report)

step review
  review_once(target: repo) -> report

step publish
  report |> route done",
    r"//! One subflow, run once per item inside a distribute region.
workflow review_wave
  input repos: [String]
  outcome done: type [Verdict], route success

type Verdict { ok: Bool }

worker reviewers
  action inspect(target: String) -> Verdict

subflow review_one(target: String)
  outcome out: type Verdict

  step look
    inspect(target: target) -> verdict
    route out(verdict)

step open
  distribute repo in repos

step review
  review_one(target: repo) -> verdict

step close
  collect verdict -> verdicts
  route done(verdicts)",
];

/// `timeout`: the header bound, the activity bound, and the wait bound.
pub(crate) const TIMEOUT: &[&str] = &[
    r#"//! The header timeout bounds the whole run.
workflow nightly_sweep
  timeout 2h
  input target: String
  outcome done: type String, route success

step sweep
  route done("swept " + target)"#,
    r"//! An activity timeout on the declaration, tightened at one call site.
workflow build_twice
  input tag: String
  outcome done: type Build, route success

type Build { artifact: String }

worker builders
  action build(tag: String) -> Build
    timeout 30m

step quick_probe
  build(tag: tag) -> probe
    timeout 5m

step full_build
  build(tag: probe.artifact) -> built
  route done(built)",
    r#"//! A wait with a timeout binds an optional: expiry is absence.
workflow hold_for_signoff
  input change_id: String
  signal signoff: Signoff
  outcome shipped:   type String, route success
  outcome escalated: type String, route failure

type Signoff { reviewer: String }

step hold
  wait signoff timeout 2d -> decision
  outcome approved: when decision is present, route shipped(decision.reviewer)
  outcome lapsed: otherwise, route escalated("window lapsed: " + change_id)"#,
];
