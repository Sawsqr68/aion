//! Durable operator rename (#211): record a display name.
//!
//! A display name is a LABEL over UUID identity, never an address — nothing
//! resolves a workflow by name. A rename is a RECORDED
//! [`Event::SearchAttributesUpdated`] carrying the `aion.display_name`
//! attribute, so history keeps every name that has been worn and the current
//! name is the last-write-wins fold ([`aion_core::display_name`]). Renaming
//! appends; it never rewrites the past.
//!
//! # A run is what you address; a workflow is what wears the name
//!
//! The write side takes a `(workflow, run)` pair and refuses a run whose name
//! could not land where the caller means it to (see the guards below). The READ
//! side is per-WORKFLOW: [`aion_core::display_name`] folds the attribute over
//! the whole history, so a continue-as-new successor INHERITS its predecessor's
//! name rather than starting unnamed. That asymmetry is deliberate and is why
//! `SearchAttributesUpdated` carrying no run id is safe to record only at the
//! workflow's current head — but it does mean "the run's display name" is a
//! loose way to speak about it, and the read-side docs say per-workflow.
//!
//! # Single-writer discipline
//!
//! Following pause (#204, `pause.rs`): a run with a REGISTERED handle appends
//! through that handle's own recorder — the single writer — under its lock, so
//! the append is serialised against the exit monitor and every other writer
//! through the same recorder. Only a run whose DURABLE STATUS says no live
//! recorder can exist — terminal, or `Paused` and therefore excluded from
//! respawn by design — builds a one-shot [`Recorder::resume_at`] at the durable
//! head, exactly the non-resident path pause's resume and the paused-signal
//! record use. A merely-unregistered run is NOT that: an unregistered `Running`
//! run is mid-birth or mid-recovery and its live recorder owns the head, so a
//! rename there is refused rather than raced.
//!
//! One thing rename must check that pause does not: the recorder and the
//! store's expected-sequence check are per-WORKFLOW, while
//! [`Event::SearchAttributesUpdated`] carries no run id and
//! [`aion_core::run_segment`] is positional — so a recorded name always lands
//! in the workflow's LAST run. A superseded run therefore cannot be renamed at
//! all; see the guard in [`rename`]. Never two writers; never
//! `EventStore::append` directly.
//!
//! # The continue-as-new window
//!
//! Supersession is not established the instant a run continues as new. A
//! continue-as-new is TWO appends by TWO recorders: the predecessor records
//! `WorkflowContinuedAsNew` through its own recorder and RELEASES that lock
//! (`continue_as_new.rs`), and only then does the successor's freshly minted
//! recorder read the head and append its `WorkflowStarted` — outside the
//! predecessor's lock, and before the predecessor is deregistered. In that
//! window the predecessor is still registered, still owns the head, and is not
//! yet superseded, so the positional guard above admits it: renaming it there
//! would append at the head the successor's recorder has ALREADY read as its
//! expected sequence, hard-failing the successor's `WorkflowStarted` and
//! stranding the workflow terminal-`ContinuedAsNew` with no successor run.
//!
//! What closes that window is the run's OWN `WorkflowContinuedAsNew`
//! ([`has_continued_as_new`]), checked twice: once before the lock over the
//! history read there, and again INSIDE the registered handle's recorder lock
//! over everything appended SINCE that read. The two checks partition the
//! history at the pre-lock head, so between them they cover all of it. The
//! in-lock check is the load-bearing one — the predecessor's
//! `WorkflowContinuedAsNew` is appended through that same recorder, so while
//! this rename holds the lock the event has either already landed (and the
//! delta re-read sees it) or cannot land until this rename is done, and a
//! continue-as-new that starts afterwards reads a head that already includes
//! this append. The pre-lock check cannot stand in for it: the pre-lock read
//! can precede the predecessor's own `WorkflowContinuedAsNew` append entirely.

use std::collections::HashMap;
use std::sync::Arc;

use aion_core::{
    Event, RunId, SearchAttributeSchema, SearchAttributeValue, WorkflowId, WorkflowStatus,
    run_segment, status_from_events,
};
use aion_store::EventStore;
use aion_store::visibility::VisibilityStore;
use chrono::Utc;

use crate::EngineError;
use crate::durability::Recorder;
use crate::registry::Registry;

/// Dependencies required to rename a workflow run.
///
/// Narrower than [`super::pause::PauseWorkflowContext`] because a rename never
/// respawns anything: it only appends one recorded event and refreshes
/// visibility, so no catalog, runtime, or supervision tree is involved.
pub struct RenameWorkflowContext<'a> {
    /// Durable event store used to read history and construct recorders.
    pub store: Arc<dyn EventStore>,
    /// Visibility store the recorder projects the run into.
    pub visibility_store: Arc<dyn VisibilityStore>,
    /// Active execution registry keyed by workflow/run identifiers.
    pub registry: &'a Arc<Registry>,
    /// Schema the new display name is validated against before any append.
    pub search_attribute_schema: Arc<SearchAttributeSchema>,
}

/// Records `display_name` as the workflow's current display name, addressed
/// by run (#211).
///
/// Addressed by run because which run is live decides whether the append is
/// safe; RECORDED per workflow because the event carries no run id and readers
/// fold it over the whole history (see the module docs).
///
/// Validates BEFORE any append: the trimmed name must be non-empty and the
/// `(id, run)` pair must have recorded history. There is no STATUS
/// precondition on the label itself — a completed run's name is as legitimate
/// as a running one's, and the recorded event is an ordinary
/// [`Event::SearchAttributesUpdated`] that status projections ignore. Status is
/// consulted only to choose the append path safely (see the module docs).
///
/// Returns the name exactly as recorded (trimmed).
///
/// # Errors
///
/// Returns [`EngineError::InvalidState`] when the trimmed name is empty (the
/// transport layers reject this earlier; this is the engine boundary's own
/// check); when the run has been superseded by a later run of the same
/// workflow, or has recorded its own `WorkflowContinuedAsNew` and so is about
/// to be (see the module docs) — either way the name would land on the
/// successor; or when the run is non-terminal, not `Paused`, and not resident
/// on this node — renaming it then would append behind its live writer, so it
/// is refused for the caller to retry. Returns [`EngineError::WorkflowNotFound`]
/// when no history exists for `(id, run)`, and [`EngineError::Durability`] when
/// the schema refuses the attribute or the store rejects the append. A
/// rejection appends nothing.
pub async fn rename(
    context: &RenameWorkflowContext<'_>,
    id: &WorkflowId,
    run: &RunId,
    display_name: &str,
) -> Result<String, EngineError> {
    // Validate the name first: nothing is read or appended for a blank rename.
    let display_name = display_name.trim();
    if display_name.is_empty() {
        return Err(EngineError::InvalidState {
            reason: format!("workflow {id} run {run} rename requires a non-empty display name"),
        });
    }
    // Validate against HISTORY before any append (the pause precedent): the
    // target run must exist durably.
    let history = context.store.read_history(id).await?;
    if history.is_empty() {
        return Err(crate::engine::api::workflow_not_found(id, run));
    }
    let segment = run_segment(&history, run);
    if segment.is_empty() {
        return Err(crate::engine::api::workflow_not_found(id, run));
    }
    // THE TARGET MUST BE THE WORKFLOW'S CURRENT RUN.
    //
    // [`Event::SearchAttributesUpdated`] carries no run id — unlike
    // `WorkflowPaused`/`WorkflowResumed`, which do — and [`run_segment`] is
    // POSITIONAL: it slices from a run's `WorkflowStarted` to the next one. So
    // an event appended at the workflow's head always lands in the LAST run's
    // segment, whichever run the caller named. Renaming a superseded run
    // (a continue-as-new predecessor, say) would therefore:
    //   * append behind the successor's LIVE recorder — a second writer, which
    //     hard-fails the successor's next append on the store's
    //     expected-sequence check;
    //   * record the operator's name onto the WRONG run while reporting
    //     success; and
    //   * write a visibility row for the named run carrying the successor's
    //     state.
    // A predecessor is terminal, so a status-only gate admits it. Refusing here
    // is the honest answer: a per-run name for a superseded run is not
    // representable until `SearchAttributesUpdated` carries a run id.
    if segment.last().map(Event::seq) != history.last().map(Event::seq) {
        return Err(EngineError::InvalidState {
            reason: format!(
                "workflow {id} run {run} has been superseded by a later run of the same \
                 workflow; a recorded display name would land on that later run, so only the \
                 current run can be renamed"
            ),
        });
    }
    // THE TARGET MUST NOT HAVE CONTINUED AS NEW.
    //
    // The positional guard above sees supersession only once the successor's
    // `WorkflowStarted` has landed, and a continue-as-new records its two
    // events through two different recorders with the predecessor's lock
    // released in between (see the module docs). A run that has recorded its
    // own `WorkflowContinuedAsNew` is therefore either already superseded
    // (caught above) or has a successor mid-birth that has already read the
    // head this rename would append to. Either way its label belongs to the
    // successor run, and this is the honest refusal. The same check runs again
    // under the recorder lock below, which is what actually closes the race —
    // this one refuses early, without taking a live run's lock, and is the ONLY
    // check on the one-shot path (a chain stranded by a crash between the two
    // appends has no registered handle at all).
    if has_continued_as_new(&history, run) {
        return Err(continued_as_new_refusal(id, run));
    }
    let attributes = HashMap::from([(
        aion_core::DISPLAY_NAME_ATTRIBUTE.to_owned(),
        SearchAttributeValue::String(display_name.to_owned()),
    )]);
    // The head this validation read saw. It bounds the in-lock re-read below,
    // and is the expected sequence the one-shot recorder resumes at.
    let head = history.last().map(Event::seq).unwrap_or_default();

    // A registered run (resident or suspended) owns the single-writer recorder;
    // append through THAT recorder, under its lock.
    //
    // RE-VALIDATE UNDER THE LOCK, exactly as pause and resume do: the checks
    // above ran against a history read taken before this lock was held, and a
    // continue-as-new through this same recorder can have landed in between.
    // The condition re-checked is the run's own `WorkflowContinuedAsNew` — NOT
    // the positional supersession test, which is still false in that window
    // (the predecessor is still the last segment until the successor's
    // `WorkflowStarted` lands, and that append is made by a different recorder
    // outside this lock, from a head it has ALREADY read). Appending here
    // against a run that has continued as new desyncs the successor's recorder
    // and strands the workflow with a recorded `WorkflowContinuedAsNew` and no
    // successor run — while reporting the rename a success.
    //
    // The re-read is a DELTA, not a second whole history: everything up to
    // `head` was already read and checked above, so only what landed after it
    // can change the answer, and `read_history_from` is a real range read on
    // every backend (never a read-all-then-filter). The whole-history version
    // of this check held the live run's recorder mutex across an O(history)
    // read — unbounded in a long-lived workflow — for facts it had already
    // established. What the lock must cover is the check-then-append being
    // atomic, and reading the delta under it does exactly that: the lock alone
    // is not enough, because what it serialises against is only writes THROUGH
    // this recorder, and the read that decided this rename was safe happened
    // before it.
    if let Some(handle) = context.registry.get(id, run)? {
        let recorder = handle.recorder();
        let mut recorder = recorder.lock().await;
        let appended_since = context
            .store
            .read_history_from(id, head.saturating_add(1))
            .await?;
        if has_continued_as_new(&appended_since, run) {
            return Err(continued_as_new_race_refusal(id, run));
        }
        recorder
            .record_search_attributes_updated(
                Utc::now(),
                attributes,
                &context.search_attribute_schema,
            )
            .await?;
        return Ok(display_name.to_owned());
    }

    // NO REGISTERED HANDLE. Building a one-shot recorder here is only safe when
    // no LIVE recorder can exist for this run, and "the registry has no handle"
    // does not establish that on its own: a run is also unregistered during its
    // birth window and while startup recovery is still re-registering it, and in
    // both of those a live recorder already owns the head. Appending a second
    // writer there would desync the live recorder and hard-fail its next append
    // — the exact double-writer bug the single-writer invariant exists to
    // prevent.
    //
    // So the one-shot path is gated on the DURABLE status, which is what
    // actually decides whether a live recorder can exist:
    //   * terminal — the run is finished; nothing holds a recorder;
    //   * Paused   — deliberately excluded from respawn, so it is unregistered
    //                BY DESIGN (the same posture `delegated.rs` records a
    //                paused-run signal under).
    // Anything else non-resident is transient, and the honest answer is to
    // refuse and let the caller retry rather than race a live writer.
    let status = status_from_events(segment);
    if !matches!(status, WorkflowStatus::Paused) && !status.is_terminal() {
        return Err(EngineError::InvalidState {
            reason: format!(
                "workflow {id} run {run} is {} but is not resident on this node, so renaming it \
                 now would append behind its live writer; retry once it is resident",
                super::pause::status_name(status)
            ),
        });
    }
    // A racing respawn (reopen of a terminal run, resume of a paused one) is
    // fenced by the store's expected-sequence check, never silently
    // double-written.
    //
    // So is the one concurrent writer that shares this exact path: a signal to
    // the SAME paused run, which `engine/delegated.rs` records through its own
    // `Recorder::resume_at` at the same durable head. Two one-shot recorders can
    // therefore hold the same expected sequence at once. That is a
    // correctly-refused race, not a double-writer bug: the store's
    // compare-and-set admits exactly one, the loser appends NOTHING, and the
    // history is left contiguous — neither writer ever proceeds on stale state.
    // The loser surfaces the underlying `SequenceConflict` as
    // `EngineError::Durability`, and a rename refused that way is safe to
    // retry. (Proven in `tests/rename_e2e.rs`, which holds the window open
    // rather than racing for it.)
    let mut recorder = Recorder::resume_at(id.clone(), Arc::clone(&context.store), head)
        .with_visibility(run.clone(), Arc::clone(&context.visibility_store));
    recorder
        .record_search_attributes_updated(Utc::now(), attributes, &context.search_attribute_schema)
        .await?;
    Ok(display_name.to_owned())
}

/// Whether `run` has recorded its own [`Event::WorkflowContinuedAsNew`].
///
/// Matched on the event's `parent_run_id` — the run that continued — rather
/// than on a [`run_segment`] slice, so the answer does not depend on the
/// positional slicing that the successor's not-yet-appended `WorkflowStarted`
/// is exactly what would fix.
fn has_continued_as_new(history: &[Event], run: &RunId) -> bool {
    history.iter().any(|event| {
        matches!(event, Event::WorkflowContinuedAsNew { parent_run_id, .. } if parent_run_id == run)
    })
}

/// What both continued-as-new refusals tell the operator to do about it. Shared
/// so the two checks cannot drift apart in their guidance.
const CONTINUED_AS_NEW_GUIDANCE: &str = "its successor run owns (or is about to own) the workflow head, so a recorded display name \
     would land behind the successor's own writer — rename the successor run instead";

/// The pre-lock refusal: the run had ALREADY continued as new when the rename
/// was asked for.
fn continued_as_new_refusal(id: &WorkflowId, run: &RunId) -> EngineError {
    EngineError::InvalidState {
        reason: format!(
            "workflow {id} run {run} has continued as new; {CONTINUED_AS_NEW_GUIDANCE}"
        ),
    }
}

/// The in-lock refusal: the run continued as new BETWEEN this rename's own
/// validation read and its append — the race the recorder lock exists to make
/// visible. Worded distinctly from [`continued_as_new_refusal`] because the two
/// are different facts about timing for whoever reads the error, and because a
/// refusal that cannot be attributed to the check that produced it cannot be
/// tested.
fn continued_as_new_race_refusal(id: &WorkflowId, run: &RunId) -> EngineError {
    EngineError::InvalidState {
        reason: format!(
            "workflow {id} run {run} continued as new while this rename was in flight; \
             {CONTINUED_AS_NEW_GUIDANCE}"
        ),
    }
}
