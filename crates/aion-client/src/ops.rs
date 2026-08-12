//! start/signal/query/cancel/list/describe over the transport.

use std::num::NonZeroU64;
use std::time::Duration;

use aion_core::{
    Event, Payload, RunId, WorkflowFilter, WorkflowId, WorkflowStatus, WorkflowSummary,
};
use aion_proto::{
    ProtoCancelRequest, ProtoDescribeWorkflowRequest, ProtoListWorkflowsRequest, ProtoPauseRequest,
    ProtoPayload, ProtoQueryRequest, ProtoReopenRequest, ProtoResumeRequest, ProtoRunId,
    ProtoSignalRequest, ProtoWorkflowId, ProtoWorkflowStatus, WireError, decode_core_value,
    decode_event, decode_workflow_summary, encode_core_value, proto_query_response,
};
use aion_store::visibility::ListWorkflowsFilter;

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::client::Client;
use crate::error::ClientError;
use crate::payload::{from_payload, to_payload};
use crate::stream::{EventStream, SubscribeTarget, event_stream, event_stream_from};

/// Pagination options accepted by [`Client::list`].
///
/// The current AW protobuf carries `request_id` through the filter envelope,
/// but not `limit` or `cursor`; populated `limit`/`cursor` values return
/// [`ClientError::InvalidArgument`] instead of being silently ignored.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ListPage {
    /// Caller request identifier carried in the current filter envelope.
    pub request_id: Option<String>,
    /// Requested page size reserved by the contract.
    pub limit: Option<usize>,
    /// Continuation cursor reserved by the contract.
    pub cursor: Option<String>,
}

/// Workflow detail returned by [`Client::describe`].
#[derive(Clone, Debug, PartialEq)]
pub struct WorkflowDescription {
    /// Lightweight workflow summary reused from `aion-core`.
    pub summary: WorkflowSummary,
    /// Optional event history when the server includes it.
    pub history: Vec<Event>,
}

/// Outcome of [`Client::reopen`]: the reopened run and its projected status.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReopenOutcome {
    /// The reopened concrete run identifier (now live again).
    pub run_id: RunId,
    /// The projected status after the reopen (Running).
    pub status: WorkflowStatus,
}

/// Outcome of [`Client::pause`]: the paused run and its projected status (#204).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PauseOutcome {
    /// The paused concrete run identifier.
    pub run_id: RunId,
    /// The projected status after the pause (Paused).
    pub status: WorkflowStatus,
}

/// Outcome of [`Client::resume`]: the resumed run and its projected status (#204).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResumeOutcome {
    /// The resumed concrete run identifier (now live again).
    pub run_id: RunId,
    /// The projected status after the resume (Running).
    pub status: WorkflowStatus,
}

impl Client {
    /// Sends a signal to the latest run, or to `run_id` when supplied.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] when transport, server, or request conversion fails.
    pub async fn signal(
        &self,
        workflow_id: &WorkflowId,
        run_id: Option<&RunId>,
        name: impl Into<String>,
        payload: Payload,
    ) -> Result<(), ClientError> {
        self.transport
            .signal(ProtoSignalRequest {
                namespace: self.namespace().to_owned(),
                workflow_id: Some(ProtoWorkflowId::from(workflow_id.clone())),
                run_id: run_id.cloned().map(ProtoRunId::from),
                signal_name: name.into(),
                payload: Some(ProtoPayload::from(payload)),
            })
            .await?;
        Ok(())
    }

    /// Serializes `value` as JSON and sends it as a signal payload.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError::InvalidArgument`] when serialization fails, or the
    /// delegated signal error otherwise.
    pub async fn signal_typed<T>(
        &self,
        workflow_id: &WorkflowId,
        run_id: Option<&RunId>,
        name: impl Into<String>,
        value: &T,
    ) -> Result<(), ClientError>
    where
        T: Serialize + ?Sized,
    {
        self.signal(workflow_id, run_id, name, to_payload(value)?)
            .await
    }

    /// Queries the latest run, or `run_id` when supplied, with a local deadline.
    ///
    /// `args` is the argument document handed to the workflow's registered
    /// query handler. A query that takes no arguments passes
    /// [`Payload::json_null`] — the canonical "nothing supplied" document
    /// every carrier agrees on. The server refuses arguments that are not a
    /// well-formed JSON document with [`ClientError::InvalidArgument`].
    ///
    /// # Errors
    ///
    /// Returns [`ClientError::QueryTimeout`] when `deadline` elapses.
    pub async fn query(
        &self,
        workflow_id: &WorkflowId,
        run_id: Option<&RunId>,
        name: impl Into<String>,
        args: Payload,
        deadline: Duration,
    ) -> Result<Payload, ClientError> {
        let response = tokio::time::timeout(
            deadline,
            self.transport.query(ProtoQueryRequest {
                namespace: self.namespace().to_owned(),
                workflow_id: Some(ProtoWorkflowId::from(workflow_id.clone())),
                run_id: run_id.cloned().map(ProtoRunId::from),
                query_name: name.into(),
                arguments: Some(aion_proto::ProtoPayload::from(args)),
            }),
        )
        .await
        .map_err(|_| {
            ClientError::query_timeout(format!(
                "query deadline of {deadline:?} elapsed before the server replied"
            ))
        })??;

        match response.outcome {
            Some(proto_query_response::Outcome::Result(payload)) => {
                Payload::try_from(payload).map_err(ClientError::from_wire_error)
            }
            Some(proto_query_response::Outcome::Error(error)) => Err(query_error(error)),
            None => Err(ClientError::server("query response outcome is missing")),
        }
    }

    /// Serializes `args` as JSON, queries a workflow, and deserializes the JSON result.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError::InvalidArgument`] when serialization or result
    /// decoding fails, or the delegated query error otherwise.
    pub async fn query_typed<A, R>(
        &self,
        workflow_id: &WorkflowId,
        run_id: Option<&RunId>,
        name: impl Into<String>,
        args: &A,
        deadline: Duration,
    ) -> Result<R, ClientError>
    where
        A: Serialize + ?Sized,
        R: DeserializeOwned,
    {
        let payload = self
            .query(
                workflow_id,
                run_id,
                name,
                query_args_payload(args)?,
                deadline,
            )
            .await?;
        from_payload(&payload)
    }

    /// Requests cancellation of the latest run, or `run_id` when supplied.
    ///
    /// Success means the server accepted the cancellation request; it is not a
    /// confirmation that the workflow has reached a terminal cancelled state.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] when transport, server, or request conversion fails.
    pub async fn cancel(
        &self,
        workflow_id: &WorkflowId,
        run_id: Option<&RunId>,
        reason: impl Into<String>,
    ) -> Result<(), ClientError> {
        self.transport
            .cancel(ProtoCancelRequest {
                namespace: self.namespace().to_owned(),
                workflow_id: Some(ProtoWorkflowId::from(workflow_id.clone())),
                run_id: run_id.cloned().map(ProtoRunId::from),
                reason: reason.into(),
            })
            .await?;
        Ok(())
    }

    /// Reopens a terminal-reopenable run (Failed or Cancelled), re-driving it
    /// from where it left off. Targets the latest run, or `run_id` when supplied.
    ///
    /// Returns the reopened run and its projected status (Running). A run that is
    /// not a reopenable terminal (not terminal, terminal for a non-reopenable
    /// reason, or already Running) returns [`ClientError::InvalidState`]; an
    /// absent workflow returns [`ClientError::NotFound`].
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] when transport, server, or response conversion fails.
    pub async fn reopen(
        &self,
        workflow_id: &WorkflowId,
        run_id: Option<&RunId>,
    ) -> Result<ReopenOutcome, ClientError> {
        let response = self
            .transport
            .reopen(ProtoReopenRequest {
                namespace: self.namespace().to_owned(),
                workflow_id: Some(ProtoWorkflowId::from(workflow_id.clone())),
                run_id: run_id.cloned().map(ProtoRunId::from),
            })
            .await?;
        let run_id = response
            .run_id
            .ok_or_else(|| ClientError::server("reopen response run id is missing"))?
            .try_into()
            .map_err(ClientError::from_wire_error)?;
        let status = ProtoWorkflowStatus::try_from(response.status)
            .map_err(|_error| ClientError::server("reopen response status is unknown"))
            .and_then(|status| {
                WorkflowStatus::try_from(status).map_err(ClientError::from_wire_error)
            })?;
        Ok(ReopenOutcome { run_id, status })
    }

    /// Pauses a live `Running` run, durably holding new activity dispatch (#204).
    /// Targets the latest run, or `run_id` when supplied. Returns the run and its
    /// projected status (Paused). A run that is not `Running` returns
    /// [`ClientError::InvalidState`]; an absent workflow returns
    /// [`ClientError::NotFound`].
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] when transport, server, or response conversion fails.
    pub async fn pause(
        &self,
        workflow_id: &WorkflowId,
        run_id: Option<&RunId>,
        reason: impl Into<String>,
    ) -> Result<PauseOutcome, ClientError> {
        let response = self
            .transport
            .pause(ProtoPauseRequest {
                namespace: self.namespace().to_owned(),
                workflow_id: Some(ProtoWorkflowId::from(workflow_id.clone())),
                run_id: run_id.cloned().map(ProtoRunId::from),
                reason: reason.into(),
            })
            .await?;
        let run_id = response
            .run_id
            .ok_or_else(|| ClientError::server("pause response run id is missing"))?
            .try_into()
            .map_err(ClientError::from_wire_error)?;
        let status = ProtoWorkflowStatus::try_from(response.status)
            .map_err(|_error| ClientError::server("pause response status is unknown"))
            .and_then(|status| {
                WorkflowStatus::try_from(status).map_err(ClientError::from_wire_error)
            })?;
        Ok(PauseOutcome { run_id, status })
    }

    /// Resumes a `Paused` run, releasing the dispatch hold (#204). Targets the
    /// latest run, or `run_id` when supplied. Returns the run and its projected
    /// status (Running). A run that is not `Paused` returns
    /// [`ClientError::InvalidState`]; an absent workflow returns
    /// [`ClientError::NotFound`].
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] when transport, server, or response conversion fails.
    pub async fn resume(
        &self,
        workflow_id: &WorkflowId,
        run_id: Option<&RunId>,
    ) -> Result<ResumeOutcome, ClientError> {
        let response = self
            .transport
            .resume(ProtoResumeRequest {
                namespace: self.namespace().to_owned(),
                workflow_id: Some(ProtoWorkflowId::from(workflow_id.clone())),
                run_id: run_id.cloned().map(ProtoRunId::from),
            })
            .await?;
        let run_id = response
            .run_id
            .ok_or_else(|| ClientError::server("resume response run id is missing"))?
            .try_into()
            .map_err(ClientError::from_wire_error)?;
        let status = ProtoWorkflowStatus::try_from(response.status)
            .map_err(|_error| ClientError::server("resume response status is unknown"))
            .and_then(|status| {
                WorkflowStatus::try_from(status).map_err(ClientError::from_wire_error)
            })?;
        Ok(ResumeOutcome { run_id, status })
    }

    /// Lists workflows matching a filter.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] when transport, server, or response conversion fails.
    pub async fn list(
        &self,
        filter: &WorkflowFilter,
        page: ListPage,
    ) -> Result<Vec<WorkflowSummary>, ClientError> {
        validate_list_page(&page)?;
        let namespace = self.namespace().to_owned();
        let filter = workflow_filter_to_visibility(filter)?;
        let filter = encode_core_value(namespace.clone(), page.request_id, &filter)
            .map_err(ClientError::from_wire_error)?;
        let response = self
            .transport
            .list_workflows(ProtoListWorkflowsRequest {
                namespace,
                filter: Some(filter),
            })
            .await?;

        response
            .summaries
            .iter()
            .map(decode_visibility_summary)
            .map(|result| result.map_err(ClientError::from_wire_error))
            .collect()
    }

    /// Describes the latest run, or `run_id` when supplied.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] when transport, server, or response conversion fails.
    pub async fn describe(
        &self,
        workflow_id: &WorkflowId,
        run_id: Option<&RunId>,
    ) -> Result<WorkflowDescription, ClientError> {
        let response = self
            .transport
            .describe_workflow(ProtoDescribeWorkflowRequest {
                namespace: self.namespace().to_owned(),
                workflow_id: Some(ProtoWorkflowId::from(workflow_id.clone())),
                run_id: run_id.cloned().map(ProtoRunId::from),
                include_history: true,
            })
            .await?;
        let summary = response
            .summary
            .as_ref()
            .ok_or_else(|| ClientError::server("describe response summary is missing"))
            .and_then(|summary| {
                decode_workflow_summary(summary).map_err(ClientError::from_wire_error)
            })?;
        let history = response
            .history
            .iter()
            .map(decode_event)
            .map(|result| result.map_err(ClientError::from_wire_error))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(WorkflowDescription { summary, history })
    }

    /// Subscribes to events for a workflow.
    #[must_use]
    pub fn subscribe_workflow(&self, workflow_id: &WorkflowId) -> EventStream {
        event_stream(
            self.transport.clone(),
            self.namespace().to_owned(),
            SubscribeTarget::Workflow {
                workflow_id: workflow_id.clone(),
            },
        )
    }

    /// Subscribes to events for a workflow, attaching from an explicit
    /// per-workflow sequence cursor.
    ///
    /// `resume_from` is the first sequence number wanted (`resume_from_seq`
    /// on the wire); `1` replays the workflow's full recorded history before
    /// splicing into the live stream, gap-free and duplicate-free.
    #[must_use]
    pub fn subscribe_workflow_from(
        &self,
        workflow_id: &WorkflowId,
        resume_from: NonZeroU64,
    ) -> EventStream {
        event_stream_from(
            self.transport.clone(),
            self.namespace().to_owned(),
            workflow_id.clone(),
            resume_from,
        )
    }

    /// Subscribes to events selected by the supplied workflow filter.
    #[must_use]
    pub fn subscribe(&self, filter: WorkflowFilter) -> EventStream {
        event_stream(
            self.transport.clone(),
            self.namespace().to_owned(),
            SubscribeTarget::Filtered { filter },
        )
    }

    /// Subscribes to every event visible to this client namespace.
    #[must_use]
    pub fn subscribe_firehose(&self) -> EventStream {
        event_stream(
            self.transport.clone(),
            self.namespace().to_owned(),
            SubscribeTarget::Firehose,
        )
    }
}

pub(crate) fn operation_namespace(client: &Client, namespace: Option<String>) -> String {
    namespace.unwrap_or_else(|| client.namespace().to_owned())
}

/// Serialize typed query arguments into the payload the wire carries.
///
/// A value that serializes to JSON `null` needs no special case: `null` *is*
/// the canonical "no arguments" document (see [`Payload::json_null`]), so the
/// serialized bytes are already exactly what a no-argument query sends.
fn query_args_payload<T>(args: &T) -> Result<Payload, ClientError>
where
    T: Serialize + ?Sized,
{
    to_payload(args)
}

fn validate_list_page(page: &ListPage) -> Result<(), ClientError> {
    if page.limit.is_some() || page.cursor.is_some() {
        return Err(ClientError::invalid_argument(
            "list pagination limit/cursor are reserved by the contract and \
             not yet carried by the wire",
        ));
    }
    Ok(())
}

fn workflow_filter_to_visibility(
    filter: &WorkflowFilter,
) -> Result<ListWorkflowsFilter, ClientError> {
    if filter.parent.is_some() {
        return Err(ClientError::invalid_argument(
            "parent workflow filters are not carried by the visibility wire contract",
        ));
    }

    Ok(ListWorkflowsFilter {
        workflow_type: filter.workflow_type.clone(),
        status: filter.status,
        started_after: filter.started_after,
        started_before: filter.started_before,
        ..ListWorkflowsFilter::default()
    })
}

fn decode_visibility_summary(
    envelope: &aion_proto::WireEnvelope,
) -> Result<WorkflowSummary, WireError> {
    let summary = decode_core_value::<aion_store::visibility::WorkflowSummary>(envelope)?;
    Ok(WorkflowSummary {
        workflow_id: summary.workflow_id,
        workflow_type: summary.workflow_type,
        status: summary.status,
        started_at: summary.start_time,
        ended_at: summary.close_time,
        parent: None,
        failed_step: summary.failed_step,
        failure_reason: summary.failure_reason,
        display_name: aion_core::display_name_from_attributes(&summary.search_attributes),
    })
}

pub(crate) fn decode_required_workflow_id(
    value: Option<ProtoWorkflowId>,
    context: &str,
) -> Result<WorkflowId, ClientError> {
    value
        .ok_or_else(|| ClientError::server(format!("{context} workflow id is missing")))?
        .try_into()
        .map_err(ClientError::from_wire_error)
}

pub(crate) fn decode_required_run_id(
    value: Option<ProtoRunId>,
    context: &str,
) -> Result<RunId, ClientError> {
    value
        .ok_or_else(|| ClientError::server(format!("{context} run id is missing")))?
        .try_into()
        .map_err(ClientError::from_wire_error)
}

/// Maps a `QueryResponse.error` payload through the shared wire taxonomy.
///
/// The server reports query-handler application failures with the dedicated
/// `query_failed` wire code, so the shared map yields [`ClientError::QueryFailed`]
/// directly; `backend` stays an unexpected server fault.
fn query_error(error: aion_proto::ProtoWireError) -> ClientError {
    ClientError::from_proto_wire_error(error)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use aion_core::{ContentType, Payload, WorkflowFilter, WorkflowId, WorkflowStatus};
    use aion_proto::{
        ProtoCancelResponse, ProtoDescribeWorkflowResponse, ProtoListWorkflowsResponse,
        ProtoQueryResponse, ProtoReopenResponse, ProtoRunId, ProtoSignalResponse,
        ProtoStartWorkflowResponse, ProtoWorkflowId, ProtoWorkflowStatus, WireError,
        encode_core_value, encode_workflow_summary, proto_query_response,
    };
    use async_trait::async_trait;
    use chrono::Utc;
    use futures::StreamExt;
    use futures::stream;
    use tokio::sync::Mutex;

    use super::ListPage;
    use crate::client::{Client, ClientBuilder, ClientConfig};
    use crate::error::ClientError;
    use crate::start::{DisplayNameNotApplied, StartOptions};
    use crate::transport::{SubscriptionAttempt, WorkflowTransport};

    #[derive(Default)]
    struct StubTransport {
        last_start: Mutex<Option<aion_proto::ProtoStartWorkflowRequest>>,
        last_signal: Mutex<Option<aion_proto::ProtoSignalRequest>>,
        last_query: Mutex<Option<aion_proto::ProtoQueryRequest>>,
        last_cancel: Mutex<Option<aion_proto::ProtoCancelRequest>>,
        last_reopen: Mutex<Option<aion_proto::ProtoReopenRequest>>,
        last_list: Mutex<Option<aion_proto::ProtoListWorkflowsRequest>>,
        last_describe: Mutex<Option<aion_proto::ProtoDescribeWorkflowRequest>>,
        start_error: Mutex<Option<ClientError>>,
        signal_error: Mutex<Option<ClientError>>,
        query_response: Mutex<Option<Result<ProtoQueryResponse, ClientError>>>,
        reopen_response: Mutex<Option<Result<ProtoReopenResponse, ClientError>>>,
        /// How many starts actually reached the transport. `last_start` alone
        /// cannot distinguish "deduped, never sent" from "sent again with the
        /// same values", which is exactly what the #211 replay tests assert.
        start_calls: std::sync::atomic::AtomicUsize,
    }

    #[async_trait]
    impl WorkflowTransport for StubTransport {
        async fn start_workflow(
            &self,
            request: aion_proto::ProtoStartWorkflowRequest,
        ) -> Result<ProtoStartWorkflowResponse, ClientError> {
            self.start_calls
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            *self.last_start.lock().await = Some(request);
            if let Some(error) = self.start_error.lock().await.take() {
                return Err(error);
            }
            Ok(ProtoStartWorkflowResponse {
                workflow_id: Some(ProtoWorkflowId::from(workflow_id())),
                run_id: Some(ProtoRunId::from(run_id())),
            })
        }

        async fn signal(
            &self,
            request: aion_proto::ProtoSignalRequest,
        ) -> Result<ProtoSignalResponse, ClientError> {
            *self.last_signal.lock().await = Some(request);
            if let Some(error) = self.signal_error.lock().await.take() {
                return Err(error);
            }
            Ok(ProtoSignalResponse {})
        }

        async fn query(
            &self,
            request: aion_proto::ProtoQueryRequest,
        ) -> Result<ProtoQueryResponse, ClientError> {
            *self.last_query.lock().await = Some(request);
            if let Some(response) = self.query_response.lock().await.take() {
                return response;
            }
            Ok(ProtoQueryResponse {
                outcome: Some(proto_query_response::Outcome::Result(
                    aion_proto::ProtoPayload::from(payload("result")),
                )),
            })
        }

        async fn cancel(
            &self,
            request: aion_proto::ProtoCancelRequest,
        ) -> Result<ProtoCancelResponse, ClientError> {
            *self.last_cancel.lock().await = Some(request);
            Ok(ProtoCancelResponse {})
        }

        async fn reopen(
            &self,
            request: aion_proto::ProtoReopenRequest,
        ) -> Result<ProtoReopenResponse, ClientError> {
            *self.last_reopen.lock().await = Some(request);
            if let Some(response) = self.reopen_response.lock().await.take() {
                return response;
            }
            Ok(ProtoReopenResponse {
                run_id: Some(ProtoRunId::from(run_id())),
                status: ProtoWorkflowStatus::Running as i32,
            })
        }

        async fn pause(
            &self,
            _request: aion_proto::ProtoPauseRequest,
        ) -> Result<aion_proto::ProtoPauseResponse, ClientError> {
            Ok(aion_proto::ProtoPauseResponse {
                run_id: Some(ProtoRunId::from(run_id())),
                status: ProtoWorkflowStatus::Paused as i32,
            })
        }

        async fn resume(
            &self,
            _request: aion_proto::ProtoResumeRequest,
        ) -> Result<aion_proto::ProtoResumeResponse, ClientError> {
            Ok(aion_proto::ProtoResumeResponse {
                run_id: Some(ProtoRunId::from(run_id())),
                status: ProtoWorkflowStatus::Running as i32,
            })
        }

        async fn list_workflows(
            &self,
            request: aion_proto::ProtoListWorkflowsRequest,
        ) -> Result<ProtoListWorkflowsResponse, ClientError> {
            *self.last_list.lock().await = Some(request);
            Ok(ProtoListWorkflowsResponse {
                summaries: vec![
                    encode_core_value("tenant-a", None, &visibility_summary())
                        .map_err(ClientError::from_wire_error)?,
                ],
            })
        }

        async fn describe_workflow(
            &self,
            request: aion_proto::ProtoDescribeWorkflowRequest,
        ) -> Result<ProtoDescribeWorkflowResponse, ClientError> {
            *self.last_describe.lock().await = Some(request);
            Ok(ProtoDescribeWorkflowResponse {
                summary: Some(
                    encode_workflow_summary("tenant-a", None, &summary())
                        .map_err(ClientError::from_wire_error)?,
                ),
                history: Vec::new(),
            })
        }

        async fn subscribe(
            &self,
            _: aion_proto::SubscriptionRequest,
            _: Option<u64>,
        ) -> Result<SubscriptionAttempt, ClientError> {
            Ok(SubscriptionAttempt::new(stream::empty().boxed()))
        }
    }

    fn client_with(stub: Arc<StubTransport>) -> Client {
        Client::from_transport(
            ClientConfig::from(
                ClientBuilder::new("http://localhost:50051").with_namespace("tenant-a"),
            ),
            stub,
        )
    }

    fn workflow_id() -> WorkflowId {
        WorkflowId::new_v4()
    }

    fn run_id() -> aion_core::RunId {
        aion_core::RunId::new_v4()
    }

    fn payload(label: &str) -> Payload {
        Payload::new(
            ContentType::Json,
            format!("{{\"label\":\"{label}\"}}").into_bytes(),
        )
    }

    fn summary() -> aion_core::WorkflowSummary {
        aion_core::WorkflowSummary {
            workflow_id: workflow_id(),
            workflow_type: String::from("checkout"),
            status: WorkflowStatus::Running,
            started_at: Utc::now(),
            ended_at: None,
            parent: None,
            failed_step: None,
            failure_reason: None,
            display_name: None,
        }
    }

    fn visibility_summary() -> aion_store::visibility::WorkflowSummary {
        aion_store::visibility::WorkflowSummary {
            workflow_id: workflow_id(),
            run_id: run_id(),
            workflow_type: String::from("checkout"),
            status: WorkflowStatus::Running,
            start_time: Utc::now(),
            close_time: None,
            failed_step: None,
            failure_reason: None,
            search_attributes: std::collections::HashMap::new(),
        }
    }

    #[tokio::test]
    async fn start_maps_request_and_returns_handle() -> Result<(), ClientError> {
        let stub = Arc::new(StubTransport::default());
        let client = client_with(Arc::clone(&stub));

        let result = client
            .start("checkout", payload("input"), StartOptions::default())
            .await?;

        let recorded = stub.last_start.lock().await.clone();
        assert!(recorded.is_some());
        let request = recorded.ok_or_else(|| ClientError::server("missing recorded start"))?;
        assert_eq!(request.namespace, "tenant-a");
        assert_eq!(request.workflow_type, "checkout");
        assert!(request.input.is_some());
        assert_ne!(
            result.handle.workflow_id(),
            &WorkflowId::new(uuid::Uuid::nil())
        );
        assert_eq!(result.display_name_not_applied, None);
        Ok(())
    }

    #[tokio::test]
    async fn start_idempotency_replays_identical_and_rejects_conflicts() -> Result<(), ClientError>
    {
        let stub = Arc::new(StubTransport::default());
        let client = client_with(Arc::clone(&stub));
        let opts = StartOptions {
            namespace: None,
            idempotency_key: Some(String::from("retry-key")),
            routing_key: None,
            task_queue: None,
            display_name: None,
        };

        let original = client
            .start("checkout", payload("input"), opts.clone())
            .await?;
        let replayed = client
            .start("checkout", payload("input"), opts.clone())
            .await?;
        let conflict = client.start("checkout", payload("other"), opts).await;

        assert_eq!(replayed, original);
        assert!(
            matches!(conflict, Err(ClientError::AlreadyExists { .. })),
            "got {conflict:?}"
        );
        Ok(())
    }

    /// #211 fingerprint ruling: a display name is NOT part of the idempotency
    /// fingerprint, so two starts differing only in name are the SAME act and
    /// dedupe to one run — but the second name is not silently applied and not
    /// silently dropped. The caller gets the EXISTING run wearing its EXISTING
    /// name, plus a note saying which name was not applied.
    #[tokio::test]
    async fn a_replay_with_a_different_name_dedupes_and_reports_the_unapplied_name()
    -> Result<(), ClientError> {
        let stub = Arc::new(StubTransport::default());
        let client = client_with(Arc::clone(&stub));
        let named = |name: Option<&str>| StartOptions {
            idempotency_key: Some(String::from("retry-key")),
            display_name: name.map(str::to_owned),
            ..StartOptions::default()
        };

        let first = client
            .start(
                "checkout",
                payload("input"),
                named(Some("Nightly settlement")),
            )
            .await?;
        assert_eq!(
            first.display_name_not_applied, None,
            "a first start applies the name it asked for"
        );

        // Same act, different label: one run, and the difference is REPORTED.
        let replayed = client
            .start("checkout", payload("input"), named(Some("Something else")))
            .await?;
        assert_eq!(
            replayed.handle, first.handle,
            "the label carries no identity, so the act deduped to one run"
        );
        assert_eq!(
            replayed.display_name_not_applied,
            Some(DisplayNameNotApplied {
                requested: String::from("Something else"),
                standing: Some(String::from("Nightly settlement")),
            }),
            "the name that was NOT applied must be reported, never dropped in silence"
        );

        // Asking for NO name drops nothing, so it reports nothing — even
        // against a run that wears a name of its own. See the four-combination
        // test below for the whole rule.
        let replayed = client
            .start("checkout", payload("input"), named(None))
            .await?;
        assert_eq!(
            replayed.display_name_not_applied, None,
            "a caller that requested no name had no name dropped"
        );

        // Asking for the SAME name is not a difference and reports nothing.
        let replayed = client
            .start(
                "checkout",
                payload("input"),
                named(Some("Nightly settlement")),
            )
            .await?;
        assert_eq!(replayed.display_name_not_applied, None);

        // Surrounding whitespace is not a difference either: the server trims
        // before recording, so the run already wears the trimmed name.
        let replayed = client
            .start(
                "checkout",
                payload("input"),
                named(Some("  Nightly settlement  ")),
            )
            .await?;
        assert_eq!(
            replayed.display_name_not_applied, None,
            "whitespace the server would have erased is not an unapplied name"
        );

        // Exactly ONE start reached the transport across all five calls.
        assert_eq!(
            stub.start_calls.load(std::sync::atomic::Ordering::SeqCst),
            1
        );
        Ok(())
    }

    /// #211: the WHOLE rule for when an idempotent replay reports an unapplied
    /// display name, one case per combination of what the replay asked for and
    /// what the standing run wears.
    ///
    /// The report is raised only when the caller ASKED for a name the standing
    /// run does not wear. A caller that asked for nothing dropped nothing, so
    /// it is told nothing — the report's absence is exactly the statement that
    /// no requested name went missing, and firing it for an empty request would
    /// say the opposite of the truth.
    #[tokio::test]
    async fn the_unapplied_name_report_fires_only_for_a_replay_that_asked_for_a_name()
    -> Result<(), ClientError> {
        /// One combination: what the FIRST start requested (and so what the
        /// standing run wears), what the REPLAY requests, and the report the
        /// replay must produce.
        struct Case {
            label: &'static str,
            standing: Option<&'static str>,
            requested: Option<&'static str>,
            expected: Option<DisplayNameNotApplied>,
        }

        let cases = [
            Case {
                label: "requested none, standing none",
                standing: None,
                requested: None,
                expected: None,
            },
            Case {
                label: "requested none, standing some",
                standing: Some("Nightly"),
                requested: None,
                expected: None,
            },
            Case {
                label: "requested some, standing none",
                standing: None,
                requested: Some("Nightly"),
                expected: Some(DisplayNameNotApplied {
                    requested: String::from("Nightly"),
                    standing: None,
                }),
            },
            Case {
                label: "requested some, standing a different some",
                standing: Some("Nightly"),
                requested: Some("Weekly"),
                expected: Some(DisplayNameNotApplied {
                    requested: String::from("Weekly"),
                    standing: Some(String::from("Nightly")),
                }),
            },
            Case {
                label: "requested some, standing the same some",
                standing: Some("Nightly"),
                requested: Some("Nightly"),
                expected: None,
            },
        ];

        for Case {
            label,
            standing,
            requested,
            expected,
        } in cases
        {
            let stub = Arc::new(StubTransport::default());
            let client = client_with(Arc::clone(&stub));
            let named = |name: Option<&str>| StartOptions {
                idempotency_key: Some(String::from("retry-key")),
                display_name: name.map(str::to_owned),
                ..StartOptions::default()
            };

            let first = client
                .start("checkout", payload("input"), named(standing))
                .await?;
            assert_eq!(
                first.display_name_not_applied, None,
                "{label}: a first start applies the name it asked for"
            );

            let replayed = client
                .start("checkout", payload("input"), named(requested))
                .await?;
            assert_eq!(
                replayed.handle, first.handle,
                "{label}: the name carries no identity, so the act deduped to one run"
            );
            assert_eq!(
                replayed.display_name_not_applied, expected,
                "{label}: the report must fire exactly when a REQUESTED name was not applied"
            );
            assert_eq!(
                stub.start_calls.load(std::sync::atomic::Ordering::SeqCst),
                1,
                "{label}: the replay must not reach the transport"
            );
        }
        Ok(())
    }

    /// #211: a blank display name is refused at the SDK boundary, matching the
    /// server, which refuses a present-but-blank name with `invalid_input`
    /// rather than reading it as "unnamed". Absence is how a caller says
    /// "unnamed"; forwarding a blank would only buy a round trip to the same
    /// refusal.
    #[tokio::test]
    async fn a_blank_display_name_is_refused() -> Result<(), ClientError> {
        let stub = Arc::new(StubTransport::default());
        let client = client_with(Arc::clone(&stub));

        for blank in ["", "   ", "\t\n "] {
            let result = client
                .start(
                    "checkout",
                    payload("input"),
                    StartOptions {
                        display_name: Some(String::from(blank)),
                        ..StartOptions::default()
                    },
                )
                .await;
            assert!(
                matches!(result, Err(ClientError::InvalidArgument { .. })),
                "blank {blank:?} must be refused, got {result:?}"
            );
        }
        assert_eq!(
            stub.start_calls.load(std::sync::atomic::Ordering::SeqCst),
            0
        );
        Ok(())
    }

    #[tokio::test]
    async fn start_idempotency_treats_a_changed_route_as_a_different_request()
    -> Result<(), ClientError> {
        for (label, second) in [
            (
                "task_queue",
                StartOptions {
                    task_queue: Some(String::from("payments")),
                    ..StartOptions::default()
                },
            ),
            (
                "routing_key",
                StartOptions {
                    routing_key: Some(String::from("tenant-7")),
                    ..StartOptions::default()
                },
            ),
        ] {
            let stub = Arc::new(StubTransport::default());
            let client = client_with(Arc::clone(&stub));
            let key = Some(String::from("retry-key"));
            let first = StartOptions {
                idempotency_key: key.clone(),
                ..StartOptions::default()
            };
            let second = StartOptions {
                idempotency_key: key,
                ..second
            };

            client.start("checkout", payload("input"), first).await?;
            let conflict = client.start("checkout", payload("input"), second).await;

            assert!(
                matches!(conflict, Err(ClientError::AlreadyExists { .. })),
                "reusing a key with a different {label} must conflict, got {conflict:?}"
            );
        }
        Ok(())
    }

    #[tokio::test]
    async fn signal_maps_latest_run_and_error() {
        let stub = Arc::new(StubTransport::default());
        *stub.signal_error.lock().await = Some(ClientError::not_found("workflow was not found"));
        let client = client_with(Arc::clone(&stub));
        let id = workflow_id();

        let result = client.signal(&id, None, "approve", payload("signal")).await;

        assert_eq!(
            result,
            Err(ClientError::not_found("workflow was not found"))
        );
        let recorded = stub.last_signal.lock().await.clone();
        assert!(recorded.is_some());
        let Some(request) = recorded else {
            return;
        };
        assert!(request.run_id.is_none());
    }

    #[tokio::test]
    async fn query_maps_result_error_and_deadline() -> Result<(), ClientError> {
        let stub = Arc::new(StubTransport::default());
        *stub.query_response.lock().await = Some(Ok(ProtoQueryResponse {
            outcome: Some(proto_query_response::Outcome::Error(
                aion_proto::ProtoWireError::from(WireError::query_timeout("slow")),
            )),
        }));
        let client = client_with(Arc::clone(&stub));
        let id = workflow_id();

        let result = client
            .query(
                &id,
                Some(&run_id()),
                "state",
                Payload::json_null(),
                Duration::from_secs(1),
            )
            .await;

        assert_eq!(result, Err(ClientError::query_timeout("slow")));
        let recorded = stub.last_query.lock().await.clone();
        assert!(recorded.is_some());
        let request = recorded.ok_or_else(|| ClientError::server("missing query"))?;
        assert!(request.run_id.is_some());
        Ok(())
    }

    #[tokio::test]
    async fn query_forwards_its_arguments_onto_the_wire() -> Result<(), ClientError> {
        let stub = Arc::new(StubTransport::default());
        *stub.query_response.lock().await = Some(Ok(ProtoQueryResponse {
            outcome: Some(proto_query_response::Outcome::Result(
                aion_proto::ProtoPayload::from(payload("answer")),
            )),
        }));
        let client = client_with(Arc::clone(&stub));

        let returned = client
            .query(
                &workflow_id(),
                Some(&run_id()),
                "state",
                payload("args"),
                Duration::from_secs(1),
            )
            .await?;

        assert_eq!(returned, payload("answer"));
        let request = stub
            .last_query
            .lock()
            .await
            .clone()
            .ok_or_else(|| ClientError::server("missing query"))?;
        // The caller's arguments reach the wire byte-exact: this client is a
        // carrier, not a place that reshapes or drops the request.
        assert_eq!(
            request.arguments,
            Some(aion_proto::ProtoPayload::from(payload("args")))
        );
        Ok(())
    }

    #[tokio::test]
    async fn a_no_argument_query_sends_the_canonical_null_document() -> Result<(), ClientError> {
        let stub = Arc::new(StubTransport::default());
        *stub.query_response.lock().await = Some(Ok(ProtoQueryResponse {
            outcome: Some(proto_query_response::Outcome::Result(
                aion_proto::ProtoPayload::from(payload("answer")),
            )),
        }));
        let client = client_with(Arc::clone(&stub));

        // `&()` is how a caller says "this query takes nothing"; it must reach
        // the wire as the same `null` document every other carrier sends, not
        // as empty bytes no decoder can read.
        let _: serde_json::Value = client
            .query_typed(
                &workflow_id(),
                Some(&run_id()),
                "state",
                &(),
                Duration::from_secs(1),
            )
            .await?;

        let request = stub
            .last_query
            .lock()
            .await
            .clone()
            .ok_or_else(|| ClientError::server("missing query"))?;
        assert_eq!(
            request.arguments,
            Some(aion_proto::ProtoPayload::from(Payload::json_null()))
        );
        Ok(())
    }

    #[tokio::test]
    async fn query_failed_outcome_error_maps_to_query_failed() -> Result<(), ClientError> {
        let stub = Arc::new(StubTransport::default());
        *stub.query_response.lock().await = Some(Ok(ProtoQueryResponse {
            outcome: Some(proto_query_response::Outcome::Error(
                aion_proto::ProtoWireError::from(WireError::query_failed("handler raised")),
            )),
        }));
        let client = client_with(Arc::clone(&stub));

        let result = client
            .query(
                &workflow_id(),
                Some(&run_id()),
                "state",
                Payload::json_null(),
                Duration::from_secs(1),
            )
            .await;

        assert_eq!(result, Err(ClientError::query_failed("handler raised")));
        Ok(())
    }

    #[tokio::test]
    async fn backend_outcome_error_is_a_server_fault_not_query_failed() -> Result<(), ClientError> {
        // `backend` in QueryResponse.error is an unexpected server fault; the
        // application-level handler failure has its own `query_failed` code.
        let stub = Arc::new(StubTransport::default());
        *stub.query_response.lock().await = Some(Ok(ProtoQueryResponse {
            outcome: Some(proto_query_response::Outcome::Error(
                aion_proto::ProtoWireError::from(WireError::backend("store down")),
            )),
        }));
        let client = client_with(Arc::clone(&stub));

        let result = client
            .query(
                &workflow_id(),
                Some(&run_id()),
                "state",
                Payload::json_null(),
                Duration::from_secs(1),
            )
            .await;

        assert_eq!(result, Err(ClientError::server("store down")));
        Ok(())
    }

    #[tokio::test]
    async fn query_typed_decodes_no_arg_query_result() -> Result<(), ClientError> {
        #[derive(serde::Deserialize, PartialEq, Eq, Debug)]
        struct QueryResult {
            label: String,
        }

        let stub = Arc::new(StubTransport::default());
        let client = client_with(Arc::clone(&stub));
        let id = workflow_id();

        let result: QueryResult = client
            .query_typed(&id, Some(&run_id()), "state", &(), Duration::from_secs(1))
            .await?;

        assert_eq!(
            result,
            QueryResult {
                label: String::from("result")
            }
        );
        assert!(stub.last_query.lock().await.is_some());
        Ok(())
    }

    /// The anti-silent-drop property, stated positively.
    ///
    /// This test used to assert that non-empty typed arguments were REFUSED,
    /// because the wire could not carry them and dropping them silently was
    /// the failure to avoid. The wire carries them now, so the same property
    /// is proven the other way: the exact serialized document reaches the
    /// request. A regression that dropped arguments again would leave
    /// `arguments` absent and fail here — the refusal test could not have
    /// caught that, since it never inspected a forwarded request.
    #[tokio::test]
    async fn query_typed_forwards_non_empty_args_without_silent_drop() -> Result<(), ClientError> {
        let stub = Arc::new(StubTransport::default());
        let client = client_with(Arc::clone(&stub));
        let id = workflow_id();

        let _: serde_json::Value = client
            .query_typed(
                &id,
                Some(&run_id()),
                "state",
                &serde_json::json!({ "filter": "open" }),
                Duration::from_secs(1),
            )
            .await?;

        let request = stub
            .last_query
            .lock()
            .await
            .clone()
            .ok_or_else(|| ClientError::server("missing query"))?;
        let arguments = request
            .arguments
            .ok_or_else(|| ClientError::server("query arguments were dropped"))?;
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&arguments.bytes)
                .map_err(|error| ClientError::server(error.to_string()))?,
            serde_json::json!({ "filter": "open" })
        );
        Ok(())
    }

    #[tokio::test]
    async fn cancel_list_and_describe_map_requests() -> Result<(), ClientError> {
        let stub = Arc::new(StubTransport::default());
        let client = client_with(Arc::clone(&stub));
        let id = workflow_id();
        let run = run_id();

        client.cancel(&id, Some(&run), "not needed").await?;
        let listed = client
            .list(&WorkflowFilter::default(), ListPage::default())
            .await?;
        let described = client.describe(&id, None).await?;

        assert!(stub.last_cancel.lock().await.is_some());
        assert!(stub.last_list.lock().await.is_some());
        let describe = stub
            .last_describe
            .lock()
            .await
            .clone()
            .ok_or_else(|| ClientError::server("missing describe"))?;
        assert!(describe.run_id.is_none());
        assert!(describe.include_history);
        assert_eq!(listed.len(), 1);
        assert_eq!(described.history.len(), 0);
        Ok(())
    }

    #[tokio::test]
    async fn reopen_returns_running_run_and_maps_request() -> Result<(), ClientError> {
        let stub = Arc::new(StubTransport::default());
        let client = client_with(Arc::clone(&stub));
        let id = workflow_id();
        let run = run_id();

        let outcome = client.reopen(&id, Some(&run)).await?;

        assert_eq!(outcome.status, WorkflowStatus::Running);
        let request = stub
            .last_reopen
            .lock()
            .await
            .clone()
            .ok_or_else(|| ClientError::server("missing reopen"))?;
        assert_eq!(request.namespace, "tenant-a");
        assert!(request.run_id.is_some());
        Ok(())
    }

    /// The `InvalidState` wire code maps to the distinct typed
    /// [`ClientError::InvalidState`], never conflated with not-found.
    #[tokio::test]
    async fn reopen_maps_invalid_state_to_distinct_typed_error() -> Result<(), ClientError> {
        let stub = Arc::new(StubTransport::default());
        *stub.reopen_response.lock().await = Some(Err(ClientError::from_wire_error(
            WireError::invalid_state_with_type("InvalidState", "run is not reopenable"),
        )));
        let client = client_with(Arc::clone(&stub));

        let result = client.reopen(&workflow_id(), None).await;

        assert!(
            matches!(result, Err(ClientError::InvalidState { .. })),
            "got {result:?}"
        );
        Ok(())
    }
}
