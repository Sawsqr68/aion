use aion_core::{ActivityEvent, ActivityEventKind, ActivityId, RunId, WorkflowId};
use chrono::{DateTime, Utc};
use serde_json::json;

use super::{CHILD_ID, CHILD_RUN, PARENT_ID, PARENT_RUN};

pub(super) fn backfill_event() -> ActivityEvent {
    ActivityEvent {
        workflow_id: WorkflowId::new(uuid::Uuid::from_u128(PARENT_ID)),
        run_id: RunId::new(uuid::Uuid::from_u128(PARENT_RUN)),
        activity_id: ActivityId::from_sequence_position(7),
        attempt: 1,
        agent_id: uuid::Uuid::from_u128(0x2444),
        agent_role: "parent".to_owned(),
        emitted_at: DateTime::<Utc>::from_timestamp(1_700_000_000, 0)
            .expect("fixed backfill fixture timestamp is valid"),
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
        emitted_at: DateTime::<Utc>::from_timestamp(1_700_000_001, 0)
            .expect("fixed transcript fixture timestamp is valid"),
        worker_seq: 1,
        store_seq: Some(1),
        ephemeral: false,
        kind: ActivityEventKind::Raw {
            source: "post-attach-live-leg".to_owned(),
            value: json!({"line": "visible only through the live socket"}),
        },
    }
}
