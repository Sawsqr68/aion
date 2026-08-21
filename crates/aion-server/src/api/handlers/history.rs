//! Shared scoped, windowed workflow-history reads.

use aion_core::Event;
use aion_proto::{
    ProtoDescribeWorkflowRequest, ProtoReadHistoryRequest, ProtoReadHistoryResponse, WireError,
    convert::encode_event,
};

use super::payload::required_workflow_id;
use crate::{CallerIdentity, NamespaceGuard, NamespaceOperation, ServerError, WorkflowTarget};

pub(crate) const DEFAULT_HISTORY_LIMIT: u32 = 500;
pub(crate) const MAX_WINDOW_LIMIT: u32 = 2_000;

#[derive(Debug)]
pub(crate) struct HistoryWindow {
    pub(crate) namespace: String,
    pub(crate) events: Vec<Event>,
    pub(crate) next_from_seq: Option<u64>,
    pub(crate) head_seq: u64,
}

pub(crate) async fn history_window(
    guard: &NamespaceGuard,
    caller: &CallerIdentity,
    request: ProtoReadHistoryRequest,
) -> Result<HistoryWindow, WireError> {
    let workflow_id = required_workflow_id(request.workflow_id.clone())?;
    let describe = ProtoDescribeWorkflowRequest {
        namespace: request.namespace,
        workflow_id: request.workflow_id,
        run_id: None,
        include_history: false,
    };
    let target = WorkflowTarget::workflow(&workflow_id);
    let scoped = guard
        .scope(caller, &NamespaceOperation::describe(&describe, target))
        .await
        .map_err(|error| error.to_wire_error())?;
    let engine = scoped.engine().map_err(|error| error.to_wire_error())?;
    let from_seq = request.from_seq.unwrap_or(0);
    let limit = request
        .limit
        .unwrap_or(DEFAULT_HISTORY_LIMIT)
        .clamp(1, MAX_WINDOW_LIMIT) as usize;
    let ranged = engine
        .store()
        .read_history_from(&workflow_id, from_seq)
        .await
        .map_err(|error| ServerError::from(error).to_wire_error())?;

    let (mut events, head_seq) = if ranged.is_empty() {
        let mut history = engine
            .store()
            .read_history(&workflow_id)
            .await
            .map_err(|error| ServerError::from(error).to_wire_error())?;
        let head_seq = history.last().map_or(0, Event::seq);
        history.retain(|event| event.seq() >= from_seq);
        (history, head_seq)
    } else {
        let head_seq = ranged.last().map_or(0, Event::seq);
        (ranged, head_seq)
    };

    let next_from_seq = events.get(limit).map(Event::seq);
    events.truncate(limit);
    Ok(HistoryWindow {
        namespace: scoped.namespace().to_owned(),
        events,
        next_from_seq,
        head_seq,
    })
}

pub(crate) async fn read_history(
    guard: &NamespaceGuard,
    caller: &CallerIdentity,
    request: ProtoReadHistoryRequest,
) -> Result<ProtoReadHistoryResponse, WireError> {
    let window = history_window(guard, caller, request).await?;
    let events = window
        .events
        .iter()
        .map(|event| encode_event(window.namespace.clone(), None, event))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ProtoReadHistoryResponse {
        events,
        next_from_seq: window.next_from_seq,
        head_seq: window.head_seq,
    })
}
