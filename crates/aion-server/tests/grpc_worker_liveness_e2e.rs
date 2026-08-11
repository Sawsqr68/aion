//! End-to-end proof that a gRPC-delivered worker gets a first-class liveness
//! path, clears the SAME dispatch probation a liminal worker serves, and then
//! RECEIVES A DISPATCH (#197).
//!
//! # The defect this pins
//!
//! Dispatch eligibility is a verdict of the liveness probe. The probe used to
//! enumerate only liminal connections, so a `WorkerDelivery::Grpc` worker was
//! never pinged once: its probation opened at registration and could never bank
//! an answer, `select_worker` filtered it out forever, and `aion worker awl
//! serve` — the path the mm_capture collector needs — could register
//! successfully and never be given a single task.
//!
//! Nothing is faked below the test's own worker. The real tonic
//! `WorkerProtocol` service runs on a loopback port, the real
//! `ConnectedWorkerRegistry` holds the registration, the real `LivenessProbe`
//! forms and publishes the verdict, and the real `WorkerActivityDispatcher`
//! performs the dispatch. The test plays the WORKER, at the wire — answering
//! liveness pings exactly as the SDK runtime does — because the whole question
//! is what the server concludes from a worker's answers.
//!
//! # Rounds, not sleeps
//!
//! Probation is stated in ROUNDS (`LivenessProbe::probe_once`), never in
//! wall-clock sleeps against a running timer. A probation asserted by sleeping
//! measures this machine's scheduler; a probation asserted in rounds measures
//! the law.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use aion::{ActivityDispatch, ActivityDispatcher as _};
use aion_core::{ActivityId, WorkflowId};
use aion_proto::generated::worker_protocol_client::WorkerProtocolClient;
use aion_proto::generated::{self, server_to_worker, worker_to_server};
use aion_server::ServerState;
use aion_server::api::worker_grpc::worker_service;
use aion_server::config::{
    AuthConfig, AuthoringConfig, DeployConfig, ListenConfig, MetricsConfig, NamespaceConfig,
    NamespaceMode, OpsConsoleAssetSource, OpsConsoleConfig, RuntimeConfig, WebSocketConfig,
    WorkerConfig,
};
use aion_server::worker::{ConnectedWorkerRegistry, LivenessProbe, WorkerActivityDispatcher};
use aion_server::{NamespaceResolver, StaticScheduleNamespaces, StaticWorkflowNamespaces};
use tokio::net::TcpListener;
use tokio_stream::wrappers::{ReceiverStream, TcpListenerStream};

#[path = "test_support/capture.rs"]
mod capture;

use capture::{CaptureLayer, CapturedEvent, capture_log, render};
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::registry::Registry;

type TestError = Box<dyn std::error::Error>;

const NAMESPACE: &str = "default";
const TASK_QUEUE: &str = "default";
const ACTIVITY_TYPE: &str = "greet";

/// The window every probe in this file derives from. Four seconds gives a
/// one-second cadence (`sweep_interval` clamps the quarter-window to a 1s
/// floor) and, more importantly, a freshness window four cadences wide — so a
/// proof banked in one round cannot age out during the next assertion and turn
/// a law failure into a timing failure.
const HEARTBEAT_WINDOW: Duration = Duration::from_secs(4);

fn runtime_config() -> RuntimeConfig {
    RuntimeConfig {
        listen: ListenConfig {
            grpc: SocketAddr::from(([127, 0, 0, 1], 0)),
            http: SocketAddr::from(([127, 0, 0, 1], 0)),
        },
        tls: None,
        auth: AuthConfig {
            enabled: false,
            jwks_url: None,
            jwks_refresh_seconds: 300,
        },
        ops_console: OpsConsoleConfig {
            source: OpsConsoleAssetSource::Embedded,
        },
        namespace: NamespaceConfig {
            mode: NamespaceMode::SharedEngine,
        },
        worker: WorkerConfig {
            heartbeat_window: HEARTBEAT_WINDOW,
            ..Default::default()
        },
        websocket: WebSocketConfig {
            outbound_buffer_bound: 32,
            event_broadcast_capacity: Some(64),
            cluster_broadcast_capacity: Some(64),
        },
        workflow_packages: Vec::new(),
        deploy: DeployConfig::default(),
        authoring: AuthoringConfig::default(),
        dev: aion_server::config::DevConfig::default(),
        outbox: aion_server::config::OutboxConfig::default(),
        observability: aion_server::config::ObservabilityConfig::default(),
        mcp: aion_server::config::ResolvedMcpConfig::default(),
        scheduler_threads: 1,
        query_timeout: Some(Duration::from_secs(10)),
        default_namespace: NAMESPACE.to_owned(),
        auto_create: aion_server::config::AutoCreate::Open,
        max_in_flight_activities: aion_server::config::DEFAULT_MAX_IN_FLIGHT_ACTIVITIES,
        drain_timeout: Duration::from_secs(30),
        metrics: MetricsConfig { enabled: false },
        owned_shards: Vec::new(),
        cors_allowed_origins: Vec::new(),
    }
}

fn greet_request(input: &str) -> ActivityDispatch {
    ActivityDispatch {
        namespace: NAMESPACE.to_owned(),
        task_queue: TASK_QUEUE.to_owned(),
        node: None,
        workflow_id: WorkflowId::new_v4(),
        run_id: aion_core::RunId::new_v4(),
        activity_id: ActivityId::from_sequence_position(0),
        name: ACTIVITY_TYPE.to_owned(),
        input: input.to_owned(),
        config: "{}".to_owned(),
        attempt: 1,
        labels: std::collections::BTreeMap::new(),
        advisory: false,
    }
}

/// A live in-process worker gRPC service plus one registered worker stream and
/// the production liveness probe over the same state.
struct Harness {
    state: ServerState,
    registry: ConnectedWorkerRegistry,
    probe: LivenessProbe,
    worker_id: aion_server::worker::WorkerId,
    /// Keeps the worker's outbound request stream open for the test duration.
    worker_tx: tokio::sync::mpsc::Sender<generated::WorkerToServer>,
    inbound: tonic::Streaming<generated::ServerToWorker>,
    server: tokio::task::JoinHandle<Result<(), tonic::transport::Error>>,
}

impl Harness {
    /// Boot the real worker gRPC service on a loopback port, connect one worker
    /// stream registered for [`ACTIVITY_TYPE`], and build the production probe
    /// over the SAME state the service registered into.
    async fn start() -> Result<Self, TestError> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let registry = ConnectedWorkerRegistry::default();
        let resolver = NamespaceResolver::authorization_only(
            NamespaceMode::SharedEngine,
            StaticWorkflowNamespaces::default(),
            StaticScheduleNamespaces::default(),
        );
        let state =
            ServerState::from_parts_with_registry(resolver, runtime_config(), registry.clone());
        let server = tokio::spawn(
            tonic::transport::Server::builder()
                .add_service(worker_service(state.clone()))
                .serve_with_incoming(TcpListenerStream::new(listener)),
        );

        let mut client = WorkerProtocolClient::connect(format!("http://{address}")).await?;
        let (worker_tx, worker_rx) = tokio::sync::mpsc::channel::<generated::WorkerToServer>(8);
        worker_tx
            .send(generated::WorkerToServer {
                message: Some(worker_to_server::Message::Register(
                    generated::RegisterWorker {
                        namespaces: vec![NAMESPACE.to_owned()],
                        activity_types: vec![ACTIVITY_TYPE.to_owned()],
                        task_queue: TASK_QUEUE.to_owned(),
                        node: String::new(),
                        activities: Vec::new(),
                        identity: "liveness-test-worker".to_owned(),
                        instance: None,
                    },
                )),
            })
            .await?;
        let mut request = tonic::Request::new(ReceiverStream::new(worker_rx));
        request
            .metadata_mut()
            .insert("x-aion-namespaces", NAMESPACE.parse()?);
        let mut inbound = client.stream_worker(request).await?.into_inner();

        let first = inbound
            .message()
            .await?
            .and_then(|frame| frame.message)
            .ok_or("response stream ended before the RegisterAck")?;
        let server_to_worker::Message::RegisterAck(ack) = first else {
            return Err(format!("first response frame must be RegisterAck, got {first:?}").into());
        };
        let worker_id = aion_server::worker::WorkerId::from_value(ack.worker_id);

        // The production probe, over the SAME registry and the SAME waiter
        // registry the gRPC service delivers answers into. `notifier: None` is
        // this boot's truth: it hosts no liminal listener.
        let probe = LivenessProbe::across_transports(
            None,
            Some(state.grpc_liveness_waiters().clone()),
            state.heartbeat_tracker().clone(),
            registry.clone(),
            HEARTBEAT_WINDOW,
        );

        Ok(Self {
            state,
            registry,
            probe,
            worker_id,
            worker_tx,
            inbound,
            server,
        })
    }

    /// Build the production dispatcher over the harness's shared state — the
    /// same wiring `ServerState::build_with_store_arc` hands the engine.
    fn dispatcher(&self) -> WorkerActivityDispatcher {
        WorkerActivityDispatcher::new(
            self.registry.clone(),
            NAMESPACE,
            self.state.heartbeat_tracker().clone(),
        )
        .with_pending(self.state.pending_activities().clone())
        .with_drain_state(self.state.drain_state().clone())
    }

    /// Run one probe round while the worker ANSWERS its ping, exactly as the
    /// SDK runtime does: read the next server frame, and if it is a liveness
    /// ping, echo its sequence straight back.
    ///
    /// The round and the answer must run concurrently — the round does not
    /// return until it has waited out the answer or the cadence, so answering
    /// after it returned would answer nothing.
    async fn answered_round(&mut self, sequence: u64) -> Result<(), TestError> {
        let Self {
            probe,
            inbound,
            worker_tx,
            ..
        } = self;
        let (_, answered) = tokio::join!(
            probe.probe_once(sequence),
            answer_next_ping(inbound, worker_tx)
        );
        answered
    }

    /// Run one probe round while the worker stays SILENT: the ping is read off
    /// the stream and deliberately not answered.
    ///
    /// Reading it matters. A worker that never drains its stream is a different
    /// failure (a backed-up channel, diagnosed as unaskable); this models the
    /// worker that receives the ping and does not reply.
    async fn silent_round(&mut self, sequence: u64) -> Result<(), TestError> {
        let Self { probe, inbound, .. } = self;
        let (_, drained) = tokio::join!(probe.probe_once(sequence), drain_next_ping(inbound));
        drained
    }

    /// Whether the published verdict currently admits this worker to dispatch.
    fn is_eligible(&self) -> Result<bool, TestError> {
        Ok(!self.registry.is_dispatch_ineligible(self.worker_id)?)
    }

    /// Wait for the next `ActivityTask` pushed down the worker stream, skipping
    /// any liveness ping that arrives first (and answering it, so a dispatch
    /// wait can never itself cause a withdrawal).
    async fn next_task(&mut self) -> Result<generated::ActivityTask, TestError> {
        while let Some(message) = self.inbound.message().await? {
            match message.message {
                Some(server_to_worker::Message::Task(task)) => return Ok(task),
                Some(server_to_worker::Message::LivenessPing(ping)) => {
                    self.worker_tx
                        .send(generated::WorkerToServer {
                            message: Some(worker_to_server::Message::LivenessAnswer(
                                generated::LivenessAnswer {
                                    liveness_ping: ping.liveness_ping,
                                },
                            )),
                        })
                        .await?;
                }
                _ => {}
            }
        }
        Err("worker stream closed before a task was delivered".into())
    }

    /// Report a successful activity result back over the worker stream.
    async fn complete(
        &self,
        task: generated::ActivityTask,
        result_json: &[u8],
    ) -> Result<(), TestError> {
        self.worker_tx
            .send(generated::WorkerToServer {
                message: Some(worker_to_server::Message::Result(
                    generated::ActivityResult {
                        workflow_id: task.workflow_id,
                        activity_id: task.activity_id,
                        run_id: task.run_id,
                        completion_token: task.completion_token,
                        outcome: Some(generated::activity_result::Outcome::Result(
                            generated::Payload {
                                content_type: "application/json".to_owned(),
                                bytes: result_json.to_vec(),
                            },
                        )),
                    },
                )),
            })
            .await?;
        Ok(())
    }
}

/// Read the next server frame and echo a liveness ping's sequence back.
///
/// A non-ping frame is an error rather than a skip: these rounds run against an
/// idle worker, so the ONLY frame that can arrive is the ping. Skipping
/// silently would let a test where no ping was ever sent pass by waiting out a
/// stream that had nothing on it.
async fn answer_next_ping(
    inbound: &mut tonic::Streaming<generated::ServerToWorker>,
    worker_tx: &tokio::sync::mpsc::Sender<generated::WorkerToServer>,
) -> Result<(), TestError> {
    let ping = next_ping(inbound).await?;
    worker_tx
        .send(generated::WorkerToServer {
            message: Some(worker_to_server::Message::LivenessAnswer(
                generated::LivenessAnswer {
                    liveness_ping: ping.liveness_ping,
                },
            )),
        })
        .await?;
    Ok(())
}

/// Read the next server frame and throw the ping away unanswered.
async fn drain_next_ping(
    inbound: &mut tonic::Streaming<generated::ServerToWorker>,
) -> Result<(), TestError> {
    next_ping(inbound).await.map(drop)
}

/// The next frame off the worker's stream, required to be a liveness ping.
async fn next_ping(
    inbound: &mut tonic::Streaming<generated::ServerToWorker>,
) -> Result<generated::LivenessPing, TestError> {
    let frame = inbound
        .message()
        .await?
        .ok_or("the worker stream closed while waiting for a liveness ping")?
        .message
        .ok_or("the server sent an empty frame")?;
    match frame {
        server_to_worker::Message::LivenessPing(ping) => Ok(ping),
        other => Err(format!(
            "the server did not ping this gRPC worker; it sent {other:?} instead. A worker that \
             is never asked can never answer, and can therefore never clear its dispatch \
             probation"
        )
        .into()),
    }
}

/// 🔴 THE DEFECT, PINNED. A gRPC-registered worker that answers every liveness
/// ping clears the SAME probation a liminal worker serves and becomes
/// dispatch-eligible.
///
/// Before this lane the probe never enumerated gRPC deliveries, so the first
/// round sent no ping at all, `next_ping` found no frame, and the worker sat
/// ineligible for the life of the process.
///
/// The three assertions are ordered so the test cannot pass vacuously: the
/// worker must be ineligible at registration (a test that started eligible
/// would prove nothing about probation), must still be ineligible after ONE
/// answer (a test that cleared on one answer would prove nothing about a
/// CONSECUTIVE run), and eligible only on the second.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_grpc_worker_clears_the_same_probation_a_liminal_worker_serves() -> Result<(), TestError>
{
    let mut harness = Harness::start().await?;

    assert!(
        !harness.is_eligible()?,
        "precondition: registration opens a probation and grants nothing — the handshake ack is \
         SENT by this server and nothing reports it was received"
    );

    harness.answered_round(1).await?;
    assert!(
        !harness.is_eligible()?,
        "one answered ping is not a RUN of answers; a link that answers one probe in three must \
         not accrue its way to eligibility"
    );

    harness.answered_round(2).await?;
    assert!(
        harness.is_eligible()?,
        "a gRPC worker that answered a full consecutive run of liveness pings has PROVED the \
         server can reach its dispatch path, and must be admitted to dispatch on exactly the \
         terms a liminal worker is"
    );

    harness.server.abort();
    Ok(())
}

/// THE CLOSING E2E: a gRPC worker that clears probation RECEIVES A DISPATCH and
/// round-trips its result.
///
/// This is the test that unblocks `aion worker awl serve`. Eligibility is not
/// the deliverable — a task in the worker's hands is.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_cleared_grpc_worker_receives_a_dispatch() -> Result<(), TestError> {
    let mut harness = Harness::start().await?;
    for sequence in 1..=2 {
        harness.answered_round(sequence).await?;
    }
    assert!(
        harness.is_eligible()?,
        "precondition: the worker cleared its probation"
    );

    let dispatcher = Arc::new(harness.dispatcher());
    let dispatch = tokio::spawn(futures::future::lazy(move |_| {
        dispatcher.dispatch(greet_request(r#"{"name":"world"}"#))
    }));

    let task = harness.next_task().await?;
    assert_eq!(task.activity_type, ACTIVITY_TYPE);
    harness
        .complete(task, br#"{"greeting":"hello world"}"#)
        .await?;
    let result = dispatch.await.map_err(|error| error.to_string())?;
    assert_eq!(result, Ok(r#"{"greeting":"hello world"}"#.to_owned()));

    harness.server.abort();
    Ok(())
}

/// THE CONTROL for the test above, and the load-bearing half of the defect: a
/// gRPC worker that has NOT cleared its probation is not dispatched to.
///
/// Without this, a probe that admitted everything unconditionally would satisfy
/// every eligibility assertion in this file. The refusal must also be the R3
/// one — a pool with a registered-but-excluded worker is not an empty pool.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_unprobated_grpc_worker_is_not_dispatched_to() -> Result<(), TestError> {
    let harness = Harness::start().await?;
    // One round, unanswered on purpose: it publishes the verdict that excludes
    // this worker. Without a round nothing is published at all and the worker
    // would be selected by an empty exclusion set.
    harness.probe.probe_once(1).await;
    assert!(
        !harness.is_eligible()?,
        "precondition: the worker has proved nothing"
    );

    assert!(
        harness
            .registry
            .select_worker(NAMESPACE, TASK_QUEUE, ACTIVITY_TYPE, None)?
            .is_none(),
        "an excluded worker must not be selected: a dispatch to a worker the server cannot prove \
         it reaches can only fail"
    );
    assert_eq!(
        harness
            .registry
            .ineligible_workers_for(NAMESPACE, TASK_QUEUE, ACTIVITY_TYPE, None)?,
        1,
        "R3: the pool is NOT empty — one worker is registered and excluded, and a refusal that \
         says 'no worker registered' sends the operator to start a second one that would serve \
         the same probation"
    );

    harness.server.abort();
    Ok(())
}

/// R2, mid-probation silence: answer, silence, answer, answer → cleared on the
/// second consecutive PAIR, not on the interrupted first.
///
/// The interruption must reset the count to zero rather than decrement it. A
/// cumulative counter would clear here after the third answer, so the assertion
/// after round 3 is the one that distinguishes the two designs.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mid_probation_silence_restarts_the_run_from_zero() -> Result<(), TestError> {
    let mut harness = Harness::start().await?;

    harness.answered_round(1).await?;
    harness.silent_round(2).await?;
    assert!(
        !harness.is_eligible()?,
        "an interrupted probation is still a probation"
    );

    harness.answered_round(3).await?;
    assert!(
        !harness.is_eligible()?,
        "the answer BEFORE the silence must not count toward the run after it — a cumulative \
         counter would admit the worker here on two lifetime answers"
    );

    harness.answered_round(4).await?;
    assert!(
        harness.is_eligible()?,
        "two CONSECUTIVE answers is the whole requirement, and this pair is consecutive"
    );

    harness.server.abort();
    Ok(())
}

/// R2, mid-probation silence says NOTHING out loud beyond the probation it
/// already announced.
///
/// Nothing that was held was lost, so a WARN here would be an alarm on an
/// ordinary worker start — the exact false alarm the three-transition
/// announcement law was written to end. Asserted by REASON: the captured events
/// are filtered on what they SAY, not on how many there are.
#[tokio::test]
async fn mid_probation_silence_raises_no_withdrawal_alarm() -> Result<(), TestError> {
    let events = capture_log();
    // Thread-scoped, over a CURRENT-THREAD runtime: every future this test
    // awaits — the probe round, the verdict, the announcements — runs on this
    // very thread, so the capture demonstrably covers the code under test. A
    // multi-thread runtime would move the probe onto a worker thread with no
    // subscriber, and the absence assertions below would pass on an empty log.
    let _guard = tracing::subscriber::set_default(Registry::default().with(CaptureLayer {
        events: std::sync::Arc::clone(&events),
    }));

    let mut harness = Harness::start().await?;
    harness.answered_round(1).await?;
    harness.silent_round(2).await?;

    let captured = events.lock().map_err(|_| "capture log poisoned")?.clone();
    let withdrawals = withdrawal_events(&captured);
    assert!(
        withdrawals.is_empty(),
        "a worker that has never HELD eligibility cannot lose it, and announcing a withdrawal \
         here tells an operator their healthy worker's dispatch path is dead. Said instead: {}",
        render(&withdrawals)
    );
    // The vacuity control: this test must be able to SEE a withdrawal. If the
    // capture were mis-installed (a thread-scoped subscriber over work that
    // runs elsewhere) the assertion above would pass on an empty log and prove
    // nothing at all.
    let probations = captured
        .iter()
        .filter(|event| event.message.contains("SERVING ITS DISPATCH PROBATION"))
        .count();
    assert!(
        probations > 0,
        "the capture must be able to observe the probe's announcements; it saw: {}",
        render(&captured)
    );

    harness.server.abort();
    Ok(())
}

/// R2, post-clearance: eligibility that was actually HELD is withdrawn with the
/// WARN transition, and recovery is announced with the INFO all-clear.
///
/// Both transitions are asserted by their REASONS — the withdrawal must say the
/// worker HAD proved its path and the server can no longer prove it, and the
/// all-clear must say the run of answers is complete. Asserting on counts or on
/// levels alone would be satisfied by any two events of the right severity,
/// including the probation-opening INFO that fires on every healthy start.
#[tokio::test]
async fn post_clearance_silence_withdraws_loudly_and_recovery_says_the_all_clear()
-> Result<(), TestError> {
    let events = capture_log();
    // Thread-scoped, over a CURRENT-THREAD runtime: every future this test
    // awaits — the probe round, the verdict, the announcements — runs on this
    // very thread, so the capture demonstrably covers the code under test. A
    // multi-thread runtime would move the probe onto a worker thread with no
    // subscriber, and the absence assertions below would pass on an empty log.
    let _guard = tracing::subscriber::set_default(Registry::default().with(CaptureLayer {
        events: std::sync::Arc::clone(&events),
    }));

    let mut harness = Harness::start().await?;
    for sequence in 1..=2 {
        harness.answered_round(sequence).await?;
    }
    assert!(
        harness.is_eligible()?,
        "precondition: eligibility was genuinely HELD before it was lost — a test that starts \
         ineligible measures a probation, not a withdrawal"
    );

    harness.silent_round(3).await?;
    assert!(
        !harness.is_eligible()?,
        "a worker whose dispatch path the server can no longer prove is excluded that same round"
    );
    let captured = events.lock().map_err(|_| "capture log poisoned")?.clone();
    let withdrawals = withdrawal_events(&captured);
    assert_eq!(
        withdrawals.len(),
        1,
        "exactly one withdrawal, naming the reason. Captured: {}",
        render(&captured)
    );
    let withdrawal = withdrawals.first().ok_or("withdrawal vanished")?;
    assert_eq!(withdrawal.level, tracing::Level::WARN);
    assert_eq!(
        withdrawal.fields.get("transport").map(String::as_str),
        Some("grpc"),
        "the operator must be told WHICH link to look at: {withdrawal:?}"
    );

    // Recovery: a full run of consecutive answers, and the all-clear said out
    // loud. An alarm nobody cancels is read as ongoing.
    for sequence in 4..=5 {
        harness.answered_round(sequence).await?;
    }
    assert!(
        harness.is_eligible()?,
        "recovery requires the FULL run again, and this is it"
    );
    let captured = events.lock().map_err(|_| "capture log poisoned")?.clone();
    let all_clears: Vec<CapturedEvent> = captured
        .iter()
        .filter(|event| event.message.contains("DISPATCH ELIGIBILITY RESTORED"))
        .cloned()
        .collect();
    assert_eq!(
        all_clears.len(),
        1,
        "the recovery must be announced exactly once. Captured: {}",
        render(&captured)
    );
    let all_clear = all_clears.first().ok_or("all-clear vanished")?;
    assert_eq!(all_clear.level, tracing::Level::INFO);
    assert!(
        all_clear.message.contains("answered a full run"),
        "the all-clear must say WHY it is clear: {all_clear:?}"
    );

    harness.server.abort();
    Ok(())
}

/// The withdrawal announcements, selected by what they SAY.
///
/// Reason-anchored on purpose: a filter on `Level::WARN` alone would also match
/// the two ping-failure lines the same round emits, and a test that counted
/// those would report a withdrawal on a worker that never held eligibility.
fn withdrawal_events(captured: &[CapturedEvent]) -> Vec<CapturedEvent> {
    captured
        .iter()
        .filter(|event| event.message.contains("WITHDRAWING DISPATCH ELIGIBILITY"))
        .cloned()
        .collect()
}
