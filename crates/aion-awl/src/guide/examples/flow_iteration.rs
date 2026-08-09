//! Examples for bounded iteration, waits, timers, and failure handling.

/// `loop`: convergence on a flag, and a counted run of passes.
pub(crate) const LOOP: &[&str] = &[
    r#"//! Improve the draft until it is ready, at most three passes.
workflow refine_draft
  input topic: String
  outcome done:  type Draft,  route success
  outcome stuck: type String, route failure

type Draft { body: String, ready: Bool }

worker writers
  action improve(topic: String, prior: Draft) -> Draft

step refine
  loop draft = Draft(body: "", ready: false)
    improve(topic: topic, prior: draft) -> draft
    until draft.ready
    max 3
  outcome ready: when draft.ready, route done(draft)
  outcome exhausted: otherwise, route stuck("the draft never settled")"#,
    r"//! The threaded value carries state between passes; rounds are counted.
workflow tune_model
  input target: String
  outcome done:  type Int,    route success
  outcome stuck: type String, route failure

type Tuning { score: Int, good_enough: Bool }

worker lab
  action tune(target: String, prior: Tuning) -> Tuning

step tune
  loop tuning = Tuning(score: 0, good_enough: false) counting rounds
    tune(target: target, prior: tuning) -> tuning
    until tuning.good_enough
    max 8
  outcome tuned: when tuning.good_enough, route done(rounds)
  outcome gave_up: otherwise, route stuck(target)",
];

/// `counting`: the completed-iteration binding, its one form.
pub(crate) const COUNTING: &[&str] = &[
    r"//! `counting` binds how many iterations the loop completed.
workflow poll_job
  input job: String
  outcome done:  type Int,    route success
  outcome stuck: type String, route failure

type Status { ready: Bool }

worker jobs
  action poll(job: String, prior: Status) -> Status

step watch
  loop status = Status(ready: false) counting polls
    poll(job: job, prior: status) -> status
    until status.ready
    max 10
  outcome ready: when status.ready, route done(polls)
  outcome gave_up: otherwise, route stuck(job)",
];

/// `until`: a single flag, and a compound condition.
pub(crate) const UNTIL: &[&str] = &[
    r"//! `until` ends the loop, tested after each pass of the body.
workflow stabilize_build
  input tag: String
  outcome done:  type Build,  route success
  outcome stuck: type String, route failure

type Build { green: Bool }

worker ci
  action rebuild(tag: String, prior: Build) -> Build

step converge
  loop build = Build(green: false)
    rebuild(tag: tag, prior: build) -> build
    until build.green
    max 5
  outcome green: when build.green, route done(build)
  outcome exhausted: otherwise, route stuck(tag)",
    r"//! A compound condition: stop only when ready AND approved.
workflow draft_and_signoff
  input topic: String
  outcome done:  type Draft,  route success
  outcome stuck: type String, route failure

type Draft { ready: Bool, approved: Bool }

worker desks
  action revise(topic: String, prior: Draft) -> Draft

step converge
  loop draft = Draft(ready: false, approved: false)
    revise(topic: topic, prior: draft) -> draft
    until draft.ready and draft.approved
    max 6
  outcome settled: when draft.ready and draft.approved, route done(draft)
  outcome exhausted: otherwise, route stuck(topic)",
];

/// `max`: the loop ceiling, and the step visit bound.
pub(crate) const MAX: &[&str] = &[
    r"//! `max` is the mandatory ceiling; exhaustion is a distinct outcome.
workflow negotiate_quote
  input request: String
  outcome done:  type Offer,  route success
  outcome stuck: type String, route failure

type Offer { accepted: Bool }

worker brokers
  action haggle(request: String, prior: Offer) -> Offer

step negotiate
  loop offer = Offer(accepted: false)
    haggle(request: request, prior: offer) -> offer
    until offer.accepted
    max 4
  outcome agreed: when offer.accepted, route done(offer)
  outcome exhausted: otherwise, route stuck(request)",
    r"//! The other place `max` stands: a visit bound on a step in a flow loop.
workflow polish_copy
  input text: String
  outcome done: type Verdict, route success

type Verdict { ok: Bool }

worker desks
  action review(text: String) -> Verdict

step review
  review(text: text) -> verdict

step judge
  outcome again: when not verdict.ok and visits < 2, route review
  outcome settled: otherwise, route done(verdict)
  max 2 visits",
];

/// `visits`: read in a guard, and bounded by an expression.
pub(crate) const VISITS: &[&str] = &[
    r"//! `max 3 visits` bounds the flow loop; `visits` reads the count in guards.
workflow recheck_repo
  input repo: String
  outcome done: type Verdict, route success

type Verdict { ok: Bool }

worker ci
  action check(repo: String) -> Verdict

step attempt
  check(repo: repo) -> verdict

step judge
  outcome again: when not verdict.ok and visits < 3, route attempt
  outcome settled: otherwise, route done(verdict)
  max 3 visits",
    r"//! The visit bound can be an expression over inputs and consts.
workflow bounded_polish
  input text: String
  input max_rounds: Int
  outcome done: type Verdict, route success

type Verdict { ok: Bool }

worker desks
  action polish(text: String) -> Verdict

step attempt
  polish(text: text) -> verdict

step judge
  outcome again: when not verdict.ok and visits < max_rounds, route attempt
  outcome settled: otherwise, route done(verdict)
  max max_rounds visits",
];

/// `wait`: untimed delivery, and a timed wait whose expiry is absence.
pub(crate) const WAIT: &[&str] = &[
    r#"//! Take delivery of the declared signal, binding its payload.
workflow hold_for_go
  input build_id: String
  signal go: Approval
  outcome done: type String, route success

type Approval { approver: String }

step hold
  wait go -> approval
  route done(approval.approver + " released " + build_id)"#,
    r#"//! A timed wait binds an optional, because expiry is absence.
workflow ask_signoff
  input change_id: String
  signal signoff: Signoff
  outcome shipped:   type String, route success
  outcome escalated: type String, route failure

type Signoff { reviewer: String }

step hold
  wait signoff timeout 2d -> decision
  outcome approved: when decision is present, route shipped(decision.reviewer)
  outcome lapsed: otherwise, route escalated("lapsed: " + change_id)"#,
];

/// `sleep`: a cooldown, and a timer between polls inside a loop.
pub(crate) const SLEEP: &[&str] = &[
    r"//! Sleep on a durable timer; the run survives a restart mid-sleep.
workflow cool_down
  input build_id: String
  outcome done: type String, route success

step pause
  sleep 30s
  route done(build_id)",
    r"//! Sleep between polls: a durable timer inside the loop body.
workflow await_export
  input job: String
  outcome done:  type Status, route success
  outcome stuck: type String, route failure

type Status { finished: Bool }

worker exports
  action poll(job: String, prior: Status) -> Status

step watch
  loop status = Status(finished: false)
    sleep 30s
    poll(job: job, prior: status) -> status
    until status.finished
    max 20
  outcome finished: when status.finished, route done(status)
  outcome timed_out: otherwise, route stuck(job)",
];

/// `on`: compensation to a failure outcome, and back into the graph.
pub(crate) const ON: &[&str] = &[
    r#"//! `on failure` runs when the step faults, and must end in a route.
workflow publish_package
  input name: String
  outcome done:   type String, route success
  outcome failed: type String, route failure

worker registry
  action publish(name: String) -> Nil
  action yank(name: String) -> Nil

step publish
  publish(name: name)
  route done(name)

  on failure
    yank(name: name)
    route failed("publish failed; yanked")"#,
    r#"//! Compensation may route back into the graph, not only to an outcome.
workflow deploy_with_rollback
  input build: String
  outcome done:   type String, route success
  outcome failed: type String, route failure

worker ops
  action deploy(build: String) -> Nil
  action rollback(build: String) -> Nil
  action report(build: String) -> Nil

step deploy
  deploy(build: build)
  route done(build)

  on failure
    rollback(build: build)
    route notify

step notify
  report(build: build)
  route failed("deploy failed; rolled back: " + build)"#,
];
