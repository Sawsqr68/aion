//! Transport backed by an in-process [`aion::Engine`].
//!
//! Event subscriptions honour the same resume/replay-splice contract as the
//! server's `/events/stream` endpoint, built directly on [`aion::Engine`]
//! seams (`Engine::subscribe` for the live broadcast, `engine.store()` for
//! the history snapshot — never a client-held stream over engine internals):
//!
//! 1. attach the live broadcast subscription FIRST (time T0);
//! 2. snapshot recorded history via `engine.store().read_history` (T1 > T0);
//! 3. validate the cursor against the snapshot head;
//! 4. splice: replay `[resume_from_seq ..= head]` from the snapshot, then the
//!    live tail filtered to `seq > head`.
//!
//! Gap-free: publish strictly follows durable commit, so every event with
//! `seq > head` was committed — and therefore broadcast — after T0.
//! Duplicate-free: the live filter drops every `seq <= head`, so an event
//! present in both the snapshot and the broadcast is emitted exactly once,
//! from the snapshot. Engine-side lag is never silent: each
//! `Err(EventStreamLagged)` item surfaces as `Err(ClientError::Unavailable)`
//! so the resume loop reconnects with its cursor.

use std::sync::Arc;

use aion_core::Event;
use async_trait::async_trait;
use futures::stream::BoxStream;
use futures::{StreamExt, stream};

use crate::error::ClientError;
use crate::transport::contract::{SubscriptionAttempt, WorkflowTransport};

/// Transport backed by an in-process [`aion::Engine`].
pub struct EmbeddedWorkflowTransport {
    engine: Arc<aion::Engine>,
}

impl EmbeddedWorkflowTransport {
    /// Creates an embedded transport for `engine`.
    #[must_use]
    pub fn new(engine: Arc<aion::Engine>) -> Self {
        Self { engine }
    }

    /// Resolve the target run id: the supplied one, or the latest run from the
    /// workflow's run chain when omitted (mirrors the server's `resolve_run_id`).
    async fn resolve_run_id(
        &self,
        workflow_id: &aion_core::WorkflowId,
        run_id: Option<aion_proto::ProtoRunId>,
    ) -> Result<aion_core::RunId, ClientError> {
        if let Some(run_id) = run_id {
            return run_id.try_into().map_err(ClientError::from_wire_error);
        }
        let chain = self
            .engine
            .store()
            .read_run_chain(workflow_id)
            .await
            .map_err(|error| store_error_class(&error, error.to_string()))?;
        chain
            .last()
            .map(|summary| summary.run_id.clone())
            .ok_or_else(|| ClientError::not_found(format!("workflow {workflow_id} not found")))
    }
}

#[async_trait]
impl WorkflowTransport for EmbeddedWorkflowTransport {
    async fn start_workflow(
        &self,
        request: aion_proto::ProtoStartWorkflowRequest,
    ) -> Result<aion_proto::ProtoStartWorkflowResponse, ClientError> {
        let input = request
            .input
            .ok_or_else(|| ClientError::invalid_argument("start request input payload is missing"))
            .and_then(|payload| {
                aion_core::Payload::try_from(payload).map_err(ClientError::from_wire_error)
            })?;
        // The embedded engine is single-tenant and in-process: there is no
        // namespace authority stamping visibility attributes, so the start
        // carries no search attributes.
        let handle = self
            .engine
            .start_workflow(
                &request.workflow_type,
                input,
                std::collections::HashMap::new(),
                String::from("default"),
            )
            .await
            .map_err(|error| map_engine_error(&error))?;
        Ok(aion_proto::ProtoStartWorkflowResponse {
            workflow_id: Some(aion_proto::ProtoWorkflowId::from(
                handle.workflow_id().clone(),
            )),
            run_id: Some(aion_proto::ProtoRunId::from(handle.run_id().clone())),
        })
    }

    async fn signal(
        &self,
        request: aion_proto::ProtoSignalRequest,
    ) -> Result<aion_proto::ProtoSignalResponse, ClientError> {
        let workflow_id = decode_required_workflow_id(request.workflow_id)?;
        let run_id = decode_required_run_id(request.run_id)?;
        let payload = request
            .payload
            .ok_or_else(|| ClientError::invalid_argument("signal request payload is missing"))
            .and_then(|payload| {
                aion_core::Payload::try_from(payload).map_err(ClientError::from_wire_error)
            })?;
        self.engine
            .signal(&workflow_id, &run_id, request.signal_name, payload)
            .await
            .map_err(|error| map_engine_error(&error))?;
        Ok(aion_proto::ProtoSignalResponse {})
    }

    async fn query(
        &self,
        request: aion_proto::ProtoQueryRequest,
    ) -> Result<aion_proto::ProtoQueryResponse, ClientError> {
        let workflow_id = decode_required_workflow_id(request.workflow_id)?;
        let run_id = decode_required_run_id(request.run_id)?;
        // Absent arguments mean the caller supplied none; the handler still
        // receives one well-formed document.
        let arguments = request.arguments.map_or_else(
            || Ok(aion_core::Payload::json_null()),
            |arguments| {
                aion_core::Payload::try_from(arguments).map_err(ClientError::from_wire_error)
            },
        )?;
        let payload = self
            .engine
            .query(&workflow_id, &run_id, request.query_name, arguments)
            .await
            .map_err(|error| map_engine_error(&error))?;
        Ok(aion_proto::ProtoQueryResponse {
            outcome: Some(aion_proto::proto_query_response::Outcome::Result(
                aion_proto::ProtoPayload::from(payload),
            )),
        })
    }

    async fn cancel(
        &self,
        request: aion_proto::ProtoCancelRequest,
    ) -> Result<aion_proto::ProtoCancelResponse, ClientError> {
        let workflow_id = decode_required_workflow_id(request.workflow_id)?;
        let run_id = decode_required_run_id(request.run_id)?;
        self.engine
            .cancel(&workflow_id, &run_id, request.reason)
            .await
            .map_err(|error| map_engine_error(&error))?;
        Ok(aion_proto::ProtoCancelResponse {})
    }

    async fn reopen(
        &self,
        request: aion_proto::ProtoReopenRequest,
    ) -> Result<aion_proto::ProtoReopenResponse, ClientError> {
        let workflow_id = decode_required_workflow_id(request.workflow_id)?;
        let run_id = self.resolve_run_id(&workflow_id, request.run_id).await?;
        let handle = self
            .engine
            .reopen_workflow(&workflow_id, &run_id)
            .await
            .map_err(|error| map_engine_error(&error))?;
        Ok(aion_proto::ProtoReopenResponse {
            run_id: Some(handle.run_id().clone().into()),
            status: aion_proto::ProtoWorkflowStatus::from(handle.cached_status()) as i32,
        })
    }

    async fn pause(
        &self,
        request: aion_proto::ProtoPauseRequest,
    ) -> Result<aion_proto::ProtoPauseResponse, ClientError> {
        let workflow_id = decode_required_workflow_id(request.workflow_id)?;
        let run_id = self.resolve_run_id(&workflow_id, request.run_id).await?;
        let reason = if request.reason.is_empty() {
            None
        } else {
            Some(request.reason)
        };
        let handle = self
            .engine
            .pause_workflow(&workflow_id, &run_id, reason, None)
            .await
            .map_err(|error| map_engine_error(&error))?;
        Ok(aion_proto::ProtoPauseResponse {
            run_id: Some(handle.run_id().clone().into()),
            status: aion_proto::ProtoWorkflowStatus::Paused as i32,
        })
    }

    async fn resume(
        &self,
        request: aion_proto::ProtoResumeRequest,
    ) -> Result<aion_proto::ProtoResumeResponse, ClientError> {
        let workflow_id = decode_required_workflow_id(request.workflow_id)?;
        let run_id = self.resolve_run_id(&workflow_id, request.run_id).await?;
        let handle = self
            .engine
            .resume_paused_workflow(&workflow_id, &run_id, None)
            .await
            .map_err(|error| map_engine_error(&error))?;
        Ok(aion_proto::ProtoResumeResponse {
            run_id: Some(handle.run_id().clone().into()),
            status: aion_proto::ProtoWorkflowStatus::Running as i32,
        })
    }

    async fn list_workflows(
        &self,
        request: aion_proto::ProtoListWorkflowsRequest,
    ) -> Result<aion_proto::ProtoListWorkflowsResponse, ClientError> {
        let filter = match request.filter.as_ref() {
            Some(filter) => {
                aion_proto::decode_workflow_filter(filter).map_err(ClientError::from_wire_error)?
            }
            None => aion_core::WorkflowFilter::default(),
        };
        let summaries = self
            .engine
            .list_workflows(filter)
            .await
            .map_err(|error| map_engine_error(&error))?
            .iter()
            .map(|summary| {
                aion_proto::encode_workflow_summary(request.namespace.clone(), None, summary)
            })
            .map(|result| result.map_err(ClientError::from_wire_error))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(aion_proto::ProtoListWorkflowsResponse { summaries })
    }

    async fn describe_workflow(
        &self,
        request: aion_proto::ProtoDescribeWorkflowRequest,
    ) -> Result<aion_proto::ProtoDescribeWorkflowResponse, ClientError> {
        let workflow_id = decode_required_workflow_id(request.workflow_id)?;
        let run_id = self.resolve_run_id(&workflow_id, request.run_id).await?;
        let history = self
            .engine
            .store()
            .read_history(&workflow_id)
            .await
            // This read goes straight to the store, so the refusal arrives with
            // no `EngineError` around it — but it is the same refusal, and a
            // `NotOwner` here means exactly what it means everywhere else: ask a
            // different owner. Collapsing it into `server` told the caller its
            // request was unanswerable when a re-route would have served it.
            .map_err(|error| store_error_class(&error, error.to_string()))?;
        let Some(summary) = aion_core::WorkflowSummary::from_history(&history) else {
            return Err(ClientError::not_found(format!(
                "workflow {workflow_id} has no recorded history"
            )));
        };
        let summary = Some(
            aion_proto::encode_workflow_summary(request.namespace.clone(), None, &summary)
                .map_err(ClientError::from_wire_error)?,
        );
        let history_head_seq = history.last().map_or(0, aion_core::Event::seq);
        let terminal_event = aion_core::current_lease_terminal(&history)
            .map(|event| aion_proto::encode_event(request.namespace.clone(), None, event))
            .transpose()
            .map_err(ClientError::from_wire_error)?;
        let history = if request.include_history {
            history
                .iter()
                .map(|event| aion_proto::encode_event(request.namespace.clone(), None, event))
                .map(|result| result.map_err(ClientError::from_wire_error))
                .collect::<Result<Vec<_>, _>>()?
        } else {
            Vec::new()
        };
        Ok(aion_proto::ProtoDescribeWorkflowResponse {
            summary,
            history,
            run_id: Some(run_id.into()),
            history_head_seq,
            terminal_event,
        })
    }

    async fn read_history(
        &self,
        request: aion_proto::ProtoReadHistoryRequest,
    ) -> Result<aion_proto::ProtoReadHistoryResponse, ClientError> {
        let workflow_id = decode_required_workflow_id(request.workflow_id)?;
        let from_seq = request.from_seq.unwrap_or(0);
        let mut events = self
            .engine
            .store()
            .read_history_from(&workflow_id, from_seq)
            .await
            .map_err(|error| store_error_class(&error, error.to_string()))?;
        let head_seq = events.last().map_or(0, aion_core::Event::seq);
        let next_from_seq = request
            .limit
            .and_then(|limit| events.get(limit as usize).map(aion_core::Event::seq));
        if let Some(limit) = request.limit {
            events.truncate(limit as usize);
        }
        let events = events
            .iter()
            .map(|event| aion_proto::encode_event(request.namespace.clone(), None, event))
            .map(|result| result.map_err(ClientError::from_wire_error))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(aion_proto::ProtoReadHistoryResponse {
            events,
            next_from_seq,
            head_seq,
        })
    }

    async fn subscribe(
        &self,
        request: aion_proto::SubscriptionRequest,
        resume_from_sequence: Option<u64>,
    ) -> Result<SubscriptionAttempt, ClientError> {
        let (workflow_target, filter) = embedded_subscription_target(request)?;
        // T0: attach to the live broadcast BEFORE any history snapshot — one
        // half of the gap-free splice proof (mirrors the server's
        // subscribe-then-snapshot ordering).
        let live = self.engine.subscribe(filter);
        let events = match (&workflow_target, resume_from_sequence) {
            (Some(workflow_id), Some(resume_from_seq)) => {
                // T1 (> T0): snapshot recorded history, then validate the
                // cursor against its head and build the dedupe splice.
                let history = self
                    .engine
                    .store()
                    .read_history(workflow_id)
                    .await
                    .map_err(|error| store_error_class(&error, error.to_string()))?;
                splice_resume(live, history, resume_from_seq)?
            }
            (None, Some(_)) => {
                return Err(ClientError::invalid_argument(
                    "filtered and firehose event streams are live-only by design; resume \
                     cursors are valid for per-workflow subscriptions only",
                ));
            }
            (_, None) => map_lag(live),
        };
        // Per-workflow streams end at the run's terminal event, exactly like
        // the server socket; callers walk continue-as-new chains by
        // resubscribing with their cursor.
        Ok(SubscriptionAttempt::new(match workflow_target {
            Some(_) => close_after_terminal(events),
            None => events,
        }))
    }
}

/// Validates a resume cursor against a history snapshot and builds the
/// replay/live splice (see the module docs for the gap/duplicate proof).
fn splice_resume(
    live: BoxStream<'static, Result<Event, aion::EventStreamLagged>>,
    history: Vec<Event>,
    resume_from_seq: u64,
) -> Result<BoxStream<'static, Result<Event, ClientError>>, ClientError> {
    if resume_from_seq == 0 {
        return Err(ClientError::invalid_argument(
            "resume_from_seq must be >= 1 (the first sequence number wanted)",
        ));
    }
    let head = history.last().map_or(0, Event::seq);
    if resume_from_seq > head.saturating_add(1) {
        return Err(ClientError::invalid_argument(format!(
            "resume_from_seq {resume_from_seq} is ahead of recorded history (head seq {head}); \
             the largest valid cursor is {}",
            head.saturating_add(1)
        )));
    }

    let mut history = history;
    let replay_start = history.partition_point(|event| event.seq() < resume_from_seq);
    let replay = history.split_off(replay_start);
    let tail = live.filter(move |item| {
        let keep = match item {
            Ok(event) => event.seq() > head,
            // Lag is information, never filtered away.
            Err(aion::EventStreamLagged { .. }) => true,
        };
        futures::future::ready(keep)
    });

    Ok(stream::iter(replay.into_iter().map(Ok))
        .chain(map_lag(tail.boxed()))
        .boxed())
}

/// Maps engine-side lag items to retryable [`ClientError::Unavailable`] so
/// the resume loop reconnects with its cursor instead of silently gapping.
fn map_lag(
    live: BoxStream<'static, Result<Event, aion::EventStreamLagged>>,
) -> BoxStream<'static, Result<Event, ClientError>> {
    live.map(|item| {
        item.map_err(|lagged| {
            ClientError::from_wire_error(aion_proto::WireError::lagged(lagged.to_string()))
        })
    })
    .boxed()
}

/// Ends the stream after the first terminal workflow event, mirroring the
/// server socket's per-workflow run-boundary close.
fn close_after_terminal(
    events: BoxStream<'static, Result<Event, ClientError>>,
) -> BoxStream<'static, Result<Event, ClientError>> {
    stream::unfold(Some(events), |state| async move {
        let mut events = state?;
        let item = events.next().await?;
        // The terminal event is delivered and the inner stream is dropped
        // immediately afterwards (releasing the broadcast receiver), so the
        // close is eager — it never waits for a further event to be polled.
        let closed = matches!(&item, Ok(event) if is_terminal_workflow_event(event));
        Some((item, if closed { None } else { Some(events) }))
    })
    .boxed()
}

fn is_terminal_workflow_event(event: &Event) -> bool {
    matches!(
        event,
        Event::WorkflowCompleted { .. }
            | Event::WorkflowFailed { .. }
            | Event::WorkflowCancelled { .. }
            | Event::WorkflowTimedOut { .. }
            | Event::WorkflowContinuedAsNew { .. }
    )
}

fn decode_required_workflow_id(
    value: Option<aion_proto::ProtoWorkflowId>,
) -> Result<aion_core::WorkflowId, ClientError> {
    value
        .ok_or_else(|| ClientError::invalid_argument("request workflow id is missing"))?
        .try_into()
        .map_err(ClientError::from_wire_error)
}

fn decode_required_run_id(
    value: Option<aion_proto::ProtoRunId>,
) -> Result<aion_core::RunId, ClientError> {
    value
        .ok_or_else(|| ClientError::invalid_argument("request run id is missing"))?
        .try_into()
        .map_err(ClientError::from_wire_error)
}

/// Maps a wire subscription request onto the engine filter surface plus the
/// per-workflow target the splice and run-boundary close key on.
fn embedded_subscription_target(
    request: aion_proto::SubscriptionRequest,
) -> Result<(Option<aion_core::WorkflowId>, aion::EventFilter), ClientError> {
    match request.subscription {
        Some(aion_proto::subscription_request::Subscription::PerWorkflow(subscription)) => {
            let workflow_id = subscription
                .workflow_id
                .ok_or_else(|| {
                    ClientError::invalid_argument(
                        "per-workflow subscription requires a workflow id",
                    )
                })?
                .try_into()
                .map_err(ClientError::from_wire_error)?;
            Ok((
                Some(aion_core::WorkflowId::clone(&workflow_id)),
                aion::EventFilter {
                    workflow_id: Some(workflow_id),
                    run: None,
                    family: None,
                },
            ))
        }
        Some(
            aion_proto::subscription_request::Subscription::Filtered(_)
            | aion_proto::subscription_request::Subscription::Firehose(_),
        ) => Ok((None, aion::EventFilter::default())),
        Some(aion_proto::subscription_request::Subscription::Cluster(_)) => {
            // The WS3 cluster topology/ownership channel is a server-side
            // projection of distributed cluster state. The embedded in-process
            // transport drives a single local engine with no cluster topology to
            // project, so a cluster subscription is not serviceable here; reject it
            // cleanly rather than silently degrading to a workflow event stream.
            Err(ClientError::invalid_argument(
                "cluster topology subscriptions are not supported by the embedded in-process \
                 transport; connect to an aion-server over gRPC/WebSocket to subscribe to the \
                 cluster channel",
            ))
        }
        Some(aion_proto::subscription_request::Subscription::Transcript(_)) => {
            // The NOI-5b agent-observability transcript channel is a server-side
            // projection over the durable `O` keyspace + the server's transcript
            // sequencer. The embedded in-process transport has no such server
            // bridge, so a transcript subscription is not serviceable here; reject
            // it cleanly rather than degrading to a workflow event stream.
            Err(ClientError::invalid_argument(
                "agent-observability transcript subscriptions are not supported by the embedded \
                 in-process transport; connect to an aion-server over gRPC/WebSocket to subscribe \
                 to the transcript channel",
            ))
        }
        None => Err(ClientError::invalid_argument(
            "subscription request is missing its subscription variant",
        )),
    }
}

/// Translate an engine failure into the client-facing error class.
///
/// # The `_` arm is a family default, and it is not free
///
/// `EngineError` has 51 variants and is not `#[non_exhaustive]`, so an
/// exhaustive match here *would* compile-fail on every new variant. That is
/// deliberately not what this does: a 51-arm list in a transport adapter is a
/// list nothing reads, and it would go stale as silently as the wildcard does.
/// The wildcard is chosen, with its cost stated — **a new variant lands in the
/// generic server bucket and the compiler will not say so.**
///
/// What makes that acceptable is the direction it fails in. `server` is the
/// "something went wrong, this is not your request's fault and not a wait"
/// class; landing there is uninformative, never *wrong* in a way that makes a
/// caller retry something unretryable or give up on something transient. Every
/// arm above it exists because its variant would be actively mis-served by that
/// default — a not-found retried forever, a shutdown reported as an engine bug.
///
/// So the rule for adding an arm is not "is this variant new" but "would the
/// generic bucket mislead a caller about what to DO". Only those get named.
fn map_engine_error(error: &aion::EngineError) -> ClientError {
    match error {
        aion::EngineError::WorkflowNotFound { .. } => ClientError::not_found(error.to_string()),
        // Reopen precondition failure (AD-012): distinct typed variant, never
        // conflated with not-found or the generic server bucket.
        aion::EngineError::InvalidState { .. } => ClientError::invalid_state(error.to_string()),
        // The caller's own payload did not satisfy the contract the target
        // package declares. Both are raised BEFORE anything is recorded, so
        // there is nothing to clean up and nothing to retry — the remedy is to
        // send different bytes. `server` would say the opposite: "not your
        // fault, try again", which is how a caller ends up retrying a payload
        // that can never be accepted. `aion-server` classifies these two
        // through `declared_contract_wire` for the same reason; this is the
        // embedded surface reaching the same verdict, not a second opinion.
        aion::EngineError::StartInputRefused { .. } | aion::EngineError::SignalRefused { .. } => {
            ClientError::invalid_argument(error.to_string())
        }
        // Six refusals about the state of a run, or of the package a run would
        // start from, rather than the health of the engine — all reachable
        // through the embedded surface, all classified `invalid_state` by
        // `aion-server`, and all erased into "engine bug" by the generic
        // bucket, which is the wrong thing to say about any of them.
        //
        // Two are transient by construction (`TerminalWriterUnavailable`,
        // `TerminalWriterHeld`): a writer reservation lives across one terminal
        // transition. Two name a precondition the caller can act on: redeploy
        // and cancel through the ordinary path (`RunIsRecoverable`), or address
        // this engine's missing startup verdict (`NoResidencyVerdict`). The
        // last two are admission refusals raised before the run starts — the
        // package's declared contract does not identify what the request names
        // (`ContractIdentity`), or it declares no task queue to serve from
        // (`NoQueueDeclaration`) — and the remedy for both is a redeploy.
        aion::EngineError::TerminalWriterUnavailable { .. }
        | aion::EngineError::TerminalWriterHeld { .. }
        | aion::EngineError::RunIsRecoverable { .. }
        | aion::EngineError::NoResidencyVerdict { .. }
        | aion::EngineError::ContractIdentity { .. }
        | aion::EngineError::NoQueueDeclaration { .. } => {
            ClientError::invalid_state(error.to_string())
        }
        // 🔴 BOTH SHAPES A STORE REFUSAL ARRIVES IN, CLASSIFIED IN ONE PLACE.
        //
        // An earlier revision matched only the bare `Store(..)` shape and its
        // test hand-built that shape, so the arm looked covered and fired on
        // nothing a caller could provoke: every durable write these operations
        // trigger goes through the `Recorder`, which returns `DurabilityError`,
        // and the engine wraps that as `EngineError::Durability` — so the
        // production shape of a lost-ownership refusal is
        // `Durability(Store(NotOwner))`. Both shapes now route through
        // `store_error_class`, which is the only place the rule is written.
        aion::EngineError::Store(store)
        | aion::EngineError::Durability(aion::durability::DurabilityError::Store(store)) => {
            store_error_class(store, error.to_string())
        }
        // Live-query dispatch. `aion-server` gives this family five distinct
        // wire codes and the caller taxonomy has a class for each; collapsing
        // them into `server` left `unknown_query`, `query_timeout`,
        // `query_failed` and the query half of `not_running` unreachable
        // in-process — four classes that exist solely for this operation. The
        // same caller code branches correctly over the wire and could not
        // branch at all here.
        aion::EngineError::Query(query) => query_error_class(query, error),
        // This engine has stopped serving. `aion-server`'s `wire_from_engine`
        // (`error.rs:645`) answers `not_running`, and an over-the-wire caller of
        // these operations therefore sees `ClientError::NotRunning`; an earlier
        // revision of this arm said `unavailable`, which is the transport-level
        // class the SDK also raises for a dial failure. That told an in-process
        // caller its ENDPOINT was unreachable when what had happened was that
        // the engine it holds is shutting down.
        //
        // ⚠️ "Mirror the server" is not one answer — the server gives two, and
        // saying otherwise would hide a real divergence. `wire_from_engine` is
        // the mapping that governs the eight engine operations THIS transport
        // exposes, and it is the one mirrored here. Two control-plane handlers
        // answer `Unavailable` for the same variant —
        // `aion-server/src/authoring/handlers.rs:404-406` and
        // `api/handlers/deploy.rs:415` — and both serve deploy/authoring
        // operations that this transport does not expose, so neither is
        // reachable through a caller holding an `EmbeddedWorkflowTransport`.
        // Recorded rather than reconciled: a shared classifier is what would
        // actually force the two into agreement, and that needs a crate both
        // sides can depend on.
        //
        // `EngineTaskEpochClosed` deliberately has no arm. It is constructed at
        // exactly one site — the terminal-append boundary in the engine's
        // process-exit path — which is reached by the completion monitor, never
        // by any of the operations this transport exposes. An arm for it would
        // be a classification nobody can observe, and a wrong claim about what
        // this surface can return.
        // 🔴 CARRY THE DISCRIMINATOR, because the class alone is a false hint.
        // `not_running` is the right CLASS and mirrors `wire_from_engine` — but
        // the operator-facing hint for a bare `not_running` says the run is no
        // longer running and points at `aion list --status running`, which is
        // false twice here: the run is fine, it is the ENGINE that stopped
        // serving, and `aion list` would fail identically. That is the same
        // defect this lane's F4 fix cited when it moved `EngineTaskEpochClosed`
        // OFF `not_running`; leaving the discriminator behind would have this
        // lane arguing both sides of one rule.
        //
        // The wire surface already carries it — `wire_from_engine` builds
        // `not_running_with_type("ShuttingDown", …)` — so attaching it here
        // makes the embedded and wire surfaces agree rather than diverge, and
        // gives `render.rs` the one fact it needs to say something true.
        aion::EngineError::ShuttingDown => ClientError::NotRunning {
            detail: crate::ErrorDetail::with_type(error.to_string(), "ShuttingDown"),
        },
        _ => ClientError::server(error.to_string()),
    }
}

/// The class a store-layer refusal carries, whichever shape it reached
/// [`map_engine_error`] in.
///
/// Mirrors `aion-server`'s `wire_from_store` composed with
/// [`ClientError::from_wire_error`], so the same underlying refusal names the
/// same class whether the caller is in-process or across the wire.
///
/// `not_owner` is the one class that names a ROUTING failure — the endpoint
/// answered, it simply does not own this target's shard, so the remedy is a
/// DIFFERENT owner rather than a retry against this one. Collapsing it into
/// `server` costs the caller that distinction entirely: `server` says the
/// request is unanswerable, and a caller that believes it abandons work a
/// re-route would have served.
///
/// ⚠️ Nothing in this crate retries it AUTOMATICALLY, and an earlier revision of
/// this comment claimed otherwise. `aion-client`'s only retry classifier is
/// `stream.rs`'s `is_retryable`, which is `matches!(error, ClientError::
/// Unavailable { .. })` — `NotOwner` is not in it, so `ResumingEventStream`
/// terminates the stream on one. The distinction this arm preserves is
/// therefore one the CALLER acts on, and the surface that already does is the
/// CLI's operator hint (`aion-cli/src/render.rs:175-178`), which tells the
/// operator to retry or point `--endpoint` at another node instead of sending
/// them hunting a network fault that does not exist.
///
/// Takes the caller-facing `message` separately rather than deriving it, because
/// a store refusal reaches this transport in three shapes and only two of them
/// have an `EngineError` to render: wrapped as `EngineError::Store`, wrapped as
/// `EngineError::Durability(Store(..))`, and — at the three call sites that read
/// the store directly rather than through an engine API — as a bare
/// [`aion_store::StoreError`] with no wrapper at all. The rule is written once
/// and all three shapes are held to it.
fn store_error_class(error: &aion_store::StoreError, message: String) -> ClientError {
    match error {
        aion_store::StoreError::NotOwner { .. } => ClientError::not_owner(message),
        aion_store::StoreError::NotFound { .. } => ClientError::not_found(message),
        // A `SequenceConflict` is this codebase's single-writer invariant
        // violation — a double-writer bug, not a caller mistake and not an
        // idempotency conflict — and `Backend`/`Serialization` are
        // infrastructure faults. All three are the engine's to answer for, and
        // `from_wire_error` puts all three in the generic bucket for a caller
        // arriving over the wire.
        aion_store::StoreError::SequenceConflict { .. }
        | aion_store::StoreError::Backend(_)
        | aion_store::StoreError::Serialization(_) => ClientError::server(message),
    }
}

/// The class a live-query dispatch failure carries.
///
/// Mirrors `aion-server`'s `query_wire` composed with
/// [`ClientError::from_wire_error`]. Every arm is a distinct remedy: name a
/// query the workflow declares, wait or retry, address a run that is not
/// answering, look up a workflow that does not exist, or fix the handler.
fn query_error_class(error: &aion::QueryError, source: &aion::EngineError) -> ClientError {
    match error {
        aion::QueryError::UnknownQuery(_) => ClientError::unknown_query(source.to_string()),
        aion::QueryError::Timeout => ClientError::query_timeout(source.to_string()),
        // A run that cannot answer and a reply channel that closed are the same
        // fact to a caller: the workflow is not there to answer right now.
        aion::QueryError::NotRunning(_) | aion::QueryError::ReplyDropped => {
            ClientError::not_running(source.to_string())
        }
        aion::QueryError::Unknown(_) => ClientError::not_found(source.to_string()),
        // The handler ran and reported an application-level failure — the
        // workflow author's to fix, and distinct from the engine failing to
        // deliver the query at all.
        aion::QueryError::HandlerFailed { .. } => ClientError::query_failed(source.to_string()),
        // The caller sent arguments the engine cannot carry to a handler: the
        // caller's request to fix, never the workflow's or the server's.
        aion::QueryError::InvalidArguments { .. } => {
            ClientError::invalid_argument(source.to_string())
        }
        aion::QueryError::Engine(_) => ClientError::server(source.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;
    use std::time::Duration;

    use aion::EventStreamLagged;
    use aion_core::{Event, EventEnvelope, Payload, RunId, WorkflowId};
    use chrono::Utc;
    use futures::{StreamExt, stream};

    use super::{close_after_terminal, map_lag, splice_resume};
    use crate::error::ClientError;

    fn workflow_id() -> WorkflowId {
        WorkflowId::new(uuid::Uuid::from_u128(1))
    }

    fn envelope(seq: u64) -> EventEnvelope {
        EventEnvelope {
            seq,
            recorded_at: Utc::now(),
            workflow_id: workflow_id(),
        }
    }

    fn signal(seq: u64) -> Result<Event, aion_core::PayloadError> {
        Ok(Event::SignalReceived {
            envelope: envelope(seq),
            name: format!("signal-{seq}"),
            payload: Payload::from_json(&serde_json::json!({ "seq": seq }))?,
        })
    }

    fn completed(seq: u64) -> Result<Event, aion_core::PayloadError> {
        Ok(Event::WorkflowCompleted {
            envelope: envelope(seq),
            result: Payload::from_json(&serde_json::json!({ "seq": seq }))?,
        })
    }

    fn history(seqs: std::ops::RangeInclusive<u64>) -> Result<Vec<Event>, aion_core::PayloadError> {
        seqs.map(signal).collect()
    }

    fn live(
        items: Vec<Result<Event, EventStreamLagged>>,
    ) -> futures::stream::BoxStream<'static, Result<Event, EventStreamLagged>> {
        stream::iter(items).boxed()
    }

    async fn delivered_seqs(
        events: futures::stream::BoxStream<'static, Result<Event, ClientError>>,
    ) -> Result<Vec<u64>, ClientError> {
        events
            .map(|item| item.map(|event| event.seq()))
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect()
    }

    #[tokio::test]
    async fn cursor_zero_is_invalid_argument() -> Result<(), Box<dyn std::error::Error>> {
        let error = splice_resume(live(Vec::new()), history(1..=3)?, 0).err();

        let Some(ClientError::InvalidArgument { detail }) = error else {
            return Err(format!("cursor 0 must be InvalidArgument, got {error:?}").into());
        };
        assert!(detail.message.contains(">= 1"), "detail: {detail}");
        Ok(())
    }

    #[tokio::test]
    async fn cursor_ahead_of_history_is_invalid_argument() -> Result<(), Box<dyn std::error::Error>>
    {
        let error = splice_resume(live(Vec::new()), history(1..=5)?, 7).err();

        let Some(ClientError::InvalidArgument { detail }) = error else {
            return Err(format!("cursor head+2 must be InvalidArgument, got {error:?}").into());
        };
        assert!(
            detail.message.contains("ahead of recorded history"),
            "{detail}"
        );

        let empty = splice_resume(live(Vec::new()), Vec::new(), 2).err();
        assert!(
            matches!(empty, Some(ClientError::InvalidArgument { .. })),
            "cursor 2 over empty history must be rejected, got {empty:?}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn overlap_between_snapshot_and_live_is_deduplicated_contiguous_unique()
    -> Result<(), Box<dyn std::error::Error>> {
        // Snapshot holds 1..=5; the live broadcast re-emits 4 and 5 (arrived
        // between attach and snapshot) before the genuinely new 6.
        let events = splice_resume(
            live(vec![Ok(signal(4)?), Ok(signal(5)?), Ok(signal(6)?)]),
            history(1..=5)?,
            1,
        )?;

        assert_eq!(delivered_seqs(events).await?, vec![1, 2, 3, 4, 5, 6]);
        Ok(())
    }

    #[tokio::test]
    async fn mid_history_cursor_replays_suffix_only() -> Result<(), Box<dyn std::error::Error>> {
        let events = splice_resume(live(vec![Ok(signal(6)?)]), history(1..=5)?, 3)?;

        assert_eq!(delivered_seqs(events).await?, vec![3, 4, 5, 6]);
        Ok(())
    }

    #[tokio::test]
    async fn cursor_at_head_plus_one_yields_empty_replay_and_live_tail_only()
    -> Result<(), Box<dyn std::error::Error>> {
        let events = splice_resume(
            live(vec![Ok(signal(6)?), Ok(signal(7)?)]),
            history(1..=5)?,
            6,
        )?;

        assert_eq!(delivered_seqs(events).await?, vec![6, 7]);
        Ok(())
    }

    #[tokio::test]
    async fn lag_mid_splice_surfaces_unavailable_after_the_replay()
    -> Result<(), Box<dyn std::error::Error>> {
        let events = splice_resume(
            live(vec![Err(EventStreamLagged { skipped: 3 })]),
            history(1..=2)?,
            1,
        )?;
        let collected: Vec<_> = events.collect().await;

        assert_eq!(collected.len(), 3, "two replay events then the lag item");
        assert!(collected[0].is_ok() && collected[1].is_ok());
        assert!(
            matches!(
                collected[2].as_ref().err(),
                Some(ClientError::Unavailable { .. })
            ),
            "lag must surface as retryable Unavailable, never a silent gap, got {:?}",
            collected[2]
        );
        Ok(())
    }

    #[tokio::test]
    async fn per_workflow_stream_closes_after_terminal_event()
    -> Result<(), Box<dyn std::error::Error>> {
        // Terminal at seq 3 mid-replay: deliver 1..=3 and close without
        // draining the live tail (continue-as-new/terminal run boundary).
        let mut history = history(1..=2)?;
        history.push(completed(3)?);
        history.push(signal(4)?);
        let events = splice_resume(live(vec![Ok(signal(5)?)]), history, 1)?;

        assert_eq!(
            delivered_seqs(close_after_terminal(events)).await?,
            vec![1, 2, 3],
            "the stream must close after the terminal event"
        );
        Ok(())
    }

    #[tokio::test]
    async fn live_lag_maps_to_unavailable() -> Result<(), Box<dyn std::error::Error>> {
        let events = map_lag(live(vec![
            Ok(signal(1)?),
            Err(EventStreamLagged { skipped: 9 }),
        ]));
        let collected: Vec<_> = events.collect().await;

        assert_eq!(collected.len(), 2);
        assert!(
            matches!(
                collected[1].as_ref().err(),
                Some(ClientError::Unavailable { .. })
            ),
            "got {:?}",
            collected[1]
        );
        Ok(())
    }

    /// End-to-end through a real engine: the embedded resume splice delivers
    /// recorded history and live appends gap-free and duplicate-free, built
    /// on `Engine::subscribe` + `engine.store()` (the pin-note seams).
    #[tokio::test]
    async fn embedded_resume_splices_recorded_history_with_live_appends()
    -> Result<(), Box<dyn std::error::Error>> {
        use crate::stream::SubscribeTarget;
        use crate::transport::{EmbeddedWorkflowTransport, WorkflowTransport};

        let capacity = NonZeroUsize::new(16).ok_or("capacity must be non-zero")?;
        let engine = std::sync::Arc::new(
            aion::EngineBuilder::new()
                .store(aion_store::InMemoryStore::default())
                .in_memory_visibility()
                .event_streaming(capacity)
                .build()
                .await?,
        );
        let workflow_id = WorkflowId::new_v4();
        let mut recorder = aion::durability::Recorder::new(workflow_id.clone(), engine.store());
        recorder
            .record_workflow_started(
                Utc::now(),
                aion::durability::WorkflowStartRecord {
                    workflow_type: String::from("checkout"),
                    input: Payload::from_json(&serde_json::json!({ "cart": [] }))?,
                    run_id: RunId::new(uuid::Uuid::from_u128(7)),
                    parent_run_id: None,
                    parent_workflow_id: None,
                    package_version: aion_core::PackageVersion::new("a".repeat(64)),
                },
            )
            .await?;
        for seq in 2..=3 {
            recorder
                .record_signal_received(
                    Utc::now(),
                    format!("signal-{seq}"),
                    Payload::from_json(&serde_json::json!({ "seq": seq }))?,
                )
                .await?;
        }

        // Resume from seq 2: replay [2, 3] from the snapshot, then splice the
        // live append (4) with no gaps and no duplicates.
        let transport = EmbeddedWorkflowTransport::new(std::sync::Arc::clone(&engine));
        let request = SubscribeTarget::Workflow {
            workflow_id: workflow_id.clone(),
        }
        .request("default");
        let attempt = transport.subscribe(request, Some(2)).await?;
        let mut events = attempt.events;

        let mut delivered = Vec::new();
        for _ in 0..2 {
            let item = tokio::time::timeout(Duration::from_secs(2), events.next())
                .await
                .map_err(|_| "timed out waiting for a replay event")?
                .ok_or("stream ended before the replay completed")?;
            delivered.push(item?.seq());
        }
        recorder
            .record_workflow_completed(
                Utc::now(),
                Payload::from_json(&serde_json::json!({ "done": true }))?,
            )
            .await?;
        let item = tokio::time::timeout(Duration::from_secs(2), events.next())
            .await
            .map_err(|_| "timed out waiting for the live spliced event")?
            .ok_or("stream ended before the live event arrived")?;
        delivered.push(item?.seq());
        assert_eq!(delivered, vec![2, 3, 4]);

        // Seq 4 is terminal: the per-workflow stream must now close.
        let end = tokio::time::timeout(Duration::from_secs(2), events.next())
            .await
            .map_err(|_| "timed out waiting for the post-terminal close")?;
        assert!(
            end.is_none(),
            "per-workflow stream must close after the terminal event, got {end:?}"
        );

        // A cursor beyond head + 1 is rejected against the same engine.
        let ahead = transport
            .subscribe(
                SubscribeTarget::Workflow { workflow_id }.request("default"),
                Some(9),
            )
            .await
            .err();
        assert!(
            matches!(ahead, Some(ClientError::InvalidArgument { .. })),
            "cursor ahead of history must be InvalidArgument, got {ahead:?}"
        );

        engine.shutdown()?;
        Ok(())
    }

    /// 🔴 THE THREE CALL SITES THAT READ THE STORE DIRECTLY CARRY A ROUTABLE
    /// REFUSAL THROUGH — THEY DO NOT FLATTEN IT INTO "ENGINE BUG".
    ///
    /// [`map_engine_error`] is only reached by the eight operations that go
    /// through an engine API. Three do not: `resolve_run_id` reads the run
    /// chain, and `describe_workflow` and the resuming half of `subscribe` read
    /// history, all straight off `engine.store()`. Each of those refusals
    /// arrives as a bare [`aion_store::StoreError`] with no `EngineError` around
    /// it, and each was mapped by hand to `ClientError::server`.
    ///
    /// `not_owner` is the one store class that names a ROUTING failure — try a
    /// different owner, never this one. It is a distinction the CALLER acts on,
    /// not one this crate retries for it: `stream.rs`'s `is_retryable` matches
    /// only `ClientError::Unavailable`. Flattened to `server` the caller is told
    /// its request is unanswerable and gives up on work a re-route would have
    /// served, and no test could see the difference:
    /// [`aion_store::InMemoryStore`] owns every shard and so can never produce
    /// `NotOwner` at all. [`FencedHistoryStore`] is the instrument that can.
    ///
    /// The unarmed pass is the control. Without it an armed `not_owner` would be
    /// consistent with a fixture that never worked — the wrong workflow id, an
    /// engine that never built — and the test would be measuring its own setup.
    #[tokio::test]
    async fn a_directly_read_store_refusal_keeps_its_routing_class()
    -> Result<(), Box<dyn std::error::Error>> {
        use aion_store::testing::FencedHistoryStore;

        use crate::stream::SubscribeTarget;
        use crate::transport::{EmbeddedWorkflowTransport, WorkflowTransport};

        let capacity = NonZeroUsize::new(16).ok_or("capacity must be non-zero")?;
        let store = std::sync::Arc::new(FencedHistoryStore::new());
        let engine = std::sync::Arc::new(
            aion::EngineBuilder::new()
                .store_arc(
                    std::sync::Arc::clone(&store) as std::sync::Arc<dyn aion_store::EventStore>
                )
                .in_memory_visibility()
                .event_streaming(capacity)
                .build()
                .await?,
        );
        let workflow_id = WorkflowId::new_v4();
        let mut recorder = aion::durability::Recorder::new(workflow_id.clone(), engine.store());
        recorder
            .record_workflow_started(
                Utc::now(),
                aion::durability::WorkflowStartRecord {
                    workflow_type: String::from("checkout"),
                    input: Payload::from_json(&serde_json::json!({ "cart": [] }))?,
                    run_id: RunId::new(uuid::Uuid::from_u128(11)),
                    parent_run_id: None,
                    parent_workflow_id: None,
                    package_version: aion_core::PackageVersion::new("b".repeat(64)),
                },
            )
            .await?;

        let transport = EmbeddedWorkflowTransport::new(std::sync::Arc::clone(&engine));
        let describe = |include_history: bool| aion_proto::ProtoDescribeWorkflowRequest {
            namespace: String::from("default"),
            workflow_id: Some(aion_proto::ProtoWorkflowId::from(workflow_id.clone())),
            run_id: None,
            include_history,
        };

        // CONTROL, one per operation, so an armed `not_owner` below is
        // attributable to the fence rather than to a fixture that never reached
        // the read at all.
        //
        // 🔴 An earlier revision of this comment claimed "with the fence
        // disarmed every one of these succeeds" and ran a control for
        // `describe_workflow` only. That claim was not merely unproven, it was
        // impossible: this fixture records `WorkflowStarted` and nothing else,
        // so the run is NON-TERMINAL and `reopen_workflow` answers the AD-012
        // `invalid_state` this file names at `:486-487`. `reopen`'s control is
        // therefore the sharper one available — disarmed it must fail with some
        // class OTHER than `not_owner`, which still separates "the fence did
        // it" from "this call always fails".
        transport.describe_workflow(describe(true)).await?;
        transport
            .subscribe(
                SubscribeTarget::Workflow {
                    workflow_id: workflow_id.clone(),
                }
                .request("default"),
                Some(1),
            )
            .await?;
        let reopen_request = || aion_proto::ProtoReopenRequest {
            namespace: String::from("default"),
            workflow_id: Some(aion_proto::ProtoWorkflowId::from(workflow_id.clone())),
            run_id: None,
        };
        let reopen_control = transport.reopen(reopen_request()).await;
        assert_ne!(
            reopen_control.as_ref().err().map(ClientError::class),
            Some("not_owner"),
            "the reopen control answered `not_owner` with the fence DISARMED, so the armed \
             assertion below would prove nothing: {reopen_control:?}"
        );

        store.arm_fence();

        let described = transport.describe_workflow(describe(false)).await;
        assert_eq!(
            described.as_ref().err().map(ClientError::class),
            Some("not_owner"),
            "describe_workflow flattened a shard-ownership refusal into an unroutable class: \
             {described:?}"
        );

        let subscribed = transport
            .subscribe(
                SubscribeTarget::Workflow {
                    workflow_id: workflow_id.clone(),
                }
                .request("default"),
                Some(1),
            )
            .await;
        assert_eq!(
            subscribed.as_ref().err().map(ClientError::class),
            Some("not_owner"),
            "the resuming half of subscribe flattened a shard-ownership refusal into an \
             unroutable class"
        );

        // `resolve_run_id` reads the RUN CHAIN rather than history, and it is
        // reached only when the caller omits the run id — which is why
        // `reopen_request` carries `run_id: None`.
        let reopened = transport.reopen(reopen_request()).await;
        assert_eq!(
            reopened.as_ref().err().map(ClientError::class),
            Some("not_owner"),
            "resolve_run_id flattened a shard-ownership refusal into an unroutable class: \
             {reopened:?}"
        );

        // Teardown, and the fence comes down FIRST. Every other engine-backed
        // test in this file shuts its engine down; this one left the engine to
        // `Drop`, which gates the epoch but cannot await, so teardown work that
        // must finish was left racing the test's exit. Disarming before the
        // shutdown means the shutdown is measured against an honest store — a
        // shutdown error arriving through `?` here would then be a real fault
        // and not the fixture fencing its own teardown's history access.
        store.disarm_fence();
        engine.shutdown()?;
        Ok(())
    }

    /// Every arm this map NAMES must reach its own class, and the wildcard must
    /// still catch what it is for.
    ///
    /// 🔴 Structured as a table with a NEGATIVE control (`Runtime`), because the
    /// assertion that matters is not "these map somewhere" but "these map
    /// somewhere OTHER than the generic bucket". Without the control, deleting
    /// every named arm and returning `ClientError::server` for everything would
    /// still fail — but a test that only listed the named variants could not
    /// tell a correct map from one that had accidentally started naming
    /// everything.
    ///
    /// 🔴 EACH ERROR IS BUILT IN THE SHAPE PRODUCTION PRODUCES IT, NOT THE
    /// SHAPE THAT MAKES THE ARM LOOK COVERED.
    ///
    /// An earlier revision of this table listed a bare
    /// `Store(StoreError::NotOwner)` and passed — and the arm it exercised
    /// never fired on the shape a caller actually provokes. A fenced APPEND
    /// goes through the `Recorder`, which returns `DurabilityError`, so that
    /// path's caller-reachable shape is `Durability(Store(NotOwner))` — and
    /// against THAT shape the same table failed with `left: "backend"`.
    ///
    /// The bare shape is reachable too, and an earlier revision of THIS comment
    /// wrongly said it was not. `EngineError` carries `Store(#[from]
    /// StoreError)`, so every `?` on a store call inside an engine API that
    /// returns `EngineError` produces it: `list_workflows`
    /// (`api_workflow_ops.rs`, both `store.query` and `store.read_history`),
    /// `cancel` (`lifecycle/terminate.rs`) and `signal`
    /// (`engine/delegated.rs`) are three of the eight engine operations this
    /// transport calls, and all three are store READS rather than appends —
    /// which is precisely why no `Recorder` is involved and no `Durability`
    /// wrapper appears. Both shapes are listed below because both happen.
    ///
    /// ⚠️ WHAT THIS TEST DOES NOT DO. The classes are the same ones
    /// `aion-server`'s `error.rs` assigns, but this test cannot check that: the
    /// dependency runs server → client, so the server's table is not visible
    /// from here and these expectations are a TRANSCRIPTION of it. A rule
    /// written in two places with nothing forcing agreement has already
    /// drifted or will. What holds them together today is that a change to
    /// either side must be made deliberately on both; the durable fix is a
    /// shared classifier, which needs a crate both can depend on and is not in
    /// this change's scope. Naming it here rather than letting the doc comment
    /// imply a guarantee the test does not provide.
    #[test]
    fn every_named_engine_error_reaches_its_own_class() {
        use super::map_engine_error;

        for (error, expected) in engine_error_class_table() {
            assert_eq!(
                map_engine_error(&error).class(),
                expected,
                "wrong class for {error}"
            );
        }
    }

    /// F3: the CLASS is not the whole answer — `ShuttingDown` must also carry
    /// its discriminator, because the operator-facing hint keyed off a bare
    /// `not_running` tells them the run ended and points at `aion list`, which
    /// is false twice (the run is fine; that command fails the same way).
    ///
    /// The class-table test above is invariant to this: it asserts
    /// `"not_running"` and would stay green with the discriminator dropped.
    /// That is exactly why this assertion is separate.
    ///
    /// Killing mutation: revert the arm to `ClientError::not_running(...)`.
    /// `error_type` becomes `None` and the first assertion fails.
    #[test]
    fn shutting_down_carries_its_discriminator_not_only_its_class()
    -> Result<(), Box<dyn std::error::Error>> {
        let mapped = super::map_engine_error(&aion::EngineError::ShuttingDown);
        let ClientError::NotRunning { detail } = &mapped else {
            return Err(format!("ShuttingDown must keep the not_running CLASS: {mapped}").into());
        };
        assert_eq!(
            detail.error_type.as_deref(),
            Some("ShuttingDown"),
            "the wire surface builds `not_running_with_type(\"ShuttingDown\", …)`; an embedded \
             caller that loses the discriminator cannot be told anything true about why"
        );
        // CONTROL: a different error in the same class must NOT claim this
        // discriminator, or the assertion above would pass for anything.
        let other = super::map_engine_error(&aion::EngineError::Runtime {
            reason: "beamr scheduler refused".to_owned(),
        });
        assert_ne!(
            other.class(),
            "not_running",
            "control: the negative case must not share the class under test"
        );
        Ok(())
    }

    /// The table itself, lifted out of the test body so the case list can grow
    /// with the taxonomy without the assertion loop growing at all, and split
    /// by family so each half stays readable.
    fn engine_error_class_table() -> Vec<(aion::EngineError, &'static str)> {
        let mut cases = admission_and_run_state_cases();
        cases.extend(store_and_query_cases());
        cases
    }

    /// Refusals about the request, the run's state, or the engine's own
    /// availability — plus the negative control.
    fn admission_and_run_state_cases() -> Vec<(aion::EngineError, &'static str)> {
        use aion_core::{RunId, WorkflowId};

        let version = aion::ContentHash::from_bytes([7u8; 32]);
        vec![
            // The two oldest named arms, and until this revision the only two
            // with no row: the table asserted every arm reached its own class
            // while silently omitting them, so either could have been deleted
            // and every assertion here would still have passed.
            (
                aion::EngineError::WorkflowNotFound {
                    workflow_type: "orders".to_owned(),
                },
                "not_found",
            ),
            (
                aion::EngineError::InvalidState {
                    reason: "workflow w run r is Running, not terminal".to_owned(),
                },
                "invalid_state",
            ),
            (
                aion::EngineError::StartInputRefused {
                    workflow_type: "orders".to_owned(),
                    version: version.clone(),
                    reason: "field `total` is missing".to_owned(),
                },
                "invalid_input",
            ),
            (
                aion::EngineError::SignalRefused {
                    workflow_id: WorkflowId::new_v4(),
                    run_id: RunId::new_v4(),
                    signal_name: "approve".to_owned(),
                    version,
                    reason: "undeclared signal".to_owned(),
                },
                "invalid_input",
            ),
            (
                aion::EngineError::TerminalWriterUnavailable {
                    workflow_id: "w".to_owned(),
                    run_id: "r".to_owned(),
                    holder: "another reservation".to_owned(),
                },
                "invalid_state",
            ),
            (
                aion::EngineError::TerminalWriterHeld {
                    workflow_id: "w".to_owned(),
                    run_id: "r".to_owned(),
                },
                "invalid_state",
            ),
            (
                aion::EngineError::RunIsRecoverable {
                    workflow_id: "w".to_owned(),
                    run_id: "r".to_owned(),
                    version: "abc".to_owned(),
                },
                "invalid_state",
            ),
            (
                aion::EngineError::NoResidencyVerdict {
                    workflow_id: "w".to_owned(),
                    run_id: "r".to_owned(),
                },
                "invalid_state",
            ),
            (
                aion::EngineError::ContractIdentity {
                    workflow_type: "orders".to_owned(),
                    source: aion::ContractIdentityError::RedeployRequired {
                        stored_version: "orders$deadbeef".to_owned(),
                    },
                },
                "invalid_state",
            ),
            (
                aion::EngineError::NoQueueDeclaration {
                    workflow_type: "orders".to_owned(),
                    version: aion::ContentHash::from_bytes([5u8; 32]),
                    activities: "charge_card,send_receipt".to_owned(),
                },
                "invalid_state",
            ),
            (aion::EngineError::ShuttingDown, "not_running"),
            // The control: a variant this map does NOT name must still land in
            // the generic bucket. If it stops doing so, the wildcard has been
            // replaced by something that names everything, and the assertions
            // above would have passed for the wrong reason.
            (
                aion::EngineError::Runtime {
                    reason: "beamr scheduler refused".to_owned(),
                },
                "backend",
            ),
        ]
    }

    /// The two nested families, each of which reaches this map wrapped in an
    /// outer `EngineError` variant rather than as itself.
    fn store_and_query_cases() -> Vec<(aion::EngineError, &'static str)> {
        use aion_core::WorkflowId;

        vec![
            (
                aion::EngineError::Store(aion_store::StoreError::NotOwner { shard: 3 }),
                "not_owner",
            ),
            // 🔴 The shape a caller actually gets: the fence is raised inside a
            // recorded append, so it arrives wrapped in `Durability`.
            (
                aion::EngineError::Durability(aion::durability::DurabilityError::Store(
                    aion_store::StoreError::NotOwner { shard: 3 },
                )),
                "not_owner",
            ),
            (
                aion::EngineError::Durability(aion::durability::DurabilityError::Store(
                    aion_store::StoreError::NotFound {
                        workflow_id: WorkflowId::new_v4(),
                    },
                )),
                "not_found",
            ),
            // The store family's own control: a store fault that is genuinely
            // the engine's problem must NOT acquire a caller-facing class just
            // because it arrived through the same helper.
            (
                aion::EngineError::Durability(aion::durability::DurabilityError::Store(
                    aion_store::StoreError::SequenceConflict {
                        expected: 4,
                        found: 7,
                    },
                )),
                "backend",
            ),
            (
                aion::EngineError::Query(aion::QueryError::UnknownQuery("balance".to_owned())),
                "unknown_query",
            ),
            (
                aion::EngineError::Query(aion::QueryError::Timeout),
                "query_timeout",
            ),
            (
                aion::EngineError::Query(aion::QueryError::NotRunning(WorkflowId::new_v4())),
                "not_running",
            ),
            (
                aion::EngineError::Query(aion::QueryError::ReplyDropped),
                "not_running",
            ),
            (
                aion::EngineError::Query(aion::QueryError::Unknown(WorkflowId::new_v4())),
                "not_found",
            ),
            (
                aion::EngineError::Query(aion::QueryError::HandlerFailed {
                    message: "handler panicked".to_owned(),
                }),
                "query_failed",
            ),
            // The caller's own arguments were malformed: the request is the
            // thing to fix, so this is `invalid_argument` — not the handler's
            // failure and not the engine's.
            (
                aion::EngineError::Query(aion::QueryError::InvalidArguments {
                    reason: "arguments payload is not a well-formed JSON document".to_owned(),
                }),
                "invalid_input",
            ),
            // The query family's own control, and the eighth of eight arms —
            // an earlier revision of this table listed six and left this one
            // unmeasured, so a change re-classifying an engine-seam failure as
            // a caller-facing one survived green. A seam that could not deliver
            // the query is the ENGINE's failure, not the caller's, and must
            // land in the generic bucket rather than acquire a remedy the
            // caller cannot act on.
            (
                aion::EngineError::Query(aion::QueryError::Engine(
                    aion::engine_seam::EngineSeamError::Delivery {
                        reason: "mailbox send failed".to_owned(),
                    },
                )),
                "backend",
            ),
        ]
    }
}
