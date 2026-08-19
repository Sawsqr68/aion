//! What an operator is told when their queue has no connected worker.
//!
//! The two worlds this must separate are indistinguishable from the
//! connected-worker count — it is 0 in both — and telling the wrong one apart
//! from the right one is the whole point of the hint. So they are asserted
//! against each other here, never one alone.

use aion::{AdmissionReason, DeployedWorkerContract, QueueAdmission, RequiredContract};
use aion_package::{ActionBodyContract, ActionContract, ContentHash, WorkerContract};

use super::{UnservedAddress, service_demand, unserved_hint};
use crate::worker::PoolCensus;
use crate::worker::admission_audit::RememberedRefusal;

const QUEUE: &str = "desk2";

fn action(name: &str, body: Option<&str>) -> ActionContract {
    ActionContract {
        name: name.to_owned(),
        input_schema: serde_json::json!({"type": "object"}),
        output_schema: serde_json::json!({"type": "object"}),
        node: None,
        timeout: None,
        retry: None,
        advisory: false,
        agent: false,
        body: body.map(|command| ActionBodyContract::Run {
            command: command.to_owned(),
        }),
    }
}

fn reachable(actions: Vec<ActionContract>) -> QueueAdmission {
    QueueAdmission {
        required: vec![RequiredContract {
            contract: DeployedWorkerContract {
                package_version: ContentHash::from_bytes([7; 32]),
                contract: WorkerContract {
                    task_queue: QUEUE.to_owned(),
                    actions,
                },
                workflow_types: vec!["repo_gate".to_owned()],
                route_active: true,
            },
            reason: AdmissionReason::RouteActive,
        }],
        unreachable: Vec::new(),
    }
}

#[test]
fn a_queue_of_declared_bodies_demands_no_worker() {
    // The #154 shape: the tutorial's repo_gate, every action server-run. The
    // pre-flight refused the exact start that was then proven to complete
    // green with connected_workers == 0.
    let demand = service_demand(&reachable(vec![
        action("clone", Some("git clone --depth 1 $repo $into")),
        action("head", Some("git -C $dir rev-parse --short HEAD")),
    ]));
    assert!(
        demand.worker.is_empty() && demand.worker_addresses.is_empty(),
        "no reachable action is owed a worker, so zero workers serves the \
         whole queue: worker={:?}",
        demand.worker
    );
    assert_eq!(
        demand.server_run,
        vec!["clone".to_owned(), "head".to_owned()],
        "and the response must SAY who serves it, not just wave availability"
    );
}

#[test]
fn a_bodyless_action_still_demands_a_worker() {
    let demand = service_demand(&reachable(vec![action("charge", None)]));
    assert_eq!(
        demand.worker,
        vec!["charge".to_owned()],
        "nothing serves `charge`: a start would park at dispatch forever, \
         which is exactly what the pre-flight exists to refuse"
    );
    assert_eq!(
        demand.worker_addresses,
        vec![("charge".to_owned(), None)],
        "the demand carries the dispatch address supply is censused at"
    );
}

#[test]
fn a_mixed_queue_is_held_to_its_worker_actions() {
    // One server-run body must not vouch for a sibling that has none.
    let demand = service_demand(&reachable(vec![
        action("clone", Some("git clone $repo")),
        action("judge", None),
    ]));
    assert_eq!(
        demand.worker,
        vec!["judge".to_owned()],
        "`judge` is owed a worker even though `clone` is server-run"
    );
    assert_eq!(demand.server_run, vec!["clone".to_owned()]);
}

#[test]
fn an_empty_queue_demands_nothing_like_admission_does() {
    // Admission ADMITS any worker on a queue with no reachable contract; the
    // availability answer holds a start to the same standard. A start of an
    // undeployed type fails loudly at type resolution — the pre-flight must
    // not shadow that with a worker hint about actions that do not exist.
    let demand = service_demand(&QueueAdmission::default());
    assert!(demand.worker.is_empty());
    assert!(demand.worker_addresses.is_empty());
    assert!(demand.server_run.is_empty());
}

#[test]
fn a_name_bodyless_in_any_reachable_version_is_demanded() {
    // Two reachable versions disagree: the older declares `sync` bodyless,
    // the newer carries a body for it. The older version can still dispatch
    // — a live run may be pinned to it — so the name stays demanded.
    let mut admission = reachable(vec![action("sync", Some("rsync $from $to"))]);
    admission.required.push(RequiredContract {
        contract: DeployedWorkerContract {
            package_version: ContentHash::from_bytes([9; 32]),
            contract: WorkerContract {
                task_queue: QUEUE.to_owned(),
                actions: vec![action("sync", None)],
            },
            workflow_types: vec!["repo_gate".to_owned()],
            route_active: false,
        },
        reason: AdmissionReason::LiveWorkflow,
    });
    let demand = service_demand(&admission);
    assert_eq!(
        demand.worker,
        vec!["sync".to_owned()],
        "the bodyless reachable version still delegates `sync` to a worker"
    );
    assert_eq!(demand.server_run, vec!["sync".to_owned()]);
}

#[test]
fn a_pinned_and_an_unpinned_version_are_two_addresses() {
    // The same name pinned differently across reachable versions is served at
    // BOTH addresses: a run on the pinning version dispatches only to its
    // node, so a worker at the other address covers nothing for it.
    let mut pinned = action("transcribe", None);
    pinned.node = Some("gpu".to_owned());
    let mut admission = reachable(vec![pinned]);
    admission.required.push(RequiredContract {
        contract: DeployedWorkerContract {
            package_version: ContentHash::from_bytes([9; 32]),
            contract: WorkerContract {
                task_queue: QUEUE.to_owned(),
                actions: vec![action("transcribe", None)],
            },
            workflow_types: vec!["repo_gate".to_owned()],
            route_active: false,
        },
        reason: AdmissionReason::LiveWorkflow,
    });
    let demand = service_demand(&admission);
    assert_eq!(
        demand.worker_addresses,
        vec![
            ("transcribe".to_owned(), None),
            ("transcribe".to_owned(), Some("gpu".to_owned())),
        ],
        "one name, two dispatch addresses — each must be censused"
    );
    assert_eq!(demand.worker, vec!["transcribe".to_owned()]);
}

/// Endpoint-level proof: `worker_availability` itself, over REAL compiled AWL
/// packages in a REAL engine and REAL registered workers — because the pure
/// tests above cannot see the wiring, and review R1 proved a literal pre-fix
/// mutation of `available` survived the whole suite when nothing called the
/// endpoint.
mod endpoint {
    use std::net::SocketAddr;
    use std::sync::Arc;
    use std::time::Duration;

    use aion_package::{ExtractionLimits, Package};

    use super::super::{
        WorkerAvailabilityRequest, WorkerAvailabilityResponse, worker_availability,
    };
    use crate::config::{
        AuthConfig, AuthoringConfig, DeployConfig, DevConfig, ListenConfig, MetricsConfig,
        NamespaceConfig, NamespaceMode, OpsConsoleAssetSource, OpsConsoleConfig, OutboxConfig,
        RuntimeConfig, WebSocketConfig, WorkerConfig,
    };
    use crate::worker::WorkerRegistration;
    use crate::{NamespaceResolver, ServerState};

    type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

    /// The #154 shape: every reachable action on queue `gate` carries a
    /// declared `run` body, so the server serves the whole queue itself.
    const ALL_BODIED: &str = r#"//! Availability fixture: every action server-run.
workflow gate_flow
  input dir: String
  outcome done: type Report, route success

type Report { stdout: String, stderr: String }

worker gate
  action head(dir: String) -> Report
    run "git -C $dir rev-parse --short HEAD"
  action checks(dir: String) -> Report
    run "cargo clippy --manifest-path ${dir}/Cargo.toml"

step run
  head(dir: dir) -> at
  checks(dir: dir) -> checked
  route done(stdout: at.stdout, stderr: checked.stderr)
"#;

    /// A queue whose one action is bodyless: a worker is owed.
    const BODYLESS: &str = r"//! Availability fixture: a worker is owed.
workflow admission_drift
  input amount: Int
  outcome completed: type Result, route success

type Result { approved: Bool }

worker payments
  action charge(amount: Int) -> Result

step run
  charge(amount: amount) -> result
  route completed(approved: result.approved)
";

    /// A bodyless action pinned to node `gpu`: only a worker registered on
    /// that node serves it.
    const NODE_PINNED: &str = r"//! Availability fixture: a bodyless action pinned to one node.
workflow pinned_flow
  input recording_path: String
  outcome summarized: type Transcript, route success

type Transcript { text: String, minutes: Int }

worker audio
  action transcribe(recording_path: String) -> Transcript
    node gpu, timeout 3h

step transcribe
  transcribe(recording_path: recording_path) -> transcript
  route summarized(text: transcript.text, minutes: transcript.minutes)
";

    /// Mirrors `crate::state::tests::runtime_config` (cfg(test), not
    /// importable across modules): auth off, shared engine, embedded console.
    fn runtime_config() -> RuntimeConfig {
        RuntimeConfig {
            listen: ListenConfig {
                grpc: SocketAddr::from(([127, 0, 0, 1], 50051)),
                http: SocketAddr::from(([127, 0, 0, 1], 8080)),
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
                ..WorkerConfig::default()
            },
            websocket: WebSocketConfig {
                outbound_buffer_bound: 32,
                event_broadcast_capacity: Some(64),
                cluster_broadcast_capacity: Some(64),
            },
            workflow_packages: Vec::new(),
            deploy: DeployConfig::default(),
            authoring: AuthoringConfig::default(),
            dev: DevConfig::default(),
            outbox: OutboxConfig::default(),
            observability: crate::config::ObservabilityConfig::with_flush_policy(64, 0),
            mcp: crate::config::ResolvedMcpConfig::default(),
            scheduler_threads: 1,
            jit_threshold: None,
            query_timeout: Some(Duration::from_secs(10)),
            default_namespace: "default".to_owned(),
            auto_create: crate::config::AutoCreate::Open,
            max_in_flight_activities: crate::config::DEFAULT_MAX_IN_FLIGHT_ACTIVITIES,
            drain_timeout: Duration::from_secs(30),
            metrics: MetricsConfig { enabled: true },
            owned_shards: Vec::new(),
            cors_allowed_origins: Vec::new(),
        }
    }

    /// A state whose engine carries each AWL source, compiled and deployed
    /// for real — the same catalog `worker_contracts_for_admission` reads in
    /// production.
    async fn server_state(sources: &[(&str, &str)]) -> TestResult<ServerState> {
        let engine = aion::EngineBuilder::new()
            .store(aion_store::InMemoryStore::default())
            .in_memory_visibility()
            .build()
            .await?;
        for (file_name, source) in sources {
            let root = tempfile::tempdir()?;
            let prepared =
                aion_awl_package::compile_and_assemble_awl(source, root.path(), file_name)?;
            let package =
                Package::load_from_bytes(prepared.archive, ExtractionLimits::unbounded())?;
            engine.load_package(package).await?;
        }
        let resolver = NamespaceResolver::from_config(
            NamespaceConfig {
                mode: NamespaceMode::SharedEngine,
            },
            Arc::new(engine),
        );
        Ok(ServerState::from_parts(resolver, runtime_config()))
    }

    /// Register a live worker on the state's registry. The returned guard
    /// must be held: dropping it deregisters the worker.
    fn register_worker(
        state: &ServerState,
        task_queue: &str,
        node: Option<&str>,
        activity_types: &[&str],
    ) -> TestResult<WorkerRegistration> {
        let (tx, _rx) = tokio::sync::mpsc::channel(1);
        let types = activity_types
            .iter()
            .map(|name| (*name).to_owned())
            .collect::<Vec<_>>();
        Ok(state.worker_registry().register_namespaces(
            [String::from("default")],
            task_queue,
            node.map(ToOwned::to_owned),
            types.iter(),
            tx,
        )?)
    }

    fn ask(state: &ServerState, task_queue: &str) -> TestResult<WorkerAvailabilityResponse> {
        Ok(worker_availability(
            state,
            WorkerAvailabilityRequest {
                namespace: "default".to_owned(),
                task_queue: task_queue.to_owned(),
            },
        )?)
    }

    #[tokio::test]
    async fn a_declared_body_queue_is_available_with_zero_workers() -> TestResult {
        // The exact defect: the panel's pre-flight refused this queue because
        // connected_workers was 0 — and the start it refused completes green.
        let state = server_state(&[("gate_flow.awl", ALL_BODIED)]).await?;
        let response = ask(&state, "gate")?;
        assert!(
            response.available,
            "every reachable action is server-run; zero workers serves it: {response:?}"
        );
        assert_eq!(response.connected_workers, 0);
        assert_eq!(response.task_queue, "gate");
        assert_eq!(
            response.worker_actions,
            Vec::<String>::new(),
            "nothing on this queue is a worker's job"
        );
        assert_eq!(
            response.server_run_actions,
            vec!["checks".to_owned(), "head".to_owned()],
            "and the response names exactly who the server serves, sorted"
        );
        assert_eq!(response.scaffold_hint, None);
        Ok(())
    }

    #[tokio::test]
    async fn a_bodyless_queue_with_no_worker_refuses_with_a_hint() -> TestResult {
        let state = server_state(&[("admission_drift.awl", BODYLESS)]).await?;
        let response = ask(&state, "payments")?;
        assert!(
            !response.available,
            "nothing serves `charge`; a start would park at dispatch: {response:?}"
        );
        assert_eq!(response.worker_actions, vec!["charge".to_owned()]);
        assert_eq!(response.server_run_actions, Vec::<String>::new());
        assert!(
            response.scaffold_hint.is_some(),
            "the operator is told what to do about it"
        );
        Ok(())
    }

    #[tokio::test]
    async fn a_worker_advertising_the_action_serves_the_queue() -> TestResult {
        let state = server_state(&[("admission_drift.awl", BODYLESS)]).await?;
        let _guard = register_worker(&state, "payments", None, &["charge"])?;
        let response = ask(&state, "payments")?;
        assert!(
            response.available,
            "a worker advertising `charge` restores the pre-fix behaviour: {response:?}"
        );
        assert_eq!(response.connected_workers, 1);
        assert_eq!(response.scaffold_hint, None);
        Ok(())
    }

    #[tokio::test]
    async fn a_connected_worker_not_advertising_the_action_vouches_for_nothing() -> TestResult {
        // The connection-count half of the old rule inverted: one admitted
        // worker on the queue, but it does not advertise `charge`. Counting
        // connections says available; the census says no.
        let state = server_state(&[("admission_drift.awl", BODYLESS)]).await?;
        let _guard = register_worker(&state, "payments", None, &["something_else"])?;
        let response = ask(&state, "payments")?;
        assert_eq!(response.connected_workers, 1);
        assert!(
            !response.available,
            "a connection that cannot serve `charge` must not vouch for it: {response:?}"
        );
        // The hint must diagnose THIS state — a worker is connected and the
        // action is unadvertised — not claim no worker is connected while the
        // same payload says connected_workers: 1.
        let hint = response.scaffold_hint.as_deref().unwrap_or_default();
        assert!(
            hint.contains("none advertises action `charge`"),
            "the diagnosis names the unserved action: {hint}"
        );
        assert!(
            !hint.contains("No connected worker"),
            "and must not contradict connected_workers in the same payload: {hint}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn a_node_pinned_action_is_not_served_from_the_wrong_node() -> TestResult {
        let state = server_state(&[("pinned_flow.awl", NODE_PINNED)]).await?;
        let _wrong = register_worker(&state, "audio", Some("cpu"), &["transcribe"])?;
        let response = ask(&state, "audio")?;
        assert_eq!(response.connected_workers, 1);
        assert!(
            !response.available,
            "`transcribe` is pinned to `gpu`; a worker on `cpu` cannot be \
             dispatched it and a start would park: {response:?}"
        );
        let hint = response.scaffold_hint.as_deref().unwrap_or_default();
        assert!(
            hint.contains("pinned to node `gpu`"),
            "the diagnosis names the pin the worker misses: {hint}"
        );
        assert!(
            !hint.contains("No connected worker"),
            "a worker IS connected; the hint must not say otherwise: {hint}"
        );

        let _right = register_worker(&state, "audio", Some("gpu"), &["transcribe"])?;
        let served = ask(&state, "audio")?;
        assert_eq!(served.connected_workers, 2);
        assert!(
            served.available,
            "a worker on the pinned node serves the address: {served:?}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn an_undeployed_queue_is_vacuously_available() -> TestResult {
        // Admission admits anyone on an empty queue; a start of an undeployed
        // type fails loudly at type resolution, not silently at dispatch.
        let state = server_state(&[]).await?;
        let response = ask(&state, "nothing_deployed_here")?;
        assert!(response.available);
        assert_eq!(response.worker_actions, Vec::<String>::new());
        assert_eq!(response.server_run_actions, Vec::<String>::new());
        Ok(())
    }
}

fn refusal(node: Option<&str>, identity: &str, reason: &str) -> RememberedRefusal {
    RememberedRefusal {
        node: node.map(ToOwned::to_owned),
        identity: identity.to_owned(),
        reason: reason.to_owned(),
    }
}

/// An unserved address whose census saw `pool` workers on the queue,
/// `serving` of them advertising the action, `compatible` also on its node.
fn unserved(node: Option<&str>, pool: usize, serving: usize, compatible: usize) -> UnservedAddress {
    UnservedAddress {
        action: "charge".to_owned(),
        node: node.map(ToOwned::to_owned),
        census: PoolCensus {
            workers_in_pool: pool,
            workers_serving_activity: serving,
            compatible_workers: compatible,
            last_compatible_poller_age: None,
        },
    }
}

#[test]
fn with_no_refusal_and_an_empty_pool_the_operator_is_told_to_start_a_worker() {
    let hint = unserved_hint(QUEUE, &unserved(None, 0, 0, 0), &[]);
    assert!(
        hint.contains("Scaffold and run this worker"),
        "nothing is dialling this queue, so starting one IS the remedy: {hint}"
    );
    assert!(
        !hint.contains("REFUSING"),
        "and it must not invent a refusal that never happened: {hint}"
    );
}

#[test]
fn a_pool_that_does_not_advertise_the_action_is_not_called_absent() {
    // The census remedy made `unserved && connected_workers > 0` reachable;
    // the hint must not answer that state with "no connected worker" while
    // the payload beside it says connected_workers: 1 (R2 finding).
    let hint = unserved_hint(QUEUE, &unserved(None, 1, 0, 0), &[]);
    assert!(
        !hint.contains("No connected worker"),
        "a worker IS connected; the diagnosis must not contradict the count \
         in the same payload: {hint}"
    );
    assert!(
        hint.contains("none advertises action `charge`"),
        "the diagnosis names WHICH action made the queue unserved: {hint}"
    );
    assert!(
        hint.contains("Starting another copy of the same worker will not help"),
        "and stops the operator repeating the action that cannot work: {hint}"
    );
}

#[test]
fn a_pool_advertising_from_the_wrong_node_is_told_where_the_pin_is() {
    let hint = unserved_hint(QUEUE, &unserved(Some("gpu"), 2, 2, 0), &[]);
    assert!(
        hint.contains("pinned to node `gpu`"),
        "the pin is the whole problem and must be named: {hint}"
    );
    assert!(
        hint.contains("Start a worker on node `gpu`"),
        "the remedy is a worker THERE, not another one here: {hint}"
    );
    assert!(
        !hint.contains("No connected worker"),
        "two workers are connected; the hint must not say none is: {hint}"
    );
}

#[test]
fn with_a_refusal_on_record_the_operator_is_told_the_worker_is_being_refused() {
    let hint = unserved_hint(
        QUEUE,
        &unserved(None, 0, 0, 0),
        &[refusal(
            None,
            "desk2-build",
            "WORKER_CONTRACT_MISMATCH: action `charge` field `input_schema.type`",
        )],
    );
    assert!(
        !hint.contains("Scaffold and run this worker"),
        "the operator's worker IS running; this instruction cannot help them \
         and was the only signal during the #146 self-run: {hint}"
    );
    assert!(
        hint.contains("REFUSING"),
        "the hint must say what is actually happening: {hint}"
    );
    assert!(
        hint.contains("desk2-build"),
        "and WHICH build, so the operator knows which process to fix: {hint}"
    );
    assert!(
        hint.contains("input_schema.type"),
        "and WHY, carried from the gate's own diagnosis rather than paraphrased \
         into something unactionable: {hint}"
    );
}

#[test]
fn a_node_pinned_refusal_names_its_node() {
    let hint = unserved_hint(
        QUEUE,
        &unserved(None, 0, 0, 0),
        &[refusal(Some("netbox"), "desk2-build", "mismatch")],
    );
    assert!(
        hint.contains("node `netbox`"),
        "a worker serving one node is a different worker from the one serving \
         another, and an operator with several will fix the wrong one: {hint}"
    );
}

#[test]
fn further_refusals_are_counted_rather_than_dropped() {
    let address = unserved(None, 0, 0, 0);
    let one = unserved_hint(QUEUE, &address, &[refusal(None, "build-a", "mismatch")]);
    assert!(
        !one.contains("other connection"),
        "a single refusal must not claim company it does not have: {one}"
    );

    let two = unserved_hint(
        QUEUE,
        &address,
        &[
            refusal(Some("netbox"), "build-a", "mismatch"),
            refusal(Some("shell"), "build-b", "mismatch"),
        ],
    );
    assert!(
        two.contains("One other connection was refused"),
        "fixing the named one would leave the queue unserved, and an operator \
         told about one refusal will believe they are done: {two}"
    );

    let four = unserved_hint(
        QUEUE,
        &address,
        &[
            refusal(Some("a"), "build-a", "mismatch"),
            refusal(Some("b"), "build-b", "mismatch"),
            refusal(Some("c"), "build-c", "mismatch"),
            refusal(Some("d"), "build-d", "mismatch"),
        ],
    );
    assert!(
        four.contains("3 other connections were refused"),
        "the count is the count, not a fixed word: {four}"
    );
}
