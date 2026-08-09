//! The live describe projection: what a run is doing RIGHT NOW, in one shape.
//!
//! [`DescribeWorkflowResponse`](crate::DescribeWorkflowResponse) answers what
//! history records. This answers the mid-step question history alone cannot: a
//! run projected `Running` covers "an agent is three tool calls into step four"
//! and "step four was handed to a queue nobody serves in July", and an operator
//! (or an agent driving aion through a tool call) needs those told apart in one
//! read.
//!
//! # Every absence in this shape is TYPED
//!
//! The rule the whole module is built on: a reader must never be able to
//! mistake "the server cannot answer" for "the answer is nothing". So:
//!
//! - No current step is [`Option::None`] on [`DescribeLiveResponse::current_step`],
//!   which means the run is genuinely not inside an activity — not that the
//!   fold failed.
//! - A step that exists but was never dispatched has no [`LiveAttempt`], so it
//!   cannot report liveness, notes, or a transcript it structurally cannot have.
//! - The progress note distinguishes reported / none-sent / unavailable
//!   ([`HeartbeatNote`]), because notes are held in volatile per-process state:
//!   a restarted server holds none, and reporting that as "the worker has said
//!   nothing" would be a lie about a worker that may be talking continuously.
//! - A transcript stream the retention cap has closed says
//!   `retention_truncated`, so a reader seeing the last retained event knows
//!   the stream continued rather than that the agent went quiet.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    ActivityEvent, ActivityId, InterventionCapabilities, Payload, UnservedActivity, WorkflowSummary,
};

/// The one-call live view of a run.
///
/// `summary` carries the projected [`crate::WorkflowStatus`] — the status is not
/// repeated at the top level, because two copies of one projection in one
/// response is a disagreement waiting to be reported as a bug.
#[derive(Serialize, Deserialize, ts_rs::TS, Clone, Debug, PartialEq)]
pub struct DescribeLiveResponse {
    /// Workflow summary projected from authoritative history (carrying the
    /// projected status), when the workflow exists.
    pub summary: Option<WorkflowSummary>,
    /// The single step a reader is told the run is on: the open step whose last
    /// history event is the most recent. `None` when the run is not inside an
    /// activity at all (completed, failed, or blocked on a timer or signal).
    pub current_step: Option<CurrentStep>,
    /// EVERY open step, in the order the ordinals were opened.
    ///
    /// A fan-out has several steps open at once, and [`Self::current_step`]
    /// reports one of them. This list is how a reader sees the siblings rather
    /// than having to know they might exist.
    pub open_steps: Vec<OpenStep>,
    /// Every retained transcript stream of the workflow, annotated with the
    /// activity type it belongs to and whether it is the current attempt's.
    pub transcript_streams: Vec<TranscriptStreamHead>,
    /// Every activity this run's history records as dispatched and unterminated
    /// that the live fleet cannot currently serve.
    ///
    /// EMPTY is the healthy answer and the only healthy one — see
    /// [`UnservedActivity`], whose semantics this field shares exactly (it is
    /// the same projection `POST /workflows/describe` returns).
    pub unserved: Vec<UnservedActivity>,
}

/// One open step of a run: the ordinal, the address the engine stamped on it,
/// and where it has got to.
#[derive(Serialize, Deserialize, ts_rs::TS, Clone, Debug, PartialEq, Eq)]
pub struct OpenStep {
    /// Activity ordinal recorded in history.
    pub activity_id: ActivityId,
    /// Activity type the step runs.
    pub activity_type: String,
    /// Task queue stamped on the recorded `ActivityScheduled` — the address the
    /// dispatch really went to, never a re-resolution that could disagree.
    pub task_queue: String,
    /// Node affinity stamped on the recorded `ActivityScheduled`, if any.
    pub node: Option<String>,
    /// `recorded_at` of the `ActivityScheduled` that opened the ordinal.
    pub scheduled_at: DateTime<Utc>,
    /// How far the step has got in the recorded lifecycle.
    pub state: StepState,
}

/// How far an open step has got — the lifecycle position history records.
#[derive(Serialize, Deserialize, ts_rs::TS, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "state")]
pub enum StepState {
    /// Scheduled by workflow code; this segment records no dispatch for it yet.
    Scheduled,
    /// The engine DISPATCHED this attempt to its task queue.
    ///
    /// This is not proof a worker took the work: `ActivityStarted` is recorded
    /// at dispatch, before worker selection. Whether anyone is actually working
    /// on it is [`LiveAttempt::liveness`], and whether anyone COULD is
    /// [`DescribeLiveResponse::unserved`].
    Dispatched {
        /// One-based delivery attempt recorded on the `ActivityStarted`.
        attempt: u32,
        /// When the ENGINE dispatched — NOT when a worker took it.
        dispatched_at: DateTime<Utc>,
    },
    /// An operator reopen superseded this ordinal's recorded terminal; it is
    /// open again and awaiting re-dispatch on replay.
    Reopened {
        /// `recorded_at` of the `WorkflowReopened` that re-opened the ordinal.
        reopened_at: DateTime<Utc>,
    },
}

/// The current step plus, when it has been dispatched, everything live about
/// the attempt.
#[derive(Serialize, Deserialize, ts_rs::TS, Clone, Debug, PartialEq)]
pub struct CurrentStep {
    /// The step itself, as history records it.
    pub step: OpenStep,
    /// The live view of the dispatched attempt.
    ///
    /// `None` when the step is [`StepState::Scheduled`] or
    /// [`StepState::Reopened`]: there is no attempt yet, so there is nothing
    /// that could be live, could have sent a note, or could have a transcript.
    /// That is a structural absence, not a missing measurement.
    pub attempt: Option<LiveAttempt>,
}

/// Everything the server can say about the dispatched attempt of the current
/// step, right now.
#[derive(Serialize, Deserialize, ts_rs::TS, Clone, Debug, PartialEq)]
pub struct LiveAttempt {
    /// One-based delivery attempt this view describes.
    pub attempt: u32,
    /// When the engine dispatched this attempt.
    pub dispatched_at: DateTime<Utc>,
    /// Whether a connected worker owns the attempt right now.
    pub liveness: AttemptLiveness,
    /// The most recent worker progress note, or a typed reason there is none.
    pub note: HeartbeatNote,
    /// The bounded tail of the attempt's retained transcript.
    pub transcript: TranscriptTail,
}

/// Whether a connected worker owns an attempt right now.
///
/// Read from the live attempt→owner index the intervention router resolves
/// against — the SAME source `POST /workflows/attempts` enumerates, so what a
/// reader is told here and what an intervention would find cannot disagree.
#[derive(Serialize, Deserialize, ts_rs::TS, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "liveness")]
pub enum AttemptLiveness {
    /// A connected worker owns the attempt and advertises these primitives.
    ///
    /// An EMPTY capability set is first-class: an observability-only harness
    /// owns its attempt and supports no controls.
    Live {
        /// The owning worker's advertised intervention capabilities.
        capabilities: InterventionCapabilities,
    },
    /// No connected worker owns the attempt.
    ///
    /// The attempt finished, was superseded, was never leased, or its owner has
    /// disconnected. Paired with a `Dispatched` step and an empty `unserved`,
    /// this is the shape of work in the hand-off window between dispatch and
    /// lease.
    NoLiveOwner,
}

/// The most recent progress note the worker serving an attempt has reported —
/// or a typed statement of why there is none.
///
/// **Notes are VOLATILE.** They live in the server process's per-attempt
/// liveness state and are never written to the store, so a restart loses every
/// note ever reported. That is why absence has two forms and they are never
/// collapsed: [`Self::NoneSent`] is a measurement (this process is holding the
/// attempt and nothing has been reported on it), while [`Self::Unavailable`] is
/// the absence of a measurement (this process does not hold the attempt, so it
/// cannot say what was reported).
#[derive(Serialize, Deserialize, ts_rs::TS, Clone, Debug, PartialEq)]
#[serde(tag = "note")]
pub enum HeartbeatNote {
    /// The worker serving this attempt reported this note.
    Reported {
        /// The worker's opaque progress payload, exactly as it was sent.
        payload: Payload,
        /// When this server process received the reporting heartbeat.
        reported_at: DateTime<Utc>,
    },
    /// This server process holds the attempt's liveness entry and no note has
    /// been reported on it. The worker really has said nothing.
    NoneSent,
    /// This server process does not hold the attempt's liveness entry, so it
    /// cannot say whether a note was reported.
    ///
    /// The usual cause is a restart: notes began being held at
    /// `notes_held_since`, and an attempt dispatched before that instant left
    /// its notes in a process that no longer exists —
    /// `restarted_since_attempt_began` says so outright. The other causes (the
    /// attempt was swept off the tracker with its owner, or it has not been
    /// leased) are equally unmeasurable here, and every one of them is reported
    /// as this variant rather than as silence.
    Unavailable {
        /// When this server process began holding progress notes.
        notes_held_since: DateTime<Utc>,
        /// True when the attempt was dispatched BEFORE this process began
        /// holding notes — a restart demonstrably explains the absence.
        restarted_since_attempt_began: bool,
    },
}

/// The bounded tail of one attempt's retained transcript.
#[derive(Serialize, Deserialize, ts_rs::TS, Clone, Debug, PartialEq)]
#[serde(tag = "transcript")]
pub enum TranscriptTail {
    /// The caller did not ask for a tail (no `tail` in the request), so none
    /// was read. Nothing is claimed about what the stream holds.
    NotRequested,
    /// The last records of the attempt's retained transcript.
    Window {
        /// The retained events, in `store_seq` order, each carrying its
        /// `store_seq`. Empty when the stream has nothing retained.
        events: Vec<ActivityEvent>,
        /// Next durable `store_seq` for the stream at read time.
        head_seq: u64,
        /// Retained records BEFORE this window, omitted by the requested bound.
        /// Read the full stream with `POST /workflows/transcript`.
        omitted_before: u64,
        /// The retention cap has closed this stream: the cap marker is the last
        /// durable record and every later event is live-only, never persisted.
        ///
        /// A reader seeing the last retained event on a truncated stream is
        /// seeing the end of the RETENTION, not the end of the agent's output.
        retention_truncated: bool,
    },
}

/// One retained transcript stream of the workflow, annotated so a reader can
/// tell whose it is and whether it is the one still being written.
#[derive(Serialize, Deserialize, ts_rs::TS, Clone, Debug, PartialEq, Eq)]
pub struct TranscriptStreamHead {
    /// The activity within the workflow.
    pub activity_id: ActivityId,
    /// The attempt — the third stream axis. Two attempts of one activity are
    /// DISTINCT streams.
    pub attempt: u32,
    /// The activity type this stream belongs to, resolved from the run's own
    /// history. `None` when the run's history records no scheduling for the
    /// ordinal (a stream retained from a segment this history no longer holds).
    pub activity_type: Option<String>,
    /// Next `store_seq` to be written == count of retained records.
    pub head_seq: u64,
    /// Whether this stream is the current step's dispatched attempt.
    pub current: bool,
    /// The retention cap has closed this stream (see
    /// [`TranscriptTail::Window::retention_truncated`]).
    pub retention_truncated: bool,
}
