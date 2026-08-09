//! Liminal worker transport: receive pushed dispatches, execute, reply (LSUB-1).
//!
//! # What this is (bounded spike)
//!
//! This module is the SUBSCRIBER half of the cross-node work-dispatch path the
//! server's [`liminal_transport`] module dispatches into. Behind the
//! `liminal-transport` Cargo feature, a [`LiminalActivityWorker`] connects a
//! server-push client to a liminal server, then runs a serve loop: it receives a
//! server-pushed [`DispatchRequest`], executes the named activity through the
//! EXISTING [`ActivityRegistry`](crate::ActivityRegistry) (the same execution
//! path the gRPC worker uses), and answers with a correlated [`DispatchResponse`]
//! on the same connection. The default worker build (no feature) is byte-identical
//! and never links liminal.
//!
//! [`liminal_transport`]: https://docs.rs/aion-server
//!
//! # The transport it composes (LSUB-0 server push)
//!
//! Liminal's LSUB-0 primitive is a SERVER-INITIATED push: the server writes a
//! `Frame::Push` (correlation id + opaque payload) on the client's existing
//! connection, and the client answers with a correlated `Frame::PushReply`. The
//! SDK side is [`liminal_sdk::PushClient`]: a background reader thread surfaces
//! each pushed frame on a channel ([`PushClient::recv_timeout`]), and the caller
//! sends the correlated reply with [`PushClient::reply`]. This worker drives that
//! loop synchronously on a dedicated blocking thread (the push client is
//! thread-based, not async), executing each activity on a Tokio runtime handle.
//!
//! # Wire contract (must match the server byte-for-byte)
//!
//! The server side serializes its `DispatchRequest`/`DispatchResponse` (in
//! `aion-server`'s `liminal_transport`) through serde JSON. This module mirrors
//! those structs field-for-field with the SAME serde field names and the SAME
//! `aion-core` id types ([`WorkflowId`], [`RunId`]), so the JSON on the wire is
//! identical. The two crates cannot share one struct (the worker must not depend
//! on the server), so the contract is pinned by the shared field set and a wire
//! round-trip test here; any divergence is a wire-compatibility break.
//!
//! # In-band self-registration (LSUB-L2)
//!
//! The worker is SELF-DESCRIBING over the socket. [`LiminalActivityWorker::connect`]
//! builds a [`liminal::protocol::WorkerRegistration`] from the worker's
//! [`WorkerConfig`] (its `namespaces`, `task_queue`, `node`, `identity`) and the
//! activity-type names it binds in its [`ActivityRegistry`], then connects via the
//! SDK's `connect_with_registration`: a synchronous `WorkerRegister` ->
//! `WorkerRegisterAck` round-trip runs before the push reader spawns. The server's
//! installed connection-notifier turns that into a first-class connected-worker
//! registry membership, so the worker is selected the SAME way a gRPC worker is —
//! retiring the LSUB-1 out-of-band `active_connection_pids()` + hard-coded
//! server-side registration. A `Rejected` ack surfaces as a connect error
//! (the rejection reason is carried), so a worker the server declines never
//! believes it is registered.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use aion_core::{
    ActivityId, ContentType, InterventionCapabilities, InterventionCommand, InterventionOutcome,
    Payload, RunId, WorkflowId,
};
use aion_integrations::contract::DynAgentHarness;
use aion_integrations::spec::AgentRunSpec;
use liminal::protocol::WorkerRegistration;
use liminal_sdk::{PushClient, PushWriter, PushedFrame};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::activity::ActivityRegistry;
use crate::config::WorkerConfig;
use crate::context::ActivityContext;
use crate::error::WorkerError;
use crate::protocol::ActivityTask;
use crate::runtime::agent::spawn_dyn_agent;
use crate::runtime::intervention::{ControlRegistry, SessionKey};
use crate::runtime::liminal_drain::{DrainBinding, LiveWriter, spawn_event_drain};
use crate::runtime::liminal_liveness::{LivenessPing, LivenessPong, SilenceMonitor};
use crate::runtime::liminal_redial::{RedialBackoff, ServeResult};
use crate::runtime::loop_::{ActivityDispatcher, DispatchOutcome};

/// Wire request carrying one scheduled activity from the server to this worker.
///
/// Field-for-field mirror of `aion-server`'s `liminal_transport::DispatchRequest`
/// (same serde field names + `aion-core` id types), so the JSON the server pushes
/// deserializes here unchanged. See the module docs for the cross-crate contract.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DispatchRequest {
    /// Activity type this worker must execute.
    pub activity_type: String,
    /// Workflow that scheduled this fan-out activity.
    pub workflow_id: WorkflowId,
    /// Pinned ordinal of this activity within the workflow's fan-out range.
    pub ordinal: u64,
    /// Run that dispatched this ordinal, when known (continue-as-new safety).
    pub run_id: Option<RunId>,
    /// Opaque execution generation echoed verbatim in the response.
    pub completion_token: String,
    /// Stable external-effect key for this run and action site.
    pub idempotency_key: String,
    /// Opaque activity input bytes (JSON-tagged on the aion side).
    pub input: Vec<u8>,
    /// One-based delivery attempt (the gRPC `ActivityTask.attempt` mirror).
    /// Serde-defaulted to `1` so a frame from a pre-attempt server decodes as
    /// a first delivery — the exact prior behaviour.
    #[serde(default = "first_attempt")]
    pub attempt: u32,
    /// Engine-provided labels (the gRPC `ActivityTask.labels` mirror); empty
    /// on the outbox path, which has no label source.
    #[serde(default)]
    pub labels: std::collections::BTreeMap<String, String>,
    /// The server's heartbeat window in milliseconds when this dispatch is
    /// liveness-TRACKED on the server (the engine-seam bridge path), or `0`
    /// when it is not (the outbox path). Non-zero arms this worker's automatic
    /// liveness pump: beats every quarter-window on
    /// [`WORKER_LIVENESS_CHANNEL`] so the server's expiry sweeper never
    /// falsely expires a healthy long-running activity.
    #[serde(default)]
    pub heartbeat_window_ms: u64,
}

/// Serde default for [`DispatchRequest::attempt`]: a frame that predates the
/// attempt field is a first delivery.
const fn first_attempt() -> u32 {
    1
}

/// Validate one dispatch envelope and RETURN its run id.
///
/// The run is returned rather than merely checked because everything downstream
/// needs it as a plain `RunId`: the agent run spec, the session key a routed
/// intervention resolves through, and every transcript event the attempt emits
/// are all keyed on `(workflow, run, activity, attempt)`. Handing back the
/// validated value is what keeps those call sites from re-deriving it from the
/// wire `Option` — which would either re-do this check or, worse, invent a
/// substitute for a `None` that this function has already refused.
fn validate_dispatch_envelope(request: &DispatchRequest) -> Result<RunId, WorkerError> {
    // The run is validated by EXTRACTING it, first and in one place: the check
    // that a run-less envelope is invalid and the production of the value every
    // caller needs are the same expression, so they cannot drift apart into a
    // guard that passes and a caller that then substitutes something for `None`.
    let Some(run_id) = request.run_id.clone() else {
        return Err(WorkerError::decode(InvalidDispatchEnvelope {
            reason: "run_id is missing",
        }));
    };
    let reason = if request.attempt == 0 {
        Some("attempt is zero")
    } else if request.completion_token.is_empty() {
        Some("completion_token is missing")
    } else if request.idempotency_key.is_empty() {
        Some("idempotency_key is missing")
    } else {
        None
    };
    match reason {
        Some(reason) => Err(WorkerError::decode(InvalidDispatchEnvelope { reason })),
        None => Ok(run_id),
    }
}

#[derive(Debug, thiserror::Error)]
#[error("invalid liminal activity dispatch envelope: {reason}")]
struct InvalidDispatchEnvelope {
    reason: &'static str,
}

/// Reserved liminal channel this worker publishes automatic liveness beats on.
/// Byte-for-byte mirror of the server crate's constant of the same name (the
/// same cross-crate contract the dispatch/response mirrors pin).
pub const WORKER_LIVENESS_CHANNEL: &str = "aion.worker.liveness";

/// Wire liveness beat for one in-flight dispatch — the liminal mirror of the
/// gRPC liveness heartbeat (no progress payload). Field-for-field mirror of the
/// server crate's `WorkerLivenessBeat`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkerLivenessBeat {
    /// Workflow owning the in-flight activity being kept alive.
    pub workflow_id: WorkflowId,
    /// Pinned ordinal of the in-flight activity being kept alive.
    pub ordinal: u64,
}

/// Reserved liminal channel this worker announces its intervention capabilities
/// on, immediately after each (re)registration. Byte-for-byte mirror of the
/// server crate's constant of the same name. The in-band registration frame is
/// a published liminal protocol type and cannot carry aion-level capability
/// metadata, so the announcement rides this channel instead (NOI-6).
pub const WORKER_CAPABILITIES_CHANNEL: &str = "aion.worker.capabilities";

/// Wire announcement of this worker's advertised intervention capabilities.
/// Field-for-field mirror of the server crate's `WorkerCapabilitiesAnnouncement`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkerCapabilitiesAnnouncement {
    /// The neutral intervention primitives this worker's harness supports.
    pub capabilities: InterventionCapabilities,
}

/// Wire response carrying this worker's result back to the server.
///
/// Field-for-field mirror of `aion-server`'s
/// `liminal_transport::DispatchResponse`, so the server's `LiminalCompletionSource`
/// re-enters it unchanged.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DispatchResponse {
    /// Workflow the completion belongs to.
    pub workflow_id: WorkflowId,
    /// Pinned ordinal the completion correlates against.
    pub ordinal: u64,
    /// Run that issued the dispatch, echoed back for the run gate.
    pub run_id: Option<RunId>,
    /// Opaque execution generation echoed from the request.
    pub completion_token: String,
    /// Worker outcome: `Ok(result)` or `Err(reason)`.
    pub outcome: Result<String, String>,
}

/// Wire request carrying one neutral mid-run intervention command from the server
/// to this worker (NOI-6).
///
/// Field-for-field mirror of `aion-server`'s
/// `liminal_transport::InterventionRequest`. It rides the SAME server-push channel
/// as [`DispatchRequest`], distinguished by its unique required `intervention`
/// field — a [`DispatchRequest`] has none, so the serve loop demuxes the two by
/// which one deserializes. The envelope is neutral: it carries an
/// [`InterventionCommand`], never a harness type.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct InterventionRequest {
    /// The neutral command to deliver to the session owning the target attempt.
    pub intervention: InterventionCommand,
}

/// Wire reply carrying this worker's neutral intervention ack back to the server
/// (NOI-6). Field-for-field mirror of `aion-server`'s
/// `liminal_transport::InterventionReply`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct InterventionReply {
    /// The neutral applied/gated/stale outcome the operator receives.
    pub outcome: InterventionOutcome,
}

/// How long the serve loop blocks for the next server push before re-checking the
/// shutdown flag. A bounded poll lets [`LiminalActivityWorker::serve_until`] stop
/// promptly on a quiet connection rather than blocking forever.
const RECV_POLL: Duration = Duration::from_millis(100);

/// The composed agent harness a served worker drives, plus the agent activity
/// types it owns and the neutral [`InterventionCapabilities`] it advertises
/// (NOI-5b/NOI-6).
///
/// This bundles the three things [`LiminalActivityWorker::with_agent_harness`]
/// needs into one `Option`-shaped value so the production serve path
/// ([`serve_with_redial`](crate::serve_with_redial)) can thread a composed harness
/// through as a single argument — or `None` for a harness-less build, which serves
/// non-agent activities exactly as before. The harness is ERASED
/// (`Arc<dyn DynAgentHarness>`), so no concrete adapter type ever appears in this
/// platform crate.
#[derive(Clone)]
pub struct AgentHarnessConfig {
    /// The erased agent harness the worker drives for its agent activity types.
    harness: Arc<dyn DynAgentHarness>,
    /// The activity-type names routed through the harness rather than the registry.
    agent_activity_types: BTreeSet<String>,
    /// Explicit schemas for agent activities whose concrete output is harness-owned.
    activity_descriptors: Vec<aion_package::ActivityDescriptor>,
    /// The neutral intervention primitives the harness advertises.
    capabilities: InterventionCapabilities,
}

impl AgentHarnessConfig {
    /// Builds a config from a composed (erased) `harness`, the `agent_activity_types`
    /// it owns, and the `capabilities` it advertises.
    #[must_use]
    pub fn new(
        harness: Arc<dyn DynAgentHarness>,
        agent_activity_types: impl IntoIterator<Item = impl Into<String>>,
        capabilities: InterventionCapabilities,
    ) -> Self {
        Self {
            harness,
            agent_activity_types: agent_activity_types.into_iter().map(Into::into).collect(),
            activity_descriptors: Vec::new(),
            capabilities,
        }
    }

    /// The agent activity-type names this config owns — the set the serve path must
    /// ADVERTISE in registration so the server can select the worker for them.
    #[must_use]
    pub fn agent_activity_types(&self) -> &BTreeSet<String> {
        &self.agent_activity_types
    }

    /// Commit explicit typed descriptors for harness-owned activities.
    ///
    /// Descriptor names are added to the advertised activity set so a schema can
    /// never be sent for an activity the worker does not also route.
    #[must_use]
    pub fn with_activity_descriptors(
        mut self,
        descriptors: impl IntoIterator<Item = aion_package::ActivityDescriptor>,
    ) -> Self {
        self.activity_descriptors = descriptors.into_iter().collect();
        self.agent_activity_types.extend(
            self.activity_descriptors
                .iter()
                .map(|descriptor| descriptor.name.clone()),
        );
        self
    }

    /// Explicit typed descriptors carried for harness-owned activities.
    #[must_use]
    pub fn activity_descriptors(&self) -> &[aion_package::ActivityDescriptor] {
        &self.activity_descriptors
    }
}

impl std::fmt::Debug for AgentHarnessConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AgentHarnessConfig")
            .field("agent_activity_types", &self.agent_activity_types)
            .field("capabilities", &self.capabilities)
            .finish_non_exhaustive()
    }
}

/// A worker that serves activities over the liminal server-push transport.
///
/// Construct with [`LiminalActivityWorker::connect`], then drive the serve loop
/// with [`LiminalActivityWorker::serve_until`] (loops until the stop flag) or
/// [`LiminalActivityWorker::serve_one`] (handles exactly one pushed dispatch,
/// used by tests and single-shot callers). The activity registry is the SAME
/// typed registry the gRPC worker executes through.
pub struct LiminalActivityWorker {
    client: PushClient,
    registry: Arc<ActivityRegistry>,
    /// The queue handing plain typed-registry dispatches from the serve loop to
    /// the dedicated execution thread ([`spawn_plain_dispatch_executor`]).
    ///
    /// That thread drains this queue STRICTLY ONE AT A TIME, so plain activities
    /// execute exactly as serially as they did when they ran inline on the serve
    /// loop. This is not a new limit; it is the existing one, preserved once the
    /// execution stopped being what enforced it. Widening it is a configuration
    /// decision about how much work one worker may do at once, and it has teeth:
    /// the dispatches that would newly overlap are whole `cargo` builds.
    ///
    /// Why a dedicated thread and not `tokio::spawn`: the serve loop's receive
    /// (`PushClient::recv_timeout`) BLOCKS — the push client is thread-based,
    /// not async — and `serve_with_redial`, the entry point every real worker
    /// uses, drives that loop with `block_on` on a CURRENT-THREAD runtime. A
    /// `tokio::spawn`ed dispatch on that runtime is never polled, because the
    /// only task on it blocks in the receive and never yields: the dispatch is
    /// received, logged, and then silently never executed. Owning a runtime
    /// removes the dependency on the caller's runtime flavour entirely.
    plain_dispatch_tx: tokio::sync::mpsc::UnboundedSender<QueuedPlainDispatch>,
    /// The attempt back-index a pushed intervention is routed through (NOI-6). A
    /// pushed [`InterventionRequest`] is delivered to the live session owning its
    /// target `(workflow, activity, attempt)`; a command with no live owner is the
    /// attempt-scoped stale-target no-op. Shared (an `Arc` inside) with the
    /// session-spawn path that registers each running agent session.
    control: ControlRegistry,
    /// Optional agent harness this worker drives for its agent activity types
    /// (NOI-5b/NOI-6). When installed via [`Self::with_agent_harness`], an activity
    /// whose type is in [`Self::agent_activity_types`] is executed by driving the
    /// harness through [`spawn_dyn_agent`] — streaming its transcript live and
    /// self-registering the live session — instead of the plain typed registry. A
    /// worker without a harness (the default) is byte-identical to before.
    agent_harness: Option<Arc<dyn DynAgentHarness>>,
    /// The activity-type names executed through the agent harness rather than the
    /// plain registry. Empty unless [`Self::with_agent_harness`] installs a harness.
    agent_activity_types: BTreeSet<String>,
    /// The neutral intervention primitives the installed agent harness advertises —
    /// declared at construction (mirroring the server-side notifier's
    /// `with_intervention_capabilities`), because capabilities are needed to register
    /// the session in the [`ControlRegistry`] BEFORE the session starts.
    agent_capabilities: InterventionCapabilities,
    /// The shared live-connection slot every observability drain publishes
    /// through (#254). Seeded at connect with this connection's writer; the redial
    /// driver ([`serve_with_redial`](crate::serve_with_redial)) REPLACES it with a
    /// slot it refreshes on every reconnection (see [`Self::with_live_writer`]), so
    /// a drain re-resolves the survivor after a server loss instead of publishing
    /// to a dead socket forever.
    live_writer: LiveWriter,
    /// The bounded reconnect backoff the observability drain paces its re-probes
    /// with during an outage, seeded from the worker's reconnect config so the
    /// drain and the dispatch redial share one coherent schedule.
    drain_backoff: RedialBackoff,
    /// The liminal listen address this connection was dialed on, kept so every
    /// connection-level log line (a declared link death, a failed redial) names
    /// WHICH endpoint it is talking about rather than leaving the operator to
    /// guess from a candidate list.
    address: String,
    /// The connection dead-man switch: armed by the server's liveness pings and
    /// re-armed by every inbound frame. Once armed, silence past the
    /// server-declared window is a DECLARED link death — the detection that
    /// turns a wedged half-open socket from an unbounded blind wait into a
    /// logged teardown plus a redial.
    silence: SilenceMonitor,
}

impl std::fmt::Debug for LiminalActivityWorker {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LiminalActivityWorker")
            .field("client", &self.client)
            .finish_non_exhaustive()
    }
}

impl LiminalActivityWorker {
    /// Connects a server-push client to `address`, SELF-REGISTERS in-band, and
    /// starts its background reader, binding this worker's typed activity registry.
    ///
    /// The registration is built from `config` (its `namespaces`, `task_queue`,
    /// `node`, `identity`) and the activity-type names bound in `registry`, then
    /// driven through the SDK's `connect_with_registration`: the
    /// `WorkerRegister` -> `WorkerRegisterAck` round-trip completes synchronously
    /// before the push reader spawns. The server's connection-notifier turns the
    /// accepted registration into a connected-worker registry membership.
    ///
    /// # Errors
    ///
    /// Returns [`WorkerError::Transport`] when the push connection, handshake, or
    /// registration fails — INCLUDING a server-side `Rejected` registration, whose
    /// reason is carried in the error so the worker never serves while unregistered.
    pub fn connect(
        address: &str,
        config: &WorkerConfig,
        registry: Arc<ActivityRegistry>,
    ) -> Result<Self, WorkerError> {
        Self::connect_advertising(address, config, registry, &BTreeSet::new(), &[])
    }

    /// Connects like [`Self::connect`] but ADDITIONALLY advertises `agent_types` in
    /// the in-band registration (NOI-5b/NOI-6).
    ///
    /// An agent activity is driven by the installed harness, not the typed registry,
    /// so its type never appears in `registry.activity_types()`. Without advertising
    /// it here, the server could not SELECT this worker for that activity — a worker
    /// whose only activities are agent-driven would register advertising nothing. So
    /// the registration announces `registry`'s types UNION `agent_types`; the union is
    /// what makes an agent-only worker selectable. Used by the production serve path
    /// ([`serve_with_redial`](crate::serve_with_redial)) so a redialed worker
    /// re-advertises its agent types on every connection.
    ///
    /// # Errors
    ///
    /// Same as [`Self::connect`]: [`WorkerError::Transport`] on a failed connect,
    /// handshake, or registration.
    pub fn connect_advertising(
        address: &str,
        config: &WorkerConfig,
        registry: Arc<ActivityRegistry>,
        agent_types: &BTreeSet<String>,
        agent_descriptors: &[aion_package::ActivityDescriptor],
    ) -> Result<Self, WorkerError> {
        let mut registration = registration_from(config, &registry);
        for agent_type in agent_types {
            if !registration.activity_types.contains(agent_type) {
                registration.activity_types.push(agent_type.clone());
            }
        }
        for descriptor in agent_descriptors {
            if !registration
                .activities
                .iter()
                .any(|activity| activity.name == descriptor.name)
            {
                registration
                    .activities
                    .push(liminal::protocol::WorkerActivityDescriptor {
                        name: descriptor.name.clone(),
                        input_schema_json: descriptor.input_schema.to_string(),
                        output_schema_json: descriptor.output_schema.to_string(),
                    });
            }
        }
        let client = PushClient::connect_with_registration(address, registration)
            .map_err(|error| transport_error(&error))?;
        // Seed the drain slot with THIS connection's writer so a single-connection
        // serve (the direct `serve_until` callers and tests) resolves a live writer
        // immediately; the redial driver swaps in a shared, reconnect-refreshed slot
        // via `with_live_writer`. The drain backoff mirrors the dispatch reconnect
        // schedule, so both pace re-attempts identically.
        let live_writer = LiveWriter::seeded(client.writer_handle());
        let drain_backoff = RedialBackoff::new(
            config.reconnect.initial_backoff,
            config.reconnect.max_backoff,
        );
        Ok(Self {
            client,
            registry,
            // A single draining thread, preserving the concurrency plain
            // dispatches already had when they executed inline on the serve loop.
            plain_dispatch_tx: spawn_plain_dispatch_executor(),
            control: ControlRegistry::new(),
            agent_harness: None,
            agent_activity_types: BTreeSet::new(),
            agent_capabilities: InterventionCapabilities::none(),
            live_writer,
            drain_backoff,
            address: address.to_owned(),
            silence: SilenceMonitor::new(),
        })
    }

    /// Install an agent `harness` this worker drives for the given
    /// `agent_activity_types`, advertising `capabilities` (NOI-5b/NOI-6).
    ///
    /// An activity whose type is in `agent_activity_types` is executed by driving the
    /// harness through [`spawn_dyn_agent`]: its transcript streams LIVE to the server
    /// over this worker's connection, and the live session self-registers in the
    /// [`ControlRegistry`] under its `(workflow, activity, attempt)` key so a pushed
    /// intervention reaches it. `capabilities` is the harness's advertised neutral
    /// primitive set (the same set the server-side notifier advertises for this
    /// worker), gated on before a command is delivered. Every other activity type
    /// runs through the plain typed registry exactly as before; a worker built
    /// without this builder never touches the agent path.
    #[must_use]
    pub fn with_agent_harness(
        mut self,
        harness: Arc<dyn DynAgentHarness>,
        agent_activity_types: impl IntoIterator<Item = impl Into<String>>,
        capabilities: InterventionCapabilities,
    ) -> Self {
        self.agent_harness = Some(harness);
        self.agent_activity_types = agent_activity_types.into_iter().map(Into::into).collect();
        self.agent_capabilities = capabilities;
        self
    }

    /// Install the optional agent harness carried by an [`AgentHarnessConfig`], if
    /// one is supplied (NOI-5b/NOI-6).
    ///
    /// This is the single seam the production serve path
    /// ([`serve_with_redial`](crate::serve_with_redial)) threads a composed harness
    /// through: `Some(config)` applies it via [`Self::with_agent_harness`], `None`
    /// leaves the worker on the plain typed-registry path exactly as before — so a
    /// harness-less build (`--no-default-features`) is unaffected. Kept distinct from
    /// [`Self::with_agent_harness`] so the redial driver can carry the harness as one
    /// `Option`-shaped value.
    #[must_use]
    pub fn with_agent_config(self, config: Option<AgentHarnessConfig>) -> Self {
        match config {
            Some(config) => self.with_agent_harness(
                config.harness,
                config.agent_activity_types,
                config.capabilities,
            ),
            None => self,
        }
    }

    /// Adopt `slot` as the SHARED live-connection drain slot and record this
    /// connection's writer in it (#254).
    ///
    /// The redial driver ([`serve_with_redial`](crate::serve_with_redial)) owns one
    /// slot across every reconnection and threads it into each freshly-connected
    /// worker here, so every observability drain — including one spawned against a
    /// now-dead earlier connection — re-resolves the SURVIVOR the driver most
    /// recently installed. A worker served directly (without the redial driver)
    /// keeps its own per-connection slot seeded at connect; there it simply
    /// coalesces-and-drops on a loss, there being no survivor to migrate to.
    #[must_use]
    pub(crate) fn with_live_writer(mut self, slot: LiveWriter) -> Self {
        slot.set(self.client.writer_handle());
        self.live_writer = slot;
        self
    }

    /// Whether `activity_type` is driven through the installed agent harness rather
    /// than the plain typed registry — `true` only when a harness is installed AND
    /// the type is registered as an agent type.
    ///
    /// The public form of the internal routing predicate ([`Self::is_agent_activity`]),
    /// exposed so a caller (and the wiring tests) can assert a served worker actually
    /// routes an agent type to the agent path.
    #[must_use]
    pub fn drives_agent_activity(&self, activity_type: &str) -> bool {
        self.is_agent_activity(activity_type)
    }

    /// The intervention control back-index this worker routes pushed commands
    /// through (NOI-6). The session-spawn path registers each running agent session
    /// here so a routed intervention reaches the live attempt that owns it.
    #[must_use]
    pub fn control_registry(&self) -> &ControlRegistry {
        &self.control
    }

    /// Blocks up to `RECV_POLL` for the next pushed dispatch, executes it, and
    /// replies. Returns `Ok(true)` when one dispatch was served, `Ok(false)` when
    /// the poll elapsed with no push (so the caller can re-check a stop flag).
    ///
    /// # Errors
    ///
    /// Returns [`WorkerError`] when a push frame cannot be decoded, the activity
    /// reply cannot be encoded, or the reply cannot be written to the socket.
    pub async fn serve_one(&self) -> Result<bool, WorkerError> {
        match self.client.recv_timeout(RECV_POLL) {
            Ok(frame) => {
                // ANY inbound frame is proof the link is alive: a dispatch, an
                // intervention, or a liveness ping all re-arm the dead-man
                // switch, so a busy connection is never declared dead.
                self.silence.record_inbound();
                let served = self.handle_pushed_frame(frame).await;
                // Re-arm on COMPLETION too. Handling a frame no longer executes
                // anything — both dispatch kinds are handed off — but an
                // intervention or a decode still costs time on this loop, and
                // the switch measures silence while the worker is LISTENING:
                // time spent handling is not silence, it is work. Kept rather
                // than removed, because the cost of an unnecessary re-arm is
                // nothing and the cost of a missing one is a healthy link
                // declared dead.
                self.silence.record_inbound();
                served?;
                Ok(true)
            }
            // A bare timeout with no push is not an error: surface it as "nothing
            // served" so the serve loop can re-check its stop flag. Any other
            // receive error (the reader stopped, the server closed) is fatal.
            Err(error) if is_recv_timeout(&error) => Ok(false),
            Err(error) => Err(transport_error(&error)),
        }
    }

    /// Publish this worker's advertised intervention capabilities on the
    /// reserved [`WORKER_CAPABILITIES_CHANNEL`] (NOI-6), once per connection.
    ///
    /// A worker with no harness capabilities announces nothing: the empty set
    /// is already the server-side registration default (observability-only).
    /// A publish or encode fault is logged, never fatal — the worker still
    /// serves; the operator just sees no intervention controls until a
    /// reconnect re-announces.
    fn announce_capabilities(&self) {
        if self.agent_capabilities.supported.is_empty() {
            return;
        }
        let announcement = WorkerCapabilitiesAnnouncement {
            capabilities: self.agent_capabilities.clone(),
        };
        match serde_json::to_vec(&announcement) {
            Ok(payload) => {
                if let Err(error) = self
                    .client
                    .writer_handle()
                    .publish(WORKER_CAPABILITIES_CHANNEL, payload)
                {
                    tracing::warn!(%error, "failed to announce intervention capabilities");
                }
            }
            Err(error) => {
                tracing::warn!(%error, "failed to encode WorkerCapabilitiesAnnouncement");
            }
        }
    }

    /// Serves pushed dispatches until `stop` returns `true`.
    ///
    /// Re-checks `stop` every [`RECV_POLL`], so a caller can stop the worker
    /// promptly even on a quiet connection.
    ///
    /// # Errors
    ///
    /// Returns the first [`WorkerError`] a served dispatch surfaces (decode,
    /// encode, or transport).
    pub async fn serve_until<Stop>(&self, mut stop: Stop) -> Result<(), WorkerError>
    where
        Stop: FnMut() -> bool + Send,
    {
        self.announce_capabilities();
        while !stop() {
            self.serve_one().await?;
            // A single-connection serve has no survivor to migrate to, so a
            // declared link death surfaces as the transport error it is rather
            // than blocking forever on a socket nobody is on the other end of.
            if let Some(silent_for) = self.declared_link_death() {
                return Err(WorkerError::Transport {
                    source: tonic::Status::unavailable(format!(
                        "liminal worker connection to {} went silent for {}ms past the \
                         server-declared liveness window",
                        self.address,
                        u64::try_from(silent_for.as_millis()).unwrap_or(u64::MAX)
                    )),
                });
            }
        }
        Ok(())
    }

    /// Serves pushed dispatches until `stop` fires (a clean stop) or the
    /// connection drops with a transport error (the owner died), reporting which
    /// occurred and whether any dispatch was served on this connection.
    ///
    /// This is the per-connection step the candidate-cycling redial driver
    /// (`serve_with_redial`) runs: a clean stop ends the worker, a drop tells the
    /// driver to redial the next candidate, and `served_work` lets the driver
    /// reset its backoff after a connection that did useful work.
    pub(crate) async fn serve_until_drop<Stop>(&self, mut stop: Stop) -> ServeResult
    where
        Stop: FnMut() -> bool + Send,
    {
        // Once per connection: a redialed worker re-announces on the new
        // connection because the survivor's registry entry starts at the
        // registration default (observability-only).
        self.announce_capabilities();
        let mut served_work = false;
        while !stop() {
            match self.serve_one().await {
                Ok(true) => served_work = true,
                Ok(false) => {}
                // A transport drop (the connected server died) is the redial
                // trigger, not a fatal worker error: surface it so the driver
                // migrates to the next candidate and re-registers there.
                Err(_) => return ServeResult::Dropped { served_work },
            }
            // A CLOSED socket surfaces above as a receive error. A merely DEAD
            // one (peer gone without a FIN, a wedged path) never does — every
            // read times out benignly — so the dead-man switch is what turns it
            // into a drop, and hence into a redial + re-registration.
            if self.declared_link_death().is_some() {
                return ServeResult::Dropped { served_work };
            }
        }
        ServeResult::Stopped
    }

    /// Decodes one pushed frame and dispatches it by kind: an
    /// [`InterventionRequest`] (NOI-6) is routed to the live session owning its
    /// target attempt and answered with a correlated [`InterventionReply`]; anything
    /// else is a [`DispatchRequest`] executed as an activity and answered with a
    /// [`DispatchResponse`].
    ///
    /// The two share the push channel and are demuxed by which one deserializes: an
    /// [`InterventionRequest`] has a unique required `intervention` field a
    /// [`DispatchRequest`] lacks, so a dispatch frame never decodes as an
    /// intervention and vice-versa. Intervention is tried first; on a miss the frame
    /// is decoded as a dispatch (preserving the existing dispatch path exactly).
    async fn handle_pushed_frame(&self, frame: PushedFrame) -> Result<(), WorkerError> {
        let correlation_id = frame.correlation_id();
        // A liveness ping is tried FIRST and is the cheapest frame on the wire:
        // it carries the server's silence window (arming this connection's
        // dead-man switch) and is answered immediately, so the server's own
        // connection lease is refreshed by an IDLE-but-alive worker. Its unique
        // required `liveness_ping` field appears on no other pushed frame, and it
        // carries none of the required fields the other two need, so the three
        // decode disjointly.
        if let Ok(ping) = serde_json::from_slice::<LivenessPing>(frame.payload()) {
            return self.answer_liveness_ping(correlation_id, &ping);
        }
        if let Ok(request) = serde_json::from_slice::<InterventionRequest>(frame.payload()) {
            let outcome = self.control.deliver(request.intervention).await;
            let reply = InterventionReply { outcome };
            let payload = serde_json::to_vec(&reply).map_err(WorkerError::encode)?;
            return self
                .client
                .reply(correlation_id, payload)
                .map_err(|error| transport_error(&error));
        }
        let request: DispatchRequest =
            serde_json::from_slice(frame.payload()).map_err(WorkerError::decode)?;
        let run_id = validate_dispatch_envelope(&request)?;
        // Receipt is logged BEFORE execution so a dispatch that reaches the wrong
        // connection (or wedges mid-handler) is visible in the worker log — the
        // server side only sees silence either way (a lost-worker expiry), so this
        // line is the ground truth for "did the push arrive, and where".
        tracing::info!(
            activity_type = %request.activity_type,
            workflow_id = %request.workflow_id,
            ordinal = request.ordinal,
            attempt = request.attempt,
            serves = ?self.registry.activity_types(),
            agent_types = ?self.agent_activity_types,
            "received dispatch push"
        );
        // NO dispatch may block the serve loop. Anything that runs on this task
        // stops the worker receiving the frames that share this channel — a
        // mid-run intervention push, and the server's liveness ping.
        //
        // This was once true of agent dispatches only, on the reasoning that "a
        // plain activity runs inline (short, no live session)". That is false:
        // `remote_gates.awl` declares plain activities at `timeout 45m`, and on
        // run `dfd2117c` a plain `run_check` leg held this loop for 169 seconds
        // during which not one ping could be dequeued, let alone answered. The
        // server read that silence as a dead connection while the worker was
        // doing exactly the work it was sent.
        //
        // So BOTH kinds are spawned and the serve loop only ever routes.
        if self.is_agent_activity(&request.activity_type) {
            self.spawn_agent_dispatch(correlation_id, request, run_id);
            return Ok(());
        }
        self.queue_plain_dispatch(correlation_id, request, run_id);
        Ok(())
    }

    /// Hands a plain typed-registry dispatch to the dedicated execution thread so
    /// the serve loop stays free to answer liveness pings and receive
    /// interventions. The dispatch replies with its own correlated
    /// [`DispatchResponse`] when the handler returns.
    ///
    /// EXECUTION CONCURRENCY IS UNCHANGED BY THIS. One thread drains the queue
    /// serially, so plain activities still execute strictly one at a time,
    /// precisely as they did when they ran inline. That is deliberate and it is
    /// not a limit invented here — it is the limit that already existed, made
    /// explicit and moved off the serve loop. Raising it is a real decision with
    /// real consequences (a gate worker running N cargo builds at once is how a
    /// workspace reached 37 GiB in 17 minutes on 2026-07-31), so it belongs in
    /// worker configuration and to whoever owns that call, not in a bug fix.
    ///
    /// Queueing never blocks the serve loop, so a busy worker still receives and
    /// answers pings while its queued dispatch waits its turn. That is the whole
    /// point: the server learns "busy", not "dead".
    fn queue_plain_dispatch(&self, correlation_id: u64, request: DispatchRequest, run_id: RunId) {
        // The reply rides this connection's `PushWriter`, exactly as the agent
        // path's does — `PushClient` itself is not clonable and does not need
        // to be — so the context is captured per dispatch rather than owned by
        // the executor, which outlives any one connection.
        let queued = QueuedPlainDispatch {
            correlation_id,
            request,
            run_id,
            context: self.plain_dispatch_context(),
        };
        if let Err(error) = self.plain_dispatch_tx.send(queued) {
            // The executor thread is gone (its runtime failed to build, or the
            // worker is being torn down). There is no reply to send and saying
            // so is more honest than a silent return.
            let dispatch = &error.0.request;
            tracing::warn!(
                correlation_id = error.0.correlation_id,
                activity_type = %dispatch.activity_type,
                workflow_id = %dispatch.workflow_id,
                ordinal = dispatch.ordinal,
                "plain dispatch: the execution thread is gone; the dispatch was not executed and \
                 the server's held wait will be resolved by the outbox re-drive"
            );
        }
    }

    /// Arms this connection's dead-man switch with the window the server just    /// Arms this connection's dead-man switch with the window the server just
    /// declared and answers the ping on the same connection.
    ///
    /// The answer is what proves the worker→server leg to the SERVER: its
    /// arrival refreshes the connection lease, so an idle-but-alive worker is
    /// never expired as silent. A reply that cannot be written is a transport
    /// error like any other — it tears the connection down into the redial loop
    /// rather than leaving a worker that believes it answered.
    ///
    /// # Errors
    ///
    /// Returns [`WorkerError::Encode`] when the answer cannot be encoded and
    /// [`WorkerError::Transport`] when it cannot be written to the socket.
    fn answer_liveness_ping(
        &self,
        correlation_id: u64,
        ping: &LivenessPing,
    ) -> Result<(), WorkerError> {
        self.silence
            .arm(Duration::from_millis(ping.silence_window_ms));
        let answer = LivenessPong {
            liveness_pong: ping.liveness_ping,
        };
        let payload = serde_json::to_vec(&answer).map_err(WorkerError::encode)?;
        self.client
            .reply(correlation_id, payload)
            .map_err(|error| transport_error(&error))
    }

    /// The declared link death, when this connection has been silent past the
    /// server's window — logged LOUDLY exactly once, at the moment of the
    /// verdict, naming the endpoint, the silence, and the window it broke.
    ///
    /// `None` means the link is alive (or the switch is not yet armed), which is
    /// the overwhelmingly common case and emits nothing.
    fn declared_link_death(&self) -> Option<Duration> {
        let silent_for = self.silence.silent_for()?;
        tracing::warn!(
            address = %self.address,
            silent_ms = u64::try_from(silent_for.as_millis()).unwrap_or(u64::MAX),
            window_ms = self
                .silence
                .window()
                .map_or(0, |window| u64::try_from(window.as_millis()).unwrap_or(u64::MAX)),
            "liminal worker connection is DEAD: no server frame arrived within the \
             server-declared liveness window; tearing the connection down and redialing"
        );
        Some(silent_for)
    }

    /// Spawns an agent dispatch as a background task so the serve loop stays free to
    /// receive mid-run interventions, replying its own correlated [`DispatchResponse`]
    /// when the run completes.
    ///
    /// The task holds only cheap clones (the harness `Arc`, the shared
    /// [`ControlRegistry`], and a [`PushWriter`] reply/drain leg of the connection), so
    /// it outlives the borrow of `&self`. The session self-registers + streams its
    /// transcript inside [`run_agent_dispatch`].
    fn spawn_agent_dispatch(&self, correlation_id: u64, request: DispatchRequest, run_id: RunId) {
        let Some(harness) = self.agent_harness.clone() else {
            return;
        };
        let control = self.control.clone();
        let capabilities = self.agent_capabilities.clone();
        let writer = self.client.writer_handle();
        // The transcript drain publishes through the SHARED live-connection slot so
        // it survives a redial, whereas the terminal reply + liveness pump stay on
        // this connection's writer (a lost reply is re-driven by the outbox).
        let drain = DrainBinding::new(self.live_writer.clone(), self.drain_backoff);
        // Run on a DEDICATED thread with its own current-thread runtime, not
        // `tokio::spawn`: the erased agent session drives a `?Send` future (the neutral
        // `AgentSession` is `Send` but not `Sync`), which `tokio::spawn` cannot accept.
        // `block_on` has no `Send` bound, and every captured handle (the harness `Arc`,
        // the shared `ControlRegistry`, the `PushWriter`) IS `Send`, so the future is
        // built and driven entirely on the new thread and never crosses one. The
        // session self-registers so the serve loop's intervention pushes still reach it.
        std::thread::spawn(move || {
            let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                tracing::warn!("agent dispatch: failed to build runtime for the agent run");
                return;
            };
            runtime.block_on(run_agent_dispatch(
                harness,
                control,
                capabilities,
                writer,
                drain,
                AgentDispatch {
                    correlation_id,
                    request,
                    run_id,
                },
            ));
        });
    }

    /// The cheap-clone slice of this worker a spawned plain dispatch needs, so
    /// the execution can outlive the borrow of `&self` and run off the serve
    /// loop.
    ///
    /// `PushClient` and `SilenceMonitor` are deliberately NOT carried: neither
    /// is `Clone`, and neither is needed. A dispatch replies through the
    /// connection's [`PushWriter`] (exactly as the agent path does) and needs
    /// only the silence WINDOW value, not the monitor that owns it.
    fn plain_dispatch_context(&self) -> PlainDispatchContext {
        PlainDispatchContext {
            writer: self.client.writer_handle(),
            registry: self.registry.clone(),
            live_writer: self.live_writer.clone(),
            drain_backoff: self.drain_backoff,
            silence_window: self.silence.window(),
        }
    }
}

/// One plain typed-registry dispatch handed from a serve loop to the dedicated
/// execution thread, carrying the connection context it must reply on.
struct QueuedPlainDispatch {
    correlation_id: u64,
    request: DispatchRequest,
    /// The run the dispatch envelope was validated to carry. Held as a plain
    /// `RunId` (not the wire `Option`) because the executed activity's context
    /// hands it to any transcript event the handler emits, and there is no
    /// honest substitute if it were absent.
    run_id: RunId,
    context: PlainDispatchContext,
}

/// Starts the ONE thread that executes this worker's plain typed-registry
/// dispatches, and returns the queue the serve loop hands them to.
///
/// The thread owns its own current-thread runtime. That is what makes the
/// execution independent of whichever runtime the caller drives the serve loop
/// on: `serve_with_redial` — the entry point every real worker uses — drives it
/// with `block_on` on a current-thread runtime whose only task blocks inside
/// `PushClient::recv_timeout` and therefore never yields, so a `tokio::spawn`ed
/// dispatch there would be received, logged, and then never polled. Owning a
/// runtime is also exactly what the agent path already does, and for the same
/// class of reason.
///
/// The loop is SERIAL by construction: one dispatch is driven to completion
/// before the next is taken off the queue. That preserves the concurrency plain
/// dispatches had when they executed inline on the serve loop — this is not a
/// new cap, it is the old one, kept.
///
/// The thread ends when every sender is dropped, i.e. when the worker is
/// dropped, after the dispatch in flight finishes.
fn spawn_plain_dispatch_executor() -> tokio::sync::mpsc::UnboundedSender<QueuedPlainDispatch> {
    let (sender, mut queue) = tokio::sync::mpsc::unbounded_channel::<QueuedPlainDispatch>();
    std::thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => {
                // Every dispatch queued to this worker will now be refused at
                // the send site with a named warning rather than vanishing.
                tracing::error!(
                    %error,
                    "plain dispatch: failed to build the execution runtime; this worker cannot \
                     execute plain activities and every dispatch it is sent will be re-driven by \
                     the server's outbox"
                );
                return;
            }
        };
        runtime.block_on(async move {
            while let Some(queued) = queue.recv().await {
                execute_queued_plain_dispatch(queued).await;
            }
        });
    });
    sender
}

/// Executes one queued plain dispatch and writes its correlated reply.
///
/// Every failure below is terminal for THIS attempt only: no reply is written,
/// so the server's held wait is resolved by the outbox re-drive rather than by a
/// malformed answer. Each is logged, because the serve loop no longer carries
/// these errors and an unlogged one would be swallowed entirely.
async fn execute_queued_plain_dispatch(queued: QueuedPlainDispatch) {
    let QueuedPlainDispatch {
        correlation_id,
        request,
        run_id,
        context,
    } = queued;
    let response = match context.execute(&request, &run_id).await {
        Ok(response) => response,
        Err(error) => {
            tracing::warn!(
                %error,
                activity_type = %request.activity_type,
                workflow_id = %request.workflow_id,
                ordinal = request.ordinal,
                "plain dispatch: execution failed before a response could be formed; no reply is \
                 sent and the server's held wait is resolved by the outbox"
            );
            return;
        }
    };
    match serde_json::to_vec(&response) {
        Ok(payload) => {
            if let Err(error) = context.writer.reply(correlation_id, payload) {
                tracing::warn!(
                    %error,
                    workflow_id = %request.workflow_id,
                    ordinal = request.ordinal,
                    "plain dispatch: completed but its reply could not be written; the outbox \
                     re-drives the completion"
                );
            }
        }
        Err(error) => tracing::warn!(
            %error,
            workflow_id = %request.workflow_id,
            ordinal = request.ordinal,
            "plain dispatch: completed but its response could not be encoded"
        ),
    }
}

/// The cheap-clone slice of a [`LiminalActivityWorker`] needed to execute one
/// plain typed-registry dispatch away from the serve loop.
#[derive(Clone)]
struct PlainDispatchContext {
    writer: PushWriter,
    registry: Arc<ActivityRegistry>,
    live_writer: LiveWriter,
    drain_backoff: RedialBackoff,
    /// The server-declared silence window this connection learned from its
    /// liveness pings, used to arm the dispatch's liveness pump.
    silence_window: Option<Duration>,
}

impl PlainDispatchContext {
    /// Executes one plain dispatch and maps its outcome onto a
    /// [`DispatchResponse`].
    ///
    /// Agent activities never reach here — they are spawned separately in
    /// `handle_pushed_frame` and driven through the harness. This is the plain
    /// typed-registry path, carrying the LIVE transcript event drain so a
    /// handler that emits events streams them mid-run. A missing handler or a
    /// failure becomes a failure OUTCOME (a reason string), never a dropped
    /// reply, so the server always sees a correlated answer it can re-enter.
    async fn execute(
        &self,
        request: &DispatchRequest,
        run_id: &RunId,
    ) -> Result<DispatchResponse, WorkerError> {
        // Agent activities are spawned in `handle_pushed_frame` and never reach here;
        // this path is the plain typed-registry execution, now carrying the LIVE
        // transcript event drain so a handler that emits events streams them mid-run.
        // The wire's one-based delivery attempt and labels are threaded through
        // verbatim (the gRPC parity contract): a retry executes with the real
        // attempt, never a re-stamped first delivery.
        let attempt = request.attempt;
        let activity_id = ActivityId::from_sequence_position(request.ordinal);
        let task = ActivityTask {
            workflow_id: request.workflow_id.clone(),
            activity_id: activity_id.clone(),
            run_id: request.run_id.clone(),
            activity_type: request.activity_type.clone(),
            attempt,
            completion_token: request.completion_token.clone(),
            idempotency_key: request.idempotency_key.clone(),
            input: Payload::new(ContentType::Json, request.input.clone()),
            labels: request.labels.clone(),
        };
        // Keep this liveness-TRACKED dispatch beating for as long as the handler
        // genuinely runs (a no-op for the window-less outbox path). The guard
        // aborts the pump on every exit path.
        let _liveness = spawn_liveness_pump(self.writer.clone(), request, self.silence_window);
        let (event_sender, drain) = spawn_event_drain(DrainBinding::new(
            self.live_writer.clone(),
            self.drain_backoff,
        ));
        let (context, cancellation) = ActivityContext::for_workflow_with_events(
            Some(request.workflow_id.clone()),
            Some(run_id.clone()),
            activity_id,
            attempt,
            Some(request.idempotency_key.clone()),
            None,
            Some(event_sender),
        );
        // The push transport has no cooperative-cancellation channel in the spike;
        // drop the handle so the activity simply runs to completion.
        drop(cancellation);
        let outcome = self.registry.dispatch(task, context).await;
        drain.finish().await;
        let outcome = match outcome {
            Ok(outcome) => wire_outcome(outcome),
            // A worker-level dispatch fault (e.g. a missing handler) is a
            // retryable fault: another worker in the pool may serve the type,
            // matching the gRPC contract where such a session error is an
            // ACTION failure the retry policy governs, never a terminal. This is
            // deliberately NOT the transport-loss class: the harness was
            // reached and it errored, so the attempt genuinely happened.
            Err(error) => Err(format!("retryable:{error}")),
        };
        Ok(DispatchResponse {
            workflow_id: request.workflow_id.clone(),
            ordinal: request.ordinal,
            run_id: request.run_id.clone(),
            completion_token: request.completion_token.clone(),
            outcome,
        })
    }
}

impl LiminalActivityWorker {
    /// Whether `activity_type` is driven through the installed agent harness.
    fn is_agent_activity(&self, activity_type: &str) -> bool {
        self.agent_harness.is_some() && self.agent_activity_types.contains(activity_type)
    }
}

/// Drives one agent activity attempt to completion and replies its correlated
/// [`DispatchResponse`] (NOI-5b/NOI-6). Runs as a SPAWNED task so the serve loop stays
/// free to receive mid-run interventions.
///
/// It self-registers the live session in the [`ControlRegistry`] under its
/// `(workflow, activity, attempt)` key BEFORE [`spawn_dyn_agent`] starts it — so a
/// pushed intervention resolves the instant the run begins — streams the session's
/// transcript LIVE over `writer`, routes pushed commands to the session, and
/// deregisters on completion (the [`SessionGuard`](crate::runtime::intervention::SessionGuard)
/// drops on every exit path). The terminal result is replied to `correlation_id`.
async fn run_agent_dispatch(
    harness: Arc<dyn DynAgentHarness>,
    control: ControlRegistry,
    capabilities: InterventionCapabilities,
    writer: PushWriter,
    drain: DrainBinding,
    dispatch: AgentDispatch,
) {
    let AgentDispatch {
        correlation_id,
        request,
        run_id,
    } = dispatch;
    // The wire's one-based delivery attempt keys the session exactly as the
    // server binds the attempt's owner (`request_for_row` / the bridge stamp),
    // and the run keys it exactly as the server's `AttemptKey` does — the two
    // sides must agree on all four axes, or a routed command resolves the wrong
    // generation's live session.
    let attempt = request.attempt;
    let activity_id = ActivityId::from_sequence_position(request.ordinal);
    let session_key = SessionKey::new(
        request.workflow_id.clone(),
        run_id.clone(),
        activity_id.clone(),
        attempt,
    );
    // Register the live session's control leg BEFORE the run starts; the guard
    // deregisters on drop (any exit path), so a finished attempt is never routed to.
    let (control_tx, control_rx) = mpsc::unbounded_channel();
    let _session_guard = control.register(session_key, control_tx, capabilities);
    // Keep a liveness-TRACKED agent dispatch beating for the whole run (a no-op
    // for the window-less outbox path); the guard aborts the pump on every exit.
    // An agent dispatch is SPAWNED off the serve loop, so the loop stays free to
    // dequeue and answer liveness pings for the whole run — the connection
    // window adds nothing here, and the dispatch's own window arms the pump
    // exactly as before.
    let _liveness = spawn_liveness_pump(writer.clone(), &request, None);
    let spec = AgentRunSpec::new(
        request.workflow_id.clone(),
        run_id,
        activity_id,
        attempt,
        // The dispatched activity-type name is neutral run identity the harness may
        // use (e.g. to label the run); it is threaded verbatim from the request.
        request.activity_type.clone(),
        Payload::new(ContentType::Json, request.input.clone()),
    );
    // Stream the transcript LIVE while the agent runs, through the shared slot so
    // it re-resolves the survivor after a redial rather than a dead socket (#254).
    let (event_sender, drain) = spawn_event_drain(drain);
    let outcome = spawn_dyn_agent(harness.as_ref(), spec, event_sender, Some(control_rx)).await;
    drain.finish().await;
    // A harness fault is a retryable activity failure, mapped as the gRPC driver does.
    let outcome =
        outcome.unwrap_or_else(|error| crate::runtime::agent::harness_error_to_outcome(&error));
    let response = DispatchResponse {
        workflow_id: request.workflow_id.clone(),
        ordinal: request.ordinal,
        run_id: request.run_id.clone(),
        completion_token: request.completion_token.clone(),
        outcome: wire_outcome(outcome),
    };
    // Reply the terminal result to the server on the shared connection. A failed
    // reply (connection gone) is logged; the outbox re-drives the row on timeout.
    match serde_json::to_vec(&response) {
        Ok(payload) => {
            if let Err(error) = writer.reply(correlation_id, payload) {
                tracing::warn!(%error, "agent dispatch: failed to reply DispatchResponse");
            }
        }
        Err(error) => tracing::warn!(%error, "agent dispatch: failed to encode DispatchResponse"),
    }
}

/// One agent dispatch as the serve loop hands it off: the correlated frame id,
/// the wire request, and the run its envelope was validated to carry.
///
/// The three travel together on every path, exactly as they do for the plain
/// arm's [`QueuedPlainDispatch`], so they are carried as one value rather than
/// as three parallel parameters.
struct AgentDispatch {
    /// The pushed frame's correlation id, which the terminal reply answers.
    correlation_id: u64,
    /// The wire dispatch request.
    request: DispatchRequest,
    /// The run the envelope was validated to carry. Held as a plain `RunId`
    /// (not the wire `Option`) because the session key and every transcript
    /// event this attempt emits are keyed on it.
    run_id: RunId,
}

/// Starts the automatic liveness pump for one liveness-TRACKED dispatch, or
/// `None` when the request carries no heartbeat window (the outbox path, or a
/// pre-window server) — a no-op, byte-identical to the pre-pump behaviour.
///
/// The RUNTIME owns liveness on this transport exactly as it does on gRPC
/// (#176): the pump publishes a [`WorkerLivenessBeat`] for the dispatch on
/// [`WORKER_LIVENESS_CHANNEL`] every quarter of the server-assigned window
/// ([`liveness_pump_interval`], the same cadence the gRPC loop pumps), so a
/// healthy worker running a legitimately long activity is never expired by the
/// server's heartbeat sweeper, while a wedged process stops pumping and
/// correctly is. Dropping the returned guard aborts the pump, so it lives
/// exactly as long as the dispatch on every exit path.
fn spawn_liveness_pump(
    writer: PushWriter,
    request: &DispatchRequest,
    connection_window: Option<Duration>,
) -> Option<LivenessPump> {
    // Two windows can arm this pump, and either is enough. The dispatch's own
    // window arms it for a liveness-TRACKED (bridge) dispatch, as it always
    // has. The CONNECTION's window — learned from the server's liveness ping —
    // arms it for every other dispatch too, including an outbox one that
    // carries no window of its own.
    //
    // That second source is load-bearing, not a nicety: a plain activity
    // executes INLINE on the serve loop, so while it runs the worker cannot
    // dequeue (or answer) the server's pings, and nothing else would advance
    // the connection lease. A leg longer than the window would be deregistered
    // mid-work for the crime of working — observed live on legs over ~40s. The
    // pump publishes from a background task on a shared writer, so it keeps the
    // lease alive whatever the serve loop is doing.
    let window = match request.heartbeat_window_ms {
        0 => connection_window?,
        millis => Duration::from_millis(millis),
    };
    if window.is_zero() {
        return None;
    }
    let period = crate::runtime::loop_::liveness_pump_interval(window);
    let beat = WorkerLivenessBeat {
        workflow_id: request.workflow_id.clone(),
        ordinal: request.ordinal,
    };
    let payload = match serde_json::to_vec(&beat) {
        Ok(payload) => payload,
        Err(error) => {
            tracing::warn!(%error, "liveness pump: failed to encode WorkerLivenessBeat");
            return None;
        }
    };
    let handle = tokio::spawn(async move {
        let mut ticks = tokio::time::interval(period);
        ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticks.tick().await;
            // A publish fault means the connection is gone; the server's
            // disconnect path owns the dispatch from here, so stop pumping.
            if let Err(error) = writer.publish(WORKER_LIVENESS_CHANNEL, payload.clone()) {
                tracing::warn!(%error, "liveness pump: failed to publish beat; stopping");
                return;
            }
        }
    });
    Some(LivenessPump { handle })
}

/// A running liveness pump tied to one dispatch; dropping it aborts the pump.
struct LivenessPump {
    handle: tokio::task::JoinHandle<()>,
}

impl Drop for LivenessPump {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

/// Renders an activity output payload as the result string the server expects.
///
/// The server's `DispatchResponse.outcome` carries the success result as a
/// `String`; activity output is JSON-tagged bytes, so the UTF-8 view is the
/// result string. A non-UTF-8 payload (never produced by the JSON codec) is
/// rendered lossily rather than dropping the completion.
fn result_string(output: &Payload) -> String {
    String::from_utf8_lossy(output.bytes()).into_owned()
}

/// Maps one executed dispatch outcome onto the wire `outcome`, encoding a
/// failure with the SAME kind-prefixed reason vocabulary the engine seam parses
/// (`retryable:` / `terminal:`) — the liminal mirror of the classification the
/// gRPC transport carries as the typed `ActivityError.kind`, and byte-identical
/// to what the server's gRPC completion path hands the shared delivery callback.
/// Without the prefix, retryability is silently dropped at this wire boundary
/// and every remote failure re-enters aion unclassified.
fn wire_outcome(outcome: DispatchOutcome) -> Result<String, String> {
    match outcome {
        DispatchOutcome::Completed { output } => Ok(result_string(&output)),
        DispatchOutcome::Failed { failure } => {
            let prefix = if failure.is_retryable() {
                "retryable"
            } else {
                "terminal"
            };
            Err(format!("{prefix}:{}", failure.message))
        }
    }
}

/// Whether an SDK receive error is a benign poll timeout (no push arrived) rather
/// than a fatal transport fault. [`PushClient::recv_timeout`] maps both a timeout
/// and a stopped reader to [`liminal_sdk::SdkError::Connection`]; only the timeout
/// message is non-fatal, so it is distinguished by its text.
fn is_recv_timeout(error: &liminal_sdk::SdkError) -> bool {
    error
        .to_string()
        .contains("no server push arrived within the timeout")
}

/// Builds the in-band [`WorkerRegistration`] this worker announces over the
/// socket, from its [`WorkerConfig`] routing dimensions and the activity-type
/// names bound in its [`ActivityRegistry`].
///
/// `node` follows the SAME none-convention the aion registry applies on the
/// server side: an empty `config.node` carries no locality affinity (`None`), so
/// it is semantically unpinned rather than registering a distinct empty-node
/// affinity no pinned dispatch could match; a non-empty value (the default
/// hostname, or an operator-set node) is the worker's advertised node.
fn registration_from(config: &WorkerConfig, registry: &ActivityRegistry) -> WorkerRegistration {
    let node = if config.node.is_empty() {
        None
    } else {
        Some(config.node.clone())
    };
    WorkerRegistration {
        namespaces: config.namespaces.clone(),
        task_queue: config.task_queue.clone(),
        node,
        activity_types: registry.activity_types().into_iter().collect(),
        identity: config.identity.clone(),
        activities: registry
            .activity_descriptors()
            .into_iter()
            .map(|activity| liminal::protocol::WorkerActivityDescriptor {
                name: activity.name,
                input_schema_json: activity.input_schema.to_string(),
                output_schema_json: activity.output_schema.to_string(),
            })
            .collect(),
    }
}

/// Wraps a liminal SDK error as a retryable worker transport error.
fn transport_error(error: &liminal_sdk::SdkError) -> WorkerError {
    WorkerError::Transport {
        source: tonic::Status::unavailable(format!("liminal worker transport error: {error}")),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{DispatchRequest, DispatchResponse, registration_from};
    use crate::activity::ActivityRegistry;
    use crate::config::WorkerConfig;
    use aion_core::{RunId, WorkflowId};
    use uuid::Uuid;

    fn worker_config(node: &str) -> Result<WorkerConfig, Box<dyn std::error::Error>> {
        Ok(WorkerConfig::builder()
            .endpoint("127.0.0.1:0")
            .task_queue("gpu")
            .identity("worker-a")
            .max_concurrency(1)
            .reconnect_initial_backoff(Duration::from_millis(5))
            .reconnect_max_backoff(Duration::from_millis(20))
            .reconnect_max_attempts(3)
            .namespaces([String::from("remote"), String::from("payments")])
            .node(node)
            .build()?)
    }

    fn two_activity_registry() -> Result<ActivityRegistry, Box<dyn std::error::Error>> {
        let registry = ActivityRegistry::new()
            .register_activity("charge-card", |_input: serde_json::Value, _ctx| {
                Box::pin(async move { Ok(serde_json::json!({})) })
            })?
            .register_activity("refund", |_input: serde_json::Value, _ctx| {
                Box::pin(async move { Ok(serde_json::json!({})) })
            })?;
        Ok(registry)
    }

    /// The in-band registration is built from the worker config's routing
    /// dimensions and the activity-type names the registry binds, so the worker
    /// announces exactly what it serves. The activity types come from the same
    /// registry the worker executes through (deterministic, sorted).
    #[test]
    fn registration_carries_config_and_registry_activity_types()
    -> Result<(), Box<dyn std::error::Error>> {
        let config = worker_config("box-7")?;
        let registry = two_activity_registry()?;

        let registration = registration_from(&config, &registry);

        assert_eq!(
            registration.namespaces,
            vec![String::from("remote"), String::from("payments")]
        );
        assert_eq!(registration.task_queue, "gpu");
        assert_eq!(registration.node, Some(String::from("box-7")));
        assert_eq!(registration.identity, "worker-a");
        assert_eq!(
            registration.activity_types,
            vec![String::from("charge-card"), String::from("refund")],
            "activity types come from the bound registry, sorted"
        );
        Ok(())
    }

    /// An empty config node carries NO locality affinity (`None`), the same
    /// none-convention the server-side registry applies — a worker with no node is
    /// unpinned, not pinned to an empty node.
    #[test]
    fn registration_empty_node_is_unpinned() -> Result<(), Box<dyn std::error::Error>> {
        let config = worker_config("")?;
        let registry = two_activity_registry()?;

        let registration = registration_from(&config, &registry);

        assert_eq!(registration.node, None);
        Ok(())
    }

    /// The wire request round-trips through serde JSON with stable field names —
    /// the contract that keeps it byte-compatible with the server's struct —
    /// INCLUDING the engine-parity `attempt`/`labels` and the liveness
    /// `heartbeat_window_ms` the bridge stamps.
    #[test]
    fn dispatch_request_round_trips_through_json() -> Result<(), Box<dyn std::error::Error>> {
        let request = DispatchRequest {
            activity_type: "charge-card".to_owned(),
            workflow_id: WorkflowId::new(Uuid::new_v4()),
            ordinal: 7,
            run_id: Some(RunId::new(Uuid::new_v4())),
            completion_token: "generation-3".to_owned(),
            idempotency_key: "effect-key".to_owned(),
            input: br#"{"amount":42}"#.to_vec(),
            attempt: 3,
            labels: std::collections::BTreeMap::from([("region".to_owned(), "apac".to_owned())]),
            heartbeat_window_ms: 30_000,
        };
        let bytes = serde_json::to_vec(&request)?;
        let decoded: DispatchRequest = serde_json::from_slice(&bytes)?;
        assert_eq!(decoded, request);
        // The field names the server depends on are present in the JSON.
        let json = String::from_utf8(bytes)?;
        for field in [
            "activity_type",
            "workflow_id",
            "ordinal",
            "run_id",
            "completion_token",
            "idempotency_key",
            "input",
            "attempt",
            "labels",
            "heartbeat_window_ms",
        ] {
            assert!(json.contains(field), "wire JSON must carry `{field}`");
        }
        Ok(())
    }

    /// A frame from a pre-fencing server is refused rather than guessed current.
    #[test]
    fn dispatch_request_without_fencing_fields_is_rejected() {
        let old_frame = serde_json::json!({
            "activity_type": "charge-card",
            "workflow_id": WorkflowId::new(Uuid::new_v4()),
            "ordinal": 7,
            "run_id": null,
            "input": [123, 125],
        });
        assert!(
            serde_json::from_value::<DispatchRequest>(old_frame).is_err(),
            "a pre-fencing task must be rejected as a registration-era mismatch"
        );
    }

    /// The liveness beat round-trips with stable field names — the cross-crate
    /// contract with the server's mirrored `WorkerLivenessBeat`.
    #[test]
    fn worker_liveness_beat_round_trips_through_json() -> Result<(), Box<dyn std::error::Error>> {
        let beat = super::WorkerLivenessBeat {
            workflow_id: WorkflowId::new(Uuid::new_v4()),
            ordinal: 9,
        };
        let bytes = serde_json::to_vec(&beat)?;
        let decoded: super::WorkerLivenessBeat = serde_json::from_slice(&bytes)?;
        assert_eq!(decoded, beat);
        let json = String::from_utf8(bytes)?;
        for field in ["workflow_id", "ordinal"] {
            assert!(json.contains(field), "beat JSON must carry `{field}`");
        }
        // The channel name is the pinned cross-crate contract.
        assert_eq!(super::WORKER_LIVENESS_CHANNEL, "aion.worker.liveness");
        Ok(())
    }

    /// The capabilities announcement round-trips with stable field names — the
    /// cross-crate contract with the server's mirrored
    /// `WorkerCapabilitiesAnnouncement` — and the channel name is pinned.
    #[test]
    fn worker_capabilities_announcement_round_trips_through_json()
    -> Result<(), Box<dyn std::error::Error>> {
        let announcement = super::WorkerCapabilitiesAnnouncement {
            capabilities: aion_core::InterventionCapabilities {
                supported: vec![
                    aion_core::InterventionPrimitive::InjectMessage,
                    aion_core::InterventionPrimitive::Cancel,
                ],
            },
        };
        let bytes = serde_json::to_vec(&announcement)?;
        let decoded: super::WorkerCapabilitiesAnnouncement = serde_json::from_slice(&bytes)?;
        assert_eq!(decoded, announcement);
        let json = String::from_utf8(bytes)?;
        for field in ["capabilities", "supported"] {
            assert!(json.contains(field), "wire JSON must carry `{field}`");
        }
        assert_eq!(
            super::WORKER_CAPABILITIES_CHANNEL,
            "aion.worker.capabilities"
        );
        Ok(())
    }

    /// An intervention request round-trips and is demuxed from a dispatch request:
    /// a dispatch JSON must NOT decode as an intervention (no `intervention` field),
    /// which is exactly what lets the serve loop tell the two pushes apart.
    #[test]
    fn intervention_request_demuxes_from_a_dispatch_request()
    -> Result<(), Box<dyn std::error::Error>> {
        use super::{InterventionReply, InterventionRequest};
        use aion_core::{
            ActivityId, InjectPriority, InterventionCommand, InterventionKind, InterventionOutcome,
        };

        let command = InterventionCommand {
            workflow_id: WorkflowId::new(Uuid::nil()),
            run_id: RunId::new_v4(),
            activity_id: ActivityId::from_sequence_position(3),
            attempt: 1,
            issued_by: Some("operator".to_owned()),
            issued_at: chrono::Utc::now(),
            kind: InterventionKind::InjectMessage {
                text: "steer".to_owned(),
                priority: InjectPriority::Interrupt,
            },
        };
        let request = InterventionRequest {
            intervention: command,
        };
        let bytes = serde_json::to_vec(&request)?;
        assert_eq!(
            serde_json::from_slice::<InterventionRequest>(&bytes)?,
            request
        );

        // A dispatch request must NOT decode as an intervention (missing field).
        let dispatch = DispatchRequest {
            activity_type: "charge-card".to_owned(),
            workflow_id: WorkflowId::new(Uuid::new_v4()),
            ordinal: 7,
            run_id: None,
            completion_token: "generation-1".to_owned(),
            idempotency_key: "effect-key".to_owned(),
            input: b"{}".to_vec(),
            attempt: 1,
            labels: std::collections::BTreeMap::new(),
            heartbeat_window_ms: 0,
        };
        let dispatch_bytes = serde_json::to_vec(&dispatch)?;
        assert!(
            serde_json::from_slice::<InterventionRequest>(&dispatch_bytes).is_err(),
            "a dispatch frame must never decode as an intervention"
        );

        // The reply round-trips its neutral outcome.
        let reply = InterventionReply {
            outcome: InterventionOutcome::Applied,
        };
        let reply_bytes = serde_json::to_vec(&reply)?;
        assert_eq!(
            serde_json::from_slice::<InterventionReply>(&reply_bytes)?,
            reply
        );
        Ok(())
    }

    /// The wire response round-trips, including the `outcome` Result tagging the
    /// server's completion source matches on (`Ok`/`Err`).
    #[test]
    fn dispatch_response_round_trips_both_outcomes() -> Result<(), Box<dyn std::error::Error>> {
        let workflow_id = WorkflowId::new(Uuid::new_v4());
        let ok = DispatchResponse {
            workflow_id: workflow_id.clone(),
            ordinal: 0,
            run_id: None,
            completion_token: "generation-1".to_owned(),
            outcome: Ok(r#"{"charged":true}"#.to_owned()),
        };
        let err = DispatchResponse {
            workflow_id,
            ordinal: 1,
            run_id: None,
            completion_token: "generation-1".to_owned(),
            outcome: Err("boom".to_owned()),
        };
        for response in [ok, err] {
            let bytes = serde_json::to_vec(&response)?;
            let decoded: DispatchResponse = serde_json::from_slice(&bytes)?;
            assert_eq!(decoded, response);
        }
        Ok(())
    }
}
