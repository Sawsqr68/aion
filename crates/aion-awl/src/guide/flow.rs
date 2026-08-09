//! The words that move a run through its graph: steps, branches, loops,
//! waits, routes, and the two directions of signalling.

use super::entry::{
    BOUNDED_ITERATION, FAILURE_HANDLING, GuideEntry, OUTCOMES_AND_ROUTES, PARALLEL_WORK,
    QUERY_SURFACE, SENDING_SIGNALS, STEPS_AND_CALLS, WAITS_AND_TIMERS, WORKFLOW_HEADER, WordKind,
};

/// `step`.
pub(crate) const STEP: GuideEntry = GuideEntry::new(
    "step",
    WordKind::Reserved,
    "Declares one node of the step graph; the statements inside it run in written order.",
    STEPS_AND_CALLS,
);

/// `after`.
pub(crate) const AFTER: GuideEntry = GuideEntry::new(
    "after",
    WordKind::Reserved,
    "Names the steps a step waits for, so the dependency is written rather than inferred from position.",
    STEPS_AND_CALLS,
);

/// `fork`.
pub(crate) const FORK: GuideEntry = GuideEntry::new(
    "fork",
    WordKind::Reserved,
    "Runs work concurrently inside one step: `fork item in items` one branch per item, or a bare `fork` for named branches.",
    PARALLEL_WORK,
);

/// `join`.
pub(crate) const JOIN: GuideEntry = GuideEntry::new(
    "join",
    WordKind::Reserved,
    "Closes a `fork`, binding a collection fork's results in input order.",
    PARALLEL_WORK,
);

/// `sequential`.
pub(crate) const SEQUENTIAL: GuideEntry = GuideEntry::new(
    "sequential",
    WordKind::Reserved,
    "Runs a collection `fork` one item at a time in input order instead of in parallel.",
    PARALLEL_WORK,
);

/// `distribute`.
pub(crate) const DISTRIBUTE: GuideEntry = GuideEntry::new(
    "distribute",
    WordKind::Reserved,
    "Opens a multi-step region that runs one item track per element in parallel.",
    PARALLEL_WORK,
);

/// `sequence`.
pub(crate) const SEQUENCE: GuideEntry = GuideEntry::new(
    "sequence",
    WordKind::Reserved,
    "Opens a multi-step region that runs one item track at a time in collection order.",
    PARALLEL_WORK,
);

/// `collect`.
pub(crate) const COLLECT: GuideEntry = GuideEntry::new(
    "collect",
    WordKind::Reserved,
    "Closes the nearest region as the first statement of its close step, binding results in input order.",
    PARALLEL_WORK,
);

/// `in`.
pub(crate) const IN: GuideEntry = GuideEntry::new(
    "in",
    WordKind::Positional,
    "Names the per-item binding of a fork or a region opener, as in `fork item in items`.",
    PARALLEL_WORK,
);

/// `loop`.
pub(crate) const LOOP: GuideEntry = GuideEntry::new(
    "loop",
    WordKind::Reserved,
    "Runs a bounded value-threading loop whose body runs at least once and must rebind the threaded value.",
    BOUNDED_ITERATION,
);

/// `counting`.
pub(crate) const COUNTING: GuideEntry = GuideEntry::new(
    "counting",
    WordKind::Reserved,
    "Binds the number of completed iterations of a `loop`.",
    BOUNDED_ITERATION,
);

/// `until`.
pub(crate) const UNTIL: GuideEntry = GuideEntry::new(
    "until",
    WordKind::Reserved,
    "States the condition that ends a `loop`, tested after the body has run.",
    BOUNDED_ITERATION,
);

/// `max`.
pub(crate) const MAX: GuideEntry = GuideEntry::new(
    "max",
    WordKind::Reserved,
    "Bounds iteration: the mandatory ceiling on a `loop`, or `max N visits` on a step inside a flow loop.",
    BOUNDED_ITERATION,
);

/// `visits`.
pub(crate) const VISITS: GuideEntry = GuideEntry::new(
    "visits",
    WordKind::Reserved,
    "Completes a step's `max N visits` bound, and reads as the visit count inside that step's guards.",
    BOUNDED_ITERATION,
);

/// `wait`.
pub(crate) const WAIT: GuideEntry = GuideEntry::new(
    "wait",
    WordKind::Reserved,
    "Waits for a declared signal and binds it; a `timeout` makes that binding optional, because expiry is absence.",
    WAITS_AND_TIMERS,
);

/// `sleep`.
pub(crate) const SLEEP: GuideEntry = GuideEntry::new(
    "sleep",
    WordKind::Reserved,
    "Sleeps for a duration on a durable timer.",
    WAITS_AND_TIMERS,
);

/// `on`.
pub(crate) const ON: GuideEntry = GuideEntry::new(
    "on",
    WordKind::Reserved,
    "Opens `on failure`, the block a step runs when it faults, which must itself end in a route.",
    FAILURE_HANDLING,
);

/// `outcome`.
pub(crate) const OUTCOME: GuideEntry = GuideEntry::new(
    "outcome",
    WordKind::Reserved,
    "Declares a terminal payload in the workflow header, and a guarded arm inside a step body.",
    OUTCOMES_AND_ROUTES,
);

/// `when`.
pub(crate) const WHEN: GuideEntry = GuideEntry::new(
    "when",
    WordKind::Reserved,
    "Guards an outcome arm; the first true guard wins, and its proofs narrow optionals through that arm.",
    OUTCOMES_AND_ROUTES,
);

/// `otherwise`.
pub(crate) const OTHERWISE: GuideEntry = GuideEntry::new(
    "otherwise",
    WordKind::Reserved,
    "The complement arm of a guarded outcome set, which must come last.",
    OUTCOMES_AND_ROUTES,
);

/// `route`.
pub(crate) const ROUTE: GuideEntry = GuideEntry::new(
    "route",
    WordKind::Reserved,
    "Sends flow to another step or to a declared outcome, supplying that outcome's payload.",
    OUTCOMES_AND_ROUTES,
);

/// `success`.
pub(crate) const SUCCESS: GuideEntry = GuideEntry::new(
    "success",
    WordKind::Reserved,
    "Maps a declared outcome to the engine's completed status.",
    WORKFLOW_HEADER,
);

/// `failure`.
pub(crate) const FAILURE: GuideEntry = GuideEntry::new(
    "failure",
    WordKind::Reserved,
    "Maps a declared outcome to the engine's failed status, and after `on` names a step's compensation block.",
    WORKFLOW_HEADER,
);

/// `send`.
pub(crate) const SEND: GuideEntry = GuideEntry::new(
    "send",
    WordKind::Positional,
    "Declares an outbound signal in the header, and in a step body sends it to another running workflow.",
    SENDING_SIGNALS,
);

/// `to`.
pub(crate) const TO: GuideEntry = GuideEntry::new(
    "to",
    WordKind::Positional,
    "Addresses the target of a `send`: a workflow id, which is a `String`.",
    SENDING_SIGNALS,
);

/// `query`.
pub(crate) const QUERY: GuideEntry = GuideEntry::new(
    "query",
    WordKind::Positional,
    "Declares what an operator may ask a running instance, and the type of the answer they receive.",
    QUERY_SURFACE,
);

/// `answer`.
pub(crate) const ANSWER: GuideEntry = GuideEntry::new(
    "answer",
    WordKind::Positional,
    "Registers the value a declared query returns, computed from the bindings in scope where it stands.",
    QUERY_SURFACE,
);
