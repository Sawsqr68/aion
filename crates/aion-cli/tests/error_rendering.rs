//! End-to-end error-rendering contract for the aion-cli binary.
//!
//! Every operational failure must exit 1 with nothing on stdout and one
//! `error[<class>]: ...` report on stderr. The connect-error path runs
//! against an unroutable endpoint; the typed wire-error paths run against a
//! local tonic mock that answers exactly like `aion-server`'s gRPC API:
//! query-handler failures ride `QueryResponse.outcome.error`, everything
//! else rides a `tonic::Status` with the typed `ProtoWireError` encoded into
//! the status details.

use std::net::SocketAddr;
use std::process::Output;

use aion_proto::generated::workflow_service_server::{WorkflowService, WorkflowServiceServer};
use aion_proto::{ProtoWireError, WireError, generated};
use prost::Message as _;
use tonic::{Code, Request, Response, Status};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const WORKFLOW_ID: &str = "00000000-0000-0000-0000-000000000001";

/// An endpoint no test environment can connect to.
const UNROUTABLE_ENDPOINT: &str = "aion-cli-error-test.invalid:1";

fn run_cli(endpoint: &str, args: &[&str]) -> std::io::Result<Output> {
    std::process::Command::new(env!("CARGO_BIN_EXE_aion"))
        .args(["--endpoint", endpoint])
        .args(args)
        .output()
}

fn stderr_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Asserts the shared failure contract: exit code 1 and an empty stdout.
fn assert_failure_contract(output: &Output) {
    assert_eq!(output.status.code(), Some(1), "exit code must be 1");
    assert!(
        output.stdout.is_empty(),
        "errors must never reach stdout, got {:?}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn connect_failure_renders_unavailable_with_detail_and_hint() -> TestResult {
    let output = run_cli(UNROUTABLE_ENDPOINT, &["query", WORKFLOW_ID, "state"])?;

    assert_failure_contract(&output);
    let stderr = stderr_of(&output);
    let first_line = stderr.lines().next().unwrap_or_default();
    assert!(
        first_line.starts_with("error[unavailable]: failed to connect to Aion server: "),
        "got {first_line:?}"
    );
    assert!(
        first_line.len() > "error[unavailable]: failed to connect to Aion server: ".len(),
        "the transport error chain must follow the context, got {first_line:?}"
    );
    assert!(
        stderr.contains("hint: cannot reach the server; check --endpoint"),
        "got {stderr:?}"
    );
    Ok(())
}

/// Mock gRPC server speaking the server's exact error wire shapes.
struct MockWorkflowService;

/// Encodes a typed wire error into a status exactly like the server's
/// `status_from_wire_error`.
fn typed_status(code: Code, error: WireError) -> Status {
    let message = error.message.clone();
    let proto = ProtoWireError::from(error);
    let mut details = Vec::new();
    if proto.encode(&mut details).is_ok() {
        Status::with_details(code, message, details.into())
    } else {
        Status::new(code, message)
    }
}

fn outcome_error(error: WireError) -> generated::QueryResponse {
    let proto = ProtoWireError::from(error);
    generated::QueryResponse {
        outcome: Some(generated::query_response::Outcome::Error(
            generated::WireError {
                code: proto.code,
                message: proto.message,
                error_type: proto.error_type,
            },
        )),
    }
}

/// The engine's own words for a query against a run that is no longer running:
/// `EngineError::Query` (`"query error: {0}"`) wrapping
/// `QueryError::NotRunning` (`"workflow {0} is not running"`). The server hands
/// the wire exactly this via `query_wire`'s `source.to_string()`.
fn query_not_running_detail() -> String {
    format!("query error: workflow {WORKFLOW_ID} is not running")
}

/// The words `signal_terminal_error` builds, for a signal aimed at a run that
/// has already finished.
fn signal_terminal_detail() -> String {
    format!("workflow {WORKFLOW_ID} has already reached terminal state Completed")
}

fn off_scope<T>() -> Result<Response<T>, Status> {
    Err(Status::unimplemented("not part of this test"))
}

#[tonic::async_trait]
impl WorkflowService for MockWorkflowService {
    /// Query failures keyed by query name, one per #45 wire code.
    async fn query(
        &self,
        request: Request<generated::QueryRequest>,
    ) -> Result<Response<generated::QueryResponse>, Status> {
        match request.into_inner().query_name.as_str() {
            // query_failed rides QueryResponse.outcome.error inside an OK
            // response, exactly like the server.
            "fails" => Ok(Response::new(outcome_error(WireError::query_failed(
                "handler raised: cart is empty",
            )))),
            "slow" => Err(typed_status(
                Code::DeadlineExceeded,
                WireError::query_timeout("query window of 30ms elapsed"),
            )),
            "missing" => Err(typed_status(
                Code::InvalidArgument,
                WireError::unknown_query("no query named 'missing' is registered"),
            )),
            // 🔴 A query against a run that has already finished is NOT a
            // transport failure and does NOT carry `WorkflowTerminal`. The
            // server answers OK and rides `QueryNotRunning` on the
            // `QueryResponse.outcome.error` oneof: `api/handlers/workflows.rs`
            // routes `EngineError::Query(_)` that way, `error_engine.rs`'s
            // `query_error_type` names it `QueryNotRunning`, and `query_wire`
            // carries the engine error's own words. The server pins it itself
            // in `query_handler_returns_not_running_outcome_for_terminal_workflow`.
            //
            // `WorkflowTerminal` is built at exactly two places —
            // `signal_terminal_error` and `cancel_terminal_error` in
            // `aion-server/src/api/handlers/error.rs` — reachable only from the
            // signal and cancel handlers. Pairing it with a query is a fixture
            // no server can produce, and a fixture that cannot occur cannot pin
            // behaviour. It is exercised through `signal` below, where it is
            // real.
            "terminal" => Ok(Response::new(outcome_error(
                WireError::not_running_with_type("QueryNotRunning", query_not_running_detail()),
            ))),
            // The real `ShuttingDown`, with the message that arm actually
            // carries. Its hint must differ from `terminal`'s above: one says
            // the run is over, the other says the server is going away and the
            // run is untouched.
            "shutdown" => Err(typed_status(
                Code::FailedPrecondition,
                WireError::not_running_with_type(
                    "ShuttingDown",
                    "the engine is shutting down and is not accepting work",
                ),
            )),
            other => Err(Status::unimplemented(format!("unexpected query {other}"))),
        }
    }

    /// Signal failures keyed by signal name. This is one of the two handlers
    /// that can produce `WorkflowTerminal` — the other is `cancel` — so it is
    /// where the CLI's rendering of that type is exercised.
    async fn signal(
        &self,
        request: Request<generated::SignalRequest>,
    ) -> Result<Response<generated::SignalResponse>, Status> {
        match request.into_inner().signal_name.as_str() {
            "approve" => Err(typed_status(
                Code::NotFound,
                WireError::not_found_with_type("WorkflowNotFound", "workflow was not found"),
            )),
            // `signal_terminal_error`'s own type and own words.
            "terminal" => Err(typed_status(
                Code::FailedPrecondition,
                WireError::not_running_with_type("WorkflowTerminal", signal_terminal_detail()),
            )),
            other => Err(Status::unimplemented(format!("unexpected signal {other}"))),
        }
    }

    async fn start_workflow(
        &self,
        _: Request<generated::StartWorkflowRequest>,
    ) -> Result<Response<generated::StartWorkflowResponse>, Status> {
        off_scope()
    }

    /// The wrong-shard-owner fence, exactly as the server sends it: the typed
    /// `not_owner` wire code on gRPC `ABORTED`. Used by the routing-hint test.
    async fn cancel(
        &self,
        _: Request<generated::CancelRequest>,
    ) -> Result<Response<generated::CancelResponse>, Status> {
        Err(typed_status(
            Code::Aborted,
            WireError::not_owner("workflow shard 1 is owned by another cluster node")
                .with_error_type("NotOwner"),
        ))
    }

    async fn mint_namespace(
        &self,
        _: Request<generated::MintNamespaceRequest>,
    ) -> Result<Response<generated::MintNamespaceResponse>, Status> {
        off_scope()
    }

    async fn reopen(
        &self,
        _: Request<generated::ReopenRequest>,
    ) -> Result<Response<generated::ReopenResponse>, Status> {
        off_scope()
    }

    async fn pause(
        &self,
        _: Request<generated::PauseRequest>,
    ) -> Result<Response<generated::PauseResponse>, Status> {
        off_scope()
    }

    async fn resume(
        &self,
        _: Request<generated::ResumeRequest>,
    ) -> Result<Response<generated::ResumeResponse>, Status> {
        off_scope()
    }

    async fn rename(
        &self,
        _: Request<generated::RenameRequest>,
    ) -> Result<Response<generated::RenameResponse>, Status> {
        off_scope()
    }

    async fn list_workflows(
        &self,
        _: Request<generated::ListWorkflowsRequest>,
    ) -> Result<Response<generated::ListWorkflowsResponse>, Status> {
        off_scope()
    }

    async fn count_workflows(
        &self,
        _: Request<generated::CountWorkflowsRequest>,
    ) -> Result<Response<generated::CountWorkflowsResponse>, Status> {
        off_scope()
    }

    async fn describe_workflow(
        &self,
        _: Request<generated::DescribeWorkflowRequest>,
    ) -> Result<Response<generated::DescribeWorkflowResponse>, Status> {
        off_scope()
    }

    async fn read_history(
        &self,
        _: Request<generated::ReadHistoryRequest>,
    ) -> Result<Response<generated::ReadHistoryResponse>, Status> {
        off_scope()
    }

    async fn create_schedule(
        &self,
        _: Request<generated::CreateScheduleRequest>,
    ) -> Result<Response<generated::CreateScheduleResponse>, Status> {
        off_scope()
    }

    async fn update_schedule(
        &self,
        _: Request<generated::UpdateScheduleRequest>,
    ) -> Result<Response<generated::UpdateScheduleResponse>, Status> {
        off_scope()
    }

    async fn pause_schedule(
        &self,
        _: Request<generated::ScheduleIdRequest>,
    ) -> Result<Response<generated::PauseScheduleResponse>, Status> {
        off_scope()
    }

    async fn resume_schedule(
        &self,
        _: Request<generated::ScheduleIdRequest>,
    ) -> Result<Response<generated::ResumeScheduleResponse>, Status> {
        off_scope()
    }

    async fn delete_schedule(
        &self,
        _: Request<generated::ScheduleIdRequest>,
    ) -> Result<Response<generated::DeleteScheduleResponse>, Status> {
        off_scope()
    }

    async fn list_schedules(
        &self,
        _: Request<generated::ListSchedulesRequest>,
    ) -> Result<Response<generated::ListSchedulesResponse>, Status> {
        off_scope()
    }

    async fn describe_schedule(
        &self,
        _: Request<generated::ScheduleIdRequest>,
    ) -> Result<Response<generated::DescribeScheduleResponse>, Status> {
        off_scope()
    }
}

async fn spawn_mock_server() -> Result<SocketAddr, Box<dyn std::error::Error>> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let incoming = tonic::transport::server::TcpIncoming::from(listener);
    tokio::spawn(
        tonic::transport::Server::builder()
            .add_service(WorkflowServiceServer::new(MockWorkflowService))
            .serve_with_incoming(incoming),
    );
    Ok(address)
}

async fn run_cli_against_mock(args: &[&str]) -> Result<Output, Box<dyn std::error::Error>> {
    let address = spawn_mock_server().await?;
    let endpoint = format!("http://{address}");
    let args: Vec<String> = args.iter().map(ToString::to_string).collect();
    let output = tokio::task::spawn_blocking(move || {
        let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
        run_cli(&endpoint, &borrowed)
    })
    .await??;
    Ok(output)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn each_query_wire_code_renders_distinctly_end_to_end() -> TestResult {
    let cases = [
        (
            "fails",
            "error[query_failed]: failed to query workflow: handler raised: cart is empty"
                .to_string(),
            "hint: the workflow's query handler ran and reported this failure",
        ),
        (
            "slow",
            "error[query_timeout]: failed to query workflow: query window of 30ms elapsed"
                .to_string(),
            "hint: the query missed its deadline",
        ),
        (
            "missing",
            "error[unknown_query]: failed to query workflow: no query named 'missing' is \
             registered"
                .to_string(),
            "hint: the workflow does not register a query with this name",
        ),
        (
            "terminal",
            format!(
                "error[not_running]: failed to query workflow: {}",
                query_not_running_detail()
            ),
            "hint: the target run is no longer running",
        ),
        // Same wire CODE as `terminal`, different error TYPE, and the operator
        // must not be told the same thing. This is the end-to-end half of the
        // renderer's `ShuttingDown` guard; the unit half is in
        // `aion-cli/src/render.rs`.
        (
            "shutdown",
            "error[not_running]: failed to query workflow: the engine is shutting down and is \
             not accepting work"
                .to_string(),
            "hint: the server is shutting down",
        ),
    ];
    for (query_name, first_line, hint_fragment) in cases {
        let output = run_cli_against_mock(&["query", WORKFLOW_ID, query_name]).await?;
        assert_failure_contract(&output);
        let stderr = stderr_of(&output);
        assert_eq!(
            stderr.lines().next(),
            Some(first_line.as_str()),
            "query {query_name}: got {stderr:?}"
        );
        assert!(
            stderr.contains(hint_fragment),
            "query {query_name}: got {stderr:?}"
        );
    }
    Ok(())
}

/// The typed `error_type` must survive the STATUS-DETAILS route — the server
/// encodes a `ProtoWireError` into `tonic::Status` details, and everything the
/// CLI can say about a failure beyond its class comes from there. `shutdown` is
/// a genuine `ShuttingDown` (`aion-server/src/error.rs`) and rides that route.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn typed_error_type_from_status_details_reaches_stderr() -> TestResult {
    let output = run_cli_against_mock(&["query", WORKFLOW_ID, "shutdown"]).await?;

    assert_failure_contract(&output);
    let stderr = stderr_of(&output);
    assert!(
        stderr.contains("server error type: ShuttingDown"),
        "the wire error_type must be surfaced, got {stderr:?}"
    );
    Ok(())
}

/// The same discriminator must also survive the OTHER route — the OK response
/// carrying `QueryResponse.outcome.error`. A query against a finished run takes
/// that one, and it is decoded by different code from the status-details path
/// above, so a green there says nothing about here.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn typed_error_type_from_query_outcome_reaches_stderr() -> TestResult {
    let output = run_cli_against_mock(&["query", WORKFLOW_ID, "terminal"]).await?;

    assert_failure_contract(&output);
    let stderr = stderr_of(&output);
    assert!(
        stderr.contains("server error type: QueryNotRunning"),
        "the wire error_type must be surfaced off the outcome oneof, got {stderr:?}"
    );
    Ok(())
}

/// `WorkflowTerminal` rendered where the server actually produces it. It shares
/// the `not_running` class with `QueryNotRunning` and with `ShuttingDown`, and
/// must not inherit the shutdown hint: this run is over, the server is fine.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn signal_against_a_terminal_run_renders_workflow_terminal() -> TestResult {
    let output = run_cli_against_mock(&["signal", WORKFLOW_ID, "terminal"]).await?;

    assert_failure_contract(&output);
    let stderr = stderr_of(&output);
    assert_eq!(
        stderr.lines().next(),
        Some(
            format!(
                "error[not_running]: failed to signal workflow: {}",
                signal_terminal_detail()
            )
            .as_str()
        ),
        "got {stderr:?}"
    );
    assert!(
        stderr.contains("server error type: WorkflowTerminal"),
        "got {stderr:?}"
    );
    assert!(
        stderr.contains("hint: the target run is no longer running"),
        "got {stderr:?}"
    );
    assert!(
        !stderr.contains("shutting down"),
        "a finished run must not be reported as a server shutdown, got {stderr:?}"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn signal_not_found_renders_class_detail_error_type_and_hint() -> TestResult {
    let output = run_cli_against_mock(&["signal", WORKFLOW_ID, "approve"]).await?;

    assert_failure_contract(&output);
    let stderr = stderr_of(&output);
    assert_eq!(
        stderr.lines().next(),
        Some("error[not_found]: failed to signal workflow: workflow was not found"),
        "got {stderr:?}"
    );
    assert!(
        stderr.contains("server error type: WorkflowNotFound"),
        "got {stderr:?}"
    );
    assert!(
        stderr.contains("hint: verify the workflow id, --run-id, and --namespace"),
        "got {stderr:?}"
    );
    Ok(())
}

/// A wrong-shard-owner fence renders as its OWN class with a ROUTING hint —
/// never as `unavailable` with "check --endpoint".
///
/// The server answered; there is no connectivity fault to hunt. Collapsing the
/// two cost the failover start-path investigation real diagnosis time, so this
/// pins that the CLI now says something true. The `unavailable` rendering above
/// (`connect_failure_renders_unavailable_with_detail_and_hint`) is unchanged and
/// still asserted, so the two classes are proved distinct rather than swapped.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn not_owner_renders_a_routing_hint_not_a_connectivity_one() -> TestResult {
    let output = run_cli_against_mock(&["cancel", WORKFLOW_ID]).await?;

    assert_failure_contract(&output);
    let stderr = stderr_of(&output);
    assert_eq!(
        stderr.lines().next(),
        Some(
            "error[not_owner]: failed to cancel workflow: workflow shard 1 is \
             owned by another cluster node"
        ),
        "got {stderr:?}"
    );
    assert!(
        stderr.contains("server error type: NotOwner"),
        "got {stderr:?}"
    );
    assert!(
        stderr.contains("hint: the server answered but does not own this target's shard"),
        "got {stderr:?}"
    );
    assert!(
        !stderr.contains("cannot reach the server"),
        "the connectivity hint must NOT appear for a routing fence, got {stderr:?}"
    );
    Ok(())
}
