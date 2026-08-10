//! The authoring tools: `list_documents`, `read_document`, `check_document`,
//! `save_document`, `deploy_document`.
//!
//! Every handler is a thin shell over the same transport-agnostic seams the
//! HTTP studio routes use — `awl::documents`, `awl::check_source_in_workspace`,
//! and `awl::run_loop::deploy` — with the same authorization: the reads and
//! the check need an authenticated caller, the save and the deploy need the
//! deploy grant. No handler touches the filesystem itself; workspace
//! confinement is the seams' job and is inherited whole.

use aion_mcp::tools::service::{ToolCall, ToolFailure, ToolOutcome};
use aion_proto::WireError;
use serde_json::json;

use crate::authoring::AuthoringApiError;
use crate::awl::{self, documents::DocumentError, run_loop::RunLoopError};
use crate::mcp::args::{optional_str, required_str};
use crate::{CallerIdentity, ServerState};

use super::errors::{tool_failure, wire_code_label};

/// Run `list_documents`.
///
/// # Errors
///
/// [`ToolFailure`] when the caller is unauthenticated, the workspace is not
/// configured, or the listing fails.
pub(crate) async fn list_documents(
    state: &ServerState,
    caller: &CallerIdentity,
    _call: &ToolCall,
) -> Result<ToolOutcome, ToolFailure> {
    require_authenticated(caller)?;
    let root = workspace(state)?;
    let entries = awl::documents::list(&root)
        .await
        .map_err(|error| document_failure(&error))?;
    let count = entries.len();
    let documents = serde_json::to_value(&entries).map_err(|error| {
        tool_failure(&WireError::backend(format!(
            "document entries could not be encoded: {error}"
        )))
    })?;
    Ok(ToolOutcome {
        summary: format!("{count} document(s) in the authoring workspace"),
        structured: json!({ "documents": documents, "count": count }),
    })
}

/// Run `read_document`.
///
/// # Errors
///
/// [`ToolFailure`] when the caller is unauthenticated, the path is invalid,
/// or the document does not exist.
pub(crate) async fn read_document(
    state: &ServerState,
    caller: &CallerIdentity,
    call: &ToolCall,
) -> Result<ToolOutcome, ToolFailure> {
    require_authenticated(caller)?;
    let path = required_str(call, "path")?;
    let root = workspace(state)?;
    let document = awl::documents::read(&root, &path)
        .await
        .map_err(|error| document_failure(&error))?;
    Ok(ToolOutcome {
        summary: format!("{path} at revision {}", document.content_hash),
        structured: json!({
            "path": path,
            "source": document.source,
            "content_hash": document.content_hash,
        }),
    })
}

/// Run `check_document`.
///
/// # Errors
///
/// [`ToolFailure`] when the caller is unauthenticated, the workspace is not
/// configured, or the checker itself cannot run. Diagnostics in the source
/// are NOT a failure: they come back in the result for the model to act on.
///
/// The result is a PROJECTION of [`awl::CheckResponse`], not the whole of it:
/// the `semantic` index (spans, types, graph, and studio layout) exists for
/// the console editor and weighs hundreds of kilobytes on a real document —
/// serialized twice on this transport, once as `structuredContent` and once
/// as the text copy. A model acts on `ok`, `deploys_green`, `steps`, and the
/// `diagnostics`; those travel whole. The HTTP `/awl/check` route keeps the
/// full response untouched.
pub(crate) async fn check_document(
    state: &ServerState,
    caller: &CallerIdentity,
    call: &ToolCall,
) -> Result<ToolOutcome, ToolFailure> {
    require_authenticated(caller)?;
    let source = required_str(call, "source")?;
    let path = optional_str(call, "path");
    let root = workspace(state)?;
    let checked = awl::check_source_in_workspace(&root, &awl::CheckRequest { source, path })
        .await
        .map_err(|error| document_failure(&error))?;
    let summary = if checked.deploys_green {
        match checked.steps {
            Some(steps) => format!("check passed: {steps} step(s), deploys green"),
            None => "check passed: step count unavailable, deploys green".to_owned(),
        }
    } else {
        format!(
            "check found {} diagnostic(s); the document will not deploy until they are fixed",
            checked.diagnostics.len()
        )
    };
    let diagnostics = serde_json::to_value(&checked.diagnostics).map_err(|error| {
        tool_failure(&WireError::backend(format!(
            "check diagnostics could not be encoded: {error}"
        )))
    })?;
    Ok(ToolOutcome {
        summary,
        structured: json!({
            "ok": checked.ok,
            "deploys_green": checked.deploys_green,
            "steps": checked.steps,
            "diagnostics": diagnostics,
        }),
    })
}

/// Run `save_document`.
///
/// # Errors
///
/// [`ToolFailure`] when the caller lacks the deploy grant, the path is
/// invalid, or the write fails.
pub(crate) async fn save_document(
    state: &ServerState,
    caller: &CallerIdentity,
    call: &ToolCall,
) -> Result<ToolOutcome, ToolFailure> {
    require_deploy_grant(state, caller)?;
    let path = required_str(call, "path")?;
    let source = required_str(call, "source")?;
    let root = workspace(state)?;
    let saved = awl::documents::write(&root, &path, awl::PutDocumentRequest { source })
        .await
        .map_err(|error| document_failure(&error))?;
    Ok(ToolOutcome {
        summary: format!("saved {path} at revision {}", saved.content_hash),
        structured: json!({
            "path": path,
            "source": saved.source,
            "content_hash": saved.content_hash,
        }),
    })
}

/// Run `deploy_document`.
///
/// The whole admission path — deploy grant, drain gate, saved-revision
/// verification, compile, package, and load — is
/// [`awl::run_loop::deploy`]'s, exactly as the HTTP deploy route rides it.
///
/// # Errors
///
/// [`ToolFailure`] when the caller lacks the deploy grant, the hash does not
/// match the saved document, the checker refuses the source, or the compile
/// or load fails.
pub(crate) async fn deploy_document(
    state: &ServerState,
    caller: &CallerIdentity,
    call: &ToolCall,
) -> Result<ToolOutcome, ToolFailure> {
    // Grant first, arguments second, workspace last: an unauthorized caller
    // must learn nothing about the server's workspace configuration from the
    // shape of the refusal. `admit_mutation` inside the deploy seam stays the
    // authoritative gate; this only moves the refusal earlier.
    require_deploy_grant(state, caller)?;
    let path = required_str(call, "path")?;
    let content_hash = required_str(call, "content_hash")?;
    let root = workspace(state)?;
    let deployed = awl::run_loop::deploy(
        state,
        caller,
        &root,
        "mcp",
        awl::run_loop::DeployAuthoringRequest { path, content_hash },
    )
    .await
    .map_err(|error| run_loop_failure(&error))?;
    let summary = format!(
        "deployed {} as workflow type {} (deployment {})",
        deployed.deployment.document_path,
        deployed.deployment.workflow_type,
        deployed.deployment.deployment_id
    );
    let structured = serde_json::to_value(&deployed).map_err(|error| {
        tool_failure(&WireError::backend(format!(
            "deployment record could not be encoded: {error}"
        )))
    })?;
    Ok(ToolOutcome {
        summary,
        structured,
    })
}

/// The authenticated-caller gate the read and check tools share, mirroring the
/// HTTP studio routes' `require_authenticated`.
fn require_authenticated(caller: &CallerIdentity) -> Result<(), ToolFailure> {
    match caller.denial_reason() {
        Some(reason) => Err(tool_failure(&WireError::namespace_denied(format!(
            "AWL authoring requires an authenticated caller: {reason}"
        )))),
        None => Ok(()),
    }
}

/// The deploy-grant gate `save_document` applies directly (`deploy_document`'s
/// is inside [`awl::run_loop::deploy`], so both cannot drift from the HTTP
/// routes' `require_mutation`).
fn require_deploy_grant(state: &ServerState, caller: &CallerIdentity) -> Result<(), ToolFailure> {
    state
        .deploy_guard()
        .authorize(caller)
        .map_err(|error| tool_failure(&error.to_wire_error()))
}

/// Resolve the workspace root through the one shared derivation.
fn workspace(state: &ServerState) -> Result<std::path::PathBuf, ToolFailure> {
    awl::workspace::workspace_root(state).map_err(|error| document_failure(&error))
}

/// Map a document error onto a tool failure.
///
/// Deliberately NOT routed through [`tool_failure`]'s wire-code guidance:
/// that guidance is written for run identifiers ("use `list_runs`…"), and a
/// document that was not found needs document guidance instead. The code
/// class comes from [`DocumentError::to_wire_error`] and its label from
/// [`wire_code_label`] — the same two vocabularies every other failure uses —
/// so nothing here restates either. The detail keeps the same
/// `{code, message, error_type}` shape as [`tool_failure`]'s.
fn document_failure(error: &DocumentError) -> ToolFailure {
    let wire = error.to_wire_error();
    let message = match error {
        DocumentError::NotFound(_) => format!(
            "{} — document paths come from list_documents or a previous save; \
             use list_documents to see what exists, do not guess",
            wire.message
        ),
        DocumentError::WorkspaceUnconfigured => format!(
            "{} — the server was started without authoring.workspace_dir, so no \
             authoring tool can work until the operator configures it",
            wire.message
        ),
        _ => wire.message.clone(),
    };
    ToolFailure::new(
        message,
        json!({
            "code": wire_code_label(wire.code),
            "message": wire.message,
            "error_type": wire.error_type,
        }),
    )
}

/// Map a deploy run-loop error onto a tool failure, preserving the inner
/// classes the generic mapping would flatten.
fn run_loop_failure(error: &RunLoopError) -> ToolFailure {
    match error {
        // The guard's denial and the drain gate's refusal both carry a wire
        // error whose code (deploy_denied / backend) a caller must see; the
        // generic `run_loop::wire_error` flattens either to `backend`.
        RunLoopError::Authoring(
            AuthoringApiError::Wire(wire) | AuthoringApiError::Unavailable(wire),
        ) => tool_failure(wire),
        RunLoopError::Authoring(AuthoringApiError::TypeError(diagnostics)) => tool_failure(
            &WireError::invalid_input(diagnostics.clone()).with_error_type("TypeError"),
        ),
        // A deploy that failed on the workspace document (the path does not
        // exist, say) is a DOCUMENT refusal and carries document guidance,
        // exactly as if the same fault came from read_document.
        RunLoopError::Document(document) => document_failure(document),
        RunLoopError::RevisionMismatch { .. } => {
            let mut failure = tool_failure(&awl::run_loop::wire_error(error));
            failure.message = format!(
                "{} — the saved document changed since you last held its hash. Re-read the \
                 document (or re-save your source) and deploy with the content_hash you are \
                 handed back; never construct one",
                failure.message
            );
            failure
        }
        other => tool_failure(&awl::run_loop::wire_error(other)),
    }
}

#[cfg(test)]
mod tests {
    use aion_proto::WireError;

    use super::{DocumentError, document_failure, run_loop_failure};
    use crate::authoring::AuthoringApiError;
    use crate::awl::run_loop::RunLoopError;

    #[test]
    fn a_missing_document_gets_document_guidance_not_run_guidance() {
        let failure = document_failure(&DocumentError::NotFound("etl.awl".to_owned()));
        assert_eq!(failure.detail["code"], "not_found");
        assert_eq!(failure.detail["error_type"], "DocumentNotFound");
        assert!(
            failure.message.contains("list_documents"),
            "{}",
            failure.message
        );
        assert!(
            !failure.message.contains("list_runs"),
            "run guidance on a document refusal points the model at the wrong tool: {}",
            failure.message
        );
    }

    #[test]
    fn an_unconfigured_workspace_names_the_operator_knob() {
        let failure = document_failure(&DocumentError::WorkspaceUnconfigured);
        assert_eq!(failure.detail["code"], "backend");
        assert!(
            failure.message.contains("authoring.workspace_dir"),
            "{}",
            failure.message
        );
    }

    #[test]
    fn a_deploy_denial_keeps_its_wire_code_through_the_run_loop_wrapper() {
        let denied = RunLoopError::Authoring(AuthoringApiError::Wire(WireError::deploy_denied(
            "subject `assistant` is not authorized to deploy; \
             set x-aion-deploy: true for subject `assistant`",
        )));
        let failure = run_loop_failure(&denied);
        assert_eq!(failure.detail["code"], "deploy_denied");
        assert!(
            failure.message.contains("deploy grant"),
            "{}",
            failure.message
        );
    }

    /// A deploy that fails because the document does not exist must be the
    /// SAME refusal a read of that document gives: `DocumentNotFound`, with
    /// document guidance — the typed `RunLoopError::Document` variant exists
    /// so this cannot be flattened into a stringified revision fault.
    #[test]
    fn a_deploy_of_a_missing_document_is_a_document_refusal() {
        let failure = run_loop_failure(&RunLoopError::Document(DocumentError::NotFound(
            "ghost.awl".to_owned(),
        )));
        assert_eq!(failure.detail["code"], "not_found");
        assert_eq!(failure.detail["error_type"], "DocumentNotFound");
        assert!(
            failure.message.contains("list_documents"),
            "{}",
            failure.message
        );
    }

    #[test]
    fn a_revision_mismatch_tells_the_model_how_to_recover() {
        let mismatch = RunLoopError::RevisionMismatch {
            requested: "a".repeat(64),
            saved: "b".repeat(64),
        };
        let failure = run_loop_failure(&mismatch);
        assert_eq!(failure.detail["code"], "invalid_input");
        assert_eq!(failure.detail["error_type"], "RevisionMismatch");
        assert!(failure.message.contains("Re-read"), "{}", failure.message);
    }
}
