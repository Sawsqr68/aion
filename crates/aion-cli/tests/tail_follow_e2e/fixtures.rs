use aion_core::{
    ActivityEvent, ActivityEventKind, ActivityId, Event, EventEnvelope, PackageVersion, Payload,
    RunId, WorkflowId,
};
use aion_proto::generated;
use chrono::{DateTime, Utc};
use serde_json::json;

use super::{CHILD_ID, CHILD_RUN, PARENT_ID, PARENT_RUN, PRIOR_CHILD_ID, PRIOR_PARENT_RUN};

pub(super) fn backfill_event() -> ActivityEvent {
    ActivityEvent {
        workflow_id: WorkflowId::new(uuid::Uuid::from_u128(PARENT_ID)),
        run_id: RunId::new(uuid::Uuid::from_u128(PARENT_RUN)),
        activity_id: ActivityId::from_sequence_position(7),
        attempt: 1,
        agent_id: uuid::Uuid::from_u128(0x2444),
        agent_role: "parent".to_owned(),
        emitted_at: DateTime::<Utc>::from_timestamp_nanos(1_700_000_000_000_000_000),
        worker_seq: 1,
        store_seq: Some(1),
        ephemeral: false,
        kind: ActivityEventKind::Raw {
            source: "retained-backfill-parent".to_owned(),
            value: json!({"line": "visible through retained HTTP backfill"}),
        },
    }
}

pub(super) fn live_event() -> ActivityEvent {
    ActivityEvent {
        workflow_id: WorkflowId::new(uuid::Uuid::from_u128(CHILD_ID)),
        run_id: RunId::new(uuid::Uuid::from_u128(CHILD_RUN)),
        activity_id: ActivityId::from_sequence_position(7),
        attempt: 1,
        agent_id: uuid::Uuid::from_u128(0x2444),
        agent_role: "builder".to_owned(),
        emitted_at: DateTime::<Utc>::from_timestamp_nanos(1_700_000_001_000_000_000),
        worker_seq: 1,
        store_seq: Some(1),
        ephemeral: false,
        kind: ActivityEventKind::Raw {
            source: "post-attach-live-leg".to_owned(),
            value: json!({"line": "visible only through the live socket"}),
        },
    }
}

pub(super) fn selected_parent_live_event() -> ActivityEvent {
    ActivityEvent {
        workflow_id: WorkflowId::new(uuid::Uuid::from_u128(PARENT_ID)),
        run_id: RunId::new(uuid::Uuid::from_u128(PARENT_RUN)),
        activity_id: ActivityId::from_sequence_position(7),
        attempt: 2,
        agent_id: uuid::Uuid::from_u128(0x2445),
        agent_role: "parent".to_owned(),
        emitted_at: DateTime::<Utc>::from_timestamp_nanos(1_700_000_002_000_000_000),
        worker_seq: 1,
        store_seq: Some(1),
        ephemeral: false,
        kind: ActivityEventKind::Raw {
            source: "selected-run-live-parent".to_owned(),
            value: json!({"line": "selected generation attempt remained discoverable"}),
        },
    }
}

pub(super) fn replayed_parent_events() -> Option<[Event; 6]> {
    Some([
        parent_run_started(1, PRIOR_PARENT_RUN)?,
        child_started(2, PRIOR_CHILD_ID, "PriorBuilder")?,
        parent_activity_started(3, 9),
        parent_run_started(4, PARENT_RUN)?,
        parent_activity_started(5, 2),
        child_started(6, CHILD_ID, "Builder")?,
    ])
}

pub(super) fn child_run_events() -> Option<[Event; 2]> {
    Some([child_run_started()?, child_activity_started()])
}

fn parent_run_started(seq: u64, run_id: u128) -> Option<Event> {
    Some(Event::WorkflowStarted {
        envelope: event_envelope(seq, PARENT_ID),
        workflow_type: "parent".to_owned(),
        input: Payload::from_json(&json!({})).ok()?,
        run_id: RunId::new(uuid::Uuid::from_u128(run_id)),
        parent_run_id: None,
        package_version: PackageVersion::new("a".repeat(64)),
    })
}

fn parent_activity_started(seq: u64, attempt: u32) -> Event {
    Event::ActivityStarted {
        envelope: event_envelope(seq, PARENT_ID),
        activity_id: ActivityId::from_sequence_position(7),
        attempt,
    }
}

fn child_started(seq: u64, child_id: u128, workflow_type: &str) -> Option<Event> {
    Some(Event::ChildWorkflowStarted {
        envelope: event_envelope(seq, PARENT_ID),
        child_workflow_id: WorkflowId::new(uuid::Uuid::from_u128(child_id)),
        workflow_type: workflow_type.to_owned(),
        input: Payload::from_json(&json!({})).ok()?,
        package_version: PackageVersion::new("a".repeat(64)),
    })
}

fn child_run_started() -> Option<Event> {
    Some(Event::WorkflowStarted {
        envelope: event_envelope(1, CHILD_ID),
        workflow_type: "Builder".to_owned(),
        input: Payload::from_json(&json!({})).ok()?,
        run_id: RunId::new(uuid::Uuid::from_u128(CHILD_RUN)),
        parent_run_id: Some(RunId::new(uuid::Uuid::from_u128(PARENT_RUN))),
        package_version: PackageVersion::new("a".repeat(64)),
    })
}

fn child_activity_started() -> Event {
    Event::ActivityStarted {
        envelope: event_envelope(2, CHILD_ID),
        activity_id: ActivityId::from_sequence_position(7),
        attempt: 1,
    }
}

fn event_envelope(seq: u64, workflow_id: u128) -> EventEnvelope {
    EventEnvelope {
        seq,
        recorded_at: DateTime::<Utc>::from_timestamp_nanos(1_700_000_000_000_000_000),
        workflow_id: WorkflowId::new(uuid::Uuid::from_u128(workflow_id)),
    }
}

pub(super) fn generated_envelope(value: aion_proto::WireEnvelope) -> generated::WireEnvelope {
    generated::WireEnvelope {
        namespace: value.namespace,
        request_id: value.request_id,
        payload: value.payload.map(|payload| generated::Payload {
            content_type: payload.content_type,
            bytes: payload.bytes,
        }),
    }
}
