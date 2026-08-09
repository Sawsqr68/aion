//! tonic `WorkerProtocol` service — bidirectional stream handler.

use aion_proto::{
    ProtoActivityDescriptor, ProtoActivityResult, ProtoRegisterWorker, ProtoWorkerInstanceIdentity,
    generated::{
        self,
        worker_protocol_server::{WorkerProtocol, WorkerProtocolServer},
    },
};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, Streaming};

use crate::worker::PendingActivities;
use crate::worker::dispatch::{ActivityCompletion, ActivityCompletionSink};
use crate::worker::registry::{WorkerId, WorkerMessage};
use crate::{CallerIdentity, ServerState};

/// Cloneable tonic implementation for the worker bidirectional stream.
#[derive(Clone)]
pub struct WorkerGrpcService {
    state: ServerState,
}

impl WorkerGrpcService {
    /// Build a tonic worker service from shared server state.
    #[must_use]
    pub const fn new(state: ServerState) -> Self {
        Self { state }
    }
}

/// Construct the generated tonic server wrapper for the worker protocol.
#[must_use]
pub fn worker_service(state: ServerState) -> WorkerProtocolServer<WorkerGrpcService> {
    WorkerProtocolServer::new(WorkerGrpcService::new(state))
}

#[tonic::async_trait]
impl WorkerProtocol for WorkerGrpcService {
    type StreamWorkerStream = ReceiverStream<Result<generated::ServerToWorker, Status>>;

    async fn stream_worker(
        &self,
        request: Request<Streaming<generated::WorkerToServer>>,
    ) -> Result<Response<Self::StreamWorkerStream>, Status> {
        let metadata = request.metadata().clone();
        let caller = worker_caller_from_metadata(&metadata, &self.state).await?;
        let token_expires_at = token_expiration_from_metadata(&metadata, &self.state).await?;
        let heartbeat_grace = self.state.runtime_config().worker.heartbeat_window;
        let mut inbound = request.into_inner();

        let first = inbound
            .message()
            .await?
            .and_then(|msg| msg.message)
            .ok_or_else(|| Status::invalid_argument("first message must be RegisterWorker"))?;

        let register = match first {
            generated::worker_to_server::Message::Register(r) => decode_register(r),
            _ => {
                return Err(Status::invalid_argument(
                    "first message must be RegisterWorker",
                ));
            }
        };
        validate_worker_contracts(&self.state, &register)?;

        let (task_tx, task_rx) = mpsc::channel::<Result<generated::ServerToWorker, Status>>(32);
        let (worker_tx, worker_rx) = mpsc::channel(32);

        let registration = self
            .state
            .worker_registry()
            .accept_registration(self.state.namespace_guard(), &caller, &register, worker_tx)
            .await
            .map_err(|error| status_from_server_error(&error))?;

        let pending = self.state.pending_activities().clone();
        let heartbeat = self.state.heartbeat_tracker().clone();
        let drain = self.state.drain_state().clone();
        let registry = self.state.worker_registry().clone();
        let worker_id = registration
            .worker_id()
            .ok_or_else(|| Status::internal("worker registration missing id"))?;
        heartbeat
            .register_connection(worker_id, std::time::Instant::now())
            .map_err(|error| status_from_server_error(&error))?;
        // A worker serves a SET of namespaces; the ack echoes them joined in
        // stable order purely for the worker's logs (the RegisterAck namespace
        // field is informational, not a routing input).
        let authorized_namespace = registration
            .namespaces()
            .filter(|namespaces| !namespaces.is_empty())
            .ok_or_else(|| Status::internal("worker registration missing namespace"))?
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join(",");

        // RegisterAck ordering guarantee: the ack is enqueued on `task_tx`
        // BEFORE the write forwarder that copies dispatched tasks onto the
        // same channel is spawned, so no task frame can precede it on the
        // wire. This is a structural ordering proof, not a timing hope.
        task_tx
            .try_send(Ok(register_ack_frame(
                worker_id,
                &authorized_namespace,
                heartbeat_grace,
            )))
            .map_err(|_| Status::internal("worker response channel closed before RegisterAck"))?;

        tokio::spawn(async move {
            let write_handle = spawn_write_forwarder(worker_rx, task_tx.clone());

            // Armed BEFORE the inbound loop runs: the sweep in its `Drop`
            // fires on every exit from this task — clean stream end, stream
            // error, token expiry, even a panic unwinding `process_inbound`.
            // The unbounded dispatch wait depends on it.
            let teardown = StreamTeardown {
                worker_id,
                heartbeat: &heartbeat,
                registry: &registry,
                pending: &pending,
                drain: &drain,
            };
            let session = WorkerSession {
                worker_id,
                pending: &pending,
                heartbeat: &heartbeat,
                drain: &drain,
                token_expires_at,
                heartbeat_grace,
                task_tx: task_tx.clone(),
            };
            if let Err(status) = process_inbound(inbound, session).await {
                tracing::info!(
                    worker_id = ?worker_id,
                    %status,
                    "worker stream closed with status"
                );
            }

            write_handle.abort();
            drop(task_tx);
            drop(teardown);
            // The teardown sweep already deregistered the stream; consuming
            // the registration here is an idempotent no-op that still
            // surfaces a poisoned-lock error loudly.
            if let Err(error) = registration.deregister() {
                tracing::error!(
                    worker_id = ?worker_id,
                    %error,
                    "worker deregistration failed during stream teardown"
                );
            }
        });

        Ok(Response::new(ReceiverStream::new(task_rx)))
    }
}

/// Spawn the write forwarder: it copies registry-delivered [`WorkerMessage`]s
/// onto the worker's response stream, and — the #176 zombie fix — TERMINATES
/// the RPC when the registry drops this worker's delivery sender.
///
/// `recv` returning `None` means every delivery sender is gone: the worker
/// was DEREGISTERED while its stream stayed open (the heartbeat expiry sweep,
/// or any future administrative removal). Silently deregistering would leave
/// the worker a zombie — connected but unroutable, believing it is
/// registered, its heartbeats rejected as "not in flight", never
/// re-registering until its own stream happens to end. Ending the RPC with a
/// retryable `Unavailable` status makes the worker OBSERVE the
/// deregistration and re-register through its reconnect machinery. (On the
/// normal teardown path this task is aborted before the registration is
/// consumed, so the status is never sent to a worker that hung up; a send
/// failure means the response stream's consumer is already gone, so there is
/// no one left to signal.)
fn spawn_write_forwarder(
    mut worker_rx: mpsc::Receiver<WorkerMessage>,
    task_tx: mpsc::Sender<Result<generated::ServerToWorker, Status>>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(message) = worker_rx.recv().await {
            let msg = encode_server_to_worker(message);
            if task_tx.send(Ok(msg)).await.is_err() {
                return;
            }
        }
        let _ = task_tx
            .send(Err(Status::unavailable(
                "worker was deregistered by the server (heartbeat window expired); \
                 reconnect and re-register",
            )))
            .await;
    })
}

/// Drop guard that sweeps a torn-down worker stream's in-flight activities:
/// failed back to the engine mid-run, or parked for restart recovery under a
/// graceful drain (#207) — see [`teardown_worker_stream`].
///
/// A guard rather than a call site so the sweep cannot be skipped by any
/// exit from the stream task — including a panic unwinding the inbound
/// loop, which would otherwise leave every dispatch blocked on that worker
/// waiting forever.
struct StreamTeardown<'a> {
    worker_id: WorkerId,
    heartbeat: &'a crate::worker::HeartbeatTracker,
    registry: &'a crate::worker::ConnectedWorkerRegistry,
    pending: &'a PendingActivities,
    drain: &'a crate::shutdown::DrainState,
}

impl Drop for StreamTeardown<'_> {
    fn drop(&mut self) {
        teardown_worker_stream(
            self.worker_id,
            self.heartbeat,
            self.registry,
            self.pending,
            self.drain,
        );
    }
}

/// Sweep a torn-down worker stream's in-flight activities.
///
/// The stream is the worker's liveness. When it ends mid-run — process death,
/// network disconnect, expired token — every activity still assigned to this
/// worker must be failed back through the completion sink as a retryable
/// lost-worker error. The activity dispatch wait is unbounded by design (the
/// engine imposes no activity timeout), so this sweep is what unblocks
/// dispatches whose worker died mid-activity; the engine's retry policy then
/// decides re-dispatch.
///
/// Under a graceful drain (#207) the stream ending is the EXPECTED worker
/// response to the drain request, not a death: the worker's in-flight tasks
/// are PARKED for restart recovery instead — nothing is recorded, nothing is
/// delivered, the durable log keeps its dangling scheduled/started trail
/// exactly as a kill -9 would, and post-restart replay re-dispatches it. The
/// park-vs-fail branch keys on
/// [`DrainState::is_draining`](crate::shutdown::DrainState::is_draining): a
/// worker lost while the server is NOT draining still fails-and-retries
/// byte-identically to before.
fn teardown_worker_stream(
    worker_id: WorkerId,
    heartbeat: &crate::worker::HeartbeatTracker,
    registry: &crate::worker::ConnectedWorkerRegistry,
    pending: &PendingActivities,
    drain: &crate::shutdown::DrainState,
) {
    if drain.is_draining() {
        match heartbeat.park_disconnected_worker(worker_id, registry, pending) {
            Ok(report) if report.tasks.is_empty() => {}
            Ok(report) => {
                tracing::info!(
                    worker_id = ?worker_id,
                    parked_tasks = report.tasks.len(),
                    "worker stream ended during drain; in-flight activities \
                     parked for restart recovery"
                );
            }
            Err(error) => {
                tracing::error!(
                    worker_id = ?worker_id,
                    %error,
                    "failed to park draining worker's in-flight activities"
                );
            }
        }
    } else {
        match heartbeat.fail_disconnected_worker(worker_id, registry, pending) {
            Ok(report) if report.tasks.is_empty() => {}
            Ok(report) => {
                tracing::warn!(
                    worker_id = ?worker_id,
                    failed_tasks = report.tasks.len(),
                    "worker disconnected with in-flight activities; \
                     surfaced as transport losses, to be re-dispatched \
                     attempt-neutrally"
                );
            }
            Err(error) => {
                tracing::error!(
                    worker_id = ?worker_id,
                    %error,
                    "failed to sweep disconnected worker's in-flight activities"
                );
            }
        }
    }
    // In-flight accounting may have just reached zero; wake any drain
    // waiter so shutdown does not sit out its full timeout.
    drain.notify_activity_drained();
}

struct WorkerSession<'a> {
    worker_id: WorkerId,
    pending: &'a PendingActivities,
    heartbeat: &'a crate::worker::HeartbeatTracker,
    drain: &'a crate::shutdown::DrainState,
    token_expires_at: Option<u64>,
    heartbeat_grace: std::time::Duration,
    task_tx: mpsc::Sender<Result<generated::ServerToWorker, Status>>,
}

async fn process_inbound(
    mut inbound: Streaming<generated::WorkerToServer>,
    session: WorkerSession<'_>,
) -> Result<(), Status> {
    let mut expired_since: Option<std::time::Instant> = None;
    while let Some(msg) = inbound.message().await? {
        refresh_connection_lease(&session)?;
        let Some(inner) = msg.message else {
            continue;
        };
        match inner {
            generated::worker_to_server::Message::Result(result) => {
                let proto_result = decode_activity_result(result);
                match ActivityCompletion::try_from(proto_result) {
                    Ok(completion) => {
                        let workflow_id = completion.workflow_id.clone();
                        let activity_id = completion.activity_id.clone();
                        match session.pending.complete_activity(completion) {
                            Ok(()) => {
                                if let Err(error) = session.heartbeat.complete_task(
                                    session.worker_id,
                                    &workflow_id,
                                    &activity_id,
                                ) {
                                    // A poisoned liveness tracker would also break
                                    // the lost-worker sweep the unbounded dispatch
                                    // wait relies on — never swallow it.
                                    tracing::error!(
                                        worker_id = ?session.worker_id,
                                        workflow_id = %workflow_id,
                                        activity_id = %activity_id,
                                        %error,
                                        "failed to clear in-flight tracking for completed activity"
                                    );
                                }
                                session.drain.notify_activity_drained();
                            }
                            Err(error) => {
                                // Leave the current worker's liveness entry intact
                                // when the generation proof is wrong. Its stopped
                                // heartbeat pump will drive the normal loss/retry path.
                                tracing::error!(
                                    worker_id = ?session.worker_id,
                                    workflow_id = %workflow_id,
                                    activity_id = %activity_id,
                                    %error,
                                    "activity completion handoff failed"
                                );
                            }
                        }
                        // Ack every well-formed result frame — including
                        // duplicates with no pending waiter; their re-report
                        // obligation is equally discharged. `try_send`: a
                        // worker that stopped draining its receive side must
                        // not wedge the inbound loop; a dropped ack is
                        // recovered by the next-session re-report.
                        let ack = result_ack_frame(&workflow_id, &activity_id);
                        if let Err(error) = session.task_tx.try_send(Ok(ack)) {
                            tracing::warn!(
                                worker_id = ?session.worker_id,
                                workflow_id = %workflow_id,
                                activity_id = %activity_id,
                                %error,
                                "result ack dropped: worker stream channel unavailable"
                            );
                        }
                    }
                    Err(error) => {
                        // Malformed result: no ids to ack with. Loud, never
                        // silent — the worker's entry will re-report and
                        // re-fail visibly each session.
                        tracing::error!(
                            worker_id = ?session.worker_id,
                            %error,
                            "malformed activity result frame; no ack sent"
                        );
                    }
                }
            }
            generated::worker_to_server::Message::Register(_) => {
                warn_duplicate_registration(session.worker_id);
            }
            generated::worker_to_server::Message::Heartbeat(heartbeat_msg) => {
                // Empty task ids are the connection-level lease beat. The frame
                // already advanced the lease above; it intentionally has no
                // per-task liveness entry to update.
                if heartbeat_msg.workflow_id.is_none() && heartbeat_msg.activity_id.is_none() {
                    continue;
                }
                if let Err(error) = session.heartbeat.record_heartbeat(
                    session.worker_id,
                    decode_heartbeat(heartbeat_msg),
                    std::time::Instant::now(),
                ) {
                    // Malformed frames and heartbeats for untracked tasks
                    // are worker-side defects worth surfacing; a poisoned
                    // tracker lock is a server-side corruption signal that
                    // must never vanish silently.
                    if matches!(error, crate::ServerError::LockPoisoned { .. }) {
                        tracing::error!(
                            worker_id = ?session.worker_id,
                            %error,
                            "heartbeat tracker lock poisoned; liveness state untrustworthy"
                        );
                    } else {
                        tracing::warn!(
                            worker_id = ?session.worker_id,
                            %error,
                            "worker heartbeat rejected"
                        );
                    }
                }
                enforce_token_expiration(&session, &mut expired_since).await?;
            }
        }
    }
    Ok(())
}

fn refresh_connection_lease(session: &WorkerSession<'_>) -> Result<(), Status> {
    session
        .heartbeat
        .record_connection_activity(session.worker_id, std::time::Instant::now())
        .map(|_| ())
        .map_err(|error| {
            tracing::error!(
                worker_id = ?session.worker_id,
                %error,
                "failed to advance worker connection lease"
            );
            status_from_server_error(&error)
        })
}

fn warn_duplicate_registration(worker_id: WorkerId) {
    tracing::warn!(
        worker_id = ?worker_id,
        "ignoring subsequent RegisterWorker message; \
         only the first registration is accepted per stream"
    );
}

async fn enforce_token_expiration(
    session: &WorkerSession<'_>,
    expired_since: &mut Option<std::time::Instant>,
) -> Result<(), Status> {
    if !token_expired(session.token_expires_at) {
        return Ok(());
    }
    let first_expired = *expired_since.get_or_insert_with(std::time::Instant::now);
    let _ = session
        .task_tx
        .send(Err(Status::unauthenticated(
            "worker token expired; re-authentication required",
        )))
        .await;
    if first_expired.elapsed() >= session.heartbeat_grace {
        return Err(Status::unauthenticated("worker token expired"));
    }
    Ok(())
}

async fn worker_caller_from_metadata(
    metadata: &tonic::metadata::MetadataMap,
    state: &ServerState,
) -> Result<CallerIdentity, Status> {
    crate::api::grpc::caller_from_metadata(metadata, state).await
}

async fn token_expiration_from_metadata(
    metadata: &tonic::metadata::MetadataMap,
    state: &ServerState,
) -> Result<Option<u64>, Status> {
    if !state.runtime_config().auth.enabled {
        return Ok(None);
    }
    #[cfg(feature = "auth")]
    {
        let bearer = metadata
            .get("authorization")
            .and_then(|value| value.to_str().ok())
            .and_then(parse_bearer)
            .ok_or_else(|| Status::unauthenticated("missing bearer token"))?;
        let Some(cache) = state.jwks_cache() else {
            return Err(Status::unauthenticated("invalid bearer token"));
        };
        return cache
            .validate(&bearer)
            .await
            .map(|claims| Some(claims.expires_at()))
            .map_err(|_error| Status::unauthenticated("invalid bearer token"));
    }
    #[cfg(not(feature = "auth"))]
    {
        let _ = metadata;
        // Yield to preserve the async signature required by the auth-feature branch.
        tokio::task::yield_now().await;
        Ok(None)
    }
}

#[cfg(feature = "auth")]
fn parse_bearer(value: &str) -> Option<String> {
    let token = value.strip_prefix("Bearer ")?.trim();
    if token.is_empty() {
        return None;
    }
    Some(token.to_owned())
}

fn token_expired(expires_at: Option<u64>) -> bool {
    expires_at.is_some_and(|expires_at| {
        #[cfg(feature = "auth")]
        {
            crate::auth::jwks::is_expired(expires_at)
        }
        #[cfg(not(feature = "auth"))]
        {
            let _ = expires_at;
            false
        }
    })
}

fn status_from_server_error(error: &crate::ServerError) -> Status {
    let wire = error.to_wire_error();
    if wire.code == aion_proto::WireErrorCode::NamespaceDenied {
        Status::permission_denied(wire.message)
    } else {
        Status::internal(wire.message)
    }
}

/// Build the positive registration acknowledgement frame — the guaranteed
/// first frame on every successful worker response stream.
fn register_ack_frame(
    worker_id: WorkerId,
    namespace: &str,
    heartbeat_window: std::time::Duration,
) -> generated::ServerToWorker {
    generated::ServerToWorker {
        message: Some(generated::server_to_worker::Message::RegisterAck(
            generated::RegisterAck {
                worker_id: worker_id.value(),
                namespace: namespace.to_owned(),
                heartbeat_window_ms: u64::try_from(heartbeat_window.as_millis())
                    .unwrap_or(u64::MAX),
            },
        )),
    }
}

/// Build the per-result acknowledgement frame for a consumed `ActivityResult`.
fn result_ack_frame(
    workflow_id: &aion_core::WorkflowId,
    activity_id: &aion_core::ActivityId,
) -> generated::ServerToWorker {
    generated::ServerToWorker {
        message: Some(generated::server_to_worker::Message::ResultAck(
            generated::ResultAck {
                workflow_id: Some(generated::WorkflowId {
                    uuid: workflow_id.to_string(),
                }),
                activity_id: Some(generated::ActivityId {
                    sequence_position: activity_id.sequence_position(),
                }),
            },
        )),
    }
}

fn decode_register(r: generated::RegisterWorker) -> ProtoRegisterWorker {
    ProtoRegisterWorker {
        namespaces: r.namespaces,
        activity_types: r.activity_types,
        task_queue: r.task_queue,
        node: r.node,
        activities: r
            .activities
            .into_iter()
            .map(|activity| ProtoActivityDescriptor {
                name: activity.name,
                input_schema_json: activity.input_schema_json,
                output_schema_json: activity.output_schema_json,
            })
            .collect(),
        identity: r.identity,
        instance: r.instance.map(|instance| ProtoWorkerInstanceIdentity {
            deployment: instance.deployment,
            instance_id: instance.instance_id,
        }),
    }
}

fn validate_worker_contracts(
    state: &ServerState,
    register: &ProtoRegisterWorker,
) -> Result<(), Status> {
    let advertised = register
        .activities
        .iter()
        .map(|activity| {
            let input_schema =
                serde_json::from_str(&activity.input_schema_json).map_err(|error| {
                    Status::invalid_argument(format!(
                        "worker activity `{}` input_schema_json is invalid: {error}",
                        activity.name
                    ))
                })?;
            let output_schema =
                serde_json::from_str(&activity.output_schema_json).map_err(|error| {
                    Status::invalid_argument(format!(
                        "worker activity `{}` output_schema_json is invalid: {error}",
                        activity.name
                    ))
                })?;
            Ok(aion_package::ActivityDescriptor {
                name: activity.name.clone(),
                input_schema,
                output_schema,
            })
        })
        .collect::<Result<Vec<_>, Status>>()?;
    // Mirrors the liminal transport's no-catalog admission: a state built
    // from parts without an engine handle has no durable catalog, so no
    // deployed `.v4` contract can exist to contradict this worker. Refusing
    // here would turn every registration away forever on such a server; the
    // strict field-level check applies on every engine-backed state.
    let Ok(engine) = state.engine() else {
        tracing::warn!(
            task_queue = %register.task_queue,
            identity = %register.identity,
            "worker contract check skipped: server state has no engine handle, \
             so no deployed contracts exist to check against"
        );
        return Ok(());
    };
    // Both advertised forms travel into the gate together — see
    // [`crate::worker::contracts::WorkerAdvertisement`] for why carrying only
    // one of them makes a refusal contradict itself.
    let activity_types = register
        .activity_types
        .iter()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    crate::worker::contracts::validate_worker_contracts(
        &engine,
        state.worker_registry().admission_audit(),
        &register.task_queue,
        crate::worker::registry::optional_node(&register.node).as_deref(),
        &register.identity,
        crate::worker::contracts::WorkerAdvertisement {
            activity_types: &activity_types,
            contracts: &advertised,
        },
    )
    .map_err(|error| match error {
        crate::worker::contracts::ContractAdmissionError::Mismatch { .. } => {
            Status::failed_precondition(error.to_string())
        }
        crate::worker::contracts::ContractAdmissionError::Catalog { .. } => {
            Status::internal(error.to_string())
        }
    })
}

fn encode_server_to_worker(message: WorkerMessage) -> generated::ServerToWorker {
    let message = match message {
        WorkerMessage::ActivityTask(task) => {
            generated::server_to_worker::Message::Task(encode_task(*task))
        }
        WorkerMessage::DrainRequest => {
            generated::server_to_worker::Message::Drain(generated::DrainRequest {})
        }
    };
    generated::ServerToWorker {
        message: Some(message),
    }
}

fn encode_task(task: aion_proto::ProtoActivityTask) -> generated::ActivityTask {
    generated::ActivityTask {
        workflow_id: task
            .workflow_id
            .map(|id| generated::WorkflowId { uuid: id.uuid }),
        activity_id: task.activity_id.map(|id| generated::ActivityId {
            sequence_position: id.sequence_position,
        }),
        activity_type: task.activity_type,
        input: task.input.map(|p| generated::Payload {
            content_type: p.content_type,
            bytes: p.bytes,
        }),
        attempt: task.attempt,
        labels: task.labels,
        run_id: task.run_id.map(|id| generated::RunId { uuid: id.uuid }),
        completion_token: task.completion_token,
        idempotency_key: task.idempotency_key,
    }
}

fn decode_activity_result(r: generated::ActivityResult) -> ProtoActivityResult {
    ProtoActivityResult {
        workflow_id: r
            .workflow_id
            .map(|id| aion_proto::ProtoWorkflowId { uuid: id.uuid }),
        activity_id: r.activity_id.map(|id| aion_proto::ProtoActivityId {
            sequence_position: id.sequence_position,
        }),
        outcome: r.outcome.map(decode_outcome),
        run_id: r.run_id.map(|id| aion_proto::ProtoRunId { uuid: id.uuid }),
        completion_token: r.completion_token,
    }
}

fn decode_heartbeat(r: generated::Heartbeat) -> aion_proto::ProtoHeartbeat {
    aion_proto::ProtoHeartbeat {
        workflow_id: r
            .workflow_id
            .map(|id| aion_proto::ProtoWorkflowId { uuid: id.uuid }),
        activity_id: r.activity_id.map(|id| aion_proto::ProtoActivityId {
            sequence_position: id.sequence_position,
        }),
        progress: r.progress.map(|p| aion_proto::ProtoPayload {
            content_type: p.content_type,
            bytes: p.bytes,
        }),
    }
}

fn decode_outcome(
    outcome: generated::activity_result::Outcome,
) -> aion_proto::proto_activity_result::Outcome {
    match outcome {
        generated::activity_result::Outcome::Result(p) => {
            aion_proto::proto_activity_result::Outcome::Result(aion_proto::ProtoPayload {
                content_type: p.content_type,
                bytes: p.bytes,
            })
        }
        generated::activity_result::Outcome::Error(e) => {
            aion_proto::proto_activity_result::Outcome::Error(aion_proto::ProtoActivityError {
                kind: e.kind,
                message: e.message,
                details: e.details.map(|p| aion_proto::ProtoPayload {
                    content_type: p.content_type,
                    bytes: p.bytes,
                }),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use aion_core::{ActivityId, ContentType, Payload, WorkflowId};

    use crate::shutdown::DrainState;
    use crate::worker::dispatch::{
        ActivityCompletion, ActivityCompletionOutcome, ActivityCompletionSink,
    };
    use crate::worker::heartbeat::InFlightActivity;
    use crate::worker::registry::ConnectedWorkerRegistry;
    use crate::worker::{HeartbeatTracker, PendingActivities};

    use super::{decode_register, teardown_worker_stream};

    type TestError = Box<dyn std::error::Error>;

    #[test]
    fn decode_register_maps_tag_seven_instance_without_changing_absent_registration() {
        let generated = super::generated::RegisterWorker {
            namespaces: vec!["orders".to_owned()],
            activity_types: vec!["shell".to_owned()],
            task_queue: "shell".to_owned(),
            node: "node-a".to_owned(),
            activities: Vec::new(),
            identity: "build-a".to_owned(),
            instance: Some(super::generated::WorkerInstanceIdentity {
                deployment: "shells".to_owned(),
                instance_id: "instance-1".to_owned(),
            }),
        };
        let mapped = decode_register(generated.clone());
        let instance = mapped.instance.as_ref();
        assert_eq!(
            instance.map(|value| value.deployment.as_str()),
            Some("shells")
        );
        assert_eq!(
            instance.map(|value| value.instance_id.as_str()),
            Some("instance-1")
        );

        let mut absent = generated;
        absent.instance = None;
        let mapped_absent = decode_register(absent);
        assert!(mapped_absent.instance.is_none());
        assert_eq!(mapped_absent.identity, "build-a");
    }

    /// One tracked in-flight dispatch with a live pending waiter, ready for a
    /// stream teardown: the registered worker, the shared tracker/pending/drain
    /// state, and the waiter's receiver.
    struct TeardownFixture {
        registry: ConnectedWorkerRegistry,
        tracker: HeartbeatTracker,
        pending: PendingActivities,
        drain: DrainState,
        worker_id: crate::worker::registry::WorkerId,
        workflow_id: WorkflowId,
        activity_id: ActivityId,
        completion_token: crate::worker::CompletionToken,
        rx: std::sync::mpsc::Receiver<Result<String, String>>,
        /// Held so the registered worker stays routable until the teardown
        /// under test deregisters it (dropping the guard would race that).
        _registration: crate::worker::registry::WorkerRegistration,
    }

    fn fixture() -> Result<TeardownFixture, TestError> {
        let registry = ConnectedWorkerRegistry::default();
        let (tx, _rx) = tokio::sync::mpsc::channel(1);
        let activity_types = [String::from("greet")];
        let registration = registry.register("default", activity_types.iter(), tx)?;
        let worker_id = registration
            .worker_id()
            .ok_or("test worker registration missing id")?;
        let tracker = HeartbeatTracker::new(Duration::from_secs(5));
        let pending = PendingActivities::default();
        let workflow_id = WorkflowId::new_v4();
        let activity_id = ActivityId::from_sequence_position(0);
        let (completion_token, rx) =
            pending.insert_for_test(workflow_id.clone(), activity_id.clone())?;
        tracker.track_task(
            worker_id,
            InFlightActivity {
                workflow_id: workflow_id.clone(),
                activity_id: activity_id.clone(),
                attempt: 1,
                completion_token: completion_token.clone(),
            },
            Instant::now(),
        )?;
        Ok(TeardownFixture {
            registry,
            tracker,
            pending,
            drain: DrainState::default(),
            worker_id,
            workflow_id,
            activity_id,
            completion_token,
            rx,
            _registration: registration,
        })
    }

    /// #207: with drain begun, a stream teardown PARKS the in-flight dispatch —
    /// the waiter resolves with the ephemeral parked sentinel, no lost-worker
    /// failure is synthesized, and the tracker empties for drain accounting.
    #[test]
    fn teardown_under_drain_parks_instead_of_failing() -> Result<(), TestError> {
        let fixture = fixture()?;
        assert!(fixture.drain.begin());

        teardown_worker_stream(
            fixture.worker_id,
            &fixture.tracker,
            &fixture.registry,
            &fixture.pending,
            &fixture.drain,
        );

        let resolved = fixture.rx.recv_timeout(Duration::from_millis(200))?;
        assert_eq!(
            resolved,
            Err(aion::PARKED_ACTIVITY_REASON.to_owned()),
            "a drain teardown must resolve the waiter with the parked sentinel"
        );
        assert_eq!(fixture.tracker.in_flight_count()?, 0);
        assert!(
            !fixture.tracker.is_tracked(
                fixture.worker_id,
                &fixture.workflow_id,
                &fixture.activity_id
            )?,
            "parking must retire the tracked entry"
        );
        Ok(())
    }

    /// Regression pin: WITHOUT drain, the teardown path still resolves the
    /// waiter with a lost-worker failure — but in the TRANSPORT domain, not the
    /// action's.
    ///
    /// The old contract here was `retryable:`, and that framing was the Hit D
    /// defect: an authored retry policy governs how often the ACTION may fail,
    /// and an activity with no policy (the SDK default) turned a worker death
    /// into a TERMINAL failure. Worker loss now carries the `lost:` class, which
    /// the engine re-dispatches attempt-neutrally.
    #[test]
    fn teardown_without_drain_fails_with_the_transport_domain_lost_worker_class()
    -> Result<(), TestError> {
        let fixture = fixture()?;

        teardown_worker_stream(
            fixture.worker_id,
            &fixture.tracker,
            &fixture.registry,
            &fixture.pending,
            &fixture.drain,
        );

        let resolved = fixture.rx.recv_timeout(Duration::from_millis(200))?;
        let reason = resolved.err().ok_or("expected a lost-worker failure")?;
        assert!(
            reason.starts_with(crate::worker::WORKER_LOST_REASON_PREFIX),
            "a mid-run teardown must surface the TRANSPORT-domain loss class, never the \
             action's retry vocabulary: {reason}"
        );
        assert!(
            reason.contains("lost before reporting activity result"),
            "the failure must name worker loss: {reason}"
        );
        assert_eq!(fixture.tracker.in_flight_count()?, 0);
        Ok(())
    }

    /// R2 fencing red: after heartbeat loss resolves worker A's attempt and a
    /// retry installs worker B's waiter for the same activity, A's late result
    /// must not resolve B's generation.
    #[test]
    fn stale_worker_completion_after_heartbeat_loss_does_not_resolve_retry() -> Result<(), TestError>
    {
        let fixture = fixture()?;
        teardown_worker_stream(
            fixture.worker_id,
            &fixture.tracker,
            &fixture.registry,
            &fixture.pending,
            &fixture.drain,
        );
        let first = fixture.rx.recv_timeout(Duration::from_millis(200))?;
        assert!(
            first
                .err()
                .is_some_and(|reason| reason.starts_with(crate::worker::WORKER_LOST_REASON_PREFIX)),
            "worker A loss must release attempt 1 in the transport-loss class"
        );

        let (retry_token, retry_rx) = fixture
            .pending
            .insert_for_test(fixture.workflow_id.clone(), fixture.activity_id.clone())?;
        let rejected = fixture.pending.complete_activity(ActivityCompletion {
            workflow_id: fixture.workflow_id,
            activity_id: fixture.activity_id,
            run_id: None,
            completion_token: fixture.completion_token,
            outcome: ActivityCompletionOutcome::Succeeded(Payload::new(
                ContentType::Json,
                br#"{"worker":"A","stale":true}"#.to_vec(),
            )),
        });

        assert!(matches!(
            rejected,
            Err(crate::ServerError::ActivityCompletionRejected { .. })
        ));
        drop(retry_token);
        assert!(
            retry_rx.recv_timeout(Duration::from_millis(50)).is_err(),
            "worker A's late completion must be rejected instead of resolving worker B's retry"
        );
        Ok(())
    }
}
