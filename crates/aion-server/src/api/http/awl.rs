use aion_proto::WireError;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

use super::auth::HttpCaller;
use super::authoring::AuthoringHttpError;
use super::error::HttpWireError;
use crate::ServerState;
use crate::awl::{
    self, CheckRequest, CheckResponse, CreateDocumentRequest, CreateDocumentResponse, Diagnostic,
    DocumentEntry, DocumentResponse, EditRequest, EditResponse, FormatRequest, FormatResponse,
    PutDocumentRequest,
};

pub(crate) async fn check(
    State(state): State<ServerState>,
    HttpCaller(caller): HttpCaller,
    Json(request): Json<CheckRequest>,
) -> Result<Json<CheckResponse>, Response> {
    require_authenticated(&caller)?;
    let root = workspace(&state).map_err(IntoResponse::into_response)?;
    awl::check_source_in_workspace(&root, &request)
        .await
        .map(Json)
        .map_err(|error| DocumentHttpError(error).into_response())
}

pub(crate) async fn edit(
    HttpCaller(caller): HttpCaller,
    Json(request): Json<EditRequest>,
) -> Result<Json<EditResponse>, Response> {
    require_authenticated(&caller)?;
    Ok(Json(awl::edit_source(&request)))
}

pub(crate) async fn scaffold(
    State(state): State<ServerState>,
    HttpCaller(caller): HttpCaller,
    Json(request): Json<awl::scaffold::ScaffoldRequest>,
) -> Result<Json<awl::scaffold::ScaffoldResponse>, Response> {
    require_authenticated(&caller)?;
    let root = workspace(&state).map_err(IntoResponse::into_response)?;
    let scaffolded = tokio::task::spawn_blocking(move || awl::scaffold::scaffold(&request, &root))
        .await
        .map_err(|error| {
            DocumentHttpError(awl::documents::DocumentError::Io(std::io::Error::other(
                format!("AWL scaffold task failed: {error}"),
            )))
            .into_response()
        })?;
    scaffolded
        .map(Json)
        .map_err(|error| DocumentHttpError(error).into_response())
}

pub(crate) async fn format(
    HttpCaller(caller): HttpCaller,
    Json(request): Json<FormatRequest>,
) -> Result<Json<FormatResponse>, Response> {
    require_authenticated(&caller)?;
    awl::format_source(&request)
        .map(Json)
        .map_err(|diagnostic| FormatHttpError { diagnostic }.into_response())
}

pub(crate) async fn list_documents(
    State(state): State<ServerState>,
    HttpCaller(caller): HttpCaller,
) -> Result<Json<Vec<DocumentEntry>>, Response> {
    require_authenticated(&caller)?;
    let root = workspace(&state).map_err(IntoResponse::into_response)?;
    awl::documents::list(&root)
        .await
        .map(Json)
        .map_err(|error| DocumentHttpError(error).into_response())
}

pub(crate) async fn create_document(
    State(state): State<ServerState>,
    HttpCaller(caller): HttpCaller,
    Json(request): Json<CreateDocumentRequest>,
) -> Result<(StatusCode, Json<CreateDocumentResponse>), Response> {
    require_mutation(&state, &caller)?;
    let root = workspace(&state).map_err(IntoResponse::into_response)?;
    awl::documents::create(&root, request)
        .await
        .map(|response| (StatusCode::CREATED, Json(response)))
        .map_err(|error| DocumentHttpError(error).into_response())
}

pub(crate) async fn get_document(
    State(state): State<ServerState>,
    HttpCaller(caller): HttpCaller,
    Path(path): Path<String>,
) -> Result<Json<DocumentResponse>, Response> {
    require_authenticated(&caller)?;
    let root = workspace(&state).map_err(IntoResponse::into_response)?;
    awl::documents::read(&root, &path)
        .await
        .map(Json)
        .map_err(|error| DocumentHttpError(error).into_response())
}

pub(crate) async fn put_document(
    State(state): State<ServerState>,
    HttpCaller(caller): HttpCaller,
    Path(path): Path<String>,
    Json(request): Json<PutDocumentRequest>,
) -> Result<Json<DocumentResponse>, Response> {
    require_mutation(&state, &caller)?;
    let root = workspace(&state).map_err(IntoResponse::into_response)?;
    awl::documents::write(&root, &path, request)
        .await
        .map(Json)
        .map_err(|error| DocumentHttpError(error).into_response())
}

pub(crate) async fn get_layout(
    State(state): State<ServerState>,
    HttpCaller(caller): HttpCaller,
    Path(path): Path<String>,
) -> Result<Json<awl::layout::LayoutRecord>, Response> {
    require_authenticated(&caller)?;
    let root = workspace(&state)
        .map_err(|error| LayoutHttpError::not_configured(&error.0).into_response())?;
    awl::layout::read(&root, &path, caller.subject())
        .await
        .map(Json)
        .map_err(|error| LayoutHttpError(error).into_response())
}

pub(crate) async fn put_layout(
    State(state): State<ServerState>,
    HttpCaller(caller): HttpCaller,
    Path(path): Path<String>,
    Json(request): Json<awl::layout::LayoutRecord>,
) -> Result<Json<awl::layout::LayoutRecord>, Response> {
    require_mutation(&state, &caller)?;
    let root = workspace(&state)
        .map_err(|error| LayoutHttpError::not_configured(&error.0).into_response())?;
    awl::layout::write(&root, &path, caller.subject(), request)
        .await
        .map(Json)
        .map_err(|error| LayoutHttpError(error).into_response())
}

pub(crate) async fn emit(
    State(state): State<ServerState>,
    HttpCaller(caller): HttpCaller,
    Json(request): Json<awl::run_loop::EmitRequest>,
) -> Result<Json<awl::run_loop::EmitResponse>, RunLoopHttpError> {
    awl::run_loop::emit(&state, &caller, &request)
        .map(Json)
        .map_err(RunLoopHttpError::RunLoop)
}

pub(crate) async fn deploy_authoring(
    State(state): State<ServerState>,
    HttpCaller(caller): HttpCaller,
    Json(request): Json<awl::run_loop::DeployAuthoringRequest>,
) -> Result<Json<awl::run_loop::DeployAuthoringResponse>, RunLoopHttpError> {
    let root = workspace(&state).map_err(|error| RunLoopHttpError::Document(error.0))?;
    awl::run_loop::deploy(&state, &caller, &root, "http", request)
        .await
        .map(Json)
        .map_err(RunLoopHttpError::RunLoop)
}

pub(crate) async fn get_revision(
    State(state): State<ServerState>,
    HttpCaller(caller): HttpCaller,
    Path(hash): Path<String>,
) -> Result<Json<awl::revisions::Revision>, Response> {
    require_authenticated(&caller)?;
    let root =
        workspace(&state).map_err(|error| RunLoopHttpError::Document(error.0).into_response())?;
    awl::revisions::fetch(&root, &hash)
        .await
        .map(Json)
        .map_err(|error| RunLoopHttpError::RunLoop(error.into()).into_response())
}

pub(crate) async fn get_run_status(
    State(state): State<ServerState>,
    HttpCaller(caller): HttpCaller,
    Path(deployment_id): Path<String>,
) -> Result<Json<awl::run_loop::RunStatusResponse>, Response> {
    require_authenticated(&caller)?;
    let root =
        workspace(&state).map_err(|error| RunLoopHttpError::Document(error.0).into_response())?;
    awl::run_loop::status(&root, &deployment_id)
        .await
        .map(Json)
        .map_err(|error| RunLoopHttpError::RunLoop(error).into_response())
}

pub(crate) async fn bind_run(
    State(state): State<ServerState>,
    HttpCaller(caller): HttpCaller,
    Path(deployment_id): Path<String>,
    Json(request): Json<awl::run_loop::BindRunRequest>,
) -> Result<Json<awl::revisions::DeploymentRecord>, Response> {
    require_mutation(&state, &caller)?;
    let root =
        workspace(&state).map_err(|error| RunLoopHttpError::Document(error.0).into_response())?;
    awl::revisions::bind_run(&root, &deployment_id, request.workflow_id, request.run_id)
        .await
        .map(Json)
        .map_err(|error| RunLoopHttpError::RunLoop(error.into()).into_response())
}

pub(crate) async fn worker_availability(
    State(state): State<ServerState>,
    HttpCaller(caller): HttpCaller,
    Json(request): Json<awl::run_loop::WorkerAvailabilityRequest>,
) -> Result<Json<awl::run_loop::WorkerAvailabilityResponse>, Response> {
    require_authenticated(&caller)?;
    state
        .namespace_guard()
        .authorize_namespace(&caller, &request.namespace)
        .map_err(|error| HttpWireError(error.to_wire_error()).into_response())?;
    awl::run_loop::worker_availability(&state, request)
        .map(Json)
        .map_err(|error| RunLoopHttpError::RunLoop(error).into_response())
}

pub(crate) enum RunLoopHttpError {
    Document(awl::documents::DocumentError),
    RunLoop(awl::run_loop::RunLoopError),
}

impl IntoResponse for RunLoopHttpError {
    fn into_response(self) -> Response {
        match self {
            // Both document arms render identically: a deploy that failed on
            // the workspace document (e.g. the path does not exist) is the
            // same refusal, with the same status and error_type, as the
            // document routes' own.
            Self::Document(error) | Self::RunLoop(awl::run_loop::RunLoopError::Document(error)) => {
                DocumentHttpError(error).into_response()
            }
            Self::RunLoop(awl::run_loop::RunLoopError::Authoring(error)) => {
                AuthoringHttpError(error).into_response()
            }
            Self::RunLoop(error) => {
                let status = match error {
                    awl::run_loop::RunLoopError::WorkerRegistry(_) => {
                        StatusCode::INTERNAL_SERVER_ERROR
                    }
                    _ => StatusCode::UNPROCESSABLE_ENTITY,
                };
                (status, Json(awl::run_loop::wire_error(&error))).into_response()
            }
        }
    }
}

struct AwlAuthorizationError(WireError);

impl From<AwlAuthorizationError> for Response {
    fn from(error: AwlAuthorizationError) -> Self {
        HttpWireError(error.0).into_response()
    }
}

fn require_authenticated(caller: &crate::CallerIdentity) -> Result<(), AwlAuthorizationError> {
    if let Some(reason) = caller.denial_reason() {
        return Err(AwlAuthorizationError(WireError::namespace_denied(format!(
            "AWL studio requires an authenticated caller: {reason}"
        ))));
    }
    Ok(())
}

fn require_mutation(
    state: &ServerState,
    caller: &crate::CallerIdentity,
) -> Result<(), AwlAuthorizationError> {
    state
        .deploy_guard()
        .authorize(caller)
        .map_err(|error| AwlAuthorizationError(error.to_wire_error()))
}

/// Thin HTTP shell over the one shared workspace derivation
/// ([`awl::workspace::workspace_root`]); this adds only the HTTP error wrapper.
fn workspace(state: &ServerState) -> Result<std::path::PathBuf, DocumentHttpError> {
    awl::workspace::workspace_root(state).map_err(DocumentHttpError)
}

pub(crate) struct FormatHttpError {
    diagnostic: Diagnostic,
}

#[derive(Serialize)]
struct DiagnosticsBody {
    diagnostics: Vec<Diagnostic>,
}

impl IntoResponse for FormatHttpError {
    fn into_response(self) -> Response {
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(DiagnosticsBody {
                diagnostics: vec![self.diagnostic],
            }),
        )
            .into_response()
    }
}

pub(crate) struct LayoutHttpError(pub(crate) awl::layout::LayoutError);

impl LayoutHttpError {
    fn not_configured(error: &awl::documents::DocumentError) -> Self {
        Self(awl::layout::LayoutError::DocumentNotFound(
            error.to_string(),
        ))
    }
}

impl IntoResponse for LayoutHttpError {
    fn into_response(self) -> Response {
        let (status, error_type, message) = match self.0 {
            awl::layout::LayoutError::InvalidPath(message) => {
                (StatusCode::BAD_REQUEST, "InvalidLayoutPath", message)
            }
            awl::layout::LayoutError::DocumentNotFound(message) => {
                (StatusCode::NOT_FOUND, "DocumentNotFound", message)
            }
            awl::layout::LayoutError::Io(error) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "LayoutIoError",
                error.to_string(),
            ),
        };
        (
            status,
            Json(WireError::invalid_input(message).with_error_type(error_type)),
        )
            .into_response()
    }
}

pub(crate) struct DocumentHttpError(pub(crate) awl::documents::DocumentError);

impl IntoResponse for DocumentHttpError {
    fn into_response(self) -> Response {
        let status = match self.0 {
            awl::documents::DocumentError::InvalidPath(_)
            | awl::documents::DocumentError::InvalidName(_) => StatusCode::BAD_REQUEST,
            awl::documents::DocumentError::NotFound(_) => StatusCode::NOT_FOUND,
            awl::documents::DocumentError::Exists(_) => StatusCode::CONFLICT,
            awl::documents::DocumentError::WorkspaceUnconfigured => StatusCode::SERVICE_UNAVAILABLE,
            // `CreateRollbackFailed` deliberately keeps its own error type
            // rather than folding into a generic I/O failure: the workspace
            // has been left holding a document that was never successfully
            // created, so every retry of this create will come back as
            // `DocumentExists`, and the message names the file that has to be
            // removed by hand.
            awl::documents::DocumentError::Io(_)
            | awl::documents::DocumentError::CreateRollbackFailed { .. } => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };
        let error_type = self.0.error_type();
        let message = match self.0 {
            awl::documents::DocumentError::InvalidPath(message)
            | awl::documents::DocumentError::InvalidName(message)
            | awl::documents::DocumentError::NotFound(message)
            | awl::documents::DocumentError::Exists(message) => message,
            // The I/O body renders the inner error ALONE — no "AWL workspace
            // I/O failed" prefix — exactly as it did before `error_type` was
            // shared; `DocumentIoError` already classifies it.
            awl::documents::DocumentError::Io(error) => error.to_string(),
            error => error.to_string(),
        };
        (
            status,
            Json(WireError::invalid_input(message).with_error_type(error_type)),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use axum::response::IntoResponse;

    use super::DocumentHttpError;
    use crate::awl::documents::DocumentError;

    /// Pins the pre-refactor rendering of an I/O failure: the body's message
    /// is the inner error ALONE. The `Display` of `DocumentError::Io`
    /// prepends "AWL workspace I/O failed: ", and a fallthrough that rendered
    /// `Display` silently changed every I/O body once already — this stops
    /// the next refactor doing it again.
    #[tokio::test]
    async fn an_io_error_body_carries_the_inner_message_without_a_prefix()
    -> Result<(), Box<dyn std::error::Error>> {
        let response = DocumentHttpError(DocumentError::Io(std::io::Error::other("disk full")))
            .into_response();
        assert_eq!(
            response.status(),
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        );
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await?;
        let body: serde_json::Value = serde_json::from_slice(&bytes)?;
        assert_eq!(body["message"], "disk full", "{body}");
        assert_eq!(body["error_type"], "DocumentIoError", "{body}");
        Ok(())
    }
}
