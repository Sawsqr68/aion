//! Examples for outcomes, routes, terminal statuses, and the two directions
//! of signalling plus the query surface.

/// `outcome`: header terminals with guarded arms, and a subflow's own.
pub(crate) const OUTCOME: &[&str] = &[
    r#"//! `outcome` declares terminals in the header and guarded arms in a step.
workflow triage_alert
  input urgent: Bool
  outcome paged:  type String, route success
  outcome logged: type String, route success

step decide
  outcome page: when urgent, route paged("paged the operator")
  outcome log: otherwise, route logged("logged for morning")"#,
    r"//! A subflow declares exactly one typed outcome of its own.
workflow audit_repo
  input repo: String
  outcome done: type Audit, route success

type Audit { clean: Bool }

worker auditors
  action scan(target: String) -> Audit

subflow audit_once(target: String)
  outcome out: type Audit

  step look
    scan(target: target) -> audit
    route out(audit)

step audit
  audit_once(target: repo) -> audit

step finish
  audit |> route done",
];

/// `when`: a presence-proof guard, and total coverage of an enum.
pub(crate) const WHEN: &[&str] = &[
    r#"//! The first true `when` wins; its proofs hold through the arm.
workflow admit_finding
  input finding: Finding
  outcome done: type String, route success

type Finding { blocking: Bool, detail: String? }

step judge
  outcome real: when finding.detail is present, route done(finding.detail)
  outcome noise: otherwise, route done("no detail recorded")"#,
    r#"//! Guards over every enum variant are total; no `otherwise` needed.
workflow route_ticket
  input ticket: Ticket
  outcome done: type String, route success

type Category = Urgent | Routine | Spam
type Ticket { category: Category }

step route_it
  outcome page: when ticket.category == Urgent, route done("page")
  outcome queue: when ticket.category == Routine, route done("queue")
  outcome bin: when ticket.category == Spam, route done("discard")"#,
];

/// `otherwise`: the complement arm, its one form.
pub(crate) const OTHERWISE: &[&str] = &[
    r#"//! `otherwise` is the complement arm, and must come last.
workflow rate_change
  input risky: Bool
  outcome done: type String, route success

step rate
  outcome careful: when risky, route done("hold for review")
  outcome fine: otherwise, route done("ship it")"#,
];

/// `route`: forward to a step or outcome, and backward to form a flow loop.
pub(crate) const ROUTE: &[&str] = &[
    r#"//! `route` sends flow to another step or to a declared outcome.
workflow escalate_alert
  input urgent: Bool
  outcome done: type String, route success

step assess
  outcome page: when urgent, route page_operator
  outcome log: otherwise, route done("logged")

step page_operator
  route done("paged")"#,
    r"//! A backward route makes a flow loop; the cycle carries a visit bound.
workflow retry_gate
  input repo: String
  outcome done:   type Verdict, route success
  outcome failed: type String,  route failure

type Verdict { ok: Bool }

worker ci
  action check(repo: String) -> Verdict

step attempt
  check(repo: repo) -> verdict

step judge
  outcome again: when not verdict.ok and visits < 3, route attempt
  outcome good: when verdict.ok, route done(verdict)
  outcome exhausted: otherwise, route failed(repo)
  max 3 visits",
];

/// `success`: the completed-status mapping, its one form.
pub(crate) const SUCCESS: &[&str] = &[
    r#"//! `route success` maps the outcome to the engine's completed status.
workflow finish_job
  input job: String
  outcome done: type String, route success

step wrap
  route done("finished " + job)"#,
];

/// `failure`: the failed-status mapping, and the word after `on`.
pub(crate) const FAILURE: &[&str] = &[
    r#"//! `route failure` marks the outcome that fails the run.
workflow validate_order
  input order_id: String
  input valid: Bool
  outcome accepted: type String, route success
  outcome rejected: type String, route failure

step validate
  outcome ok: when valid, route accepted(order_id)
  outcome bad: otherwise, route rejected("invalid order: " + order_id)"#,
    r#"//! After `on`, the same word names a step's compensation block.
workflow guard_import
  input dataset: String
  outcome done:   type String, route success
  outcome failed: type String, route failure

worker importers
  action import_rows(dataset: String) -> Nil

step import
  import_rows(dataset: dataset)
  route done(dataset)

  on failure
    route failed("import faulted: " + dataset)"#,
];

/// `send`: one record payload, and all three payload forms together.
pub(crate) const SEND: &[&str] = &[
    r#"//! Signal another running workflow, addressed by its durable id.
workflow report_progress
  input coordinator_id: String
  input item: String
  send progress: Progress
  outcome done: type String, route success

type Progress { item: String, state: String }

step announce
  send progress(item: item, state: "started") to coordinator_id
  route done(item)"#,
    r#"//! Three payload forms: a record, a bare value, and `Nil` with nothing.
workflow announce_gates
  input coordinator_id: String
  input item: String
  send heartbeat: Nil
  send progress: Progress
  send verdict: String
  outcome done: type String, route success

type Progress { item: String, state: String }

step announce
  send heartbeat to coordinator_id
  send progress(item: item, state: "gated") to coordinator_id
  send verdict("ok: " + item) to coordinator_id
  route done(item)"#,
];

/// `to`: an input as the target, and this run's own id.
pub(crate) const TO: &[&str] = &[
    r#"//! `to` addresses the target: a workflow id, which is a String.
workflow notify_parent
  input parent_id: String
  input item: String
  send finished: String
  outcome done: type String, route success

step notify
  send finished("done: " + item) to parent_id
  route done(item)"#,
    r"//! Any bound String can follow `to` — even this run's own id.
workflow self_note
  input note: String
  send reminder: String
  outcome done: type String, route success

step remind
  send reminder(note) to workflow.id
  route done(note)",
];

/// `query`: a String answer, and the composite wire shapes.
pub(crate) const QUERY: &[&str] = &[
    r#"//! `query` declares what an operator may ask a running instance.
workflow track_import
  input dataset: String
  query phase: String
  outcome done: type String, route success

step ingest
  answer phase("ingesting " + dataset)
  route done(dataset)"#,
    r"//! Query types are wire types: Nil, lists, and optionals all serve.
workflow watch_harvest
  input crop: String
  query heartbeat: Nil
  query notes: [String]
  query last_note: String?
  outcome done: type String, route success

step harvest
  answer heartbeat
  answer notes([crop])
  answer last_note(crop)
  route done(crop)",
];

/// `answer`: one registration, and re-registration inside a loop.
pub(crate) const ANSWER: &[&str] = &[
    r#"//! `answer` registers the value a declared query returns.
workflow watch_backup
  input volume: String
  query progress: String
  outcome done: type String, route success

step copy
  answer progress("copying " + volume)
  route done(volume)"#,
    r#"//! Re-answer inside the loop: live progress for whoever asks.
workflow long_review
  input item: String
  input max_passes: Int
  query status: String
  outcome done:  type Verdict, route success
  outcome stuck: type String,  route failure

type Verdict { approved: Bool }

worker desks
  action evaluate(item: String) -> Verdict

step review
  loop verdict = Verdict(approved: false)
    answer status("reviewing " + item)
    evaluate(item: item) -> verdict
    until verdict.approved
    max max_passes
  answer status("settled")
  outcome approved: when verdict.approved, route done(verdict)
  outcome exhausted: otherwise, route stuck(item)"#,
];
