//! `read_transcript`: a cursor page of one step's agent transcript.
//!
//! The read returns immediately with what exists now. There is deliberately no
//! blocking tail: an MCP call that waits holds the agent that made it, and an
//! agent holding a socket open on a step that may run for an hour is an agent
//! that cannot do anything else. Following a live step is done by calling again
//! with the `next_from_seq` the previous page returned.

use aion_mcp::tools::service::{ToolCall, ToolFailure, ToolOutcome};
use aion_store::{ActivityRecord, ActivityStreamKey};
use serde_json::json;

use crate::mcp::args::{
    optional_u32, optional_u64, required_activity_id, required_run_id, required_str, required_u32,
    required_workflow_id,
};
use crate::{CallerIdentity, ServerState};

use super::errors::tool_failure;
use super::run_view::RunView;

/// Run `read_transcript`.
///
/// # Errors
///
/// [`ToolFailure`] when the arguments are malformed, the caller does not hold
/// the namespace, the run is not this workflow's, the run never dispatched the
/// named `(activity, attempt)`, or the observability store fails.
pub(crate) async fn read_transcript(
    state: &ServerState,
    caller: &CallerIdentity,
    call: &ToolCall,
) -> Result<ToolOutcome, ToolFailure> {
    let namespace = required_str(call, "namespace")?;
    let workflow_id = required_workflow_id(call, "workflow_id")?;
    // REQUIRED, not optional. A transcript handle without a run names a pile of
    // streams fused across generations rather than one conversation.
    let run_id = required_run_id(call, "run_id")?;
    let activity_id = required_activity_id(call, "activity_id")?;
    let attempt = required_u32(call, "attempt")?;
    let from_seq = optional_u64(call, "from_seq").unwrap_or(0);
    let limit = optional_u32(call, "limit")?;

    let view = RunView::read_parts(state, caller, &namespace, &workflow_id, Some(&run_id)).await?;
    view.scope
        .validate_handle(&workflow_id, &run_id, &activity_id, attempt)
        .map_err(|error| {
            ToolFailure::new(
                error.to_string(),
                json!({
                    "code": "transcript_handle_not_in_run",
                    "workflow_id": workflow_id.to_string(),
                    "run_id": run_id.to_string(),
                    "activity_id": activity_id.sequence_position(),
                    "attempt": attempt,
                }),
            )
        })?;

    let key = ActivityStreamKey::new(
        workflow_id.clone(),
        run_id.clone(),
        activity_id.clone(),
        attempt,
    );
    let publisher = state.transcript_publisher();
    let records = publisher
        .replay_from(&key, from_seq)
        .await
        .map_err(|error| tool_failure(&crate::ServerError::from(error).to_wire_error()))?;
    // A ranged read that lands past the end returns nothing and therefore
    // cannot see the head. Re-read from zero for THAT case only, so the head is
    // the stream's real head rather than an echo of the cursor the caller
    // guessed. Reporting the cursor back would tell an agent paging a live
    // stream that the stream had run further than it has.
    let head_seq = if records.is_empty() && from_seq != 0 {
        let whole = publisher
            .replay_from(&key, 0)
            .await
            .map_err(|error| tool_failure(&crate::ServerError::from(error).to_wire_error()))?;
        head_seq(&whole, 0)?
    } else {
        head_seq(&records, from_seq)?
    };
    let (events, next_from_seq) = page(records, limit)?;

    let summary = format!(
        "{} transcript event(s) for run {run_id} activity {} attempt {attempt}",
        events.len(),
        activity_id.sequence_position()
    );

    Ok(ToolOutcome {
        structured: json!({
            "workflow_id": workflow_id.to_string(),
            "run_id": run_id.to_string(),
            "activity_id": activity_id.sequence_position(),
            "attempt": attempt,
            "events": events,
            "next_from_seq": next_from_seq,
            "head_seq": head_seq,
        }),
        summary,
    })
}

/// The stream head implied by a page: one past its highest `store_seq`.
///
/// An empty page reports `from_seq`, which is exact for the ordinary
/// caught-up case (`from_seq == head`, nothing new yet) and is corrected by the
/// caller's zero re-read for the out-of-range case. Claiming zero instead would
/// tell a caller paging a live stream that the stream had been emptied.
fn head_seq(records: &[ActivityRecord], from_seq: u64) -> Result<u64, ToolFailure> {
    match records.last() {
        None => Ok(from_seq),
        Some(record) => record.store_seq.checked_add(1).ok_or_else(|| {
            ToolFailure::new(
                "the transcript stream exhausted the u64 sequence space",
                json!({ "code": "backend" }),
            )
        }),
    }
}

/// Truncate to `limit` and report the first omitted `store_seq`.
fn page(
    mut records: Vec<ActivityRecord>,
    limit: Option<u32>,
) -> Result<(Vec<aion_core::ActivityEvent>, Option<u64>), ToolFailure> {
    let Some(limit) = limit else {
        return Ok((
            records.into_iter().map(|record| record.event).collect(),
            None,
        ));
    };
    let limit = usize::try_from(limit).map_err(|error| {
        ToolFailure::new(
            format!("`limit` is too large for this platform: {error}"),
            json!({ "code": "invalid_argument", "argument": "limit" }),
        )
    })?;
    let next_from_seq = records.get(limit).map(|record| record.store_seq);
    records.truncate(limit);
    Ok((
        records.into_iter().map(|record| record.event).collect(),
        next_from_seq,
    ))
}

#[cfg(test)]
mod tests {
    use aion_core::{ActivityEvent, ActivityEventKind, ActivityId, WorkflowId};
    use aion_store::ActivityRecord;
    use chrono::Utc;
    use uuid::Uuid;

    use super::{head_seq, page};

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn record(store_seq: u64) -> ActivityRecord {
        ActivityRecord {
            store_seq,
            event: ActivityEvent {
                workflow_id: WorkflowId::new(Uuid::from_u128(1)),
                run_id: aion_core::RunId::new(Uuid::from_u128(0x11)),
                activity_id: ActivityId::from_sequence_position(0),
                attempt: 1,
                agent_id: Uuid::from_u128(2),
                agent_role: "orchestrator".to_owned(),
                emitted_at: Utc::now(),
                worker_seq: store_seq,
                store_seq: Some(store_seq),
                ephemeral: false,
                kind: ActivityEventKind::Progress {
                    detail: aion_core::ProgressDetail::Note {
                        text: format!("step {store_seq}"),
                    },
                },
            },
        }
    }

    #[test]
    fn an_empty_ranged_read_reports_the_cursor_not_zero() -> TestResult {
        assert_eq!(head_seq(&[], 40)?, 40);
        assert_eq!(head_seq(&[record(0), record(1)], 0)?, 2);
        Ok(())
    }

    #[test]
    fn a_limited_page_reports_the_first_omitted_sequence() -> TestResult {
        let records = vec![record(0), record(1), record(2), record(3)];
        let (events, next) = page(records, Some(2))?;
        assert_eq!(events.len(), 2);
        assert_eq!(next, Some(2));
        Ok(())
    }

    #[test]
    fn an_unlimited_page_has_no_next_cursor() -> TestResult {
        let (events, next) = page(vec![record(0), record(1)], None)?;
        assert_eq!(events.len(), 2);
        assert_eq!(next, None);
        Ok(())
    }

    #[test]
    fn a_page_shorter_than_the_limit_has_no_next_cursor() -> TestResult {
        let (events, next) = page(vec![record(0)], Some(10))?;
        assert_eq!(events.len(), 1);
        assert_eq!(next, None);
        Ok(())
    }
}
