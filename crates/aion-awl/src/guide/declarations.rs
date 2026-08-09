//! The words that declare things: the document, its types, its workers, and
//! the children it calls.
//!
//! Every entry carries complete example documents from [`super::examples`],
//! verified against the real parse and check pipeline by the guide tests.

use super::entry::{
    CHILDREN_AND_SUBFLOWS, DOCUMENT_STRUCTURE, GuideEntry, VALUES_AND_TYPES, WORKERS_AND_ACTIONS,
    WORKFLOW_HEADER, WordKind,
};
use super::examples;

/// `workflow`.
pub(crate) const WORKFLOW: GuideEntry = GuideEntry::new(
    "workflow",
    WordKind::Reserved,
    "Opens the document: one `.awl` file declares exactly one workflow, named here.",
    DOCUMENT_STRUCTURE,
    examples::WORKFLOW,
);

/// `input`.
pub(crate) const INPUT: GuideEntry = GuideEntry::new(
    "input",
    WordKind::Reserved,
    "Declares one field of the workflow's start contract; it is repeatable.",
    WORKFLOW_HEADER,
    examples::INPUT,
);

/// `signal`.
pub(crate) const SIGNAL: GuideEntry = GuideEntry::new(
    "signal",
    WordKind::Reserved,
    "Declares a durable signal this workflow receives; a step body takes delivery with `wait`.",
    WORKFLOW_HEADER,
    examples::SIGNAL,
);

/// `type`.
pub(crate) const TYPE: GuideEntry = GuideEntry::new(
    "type",
    WordKind::Reserved,
    "Declares a record, an enum, or a schema door, and names the type an `outcome` carries.",
    VALUES_AND_TYPES,
    examples::TYPE,
);

/// `schema`.
pub(crate) const SCHEMA: GuideEntry = GuideEntry::new(
    "schema",
    WordKind::Reserved,
    "Opens a schema door: `schema(\"path\")` imports one, `schema { … }` inlines one, and `schema of Type` derives one in constant position.",
    VALUES_AND_TYPES,
    examples::SCHEMA,
);

/// `const`.
pub(crate) const CONST: GuideEntry = GuideEntry::new(
    "const",
    WordKind::Reserved,
    "Declares a compile-time constant, which may never depend on a runtime binding.",
    VALUES_AND_TYPES,
    examples::CONST,
);

/// `json`.
pub(crate) const JSON: GuideEntry = GuideEntry::new(
    "json",
    WordKind::Reserved,
    "Opens a literal JSON body, as in `json { \"type\": \"string\" }`, and names the JSON capture selector in a worker document's `run json \"…\"` body — two distinct places.",
    VALUES_AND_TYPES,
    examples::JSON,
);

/// `of`.
pub(crate) const OF: GuideEntry = GuideEntry::new(
    "of",
    WordKind::Reserved,
    "Names the subject of `schema of Type`, the constant-position schema derivation.",
    VALUES_AND_TYPES,
    examples::OF,
);

/// `true`.
pub(crate) const TRUE: GuideEntry = GuideEntry::new(
    "true",
    WordKind::Reserved,
    "The `Bool` literal for truth.",
    VALUES_AND_TYPES,
    examples::TRUE,
);

/// `false`.
pub(crate) const FALSE: GuideEntry = GuideEntry::new(
    "false",
    WordKind::Reserved,
    "The `Bool` literal for falsity.",
    VALUES_AND_TYPES,
    examples::FALSE,
);

/// `worker`.
pub(crate) const WORKER: GuideEntry = GuideEntry::new(
    "worker",
    WordKind::Reserved,
    "Declares a typed worker contract whose name is the task queue; it is a requirement, never an implementation.",
    WORKERS_AND_ACTIONS,
    examples::WORKER,
);

/// `action`.
pub(crate) const ACTION: GuideEntry = GuideEntry::new(
    "action",
    WordKind::Reserved,
    "Declares one wire contract on a worker: named parameters, one return type, and at most one config line.",
    WORKERS_AND_ACTIONS,
    examples::ACTION,
);

/// `node`.
pub(crate) const NODE: GuideEntry = GuideEntry::new(
    "node",
    WordKind::Reserved,
    "Pins dispatch to a named worker node, on the action declaration or overridden at a call site.",
    WORKERS_AND_ACTIONS,
    examples::NODE,
);

/// `retry`.
pub(crate) const RETRY: GuideEntry = GuideEntry::new(
    "retry",
    WordKind::Reserved,
    "Sets an action's retry policy, as `retry N every DURATION` or `retry N backoff MIN..MAX`.",
    WORKERS_AND_ACTIONS,
    examples::RETRY,
);

/// `every`.
pub(crate) const EVERY: GuideEntry = GuideEntry::new(
    "every",
    WordKind::Reserved,
    "Sets the fixed interval between retries in `retry N every DURATION`.",
    WORKERS_AND_ACTIONS,
    examples::EVERY,
);

/// `backoff`.
pub(crate) const BACKOFF: GuideEntry = GuideEntry::new(
    "backoff",
    WordKind::Reserved,
    "Sets the growing retry interval in `retry N backoff MIN..MAX`.",
    WORKERS_AND_ACTIONS,
    examples::BACKOFF,
);

/// `advisory`.
pub(crate) const ADVISORY: GuideEntry = GuideEntry::new(
    "advisory",
    WordKind::Reserved,
    "Marks an action whose failure never faults its calling step; it can bind no result and be no pipe stage.",
    WORKERS_AND_ACTIONS,
    examples::ADVISORY,
);

/// `agent`.
pub(crate) const AGENT: GuideEntry = GuideEntry::new(
    "agent",
    WordKind::Reserved,
    "Marks an action as an agent seam: its one `String` parameter carries a prompt and its `String` result the reply.",
    WORKERS_AND_ACTIONS,
    examples::AGENT,
);

/// `run`.
pub(crate) const RUN: GuideEntry = GuideEntry::new(
    "run",
    WordKind::Positional,
    "Gives an action a declared command body, which the server runs itself with no sandbox of any kind.",
    WORKERS_AND_ACTIONS,
    examples::RUN,
);

/// `child`.
pub(crate) const CHILD: GuideEntry = GuideEntry::new(
    "child",
    WordKind::Reserved,
    "Declares a separate durable workflow this one may call and await.",
    CHILDREN_AND_SUBFLOWS,
    examples::CHILD,
);

/// `spawn`.
pub(crate) const SPAWN: GuideEntry = GuideEntry::new(
    "spawn",
    WordKind::Reserved,
    "Starts a child workflow without waiting for it, so it can bind no result.",
    CHILDREN_AND_SUBFLOWS,
    examples::SPAWN,
);

/// `subflow`.
pub(crate) const SUBFLOW: GuideEntry = GuideEntry::new(
    "subflow",
    WordKind::Reserved,
    "Declares an inline reusable graph with typed parameters and exactly one outcome, compiled into this workflow.",
    CHILDREN_AND_SUBFLOWS,
    examples::SUBFLOW,
);

/// `timeout`.
pub(crate) const TIMEOUT: GuideEntry = GuideEntry::new(
    "timeout",
    WordKind::Reserved,
    "Bounds the workflow run in the header, one activity on an action or call, or one `wait` — three distinct places.",
    WORKERS_AND_ACTIONS,
    examples::TIMEOUT,
);
