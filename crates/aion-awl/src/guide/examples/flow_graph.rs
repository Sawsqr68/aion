//! Examples for the step-graph vocabulary: steps, dependencies, forks, and
//! per-item regions.

/// `step`: a fall-through chain, and a pure decision step.
pub(crate) const STEP: &[&str] = &[
    r#"//! Statements inside a step run in written order; steps chain the graph.
workflow compose_greeting
  input name: String
  outcome done: type String, route success

step compose
  "hello " + name -> greeting

step deliver
  route done(greeting)"#,
    r#"//! A body-less step of only outcomes is a pure decision.
workflow sort_ticket
  input urgent: Bool
  outcome fast: type String, route success
  outcome slow: type String, route success

step triage
  outcome now: when urgent, route fast("page")
  outcome later: otherwise, route slow("queue")"#,
];

/// `after`: a single written dependency, and a diamond joining two.
pub(crate) const AFTER: &[&str] = &[
    r"//! `verify` waits for `build`: the dependency is written, not inferred.
workflow gated_build
  input tag: String
  outcome done: type Checked, route success

type Built   { artifact: String }
type Checked { ok: Bool }

worker ci
  action build(tag: String) -> Built
  action verify(artifact: String) -> Checked

step build
  build(tag: tag) -> built

step verify after build
  verify(artifact: built.artifact) -> checked
  route done(checked)",
    r#"//! A diamond: build and notes run independently, publish waits for both.
workflow release_notes
  input tag: String
  outcome done: type String, route success

type Build { artifact: String }
type Notes { text: String }

worker release
  action build(tag: String) -> Build
  action render_notes(tag: String) -> Notes

step start
  "release " + tag -> label

step build after start
  build(tag: tag) -> built

step notes after start
  render_notes(tag: tag) -> rendered

step publish after build, notes
  route done(label + " " + built.artifact + rendered.text)"#,
];

/// `fork`: the collection form, and the bare named-branch form.
pub(crate) const FORK: &[&str] = &[
    r"//! One branch per repo, checked in parallel, joined in input order.
workflow sweep_repos
  input repos: [String]
  outcome done: type [Verdict], route success

type Verdict { ok: Bool }

worker ci
  action check(repo: String) -> Verdict

step sweep
  fork repo in repos
    check(repo: repo)
  join -> verdicts
  route done(verdicts)",
    r#"//! A bare fork runs named heterogeneous calls concurrently.
workflow load_dashboard
  input user_id: String
  outcome done: type String, route success

type Profile { name: String }
type History { entries: Int }

worker accounts
  action fetch_profile(id: String) -> Profile
  action fetch_history(id: String) -> History

step load
  fork
    fetch_profile(id: user_id) -> profile
    fetch_history(id: user_id) -> history
  join
  outcome busy: when history.entries > 3, route done(profile.name + " is busy")
  outcome quiet: otherwise, route done(profile.name)"#,
];

/// `join`: binding a collection fork, and closing a named fork bare.
pub(crate) const JOIN: &[&str] = &[
    r"//! `join` closes the fork, binding the branches' results in input order.
workflow gather_reports
  input repos: [String]
  outcome done: type [Report], route success

type Report { summary: String }

worker inspectors
  action inspect(repo: String) -> Report

step gather
  fork repo in repos
    inspect(repo: repo)
  join -> reports
  route done(reports)",
    r"//! Bare `join` closes named branches; there is no collection to bind.
workflow warm_caches
  input region: String
  outcome done: type String, route success

worker infra
  action warm_dns(region: String) -> Nil
  action warm_cdn(region: String) -> Nil

step warm
  fork
    warm_dns(region: region)
    warm_cdn(region: region)
  join
  route done(region)",
];

/// `sequential`: one item at a time in a collection fork, its one form.
pub(crate) const SEQUENTIAL: &[&str] = &[
    r"//! `sequential` publishes one target at a time, in input order.
workflow publish_targets
  input targets: [String]
  outcome done: type [Receipt], route success

type Receipt { target: String }

worker publisher
  action publish(target: String) -> Receipt

step publish_all
  fork target in targets sequential
    publish(target: target)
  join -> receipts
  route done(receipts)",
];

/// `distribute`: a one-step track, and a track that is a little graph.
pub(crate) const DISTRIBUTE: &[&str] = &[
    r"//! Every host gets its own step track; the tracks run in parallel.
workflow inspect_fleet
  input hosts: [String]
  outcome done: type [Report], route success

type Report { host: String }

worker ops
  action inspect(host: String) -> Report

step open
  distribute host in hosts

step work
  inspect(host: host) -> report

step close
  collect report -> reports
  route done(reports)",
    r"//! Each item track is a little graph: inspect, then verify.
workflow remediate_fleet
  input hosts: [String]
  outcome done: type [Verdict], route success

type Fault   { detail: String }
type Verdict { ok: Bool }

worker ops
  action inspect(host: String) -> Fault
  action verify(host: String, fault: Fault) -> Verdict

step open
  distribute host in hosts

step inspect_host
  inspect(host: host) -> fault

step verify_host
  verify(host: host, fault: fault) -> verdict

step close
  collect verdict -> verdicts
  route done(verdicts)",
];

/// `sequence`: ordered tracks, and an ordered rollout with in-track retry.
pub(crate) const SEQUENCE: &[&str] = &[
    r"//! One host at a time, in collection order, each through its own track.
workflow roll_out
  input hosts: [String]
  outcome done: type [Attempt], route success

type Attempt { host: String, ok: Bool }

worker ops
  action deploy(host: String) -> Attempt

step ordered
  sequence host in hosts

step push
  deploy(host: host) -> attempt

step settle
  collect attempt -> attempts
  route done(attempts)",
    r"//! Ordered rollout with an in-track retry before each host settles.
workflow canary_rollout
  input hosts: [String]
  outcome done: type Int, route success

type Attempt { ok: Bool }

worker ops
  action deploy(host: String) -> Attempt
  action verify(attempt: Attempt) -> Attempt

step ordered
  sequence host in hosts

step push after ordered
  deploy(host: host) -> attempt

step confirm
  verify(attempt: attempt) -> checked
  outcome retry: when not checked.ok and visits < 3, route push
  outcome move_on: otherwise, route settle
  max 3 visits

step settle
  collect checked -> attempts
  attempts |> count -> rolled
  route done(rolled)",
];

/// `collect`: the strict close, and the tolerant one-optional-per-item close.
pub(crate) const COLLECT: &[&str] = &[
    r"//! `collect` opens the close step, folding the region's results back in.
workflow check_services
  input services: [String]
  outcome done: type Int, route success

type Health { ok: Bool }

worker probes
  action probe(service: String) -> Health

step open
  distribute service in services

step probe_one
  probe(service: service) -> health

step close
  collect health -> results
  results |> count -> total
  route done(total)",
    r"//! Tolerant collect: one optional slot per item, failures kept in history.
workflow best_effort_sweep
  input repos: [String]
  outcome done: type Int, route success

type Report { ok: Bool }

worker ci
  action check(repo: String) -> Report

step open
  distribute repo in repos

step check_repo
  check(repo: repo) -> report

step close
  collect report? -> reports
  reports |> count -> total
  route done(total)",
];

/// `in`: the fork binding, and the region-opener binding.
pub(crate) const IN: &[&str] = &[
    r"//! `in` names the per-item binding: each branch sees one `name`.
workflow greet_everyone
  input names: [String]
  outcome done: type [Greeting], route success

type Greeting { text: String }

worker greeter
  action greet(name: String) -> Greeting

step greet_each
  fork name in names
    greet(name: name)
  join -> greetings
  route done(greetings)",
    r"//! The same `in` names the per-item binding of a region opener.
workflow document_hosts
  input hosts: [String]
  outcome done: type [String], route success

worker ops
  action describe(host: String) -> String

step open
  distribute host in hosts

step describe_host
  describe(host: host) -> description

step close
  collect description -> descriptions
  route done(descriptions)",
];
