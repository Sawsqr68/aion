//! #266 Defect A red-first e2e: an in-flight declared-body activity adopted
//! by startup recovery must EXECUTE, not park.
//!
//! The defect: `ServerState::build_with_connected_store` ran recovery replay
//! inside `EngineBuilder::build()` (the engine builder performed startup
//! recovery itself), and installed the engine-backed seams — including the
//! `DeclaredBodySource` the `DeclaredCommandDispatcher` consults — only after
//! the engine exists. Every activity re-dispatched by recovery therefore
//! consults an EMPTY body source, silently falls through to the worker path,
//! and parks forever on a queue that by design has no pollers (declared
//! bodies ARE its service): `NO_LIVE_POLLERS`, availability deadline `never`.
//! Field case: wid `1f032477-7d7c-4999-b8c8-cf56640b4b8e` ("pulse-6"),
//! stranded identically by three successive server generations.
//!
//! # Shape: corpse-seeded two-generation restart
//!
//! Generation 1 boots the REAL full-boot constructor
//! (`ServerState::build_with_store`), deploys an AWL package whose one action
//! declares a `run` body on queue `local`, and runs a PROBE workflow to
//! completion — the control arm proving the fresh-dispatch path executes
//! declared bodies at the server, and the source of byte-exact history.
//!
//! The corpse is then seeded on a dead store (no engine alive anywhere — this
//! is fixture construction, not a second writer): the probe's own recorded
//! events up to and including `ActivityStarted` attempt 1 are re-keyed to a
//! fresh workflow/run identity, with the probe's marker path byte-swapped for
//! an equal-length corpse marker path in both input payloads. Because the
//! events are the engine's own bytes for this exact document, replay is
//! deterministic by construction — the corpse is a run that died mid-flight.
//!
//! Generation 2 boots `build_with_store` over the same database — the real
//! defective path. The assertion is outcome-level: within the deadline EVERY
//! corpse's declared body EXECUTES (history gains `ActivityCompleted` and the
//! run completes; the corpse marker file on disk is the filesystem belt).
//! Under the current ordering this is RED — adopted dispatches park and never
//! execute. Under the fixed ordering (seams installed before recovery replay
//! runs) it is GREEN.
//!
//! # Why a FLEET of corpses
//!
//! Startup recovery SPAWNS recovered processes; their replay and re-dispatch
//! run concurrently on scheduler threads while `build()` continues. The
//! defect is therefore a race the dispatch must win: a single tiny corpse in
//! an otherwise-empty boot can reach its dispatch AFTER the seam install that
//! follows `build()`, and the defect hides (observed: one corpse, green in
//! 0.85s). Production loses that race every time because its boot does real
//! work inside `build()` — which is exactly what a fleet of corpses recreates:
//! while the recovery loop is still spawning corpse N, the earlier corpses
//! reach their dispatches inside `build()`, before any seam install can
//! happen, and park. Asserting EVERY corpse completes makes the window's
//! existence the thing under test, not one lucky interleaving.

use std::path::PathBuf;
use std::time::Duration;

use aion_awl_package::compile_and_assemble_awl;
use aion_core::{Event, EventEnvelope, Payload, RunId, WorkflowId};
use aion_server::ServerState;
use aion_server::api::http::http_router;
use aion_server::config::{
    AuthConfig, AuthoringConfig, DeployConfig, ListenConfig, MetricsConfig, NamespaceConfig,
    NamespaceMode, OpsConsoleAssetSource, OpsConsoleConfig, RuntimeConfig, WebSocketConfig,
    WorkerConfig,
};
use aion_store::{ReadableEventStore, WritableEventStore, WriteToken};
use aion_store_haematite::HaematiteStore;
use axum::{body, http::Request, http::StatusCode};
use serde_json::json;
use tower::ServiceExt;

type TestError = Box<dyn std::error::Error>;

const NAMESPACE: &str = "default";
const WORKFLOW_TYPE: &str = "recovery_probe";
const MAX_ARCHIVE_BYTES: u64 = 1_048_576;
const MAX_INFLATED_BYTES: u64 = 2_097_152;

/// How long generation 2 gets to execute every adopted declared body before
/// the strand is declared. Each body is one `touch`; the fixed ordering
/// completes the whole fleet in seconds. Generous so a loaded box cannot fake
/// the strand.
const ADOPTION_DEADLINE: Duration = Duration::from_secs(120);

/// How many interrupted runs generation 2 must adopt. Sized so the recovery
/// spawn loop is still running when the first corpses reach their dispatches
/// (see the module comment) — one corpse demonstrably loses the race to the
/// seam install and hides the defect.
const CORPSE_COUNT: usize = 25;

/// How long the generation-1 probe run gets to complete (fresh-path control).
const PROBE_DEADLINE: Duration = Duration::from_secs(60);

const POLL_INTERVAL: Duration = Duration::from_millis(250);

/// The fixture document: one declared-command action on queue `local` — the
/// same shape as the field case (`local` has no pollers; the declared body IS
/// its service). The command materializes its own proof on disk.
const DOCUMENT: &str = r#"//! #266 recovery-ordering fixture.
workflow recovery_probe
  input path: String
  outcome done: type Stdout, route success

type Stdout { stdout: String }

worker local
  action mark(path: String) -> Stdout
    run "touch $path"

step mark
  mark(path: path) -> out
  route done(out)
"#;

fn runtime_config() -> RuntimeConfig {
    RuntimeConfig {
        listen: ListenConfig {
            grpc: std::net::SocketAddr::from(([127, 0, 0, 1], 0)),
            http: std::net::SocketAddr::from(([127, 0, 0, 1], 0)),
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
            heartbeat_window: Duration::from_secs(30),
            ..Default::default()
        },
        websocket: WebSocketConfig {
            outbound_buffer_bound: 32,
            event_broadcast_capacity: Some(64),
            cluster_broadcast_capacity: Some(64),
        },
        workflow_packages: Vec::new(),
        deploy: DeployConfig {
            enabled: true,
            max_archive_bytes: Some(MAX_ARCHIVE_BYTES),
            max_inflated_bytes: Some(MAX_INFLATED_BYTES),
        },
        authoring: AuthoringConfig::default(),
        dev: aion_server::config::DevConfig::default(),
        outbox: aion_server::config::OutboxConfig::default(),
        observability: aion_server::config::ObservabilityConfig::with_flush_policy(64, 0),
        mcp: aion_server::config::ResolvedMcpConfig::default(),
        scheduler_threads: 1,
        jit_threshold: None,
        query_timeout: Some(Duration::from_secs(10)),
        default_namespace: NAMESPACE.to_owned(),
        auto_create: aion_server::config::AutoCreate::Open,
        max_in_flight_activities: aion_server::config::DEFAULT_MAX_IN_FLIGHT_ACTIVITIES,
        drain_timeout: Duration::from_secs(30),
        metrics: MetricsConfig { enabled: true },
        owned_shards: Vec::new(),
        cors_allowed_origins: Vec::new(),
    }
}

/// One in-process server generation over the shared haematite file, built
/// through the REAL full-boot constructor — the path under test.
///
/// Returns the generation's OWN store handle alongside it. haematite holds an
/// exclusive writer lock on the data directory, so a caller that wants to read
/// what this generation writes must share this handle — `HaematiteStore` clones
/// share one `Arc<haematite::EventStore>` — rather than open a second.
async fn server_over(
    db_path: PathBuf,
) -> Result<(ServerState, axum::Router, HaematiteStore), TestError> {
    let store = HaematiteStore::open_or_create(
        db_path,
        haematite::NodeCacheBudget::Unlimited,
        aion_store_haematite::LockAcquisitionRuling::from_millis(250, 5),
    )
    .await?;
    let reader = store.clone();
    let state = ServerState::build_with_store(store, runtime_config()).await?;
    let router = http_router(state.clone())?;
    Ok((state, router, reader))
}

fn granted_headers(builder: axum::http::request::Builder) -> axum::http::request::Builder {
    builder
        .header("x-aion-subject", "ci")
        .header("x-aion-namespaces", NAMESPACE)
        .header("x-aion-deploy", "true")
}

async fn deploy_archive(router: &axum::Router, archive: Vec<u8>) -> Result<(), TestError> {
    let request = granted_headers(
        Request::builder()
            .uri("/deploy/packages")
            .method("POST")
            .header("content-type", "application/octet-stream"),
    )
    .body(body::Body::from(archive))?;
    let response = router.clone().oneshot(request).await?;
    let status = response.status();
    let bytes = body::to_bytes(response.into_body(), usize::MAX).await?;
    if status != StatusCode::OK {
        return Err(format!(
            "deploy must succeed, got {status}: {}",
            String::from_utf8_lossy(&bytes)
        )
        .into());
    }
    Ok(())
}

async fn start_over_http(
    router: &axum::Router,
    marker_path: &str,
) -> Result<(WorkflowId, RunId), TestError> {
    let request = granted_headers(
        Request::builder()
            .uri("/workflows/start")
            .method("POST")
            .header("content-type", "application/json"),
    )
    .body(body::Body::from(serde_json::to_vec(&json!({
        "namespace": NAMESPACE,
        "workflow_type": WORKFLOW_TYPE,
        "input": { "path": marker_path },
    }))?))?;
    let response = router.clone().oneshot(request).await?;
    let status = response.status();
    let bytes = body::to_bytes(response.into_body(), usize::MAX).await?;
    if status != StatusCode::OK {
        return Err(format!(
            "start must succeed, got {status}: {}",
            String::from_utf8_lossy(&bytes)
        )
        .into());
    }
    let body: serde_json::Value = serde_json::from_slice(&bytes)?;
    let workflow_id = body["workflow_id"]
        .as_str()
        .ok_or("start response missing workflow id")?
        .parse::<uuid::Uuid>()?;
    let run_id = body["run_id"]
        .as_str()
        .ok_or("start response missing run id")?
        .parse::<uuid::Uuid>()?;
    Ok((WorkflowId::new(workflow_id), RunId::new(run_id)))
}

/// Swaps the probe marker path for the corpse marker path inside a recorded
/// payload, byte-for-byte. The two paths are equal length by construction, so
/// the payload's encoding is otherwise untouched — replay reproduces these
/// exact bytes from the swapped workflow input.
fn swap_marker(payload: &Payload, from: &str, to: &str) -> Result<Payload, TestError> {
    let text = std::str::from_utf8(payload.bytes())?;
    Ok(Payload::new(
        payload.content_type().clone(),
        text.replace(from, to).into_bytes(),
    ))
}

fn rekeyed_envelope(envelope: &EventEnvelope, workflow_id: &WorkflowId) -> EventEnvelope {
    EventEnvelope {
        seq: envelope.seq,
        recorded_at: envelope.recorded_at,
        workflow_id: workflow_id.clone(),
    }
}

/// Re-keys one recorded probe event to the corpse identity, swapping the
/// marker path in every payload that carries it. Only the variants a
/// just-interrupted history can contain are accepted; anything else is a
/// fixture error, reported loudly.
fn rekey_event(
    event: &Event,
    corpse_workflow: &WorkflowId,
    corpse_run: &RunId,
    probe_marker: &str,
    corpse_marker: &str,
) -> Result<Event, TestError> {
    match event {
        Event::WorkflowStarted {
            envelope,
            workflow_type,
            input,
            run_id: _,
            parent_run_id,
            parent_workflow_id,
            package_version,
        } => Ok(Event::WorkflowStarted {
            envelope: rekeyed_envelope(envelope, corpse_workflow),
            workflow_type: workflow_type.clone(),
            input: swap_marker(input, probe_marker, corpse_marker)?,
            run_id: corpse_run.clone(),
            parent_run_id: parent_run_id.clone(),
            parent_workflow_id: parent_workflow_id.clone(),
            package_version: package_version.clone(),
        }),
        Event::SearchAttributesUpdated {
            envelope,
            workflow_id: _,
            attributes,
        } => Ok(Event::SearchAttributesUpdated {
            envelope: rekeyed_envelope(envelope, corpse_workflow),
            workflow_id: corpse_workflow.clone(),
            attributes: attributes.clone(),
        }),
        Event::ActivityScheduled {
            envelope,
            activity_id,
            activity_type,
            input,
            task_queue,
            node,
        } => Ok(Event::ActivityScheduled {
            envelope: rekeyed_envelope(envelope, corpse_workflow),
            activity_id: activity_id.clone(),
            activity_type: activity_type.clone(),
            input: swap_marker(input, probe_marker, corpse_marker)?,
            task_queue: task_queue.clone(),
            node: node.clone(),
        }),
        Event::ActivityStarted {
            envelope,
            activity_id,
            attempt,
        } => Ok(Event::ActivityStarted {
            envelope: rekeyed_envelope(envelope, corpse_workflow),
            activity_id: activity_id.clone(),
            attempt: *attempt,
        }),
        other => Err(format!(
            "unexpected event in the interrupted-history prefix; the fixture only \
             understands the shapes a mid-dispatch death leaves behind: {other:?}"
        )
        .into()),
    }
}

/// Settle one server generation: begin drain, then shut the engine down.
///
/// Drain-begin is load-bearing for the RED face, not a nicety: a parked
/// declared-body dispatch (the defect) sits in `select_worker_or_refuse` on
/// tokio's blocking pool, and the runtime's `Drop` joins that pool — without
/// drain the process wedges in teardown instead of reporting the verdict
/// (observed: the deadline error fired, then the harness hung forever on the
/// parked dispatch). Drain wakes every parked wait; the woken selection loop
/// meets the drain gate and refuses, the blocking task ends, and the runtime
/// can exit. This must run on EVERY exit path of a generation.
fn settle(state: &ServerState) -> Result<(), TestError> {
    if !state.drain_state().begin() {
        return Err(
            "drain had already begun on this generation — a double settle or an \
             unexpected drain would mask which path released the parked waits"
                .into(),
        );
    }
    state.engine()?.shutdown()?;
    Ok(())
}

fn event_kinds(history: &[Event]) -> Vec<String> {
    history
        .iter()
        .map(|event| {
            let debug = format!("{event:?}");
            debug
                .split([' ', '(', '{'])
                .next()
                .unwrap_or("Unknown")
                .to_owned()
        })
        .collect()
}

/// Corpse-seed the fleet on the dead store: for each corpse, the probe's own
/// recorded events up to and including `ActivityStarted` attempt 1, re-keyed
/// to a fresh identity with that corpse's marker path swapped in. No engine is
/// alive anywhere while this runs — fixture construction, not a second writer.
async fn seed_corpses(
    db_path: PathBuf,
    probe_workflow: &WorkflowId,
    probe_marker_text: &str,
    corpse_markers: &[PathBuf],
) -> Result<Vec<(WorkflowId, PathBuf)>, TestError> {
    let mut corpses: Vec<(WorkflowId, PathBuf)> = Vec::with_capacity(corpse_markers.len());
    let seeder = HaematiteStore::open_or_create(
        db_path,
        haematite::NodeCacheBudget::Unlimited,
        aion_store_haematite::LockAcquisitionRuling::from_millis(250, 5),
    )
    .await?;
    let probe_history = seeder.read_history(probe_workflow).await?;
    for marker in corpse_markers {
        let corpse_workflow = WorkflowId::new_v4();
        let corpse_run = RunId::new_v4();
        let marker_text = marker.to_string_lossy().into_owned();
        assert_eq!(
            probe_marker_text.len(),
            marker_text.len(),
            "marker paths must be equal length for the byte-exact payload swap"
        );
        let mut prefix = Vec::new();
        for event in &probe_history {
            let is_started = matches!(event, Event::ActivityStarted { .. });
            prefix.push(rekey_event(
                event,
                &corpse_workflow,
                &corpse_run,
                probe_marker_text,
                &marker_text,
            )?);
            if is_started {
                break;
            }
        }
        assert!(
            matches!(prefix.first(), Some(Event::WorkflowStarted { .. })),
            "the interrupted history must begin with WorkflowStarted: {:?}",
            event_kinds(&prefix)
        );
        assert!(
            matches!(prefix.last(), Some(Event::ActivityStarted { .. })),
            "the interrupted history must end mid-dispatch, at ActivityStarted: {:?}",
            event_kinds(&prefix)
        );
        assert!(
            prefix
                .iter()
                .any(|event| matches!(event, Event::ActivityScheduled { .. })),
            "the interrupted history must carry the activity's schedule: {:?}",
            event_kinds(&prefix)
        );
        seeder
            .append(WriteToken::recorder(), &corpse_workflow, &prefix, 0)
            .await?;
        corpses.push((corpse_workflow, marker.clone()));
    }
    Ok(corpses)
}

/// The claim, polled to a deadline: EVERY adopted dispatch EXECUTES — each
/// corpse's history gains an activity completion and its run finishes. A
/// corpse that instead ends terminally is a broken fixture or a different
/// defect, failed fast by name; corpses still pending at the deadline are
/// reported as the #266 park with their histories and marker state.
async fn await_corpse_completions(
    reader: &HaematiteStore,
    corpses: &[(WorkflowId, PathBuf)],
) -> Result<(), TestError> {
    let deadline = tokio::time::Instant::now() + ADOPTION_DEADLINE;
    let mut pending: Vec<usize> = (0..corpses.len()).collect();
    loop {
        let mut still_pending = Vec::new();
        for &index in &pending {
            let (corpse_workflow, _) = &corpses[index];
            let history = reader.read_history(corpse_workflow).await?;
            let completed = history
                .iter()
                .any(|event| matches!(event, Event::ActivityCompleted { .. }))
                && history
                    .iter()
                    .any(|event| matches!(event, Event::WorkflowCompleted { .. }));
            if completed {
                continue;
            }
            // A corpse whose replay DIED is a broken fixture or a
            // different defect — fail fast and name it rather than
            // reporting it as the park.
            if history.iter().any(|event| {
                matches!(
                    event,
                    Event::WorkflowFailed { .. }
                        | Event::WorkflowCancelled { .. }
                        | Event::WorkflowTimedOut { .. }
                )
            }) {
                return Err(format!(
                    "corpse {index:03} ended terminally without executing its declared \
                     body — replay refused the seeded history rather than adopting it: {:?}",
                    event_kinds(&history)
                )
                .into());
            }
            still_pending.push(index);
        }
        pending = still_pending;
        if pending.is_empty() {
            break Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            let mut stranded = Vec::with_capacity(pending.len());
            for &index in &pending {
                let (corpse_workflow, marker) = &corpses[index];
                let history = reader.read_history(corpse_workflow).await?;
                stranded.push(format!(
                    "corpse {index:03}: history {:?}, marker exists: {}",
                    event_kinds(&history),
                    marker.is_file(),
                ));
            }
            return Err(format!(
                "{} of {CORPSE_COUNT} adopted declared-body dispatches did not execute \
                 within {ADOPTION_DEADLINE:?}: the recovery re-dispatches parked \
                 (#266 Defect A). Stranded: [{}]",
                pending.len(),
                stranded.join("; "),
            )
            .into());
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

/// The #266 Defect A pin. RED under the defective boot ordering (recovery
/// replay inside `EngineBuilder::build()`, seams installed after): adopted
/// declared-body dispatches park on queue `local` and the corpses never
/// complete. GREEN once the seams are installed before recovery replay runs.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn adopted_declared_body_dispatch_executes_after_recovery() -> Result<(), TestError> {
    let scratch = tempfile::tempdir()?;
    let db_path = scratch.path().join("recovery-ordering.db");
    // Equal-length marker paths (16 characters each), so every corpse's
    // payload byte-swap cannot change any payload's length or shape.
    let probe_marker = scratch.path().join("marker-p-000.txt");
    let probe_marker_text = probe_marker.to_string_lossy().into_owned();
    let corpse_markers: Vec<PathBuf> = (0..CORPSE_COUNT)
        .map(|n| scratch.path().join(format!("marker-c-{n:03}.txt")))
        .collect();

    let prepared = compile_and_assemble_awl(DOCUMENT, scratch.path(), "recovery_probe.awl")?;

    // Generation 1: real full boot, deploy, and the fresh-path control arm —
    // the SAME dispatcher stack executes this declared body at the server,
    // proving the fixture and yielding byte-exact history for the corpses.
    let (state, router, generation_1_store) = server_over(db_path.clone()).await?;
    let generation_1 = async {
        deploy_archive(&router, prepared.archive).await?;
        let (probe_workflow, probe_run) = start_over_http(&router, &probe_marker_text).await?;
        let probe_result = tokio::time::timeout(
            PROBE_DEADLINE,
            state.engine()?.result(&probe_workflow, &probe_run),
        )
        .await
        .map_err(
            |_| "generation-1 probe run did not complete in time (fresh-path control failed)",
        )??;
        probe_result.map_err(|error| format!("generation-1 probe run failed: {error:?}"))?;
        Ok::<WorkflowId, TestError>(probe_workflow)
    };
    let generation_1_result = generation_1.await;
    let settle_result = settle(&state);
    let probe_workflow = generation_1_result?;
    settle_result?;
    assert!(
        probe_marker.is_file(),
        "the fresh-path control must have executed the declared body at the server"
    );
    drop(router);
    drop(state);
    // Every generation-1 handle must be GONE before the seeder opens the same
    // directory: haematite's writer lock is held until the last clone drops.
    drop(generation_1_store);

    // Corpse-seed the fleet on the dead store (no engine is alive anywhere).
    let corpses = seed_corpses(
        db_path.clone(),
        &probe_workflow,
        &probe_marker_text,
        &corpse_markers,
    )
    .await?;

    // Generation 2: the REAL defective boot path over the same database.
    // Startup recovery adopts the whole fleet; while its spawn loop is still
    // running, the earliest corpses reach their re-dispatches inside
    // `build()` — before any seam install can happen.
    let (state, _router, reader) = server_over(db_path.clone()).await?;

    // The claim: EVERY adopted dispatch EXECUTES. Each corpse's history gains
    // a completion and its run finishes; the marker files on disk are the
    // belt. A parked dispatch (the defect) produces none of these, ever.
    let verdict_result = await_corpse_completions(&reader, &corpses).await;
    let settle_result = settle(&state);
    verdict_result?;
    settle_result?;

    for (index, (_, marker)) in corpses.iter().enumerate() {
        assert!(
            marker.is_file(),
            "corpse {index:03}'s adopted declared body must have executed ON THE \
             CORPSE's input"
        );
    }

    Ok(())
}
