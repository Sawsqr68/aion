//! Server-side selector filtering for filtered subscriptions.
//!
//! `FilteredSubscription` advertises optional `workflow_type` and `status`
//! selectors. The engine's `EventFilter` has no type or status dimension, so
//! the selection runs at the socket seam, after the namespace gate proved
//! ownership and resolved the workflow's recorded type from the same durable
//! read.
//!
//! Selector semantics (documented in `docs/API.md`):
//!
//! - `workflow_type` matches when the event's workflow has that recorded type
//!   at the time the namespace gate resolved it: the initial durable read
//!   returns the head-of-history `WorkflowStarted` type at read time, which on
//!   a continue-as-new chain can briefly run ahead of an older delivered event
//!   (a one-event-loop forward-skew window) until the stream's own
//!   `WorkflowStarted` refresh self-heals the cached type. A workflow whose
//!   history records no started run never matches a type selector.
//! - `status` matches per event kind: each terminal lifecycle event matches
//!   exactly its projected status (`WorkflowCompleted` → `Completed`,
//!   `WorkflowFailed` → `Failed`, `WorkflowCancelled` → `Cancelled`,
//!   `WorkflowTimedOut` → `TimedOut`, `WorkflowContinuedAsNew` →
//!   `ContinuedAsNew`); every other LIFECYCLE event — including
//!   `WorkflowStarted` — matches `Running`. A LABEL-ONLY
//!   `SearchAttributesUpdated` (a rename) is not a lifecycle event (#211) and
//!   matches ANY status selector, because a workflow's display name changes in
//!   every status and both forcing it into `Running` and withholding it from
//!   the other buckets would be wrong. The `SearchAttributesUpdated` carrying a
//!   run's PLACEMENT is recorded atomically with `WorkflowStarted` and matches
//!   `Running` with it, so a terminal-status subscriber is not sent an
//!   attribute frame for every workflow starting in the namespace.
//! - When both selectors are present they AND together.

use aion_core::{Event, WorkflowStatus};

use crate::namespace::{NAMESPACE_ATTRIBUTE, TASK_QUEUE_ATTRIBUTE};

/// Validated subscription selectors applied before frame encoding.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SubscriptionSelector {
    /// Deliver only events of workflows with this recorded type.
    pub workflow_type: Option<String>,
    /// Deliver only events whose kind projects to this status.
    pub status: Option<WorkflowStatus>,
}

impl SubscriptionSelector {
    /// Selector that admits every event (per-workflow and firehose
    /// subscriptions carry no selectors).
    #[must_use]
    pub const fn unrestricted() -> Self {
        Self {
            workflow_type: None,
            status: None,
        }
    }

    /// Decide whether an event passes the selector. `workflow_type` is the
    /// event's workflow's recorded type as resolved by the namespace gate.
    #[must_use]
    pub fn matches(&self, event: &Event, workflow_type: Option<&str>) -> bool {
        if let Some(selected_type) = &self.workflow_type {
            // No recorded type (no started run) can never satisfy a type
            // selector — absence is not a wildcard.
            if workflow_type != Some(selected_type.as_str()) {
                return false;
            }
        }
        if let Some(selected_status) = self.status {
            // `None` is an event with NO lifecycle meaning (a rename): it
            // passes every status selector rather than being forced into one.
            if event_status(event).is_some_and(|status| status != selected_status) {
                return false;
            }
        }
        true
    }
}

/// The lifecycle status a single event's kind projects, or `None` when the
/// event carries no lifecycle meaning at all.
///
/// Terminal lifecycle events project exactly their terminal status; the rest
/// belong to a running workflow at the moment they were recorded — with ONE
/// exception, which is why this returns an `Option` rather than a status.
///
/// A LABEL-ONLY `SearchAttributesUpdated` (#211: a rename) has no lifecycle
/// meaning, and the label belongs to the WORKFLOW rather than to any one run —
/// it is folded over the whole history, so a completed run's row shows a name as
/// legitimate as a running one's, and a rename recorded now retitles every run
/// of that workflow at once. Projecting it as `Running`, as this function once
/// did, was wrong in both directions: a `Running` subscriber received the rename
/// of a finished run, and a `Completed` subscriber never received renames of the
/// runs it was actually displaying, so the name it showed went stale until a
/// refetch. Status selectors are about LIFECYCLE, so an event with no lifecycle
/// meaning answers `None` and passes every status selector instead of being
/// forced into a bucket it does not belong to.
///
/// That exemption is scoped to label-only updates, and deliberately no wider.
/// The engine also records a `SearchAttributesUpdated` in the same ATOMIC batch
/// as `WorkflowStarted` (`record_workflow_started_with_attributes`), stamping
/// the run's placement — so on a server-embedded engine every start emits one.
/// Exempting that one too would hand a `status=Completed` subscriber an
/// attribute frame for every workflow STARTING in the namespace: a delivery
/// widening with no rename to justify it, over what is often the highest-volume
/// event class there is. It accompanies a start, so it projects `Running`
/// exactly like the `WorkflowStarted` it ships with. See
/// [`is_start_time_stamp`] for how the two are told apart.
///
/// Returning `Option` rather than special-casing the caller is deliberate: it
/// puts the exception in the type, so a future event kind with no lifecycle
/// meaning cannot be given a wrong status by default.
fn event_status(event: &Event) -> Option<WorkflowStatus> {
    match event {
        Event::WorkflowCompleted { .. } => Some(WorkflowStatus::Completed),
        Event::WorkflowFailed { .. } => Some(WorkflowStatus::Failed),
        Event::WorkflowCancelled { .. } => Some(WorkflowStatus::Cancelled),
        Event::WorkflowTimedOut { .. } => Some(WorkflowStatus::TimedOut),
        Event::WorkflowContinuedAsNew { .. } => Some(WorkflowStatus::ContinuedAsNew),
        // A pause projects Paused at the moment it is recorded (#204).
        Event::WorkflowPaused { .. } => Some(WorkflowStatus::Paused),
        // A LABEL change is not a lifecycle transition (#211): no status. The
        // start-time placement stamp is not a label change — it ships with the
        // start, so it keeps the start's status.
        Event::SearchAttributesUpdated { attributes, .. } => {
            if is_start_time_stamp(attributes) {
                Some(WorkflowStatus::Running)
            } else {
                None
            }
        }
        Event::WorkflowStarted { .. }
        // A reopen returns the workflow to Running at the moment it is recorded.
        | Event::WorkflowReopened { .. }
        // A resume returns the workflow to Running at the moment it is recorded.
        | Event::WorkflowResumed { .. }
        | Event::ActivityScheduled { .. }
        | Event::ActivityStarted { .. }
        | Event::ActivityAdoptionOffered { .. }
        | Event::ActivityCompleted { .. }
        | Event::ActivityFailed { .. }
        // A side channel exhausted its budget; the workflow it warns about
        // is still running.
        | Event::ActivityAdvisoryExhausted { .. }
        | Event::ActivityCancelled { .. }
        | Event::TimerStarted { .. }
        | Event::TimerFired { .. }
        | Event::TimerCancelled { .. }
        | Event::WithTimeoutCompleted { .. }
        | Event::SignalReceived { .. }
        | Event::SignalSent { .. }
        | Event::ChildWorkflowStarted { .. }
        | Event::ChildWorkflowCompleted { .. }
        | Event::ChildWorkflowFailed { .. }
        | Event::ChildWorkflowCancelled { .. }
        | Event::ScheduleCreated { .. }
        | Event::ScheduleUpdated { .. }
        | Event::SchedulePaused { .. }
        | Event::ScheduleResumed { .. }
        | Event::ScheduleDeleted { .. }
        | Event::ScheduleTriggered { .. } => Some(WorkflowStatus::Running),
    }
}

/// Whether a `SearchAttributesUpdated` is the PLACEMENT stamp the engine records
/// atomically with `WorkflowStarted`, rather than a label change.
///
/// The two are told apart by the attributes they carry, which is exact rather
/// than a heuristic because only two sites in the tree record this event:
///
/// * `lifecycle::start` stamps the run's placement, and the server's
///   `start_search_attributes` ALWAYS writes [`NAMESPACE_ATTRIBUTE`] (the task
///   queue and display name are conditional, the namespace is not);
/// * `lifecycle::rename` records the display-name attribute and nothing else.
///
/// Both placement attributes are start-time-only by construction — the rename
/// verb cannot write either, and no verb updates a namespace or task queue after
/// the fact — so an update carrying one is the start stamp and an update
/// carrying neither is a label change.
///
/// A caller embedding the engine directly could record a start whose attributes
/// carry no placement at all; that stamp reads as a label change, so it answers
/// `None` and passes every status selector.
///
/// Name that honestly: it is a WIDENING, not a preservation. Before #211 this
/// function did not exist and EVERY `SearchAttributesUpdated` projected
/// `Running`, so a placement-less start stamp reached only `status=Running`
/// subscribers; now it reaches all of them. The widening is confined to
/// DELIVERY — `event_status` feeds subscription filtering only, and
/// `WorkflowStatus` itself is projected from history elsewhere and is untouched
/// — and it costs one extra attribute frame to status-filtered subscribers of
/// an embedded engine that stamps no placement, which the server's own
/// `start_search_attributes` never does. Narrowing it back would require the
/// selector to see the whole atomic batch rather than one event, which this
/// call site cannot; that is the reason it stands, not that nothing changed.
fn is_start_time_stamp(
    attributes: &std::collections::HashMap<String, aion_core::SearchAttributeValue>,
) -> bool {
    attributes.contains_key(NAMESPACE_ATTRIBUTE) || attributes.contains_key(TASK_QUEUE_ATTRIBUTE)
}

#[cfg(test)]
mod tests {
    use aion_core::{Event, EventEnvelope, Payload, WorkflowId, WorkflowStatus};

    use super::SubscriptionSelector;

    fn envelope(seq: u64) -> EventEnvelope {
        EventEnvelope {
            seq,
            recorded_at: chrono::Utc::now(),
            workflow_id: WorkflowId::new(uuid::Uuid::from_u128(1)),
        }
    }

    fn payload() -> Result<Payload, aion_core::PayloadError> {
        Payload::from_json(&serde_json::json!({ "label": "x" }))
    }

    fn signal(seq: u64) -> Result<Event, aion_core::PayloadError> {
        Ok(Event::SignalReceived {
            envelope: envelope(seq),
            name: "ship".to_owned(),
            payload: payload()?,
        })
    }

    fn started(seq: u64) -> Result<Event, aion_core::PayloadError> {
        Ok(Event::WorkflowStarted {
            envelope: envelope(seq),
            workflow_type: "checkout".to_owned(),
            input: payload()?,
            run_id: aion_core::RunId::new(uuid::Uuid::from_u128(1)),
            parent_run_id: None,
            package_version: aion_core::PackageVersion::new("a".repeat(64)),
        })
    }

    fn completed(seq: u64) -> Result<Event, aion_core::PayloadError> {
        Ok(Event::WorkflowCompleted {
            envelope: envelope(seq),
            result: payload()?,
        })
    }

    fn failed(seq: u64) -> Event {
        Event::WorkflowFailed {
            envelope: envelope(seq),
            error: aion_core::WorkflowError {
                message: "boom".to_owned(),
                details: None,
            },
        }
    }

    fn renamed(seq: u64) -> Event {
        Event::SearchAttributesUpdated {
            envelope: envelope(seq),
            workflow_id: WorkflowId::new(uuid::Uuid::from_u128(1)),
            attributes: std::collections::HashMap::from([(
                aion_core::DISPLAY_NAME_ATTRIBUTE.to_owned(),
                aion_core::SearchAttributeValue::String("Nightly settlement".to_owned()),
            )]),
        }
    }

    /// The start-time attribute stamp: what the engine records ATOMICALLY with
    /// `WorkflowStarted` for every start that carries a namespace, task queue,
    /// or display name — which, through the server, is every start.
    fn start_stamp(seq: u64) -> Event {
        Event::SearchAttributesUpdated {
            envelope: envelope(seq),
            workflow_id: WorkflowId::new(uuid::Uuid::from_u128(1)),
            attributes: std::collections::HashMap::from([
                (
                    crate::namespace::NAMESPACE_ATTRIBUTE.to_owned(),
                    aion_core::SearchAttributeValue::String("default".to_owned()),
                ),
                (
                    crate::namespace::TASK_QUEUE_ATTRIBUTE.to_owned(),
                    aion_core::SearchAttributeValue::String("settlement".to_owned()),
                ),
                (
                    aion_core::DISPLAY_NAME_ATTRIBUTE.to_owned(),
                    aion_core::SearchAttributeValue::String("Nightly settlement".to_owned()),
                ),
            ]),
        }
    }

    /// #211, the OTHER `SearchAttributesUpdated`: the start-time stamp is NOT a
    /// rename and must not inherit the rename's status-blindness.
    ///
    /// `record_workflow_started_with_attributes` appends this event in the same
    /// atomic batch as `WorkflowStarted`, so on a server-embedded engine every
    /// start emits one. Passing it to every status selector would hand a
    /// `status=Completed` subscriber an attribute frame for every workflow
    /// STARTING in the namespace — a delivery widening far past the rename this
    /// exemption was written for, and on a busy namespace the highest-volume
    /// event class there is.
    ///
    /// It genuinely accompanies a start, so it projects `Running` like the
    /// `WorkflowStarted` it ships with.
    #[test]
    fn a_start_time_attribute_stamp_stays_in_the_running_bucket() {
        let running = SubscriptionSelector {
            workflow_type: None,
            status: Some(WorkflowStatus::Running),
        };
        assert!(
            running.matches(&start_stamp(2), Some("checkout")),
            "the start-time stamp accompanies a start, so it belongs to Running"
        );

        for status in [
            WorkflowStatus::Completed,
            WorkflowStatus::Failed,
            WorkflowStatus::Cancelled,
            WorkflowStatus::TimedOut,
            WorkflowStatus::ContinuedAsNew,
            WorkflowStatus::Paused,
        ] {
            let selector = SubscriptionSelector {
                workflow_type: None,
                status: Some(status),
            };
            assert!(
                !selector.matches(&start_stamp(2), Some("checkout")),
                "a status={status:?} subscriber must not receive an attribute frame for every \
                 workflow STARTING in the namespace"
            );
        }

        // CONTROL: the narrowing must not swallow the rename exemption it sits
        // beside. A label-only update still reaches a terminal subscriber —
        // without this, deleting the exemption entirely would pass the above.
        let completed_only = SubscriptionSelector {
            workflow_type: None,
            status: Some(WorkflowStatus::Completed),
        };
        assert!(
            completed_only.matches(&renamed(1), Some("checkout")),
            "a label-only rename must still reach every status subscriber"
        );
    }

    /// #211: a rename is a LABEL change, not a lifecycle event, so it reaches
    /// EVERY status subscriber.
    ///
    /// Renames legally record on terminal and paused runs, which broke the old
    /// "every non-terminal event belongs to a running workflow" premise two
    /// ways at once: a `Running` subscriber was handed the rename of a
    /// finished run (wrong bucket), and a `Completed` subscriber never saw
    /// renames of the very runs it was displaying (a name that goes stale
    /// until a refetch). Status selectors are about LIFECYCLE, and a label
    /// change has no lifecycle meaning — so it passes the status arm whatever
    /// the selector asks for.
    #[test]
    fn a_rename_reaches_every_status_subscriber() -> Result<(), Box<dyn std::error::Error>> {
        for status in [
            WorkflowStatus::Running,
            WorkflowStatus::Completed,
            WorkflowStatus::Failed,
            WorkflowStatus::Cancelled,
            WorkflowStatus::TimedOut,
            WorkflowStatus::ContinuedAsNew,
            WorkflowStatus::Paused,
        ] {
            let selector = SubscriptionSelector {
                workflow_type: None,
                status: Some(status),
            };
            assert!(
                selector.matches(&renamed(1), Some("checkout")),
                "a rename must reach a status={status:?} subscriber"
            );
        }

        // CONTROL: bypassing the status arm must not bypass the TYPE arm, and
        // must not make every other event status-blind either — without these
        // the assertions above would also pass if `matches` had started
        // returning `true` unconditionally.
        let typed = SubscriptionSelector {
            workflow_type: Some("checkout".to_owned()),
            status: Some(WorkflowStatus::Completed),
        };
        assert!(!typed.matches(&renamed(1), Some("payments")));
        assert!(!typed.matches(&signal(2)?, Some("checkout")));
        Ok(())
    }

    #[test]
    fn unrestricted_selector_matches_everything() -> Result<(), Box<dyn std::error::Error>> {
        let selector = SubscriptionSelector::unrestricted();

        assert!(selector.matches(&signal(1)?, None));
        assert!(selector.matches(&completed(2)?, Some("checkout")));
        Ok(())
    }

    #[test]
    fn type_selector_matches_only_the_recorded_type() -> Result<(), Box<dyn std::error::Error>> {
        let selector = SubscriptionSelector {
            workflow_type: Some("checkout".to_owned()),
            status: None,
        };

        assert!(selector.matches(&signal(1)?, Some("checkout")));
        assert!(!selector.matches(&signal(1)?, Some("fulfillment")));
        assert!(
            !selector.matches(&signal(1)?, None),
            "a workflow with no recorded type never matches a type selector"
        );
        Ok(())
    }

    #[test]
    fn status_selector_matches_per_event_kind() -> Result<(), Box<dyn std::error::Error>> {
        let running = SubscriptionSelector {
            workflow_type: None,
            status: Some(WorkflowStatus::Running),
        };
        let completed_only = SubscriptionSelector {
            workflow_type: None,
            status: Some(WorkflowStatus::Completed),
        };
        let failed_only = SubscriptionSelector {
            workflow_type: None,
            status: Some(WorkflowStatus::Failed),
        };

        // Running matches every non-terminal event, including WorkflowStarted.
        assert!(running.matches(&started(1)?, Some("checkout")));
        assert!(running.matches(&signal(2)?, Some("checkout")));
        assert!(!running.matches(&completed(3)?, Some("checkout")));

        // Each terminal status matches exactly its terminal event kind.
        assert!(completed_only.matches(&completed(3)?, Some("checkout")));
        assert!(!completed_only.matches(&failed(3), Some("checkout")));
        assert!(!completed_only.matches(&signal(2)?, Some("checkout")));
        assert!(failed_only.matches(&failed(3), Some("checkout")));
        assert!(!failed_only.matches(&completed(3)?, Some("checkout")));
        Ok(())
    }

    fn timed_out(seq: u64) -> Event {
        Event::WorkflowTimedOut {
            envelope: envelope(seq),
            timeout: "workflow".to_owned(),
        }
    }

    #[test]
    fn status_selector_projects_workflow_timed_out_to_timed_out()
    -> Result<(), Box<dyn std::error::Error>> {
        // The ops-console stream selector must surface a `WorkflowTimedOut`
        // terminal as `TimedOut`: a `TimedOut` subscription admits it, a
        // `Running` subscription rejects it (it is terminal), and a `Failed`
        // subscription does not confuse it for a failure.
        let timed_out_only = SubscriptionSelector {
            workflow_type: None,
            status: Some(WorkflowStatus::TimedOut),
        };
        let running = SubscriptionSelector {
            workflow_type: None,
            status: Some(WorkflowStatus::Running),
        };
        let failed_only = SubscriptionSelector {
            workflow_type: None,
            status: Some(WorkflowStatus::Failed),
        };

        assert!(timed_out_only.matches(&timed_out(4), Some("checkout")));
        assert!(!timed_out_only.matches(&completed(3)?, Some("checkout")));
        assert!(!running.matches(&timed_out(4), Some("checkout")));
        assert!(!failed_only.matches(&timed_out(4), Some("checkout")));
        Ok(())
    }

    #[test]
    fn combined_selectors_and_together() -> Result<(), Box<dyn std::error::Error>> {
        let selector = SubscriptionSelector {
            workflow_type: Some("checkout".to_owned()),
            status: Some(WorkflowStatus::Completed),
        };

        assert!(selector.matches(&completed(3)?, Some("checkout")));
        assert!(
            !selector.matches(&completed(3)?, Some("fulfillment")),
            "matching status with mismatched type must not pass"
        );
        assert!(
            !selector.matches(&signal(2)?, Some("checkout")),
            "matching type with mismatched status must not pass"
        );
        Ok(())
    }
}
