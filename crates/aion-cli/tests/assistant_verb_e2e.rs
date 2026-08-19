//! `aion assistant` end to end: the real binary, against a real server.
//!
//! # What is proven, and what is deliberately not
//!
//! PROVEN: the local subcommands answer from the binary with no server at all;
//! the session subcommands compose a request the server accepts, resolve it
//! against real engine state, and render the server's own answer back to the
//! operator.
//!
//! NOT DONE HERE: starting a session. The assistant's first step is a
//! SERVER-RUN declared command that materialises a workspace on the host
//! (`git clone` into the server's workspace root) and its second dispatches a
//! real agent round. A test that started one would clone a repository and bill
//! an agent as a side effect of `cargo test`. The start path's own contract is
//! proven where it can be proven without acting: the composed input is
//! validated against the document's derived schema in
//! `src/assistant_tests.rs`, and the engine-side installation and routing are
//! proven in `aion-server`.
//!
//! # The control
//!
//! A refusal from a live server and a refusal because nothing was listening
//! both exit 1. The pair of assertions below distinguishes them, so the live
//! result is attributable to the server's answer rather than to any failure.

use std::process::{Command, Output};
use std::time::Duration;

use aion_server::ServerState;
use aion_server::api::grpc::workflow_service;
use aion_server::assistant::{
    AssistantInstall, EMBEDDED_ASSISTANT_DOCUMENT, EmbeddedAssistant,
    install_embedded_assistant_for_server,
};
use aion_server::config::{
    AuthConfig, AuthoringConfig, DeployConfig, ListenConfig, MetricsConfig, NamespaceConfig,
    NamespaceMode, OpsConsoleAssetSource, OpsConsoleConfig, RuntimeConfig, WebSocketConfig,
    WorkerConfig,
};
use serde_json::Value;

type TestError = Box<dyn std::error::Error>;

/// A workflow id that exists on no server.
const UNKNOWN_WORKFLOW_ID: &str = "d1cf6ad0-6d2b-4a0e-9a3e-1f4a2b3c4d5e";

fn runtime_config() -> RuntimeConfig {
    RuntimeConfig {
        listen: ListenConfig {
            grpc: std::net::SocketAddr::from(([127, 0, 0, 1], 0)),
            http: std::net::SocketAddr::from(([127, 0, 0, 1], 0)),
        },
        tls: None,
        cors_allowed_origins: Vec::new(),
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
        dev: aion_server::config::DevConfig::default(),
        outbox: aion_server::config::OutboxConfig::default(),
        observability: aion_server::config::ObservabilityConfig::with_flush_policy(64, 0),
        mcp: aion_server::config::ResolvedMcpConfig::default(),
        scheduler_threads: 1,
        jit_threshold: None,
        query_timeout: Some(Duration::from_secs(10)),
        default_namespace: "default".to_owned(),
        auto_create: aion_server::config::AutoCreate::default(),
        max_in_flight_activities: aion_server::config::DEFAULT_MAX_IN_FLIGHT_ACTIVITIES,
        drain_timeout: Duration::from_secs(30),
        metrics: MetricsConfig { enabled: false },
        owned_shards: Vec::new(),
    }
}

/// Boots the workflow gRPC service over an in-memory store, with the embedded
/// assistant installed exactly as `aion server` installs it at boot.
async fn serve() -> Result<(std::net::SocketAddr, tokio::task::JoinHandle<()>), TestError> {
    use tokio_stream::wrappers::TcpListenerStream;

    let state =
        ServerState::build_with_store(aion_store::InMemoryStore::default(), runtime_config())
            .await?;
    let install = install_embedded_assistant_for_server(
        &state,
        "add `liminal_listen_address` to `[outbox]` in the e2e config",
    )
    .await;
    let embedded = EmbeddedAssistant::load()?;
    assert_eq!(
        install,
        AssistantInstall::Installed {
            workflow_type: embedded.workflow_type().to_owned(),
            content_hash: embedded.content_hash().to_string(),
            task_queue: embedded.task_queue().to_owned(),
        },
        "the verb is being exercised against a server that really carries the assistant"
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let service = workflow_service(state);
    let handle = tokio::spawn(async move {
        let result = tonic::transport::Server::builder()
            .add_service(service)
            .serve_with_incoming(TcpListenerStream::new(listener))
            .await;
        if let Err(error) = result {
            eprintln!("fixture workflow server exited: {error}");
        }
    });
    Ok((address, handle))
}

/// Runs the real `aion` binary.
///
/// The error type is `io::Error`, not the suite's boxed one, because these runs
/// happen inside `spawn_blocking` and a `Box<dyn Error>` is not `Send`.
fn run_cli(args: &[&str]) -> std::io::Result<Output> {
    Command::new(env!("CARGO_BIN_EXE_aion"))
        .args(args)
        .env_remove("AION_TOKEN")
        .output()
}

fn stdout_json(output: &Output) -> Result<Value, TestError> {
    if output.status.code() != Some(0) {
        return Err(format!(
            "expected success, got {:?}; stderr: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(serde_json::from_slice(&output.stdout)?)
}

/// `aion assistant describe` answers from the binary — no `--endpoint`, no
/// server, no network — and its identity is the embedded document's.
#[test]
fn describe_answers_from_the_binary_with_no_server() -> Result<(), TestError> {
    let embedded = EmbeddedAssistant::load()?;
    let output = run_cli(&["assistant", "describe"])?;
    let report = stdout_json(&output)?;
    assert_eq!(report["workflow_type"], embedded.workflow_type());
    assert_eq!(report["content_hash"], embedded.content_hash().to_string());
    assert_eq!(report["session"]["continue_signal"], "assistant_continue");
    assert_eq!(report["session"]["status_query"], "assistant_status");
    Ok(())
}

/// `aion assistant document` prints the document itself, byte for byte — the
/// file an operator with no repository checkout feeds to `aion worker agent`.
#[test]
fn document_prints_the_embedded_document_verbatim() -> Result<(), TestError> {
    let output = run_cli(&["assistant", "document"])?;
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let printed = String::from_utf8(output.stdout)?;
    assert_eq!(printed, EMBEDDED_ASSISTANT_DOCUMENT);
    assert!(printed.contains("workflow assistant"));
    Ok(())
}

/// THE ROUND TRIP. `aion assistant status` composes the document's own query
/// against a live server, the server resolves it against real state, and the
/// CLI renders the server's answer — distinguishably from the case where
/// nothing is listening at all.
#[tokio::test(flavor = "multi_thread")]
async fn the_status_verb_round_trips_against_a_live_server() -> Result<(), TestError> {
    let (address, server) = serve().await?;
    let endpoint = address.to_string();

    let live = tokio::task::spawn_blocking(move || {
        run_cli(&[
            "assistant",
            "status",
            UNKNOWN_WORKFLOW_ID,
            "--endpoint",
            &endpoint,
        ])
    })
    .await??;
    assert_eq!(
        live.status.code(),
        Some(1),
        "an unknown session must be refused, not reported as success; stdout: {}",
        String::from_utf8_lossy(&live.stdout)
    );
    assert!(
        live.stdout.is_empty(),
        "refusals must never reach stdout: {}",
        String::from_utf8_lossy(&live.stdout)
    );
    let live_stderr = String::from_utf8_lossy(&live.stderr).into_owned();

    // The control: the same verb where nothing is listening. Both exit 1, so
    // without this the assertion above could not tell a server's answer from a
    // failure to reach one.
    let dead_port = {
        let socket = std::net::TcpListener::bind("127.0.0.1:0")?;
        socket.local_addr()?.port()
    };
    let dead_endpoint = format!("127.0.0.1:{dead_port}");
    let dead = tokio::task::spawn_blocking(move || {
        run_cli(&[
            "assistant",
            "status",
            UNKNOWN_WORKFLOW_ID,
            "--endpoint",
            &dead_endpoint,
        ])
    })
    .await??;
    assert_eq!(dead.status.code(), Some(1));
    let dead_stderr = String::from_utf8_lossy(&dead.stderr).into_owned();
    assert_ne!(
        live_stderr, dead_stderr,
        "the live server's answer must be distinguishable from never reaching a server; \
         live: {live_stderr}\ndead: {dead_stderr}"
    );
    assert!(
        live_stderr.to_lowercase().contains("not found"),
        "the live answer must be the server's own not-found for an unknown session; got: \
         {live_stderr}"
    );

    server.abort();
    Ok(())
}

/// A blank message is refused BEFORE any connection is attempted.
///
/// The endpoint names a port nothing listens on, so a verb that dialled first
/// would report a connection failure — which is what this asserted against
/// before the ordering was made structural. Getting the document's own reason
/// back instead is the proof that composition happens ahead of connection.
#[test]
fn an_empty_message_is_refused_before_any_connection() -> Result<(), TestError> {
    let output = run_cli(&[
        "assistant",
        "say",
        UNKNOWN_WORKFLOW_ID,
        "--message",
        "",
        "--endpoint",
        "127.0.0.1:1",
    ])?;
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("assistant end"),
        "the refusal must point at the verb that does finish a session; got: {stderr}"
    );
    assert!(
        !stderr.contains("connect"),
        "an argument refusal must not have dialled anything; got: {stderr}"
    );
    Ok(())
}

/// The control for the test above: the SAME verb with a valid message against
/// the SAME dead endpoint DOES report a connection failure. Without it, the
/// local refusal could be explained by a binary that never connects at all.
#[test]
fn a_valid_message_against_a_dead_endpoint_reports_a_connection_failure() -> Result<(), TestError> {
    let output = run_cli(&[
        "assistant",
        "say",
        UNKNOWN_WORKFLOW_ID,
        "--message",
        "a real message",
        "--endpoint",
        "127.0.0.1:1",
    ])?;
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("connect"),
        "a well-formed continuation must actually be dialled somewhere; got: {stderr}"
    );
    Ok(())
}

/// `describe` and `document` answer on a machine with no server: no endpoint is
/// given, and nothing is listening on the default one.
#[test]
fn the_local_subcommands_never_need_a_server() -> Result<(), TestError> {
    for args in [vec!["assistant", "describe"], vec!["assistant", "document"]] {
        let output = run_cli(&args)?;
        assert_eq!(
            output.status.code(),
            Some(0),
            "`aion {}` must not need a server; stderr: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

/// `--repo-path` and `--no-repo` are the two ways to answer one question, and
/// exactly one must be given: a session with no ground truth is a state the
/// operator chooses, never one the CLI defaults into.
#[test]
fn start_requires_the_operator_to_state_the_repository_question() -> Result<(), TestError> {
    let neither = run_cli(&["assistant", "start", "--objective", "explain AWL"])?;
    assert_eq!(
        neither.status.code(),
        Some(2),
        "clap refuses a missing required argument with exit 2; stderr: {}",
        String::from_utf8_lossy(&neither.stderr)
    );

    let both = run_cli(&[
        "assistant",
        "start",
        "--objective",
        "explain AWL",
        "--repo-path",
        "/tmp/aion",
        "--no-repo",
    ])?;
    assert_eq!(
        both.status.code(),
        Some(2),
        "naming both a repository and no repository must be refused; stderr: {}",
        String::from_utf8_lossy(&both.stderr)
    );
    Ok(())
}
