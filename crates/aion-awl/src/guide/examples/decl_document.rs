//! Examples for the document-level words: the header, types, and constants.

/// `workflow`: the opening declaration, and `workflow.id` as this run's name.
pub(crate) const WORKFLOW: &[&str] = &[
    r#"//! Greet whoever starts the run.
workflow greet_visitor
  input name: String
  outcome done: type String, route success

step greet
  "hello " + name -> greeting
  route done(greeting)"#,
    r"//! `workflow.id` names this very run — here handed to a coordinator.
workflow register_run
  input coordinator_id: String
  send registered: String
  outcome done: type String, route success

step register
  send registered(workflow.id) to coordinator_id
  route done(workflow.id)",
];

/// `input`: scalar fields, and structured record/list fields.
pub(crate) const INPUT: &[&str] = &[
    r#"//! Each `input` line declares one field of the start contract.
workflow open_ticket
  input title: String
  input urgent: Bool
  outcome done: type String, route success

step file
  outcome fast: when urgent, route done("expedited: " + title)
  outcome slow: otherwise, route done("queued: " + title)"#,
    r#"//! Inputs can be records and lists, typed like any other value.
workflow plan_rollout
  input change: Change
  input hosts: [String]
  outcome done: type String, route success

type Change { title: String }

step plan
  hosts |> count -> planned
  outcome big: when planned > 3, route done("staged rollout of " + change.title)
  outcome small: otherwise, route done("direct rollout of " + change.title)"#,
];

/// `signal`: one declared signal, and a document that receives two.
pub(crate) const SIGNAL: &[&str] = &[
    r#"//! Hold until the declared approval signal arrives.
workflow release_gate
  input build_id: String
  signal approval: Approval
  outcome done: type String, route success

type Approval { reviewer: String }

step hold
  wait approval -> decision
  route done(decision.reviewer + " approved " + build_id)"#,
    r#"//! Two declared signals: a review verdict, then a go-ahead to ship.
workflow two_gate_release
  input build_id: String
  signal verdict: Verdict
  signal go: Go
  outcome done: type String, route success

type Verdict { approved: Bool }
type Go      { operator: String }

step review
  wait verdict -> review_verdict

step ship
  wait go -> clearance
  outcome shipped: when review_verdict.approved, route done(clearance.operator)
  outcome held: otherwise, route done("held: " + build_id)"#,
];

/// `type`: a record and an enum, and a documented record with an optional.
pub(crate) const TYPE: &[&str] = &[
    r"//! Declare an enum and a record, then build one for the outcome.
workflow triage_ticket
  input note: String
  outcome done: type Triage, route success

type Category = Urgent | Routine
type Triage { category: Category, note: String }

step judge
  route done(category: Routine, note: note)",
    r"//! `///` documents a declaration; optional fields are `T?`.
workflow file_finding
  input title: String
  outcome done: type Finding, route success

/// One reviewer finding.
type Finding {
  title: String,
  /// Absent until a reviewer writes one.
  note: String?,
}

step file
  route done(title: title)",
];

/// `schema`: the inline door, constant-position derivation, and the import
/// door. The imported path resolves beside the document; the examples gate
/// supplies a root carrying exactly that file, the same way `aion awl check`
/// resolves imports beside the checked file.
pub(crate) const SCHEMA: &[&str] = &[
    r#"//! An inline schema door names a JSON shape as an AWL type.
workflow ingest_payload
  input raw: Payload
  outcome done: type Payload, route success

type Payload = schema {
  "type": "object",
  "required": ["value"],
  "properties": { "value": { "type": "string" } }
}

step accept
  route done(raw)"#,
    r"//! `schema of Type` in constant position, handed to an agent action.
workflow structured_reply
  input question: String
  outcome done: type Answer, route success

const answer_schema = schema of Answer

type Answer { reply: String }

worker assistants
  action ask(prompt: String, output_schema: String) -> Answer

step ask
  ask(prompt: question, output_schema: answer_schema) -> result
  route done(result)",
    r#"//! Import a schema file as a type; the path resolves beside the document.
workflow ingest_existing
  input raw: Existing
  outcome done: type Existing, route success

type Existing = schema("schemas/existing.schema.json")

step accept
  route done(raw)"#,
];

/// `const`: a one-line literal, and a multi-line instruction block.
pub(crate) const CONST: &[&str] = &[
    r#"//! A compile-time constant, usable anywhere a value is.
workflow announce_poll
  input feed: String
  outcome done: type String, route success

const prefix = "polling: "

step announce
  route done(prefix + feed)"#,
    r#"//! A multi-line constant: instructions for an agent seam.
workflow brief_agent
  input goal: String
  outcome done: type String, route success

const instructions = """
Work from repository evidence only.
Return one paragraph.
"""

type Reply { text: String, stop_reason: String }

worker assistants
  action ask(prompt: String) -> Reply
    agent, timeout 10m

step ask
  ask(prompt: instructions + goal) -> reply
  route done(reply.text)"#,
];

/// `json`: the literal body in constant position, and the capture selector
/// in a worker document's `run json "…"` body.
pub(crate) const JSON: &[&str] = &[
    r#"//! `json { … }` opens a literal JSON body in constant position.
workflow request_review
  input diff: String
  outcome done: type String, route success

const output_schema = json { "type": "string" }

worker reviewers
  action review(prompt: String, contract: String) -> String

step review
  review(prompt: diff, contract: output_schema) -> verdict
  route done(verdict)"#,
    r#"//! `run json` names the capture selector: stdout parses into the type.
worker probes
  action host_report(host: String) -> Report
    run json "host-tool --json --host $host"

type Report { cpu_count: Int, host: String }"#,
];

/// `of`: the subject of `schema of Type`, its one form.
pub(crate) const OF: &[&str] = &[
    r"//! `schema of Type` derives a schema constant from a declared type.
workflow plan_items
  input goal: String
  outcome done: type Plan, route success

const plan_schema = schema of Plan

type Plan { items: [String] }

worker agents
  action plan(prompt: String, output_schema: String) -> Plan

step plan
  plan(prompt: goal, output_schema: plan_schema) -> result
  route done(result)",
];

/// `true`: the Bool literal, its one honest form. Every position a `true`
/// can stand in — a construction field, a call argument, a guard — is the
/// same value position, so a second document would be the first with the
/// names changed.
pub(crate) const TRUE: &[&str] = &[r"//! `true` is the Bool literal for truth.
workflow flag_finding
  input title: String
  outcome done: type Finding, route success

type Finding { title: String, blocking: Bool }

step flag
  route done(title: title, blocking: true)"];

/// `false`: in record construction, and seeding a loop's threaded value.
pub(crate) const FALSE: &[&str] = &[
    r"//! `false` is the Bool literal for falsity.
workflow clear_finding
  input title: String
  outcome done: type Finding, route success

type Finding { title: String, blocking: Bool }

step clear
  route done(title: title, blocking: false)",
    r"//! `false` seeds the loop's threaded value before the first pass.
workflow chase_approval
  input request: String
  outcome done:  type Verdict, route success
  outcome stuck: type String,  route failure

type Verdict { approved: Bool }

worker desks
  action press(request: String, prior: Verdict) -> Verdict

step chase
  loop verdict = Verdict(approved: false)
    press(request: request, prior: verdict) -> verdict
    until verdict.approved
    max 3
  outcome approved: when verdict.approved, route done(verdict)
  outcome refused: otherwise, route stuck(request)",
];
