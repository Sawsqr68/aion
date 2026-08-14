//! Start/signal/query/cancel workflow operation handlers.

use aion_proto::{
    ProtoCancelRequest, ProtoCancelResponse, ProtoPauseRequest, ProtoPauseResponse,
    ProtoQueryRequest, ProtoQueryResponse, ProtoReopenRequest, ProtoReopenResponse,
    ProtoResumeRequest, ProtoResumeResponse, ProtoSignalRequest, ProtoSignalResponse,
    ProtoStartWorkflowRequest, ProtoStartWorkflowResponse, WireError, WireErrorCode,
    proto_query_response,
};
use tracing::{Instrument, info_span};

use super::error::{
    cancel_terminal_error, log_server_error, map_start_error, map_workflow_operation_error,
    signal_terminal_error,
};
use super::payload::{optional_payload, required_payload, required_workflow_id};
use super::runs::{resolve_run_id, terminal_status};
use crate::{
    CallerIdentity, NamespaceGuard, NamespaceMinter, NamespaceOperation, ServerError, ServerState,
    WorkflowTarget,
};

/// Handles a decoded start-workflow request.
///
/// The authorized namespace is recorded durably as the `aion.namespace` search
/// attribute in the same atomic append as the workflow's start event, so
/// ownership survives server restarts and is never tracked only in memory.
///
/// # Errors
///
/// Returns a stable [`WireError`] when the payload is missing or malformed, namespace scoping fails,
/// or the engine start call fails.
pub async fn start(
    guard: &NamespaceGuard,
    caller: &CallerIdentity,
    request: ProtoStartWorkflowRequest,
) -> Result<ProtoStartWorkflowResponse, WireError> {
    start_with_placement(guard, caller, request, None, None).await
}

/// Start a workflow, optionally with a `placement` id chosen by the routing edge
/// so the new execution lands on a locally-owned shard (R-1 unsteered-start
/// remint). `placement = None` is the default path: the engine mints the id, so
/// the single-node / non-clustered behaviour is unchanged.
///
/// `minter` is the minted-on-use safety net (Control-Plane Phase 1, S6): when
/// `Some`, the resolved-and-authorized namespace is durably minted (open) or
/// gated (closed) BEFORE the engine start, so a client that starts a workflow
/// before any worker registers still gets a durable namespace record. It is the
/// SAME [`NamespaceMinter`] policy the worker-registration seam (S5) applies, so
/// the two transports and the two mint choke-points can never diverge. `None`
/// disables the mint entirely (every unit test of the bare handler), leaving the
/// start path byte-identical.
///
/// The mint runs AFTER namespace authorization (`guard.scope`), so it is
/// auth-scoped by construction — it can only record a namespace the caller is
/// already permitted to start in. It does NOT change the immutable NSTQ
/// `aion.namespace` binding ([`start_search_attributes`]) or the start response
/// shape; the mint is purely additive.
///
/// # Errors
///
/// Identical to [`start`], plus a durable-store failure (a retryable `NotOwner`
/// fence surfaces as such) or a `closed`-policy namespace-denied error from the
/// minter, all mapped to a stable [`WireError`].
pub async fn start_with_placement(
    guard: &NamespaceGuard,
    caller: &CallerIdentity,
    request: ProtoStartWorkflowRequest,
    placement: Option<aion_core::WorkflowId>,
    minter: Option<&NamespaceMinter>,
) -> Result<ProtoStartWorkflowResponse, WireError> {
    let scoped = guard
        .scope(caller, &NamespaceOperation::start(&request))
        .await
        .map_err(|error| error.to_wire_error())?;
    let namespace = scoped.namespace().to_owned();
    // MINT-ON-START safety net (Phase 1 S6). Runs strictly AFTER the namespace
    // authorization above, so it can only ever mint a namespace the caller is
    // already authorized to start in — auth-scoped by construction. A `closed`
    // policy rejects an unknown namespace with the same namespace-denied error;
    // a quorum `NotOwner` fence propagates as the retryable wire code, never a
    // silent success. Shares the EXACT S5 policy via `NamespaceMinter`.
    if let Some(minter) = minter {
        minter
            .mint_or_gate(
                std::slice::from_ref(&namespace),
                aion_store::NamespaceOrigin::StartMint,
            )
            .await
            .map_err(|error| error.to_wire_error())?;
    }
    let input = required_payload(request.input.clone())?;
    // An empty task_queue means "not selected": fall back to the namespace's
    // default queue rather than recording an empty selection.
    let task_queue = request
        .task_queue
        .as_deref()
        .map(str::trim)
        .filter(|queue| !queue.is_empty());
    // #211: a PRESENT but blank display_name is refused, not reinterpreted.
    //
    // The server is the trust boundary — the SDKs are not the only callers, and
    // raw gRPC, the HTTP body, and the MCP `start_run` tool all reach here.
    // Trimming a blank one to "unnamed" would answer 200 to an operator who
    // believed they had named the run and say nothing about the name being
    // dropped. `None`/absent is how a caller says "unnamed"; a blank string is
    // a mistake, and the rename endpoint already refuses the same input, so
    // accepting it here would make the two surfaces disagree about what a blank
    // name means.
    let display_name = match request.display_name.as_deref().map(str::trim) {
        Some("") => {
            return Err(WireError::invalid_input(
                "display_name must not be blank; omit it to start the run unnamed",
            ));
        }
        other => other,
    };
    let span = info_span!(
        "engine_operation",
        operation = "start",
        namespace = %namespace,
        workflow_id = tracing::field::Empty,
        workflow_type = %request.workflow_type,
    );
    let search_attributes = start_search_attributes(&namespace, task_queue, display_name);
    let handle = async {
        scoped
            .engine()
            .map_err(|error| log_server_error("start", Some(&namespace), None, &error))?
            .start_workflow_with_id(
                &request.workflow_type,
                input,
                search_attributes,
                namespace.clone(),
                placement,
                // Steered-start shard derivation already happened at the edge
                // (which holds the concrete cluster store); the engine receives
                // the derived placement id, so no routing key is threaded here.
                None,
            )
            .await
            .map_err(|error| map_start_error(error, &request.workflow_type))
    }
    .instrument(span.clone())
    .await?;
    span.record("workflow_id", tracing::field::display(handle.workflow_id()));

    Ok(ProtoStartWorkflowResponse {
        workflow_id: Some(handle.workflow_id().clone().into()),
        run_id: Some(handle.run_id().clone().into()),
    })
}

/// Search attribute map stamping the authorized namespace — and, when the start
/// selected them, the default task queue and the operator-facing display name
/// (#211) — onto an execution.
///
/// All are recorded in the same atomic append as `WorkflowStarted`, so the
/// `(namespace, task_queue)` targeting selection and the label survive
/// restarts/failover and are never tracked only in memory. `task_queue` is
/// omitted when the start did not select one (the workflow falls back to the
/// namespace's default queue); `display_name` is omitted when the start did not
/// name it (it renders as its bare UUID until a rename records one).
///
/// The recorded `aion.display_name` carries no run id, so readers fold it over
/// the whole workflow history: the name stamped here is a per-WORKFLOW label a
/// continue-as-new successor inherits, not a per-run one.
pub(crate) fn start_search_attributes(
    namespace: &str,
    task_queue: Option<&str>,
    display_name: Option<&str>,
) -> std::collections::HashMap<String, aion_core::SearchAttributeValue> {
    let mut attributes = std::collections::HashMap::from([(
        crate::namespace::NAMESPACE_ATTRIBUTE.to_owned(),
        aion_core::SearchAttributeValue::String(namespace.to_owned()),
    )]);
    if let Some(task_queue) = task_queue {
        attributes.insert(
            crate::namespace::TASK_QUEUE_ATTRIBUTE.to_owned(),
            aion_core::SearchAttributeValue::String(task_queue.to_owned()),
        );
    }
    if let Some(display_name) = display_name {
        attributes.insert(
            crate::namespace::DISPLAY_NAME_ATTRIBUTE.to_owned(),
            aion_core::SearchAttributeValue::String(display_name.to_owned()),
        );
    }
    attributes
}

/// Handles a decoded signal request.
///
/// # Errors
///
/// Returns a stable [`WireError`] when IDs or payloads are missing or malformed, namespace scoping
/// fails, or the engine signal call fails.
pub async fn signal(
    guard: &NamespaceGuard,
    caller: &CallerIdentity,
    request: ProtoSignalRequest,
) -> Result<ProtoSignalResponse, WireError> {
    let workflow_id = required_workflow_id(request.workflow_id.clone())?;
    let target = WorkflowTarget::workflow(&workflow_id);
    let scoped = guard
        .scope(caller, &NamespaceOperation::signal(&request, target))
        .await
        .map_err(|error| error.to_wire_error())?;
    let namespace = scoped.namespace().to_owned();
    let engine = scoped.engine().map_err(|error| error.to_wire_error())?;
    let run_id = resolve_run_id(engine.as_ref(), &workflow_id, request.run_id.clone()).await?;
    let payload = required_payload(request.payload.clone())?;
    if let Some(status) = terminal_status(engine.as_ref(), &workflow_id).await? {
        return Err(signal_terminal_error(&workflow_id, status));
    }

    let signal_name = request.signal_name.clone();
    let span = info_span!(
        "engine_operation",
        operation = "signal",
        namespace = %namespace,
        workflow_id = %workflow_id,
        signal_name = %signal_name,
    );

    async {
        engine
            .signal(&workflow_id, &run_id, signal_name, payload)
            .await
            .map_err(|error| map_workflow_operation_error(error, &workflow_id))
    }
    .instrument(span)
    .await?;

    Ok(ProtoSignalResponse {})
}

/// Handles a decoded query request.
///
/// # Errors
///
/// Returns a stable [`WireError`] when IDs are missing or malformed, namespace scoping fails, or the
/// engine query call fails.
pub async fn query(
    guard: &NamespaceGuard,
    caller: &CallerIdentity,
    request: ProtoQueryRequest,
) -> Result<ProtoQueryResponse, WireError> {
    let workflow_id = required_workflow_id(request.workflow_id.clone())?;
    let target = WorkflowTarget::workflow(&workflow_id);
    let scoped = guard
        .scope(caller, &NamespaceOperation::query(&request, target))
        .await
        .map_err(|error| error.to_wire_error())?;
    let namespace = scoped.namespace().to_owned();
    let engine = scoped.engine().map_err(|error| error.to_wire_error())?;
    let run_id = resolve_run_id(engine.as_ref(), &workflow_id, request.run_id.clone()).await?;
    let query_name = request.query_name.clone();
    // An absent arguments field means the caller supplied none; the handler
    // still receives one well-formed document, the canonical JSON `null`.
    let arguments = optional_payload(request.arguments.clone())?;
    let span = info_span!(
        "engine_operation",
        operation = "query",
        namespace = %namespace,
        workflow_id = %workflow_id,
        query_name = %query_name,
    );

    let outcome = async {
        engine
            .query(&workflow_id, &run_id, query_name, arguments)
            .await
    }
    .instrument(span)
    .await;

    match outcome {
        Ok(result) => Ok(ProtoQueryResponse {
            outcome: Some(proto_query_response::Outcome::Result(result.into())),
        }),
        // Query-semantic failures (unknown query, timeout, not running,
        // handler failure, reply dropped) are the operation's documented
        // outcome and ride the QueryResponse.error oneof, which every SDK
        // query op parses. Namespace, not-found, and backend failures stay
        // transport-level errors, exactly as for every other operation.
        Err(error @ aion::EngineError::Query(_)) => Ok(ProtoQueryResponse {
            outcome: Some(proto_query_response::Outcome::Error(
                ServerError::from(error).to_wire_error().into(),
            )),
        }),
        Err(error) => Err(map_workflow_operation_error(error, &workflow_id)),
    }
}

/// Handles a decoded cancel request.
///
/// After the cancellation is durably recorded, asks every worker holding one of
/// this run's activities to stop it (#233). Cancelling records the fact, kills
/// the workflow's own VM process, and settles its outbox rows so nothing more is
/// dispatched — none of which reaches an activity a remote worker is ALREADY
/// executing. Without that second step a cancelled run keeps a machine busy
/// while the console truthfully reports `Cancelled`, so the operator stops
/// watching.
///
/// The ask comes AFTER the engine call returns, never before: a cancel pushed
/// ahead of the record could stop work for a cancellation that then fails to
/// persist. And it is only an ask — a failure to reach a worker is logged with
/// the worker and activity named, and does not fail the cancellation, because
/// the cancellation itself genuinely happened.
///
/// # Errors
///
/// Returns a stable [`WireError`] when IDs are missing or malformed, namespace scoping fails, or the
/// engine cancel call fails.
pub async fn cancel(
    state: &ServerState,
    guard: &NamespaceGuard,
    caller: &CallerIdentity,
    request: ProtoCancelRequest,
) -> Result<ProtoCancelResponse, WireError> {
    let workflow_id = required_workflow_id(request.workflow_id.clone())?;
    let target = WorkflowTarget::workflow(&workflow_id);
    let scoped = guard
        .scope(caller, &NamespaceOperation::cancel(&request, target))
        .await
        .map_err(|error| error.to_wire_error())?;
    let namespace = scoped.namespace().to_owned();
    let engine = scoped.engine().map_err(|error| error.to_wire_error())?;
    let run_id = resolve_run_id(engine.as_ref(), &workflow_id, request.run_id.clone()).await?;
    if let Some(status) = terminal_status(engine.as_ref(), &workflow_id).await? {
        return Err(cancel_terminal_error(&workflow_id, status));
    }

    let span = info_span!(
        "engine_operation",
        operation = "cancel",
        namespace = %namespace,
        workflow_id = %workflow_id,
    );

    async {
        engine
            .cancel(&workflow_id, &run_id, request.reason)
            .await
            .map_err(|error| map_workflow_operation_error(error, &workflow_id))
    }
    .instrument(span)
    .await?;

    // #233. The cancellation is durably recorded; now ask whoever is executing
    // this run's activities to stop. A failure to reach a worker is logged with
    // the worker and activity named (inside `cancel_in_flight_activities`) and
    // does NOT fail the response — the cancellation genuinely happened, and
    // reporting it as failed would be its own lie. A poisoned lock is the one
    // exception: it means the routing state could not be read at all, and
    // answering "cancelled" while silently asking nobody is exactly the defect
    // this closes.
    state
        .cancel_in_flight_activities(&workflow_id)
        .map_err(|error| WireError::new(WireErrorCode::Backend, error.to_string()))?;

    Ok(ProtoCancelResponse {})
}

/// Handles a decoded reopen request.
///
/// Resolves the run (latest when omitted) and calls
/// [`aion::Engine::reopen_workflow`], returning the reopened run id and its
/// projected Running status. UNLIKE [`cancel`] this does NOT pre-check terminal
/// status: the terminal-reopenable precondition is the engine's (AD-012) and the
/// handler only surfaces its typed [`aion::EngineError::InvalidState`] error.
///
/// # Errors
///
/// Returns a stable [`WireError`] when IDs are missing or malformed, namespace
/// scoping fails, or the engine reopen call fails — `invalid_state` for a
/// non-reopenable-terminal run, `not_found` for an absent workflow.
pub async fn reopen(
    guard: &NamespaceGuard,
    caller: &CallerIdentity,
    request: ProtoReopenRequest,
) -> Result<ProtoReopenResponse, WireError> {
    let workflow_id = required_workflow_id(request.workflow_id.clone())?;
    let target = WorkflowTarget::workflow(&workflow_id);
    let scoped = guard
        .scope(caller, &NamespaceOperation::reopen(&request, target))
        .await
        .map_err(|error| error.to_wire_error())?;
    let namespace = scoped.namespace().to_owned();
    let engine = scoped.engine().map_err(|error| error.to_wire_error())?;
    let run_id = resolve_run_id(engine.as_ref(), &workflow_id, request.run_id.clone()).await?;

    let span = info_span!(
        "engine_operation",
        operation = "reopen",
        namespace = %namespace,
        workflow_id = %workflow_id,
    );

    let handle = async {
        engine
            .reopen_workflow(&workflow_id, &run_id)
            .await
            .map_err(|error| map_workflow_operation_error(error, &workflow_id))
    }
    .instrument(span)
    .await?;

    Ok(ProtoReopenResponse {
        run_id: Some(handle.run_id().clone().into()),
        status: aion_proto::ProtoWorkflowStatus::from(handle.cached_status()) as i32,
    })
}

/// Handles a decoded pause request (#204).
///
/// Resolves the run (latest when omitted) and calls
/// [`aion::Engine::pause_workflow`], returning the run id and its projected
/// `Paused` status. The Running precondition is the engine's; the handler surfaces
/// its typed [`aion::EngineError::InvalidState`] error verbatim.
///
/// # Errors
///
/// Returns a stable [`WireError`] when IDs are missing/malformed, namespace
/// scoping fails, or the engine pause call fails — `invalid_state` when the run is
/// not Running, `not_found` for an absent workflow.
pub async fn pause(
    guard: &NamespaceGuard,
    caller: &CallerIdentity,
    request: ProtoPauseRequest,
) -> Result<ProtoPauseResponse, WireError> {
    let workflow_id = required_workflow_id(request.workflow_id.clone())?;
    let target = WorkflowTarget::workflow(&workflow_id);
    let scoped = guard
        .scope(
            caller,
            &NamespaceOperation::pause_workflow(&request, target),
        )
        .await
        .map_err(|error| error.to_wire_error())?;
    let namespace = scoped.namespace().to_owned();
    let engine = scoped.engine().map_err(|error| error.to_wire_error())?;
    let run_id = resolve_run_id(engine.as_ref(), &workflow_id, request.run_id.clone()).await?;
    let reason = if request.reason.is_empty() {
        None
    } else {
        Some(request.reason.clone())
    };

    let span = info_span!(
        "engine_operation",
        operation = "pause",
        namespace = %namespace,
        workflow_id = %workflow_id,
    );

    let handle = async {
        engine
            .pause_workflow(&workflow_id, &run_id, reason, None)
            .await
            .map_err(|error| map_workflow_operation_error(error, &workflow_id))
    }
    .instrument(span)
    .await?;

    Ok(ProtoPauseResponse {
        run_id: Some(handle.run_id().clone().into()),
        // Pause projects Paused regardless of the resident handle's cached status
        // (which stays Running under the dispatch-hold model).
        status: aion_proto::ProtoWorkflowStatus::Paused as i32,
    })
}

/// Handles a decoded rename request (#211).
///
/// Resolves the run (latest when omitted) and calls
/// [`aion::Engine::rename_workflow`], which records the new display name as a
/// durable `SearchAttributesUpdated` event — history keeps every name the run
/// has worn. The name is a LABEL over the UUID identity, never an address:
/// this request sets a name on an id-addressed run; nothing resolves a
/// workflow by name. Returns the run id and the name exactly as recorded
/// (trimmed).
///
/// # Errors
///
/// Returns a stable [`WireError`] when IDs are missing/malformed, the trimmed
/// name is empty (`invalid_input`), namespace scoping fails, or the engine
/// rename call fails — `not_found` for an absent workflow, and `invalid_state`
/// for a run that is neither terminal nor paused and is not resident on this
/// node (renaming it then would append behind its live writer, so the engine
/// refuses for the caller to retry rather than risk a second writer).
pub async fn rename(
    guard: &NamespaceGuard,
    caller: &CallerIdentity,
    request: aion_proto::ProtoRenameRequest,
) -> Result<aion_proto::ProtoRenameResponse, WireError> {
    let workflow_id = required_workflow_id(request.workflow_id.clone())?;
    // Validate the name BEFORE any engine work: an empty label is a caller
    // mistake, refused with nothing appended. Trim + empty-filter only (the
    // task_queue precedent) — no invented caps.
    let display_name = request.display_name.trim();
    if display_name.is_empty() {
        return Err(WireError::invalid_input(
            "display_name must not be empty; a rename records a non-empty label",
        ));
    }
    let target = WorkflowTarget::workflow(&workflow_id);
    let scoped = guard
        .scope(
            caller,
            &NamespaceOperation::rename_workflow(&request, target),
        )
        .await
        .map_err(|error| error.to_wire_error())?;
    let namespace = scoped.namespace().to_owned();
    let engine = scoped.engine().map_err(|error| error.to_wire_error())?;
    let run_id = resolve_run_id(engine.as_ref(), &workflow_id, request.run_id.clone()).await?;

    let span = info_span!(
        "engine_operation",
        operation = "rename",
        namespace = %namespace,
        workflow_id = %workflow_id,
    );

    let recorded = async {
        engine
            .rename_workflow(&workflow_id, &run_id, display_name)
            .await
            .map_err(|error| map_workflow_operation_error(error, &workflow_id))
    }
    .instrument(span)
    .await?;

    Ok(aion_proto::ProtoRenameResponse {
        run_id: Some(run_id.into()),
        display_name: recorded,
    })
}

/// Handles a decoded resume request (#204).
///
/// Resolves the run (latest when omitted) and calls
/// [`aion::Engine::resume_paused_workflow`], returning the run id and its
/// projected `Running` status.
///
/// # Errors
///
/// Returns a stable [`WireError`] when IDs are missing/malformed, namespace
/// scoping fails, or the engine resume call fails — `invalid_state` when the run
/// is not Paused, `not_found` for an absent workflow.
pub async fn resume(
    guard: &NamespaceGuard,
    caller: &CallerIdentity,
    request: ProtoResumeRequest,
) -> Result<ProtoResumeResponse, WireError> {
    let workflow_id = required_workflow_id(request.workflow_id.clone())?;
    let target = WorkflowTarget::workflow(&workflow_id);
    let scoped = guard
        .scope(
            caller,
            &NamespaceOperation::resume_workflow(&request, target),
        )
        .await
        .map_err(|error| error.to_wire_error())?;
    let namespace = scoped.namespace().to_owned();
    let engine = scoped.engine().map_err(|error| error.to_wire_error())?;
    let run_id = resolve_run_id(engine.as_ref(), &workflow_id, request.run_id.clone()).await?;

    let span = info_span!(
        "engine_operation",
        operation = "resume",
        namespace = %namespace,
        workflow_id = %workflow_id,
    );

    let handle = async {
        engine
            .resume_paused_workflow(&workflow_id, &run_id, None)
            .await
            .map_err(|error| map_workflow_operation_error(error, &workflow_id))
    }
    .instrument(span)
    .await?;

    Ok(ProtoResumeResponse {
        run_id: Some(handle.run_id().clone().into()),
        status: aion_proto::ProtoWorkflowStatus::Running as i32,
    })
}

#[cfg(test)]
mod tests {
    use aion_proto::{WireError, WireErrorCode};

    use super::super::test_support::{
        NAMESPACE, append_completed, append_failed, append_started, append_timed_out,
        assert_workflow_not_found, cancel_request, context, denied_guard, proto_payload,
        query_request, reopen_request, run_id, signal_request, workflow_id,
    };
    use super::*;

    #[tokio::test]
    async fn start_handler_scopes_then_invokes_engine_start()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = context().await?;
        let request = ProtoStartWorkflowRequest {
            namespace: NAMESPACE.to_owned(),
            workflow_type: "missing-workflow".to_owned(),
            input: Some(proto_payload()?),
            routing_key: None,
            task_queue: None,
            display_name: None,
        };

        let error = start(&context.guard, &context.caller, request).await;

        let error = error
            .err()
            .ok_or_else(|| WireError::backend("expected error"))?;
        assert_eq!(error.code, WireErrorCode::NotFound);
        assert_eq!(error.error_type.as_deref(), Some("WorkflowTypeNotFound"));
        assert_eq!(
            error.message,
            "workflow type missing-workflow is not registered"
        );
        Ok(())
    }

    #[test]
    fn start_records_namespace_only_when_no_task_queue_selected() {
        use crate::namespace::{NAMESPACE_ATTRIBUTE, TASK_QUEUE_ATTRIBUTE};

        let attributes = start_search_attributes("tenant-a", None, None);
        assert_eq!(
            attributes.get(NAMESPACE_ATTRIBUTE),
            Some(&aion_core::SearchAttributeValue::String(
                "tenant-a".to_owned()
            ))
        );
        // No selection => no task_queue attribute is recorded, so the workflow
        // falls back to the namespace's default queue.
        assert!(!attributes.contains_key(TASK_QUEUE_ATTRIBUTE));
    }

    #[test]
    fn start_records_selected_task_queue_durably_like_namespace() {
        use crate::namespace::{NAMESPACE_ATTRIBUTE, TASK_QUEUE_ATTRIBUTE};

        let attributes = start_search_attributes("tenant-a", Some("gpu"), None);
        assert_eq!(
            attributes.get(NAMESPACE_ATTRIBUTE),
            Some(&aion_core::SearchAttributeValue::String(
                "tenant-a".to_owned()
            ))
        );
        // The selected task_queue rides the SAME search-attribute map as the
        // namespace, so it lands in the same atomic WorkflowStarted append and
        // survives replay/failover exactly as the namespace does.
        assert_eq!(
            attributes.get(TASK_QUEUE_ATTRIBUTE),
            Some(&aion_core::SearchAttributeValue::String("gpu".to_owned()))
        );
    }

    /// #211: an unnamed start records no display-name attribute, so the run
    /// renders as its bare UUID.
    #[test]
    fn start_records_no_display_name_when_unnamed() {
        use crate::namespace::{DISPLAY_NAME_ATTRIBUTE, NAMESPACE_ATTRIBUTE};

        let attributes = start_search_attributes("tenant-a", None, None);
        assert_eq!(
            attributes.get(NAMESPACE_ATTRIBUTE),
            Some(&aion_core::SearchAttributeValue::String(
                "tenant-a".to_owned()
            ))
        );
        // No name => no display_name attribute is recorded; the unnamed run
        // renders as its bare UUID.
        assert!(!attributes.contains_key(DISPLAY_NAME_ATTRIBUTE));
    }

    /// #211: a named start records `aion.display_name` in the SAME
    /// search-attribute map as the namespace, so the label lands in the same
    /// atomic `WorkflowStarted` append and survives replay/failover.
    #[test]
    fn start_records_display_name_durably_like_namespace() {
        use crate::namespace::{DISPLAY_NAME_ATTRIBUTE, NAMESPACE_ATTRIBUTE};

        let attributes = start_search_attributes("tenant-a", None, Some("Nightly settlement"));
        assert_eq!(
            attributes.get(NAMESPACE_ATTRIBUTE),
            Some(&aion_core::SearchAttributeValue::String(
                "tenant-a".to_owned()
            ))
        );
        assert_eq!(
            attributes.get(DISPLAY_NAME_ATTRIBUTE),
            Some(&aion_core::SearchAttributeValue::String(
                "Nightly settlement".to_owned()
            ))
        );
    }

    /// #211: a PRESENT but blank `display_name` on START is refused rather than
    /// reinterpreted as "unnamed" — the server is the trust boundary, and a
    /// caller who believed they had named the run must not get a 200 and a bare
    /// UUID. Absent stays the way to say "unnamed".
    #[tokio::test]
    async fn start_refuses_a_blank_display_name_rather_than_starting_unnamed()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = context().await?;
        for blank in ["", "   ", "\t\n "] {
            let request = aion_proto::ProtoStartWorkflowRequest {
                namespace: NAMESPACE.to_owned(),
                workflow_type: "checkout".to_owned(),
                input: Some(proto_payload()?),
                routing_key: None,
                task_queue: None,
                display_name: Some(blank.to_owned()),
            };

            let error = start(&context.guard, &context.caller, request)
                .await
                .err()
                .ok_or_else(|| WireError::backend("expected a blank-name refusal"))?;
            assert_eq!(error.code, WireErrorCode::InvalidInput, "blank {blank:?}");
        }

        // ABSENT is still how a caller says "unnamed", and it is NOT refused —
        // without this arm the assertion above would also pass if the handler
        // had started refusing every start.
        let request = aion_proto::ProtoStartWorkflowRequest {
            namespace: NAMESPACE.to_owned(),
            workflow_type: "checkout".to_owned(),
            input: Some(proto_payload()?),
            routing_key: None,
            task_queue: None,
            display_name: None,
        };
        let error = start(&context.guard, &context.caller, request)
            .await
            .err()
            .ok_or_else(|| WireError::backend("expected an error"))?;
        assert_ne!(
            error.code,
            WireErrorCode::InvalidInput,
            "an absent name must not be refused as invalid input"
        );
        Ok(())
    }

    /// #211: an empty rename is refused as `invalid_input` BEFORE any scoping
    /// or engine work — a rename records a non-empty label or nothing.
    #[tokio::test]
    async fn rename_handler_rejects_blank_display_name() -> Result<(), Box<dyn std::error::Error>> {
        let context = context().await?;
        let request = aion_proto::ProtoRenameRequest {
            namespace: NAMESPACE.to_owned(),
            workflow_id: Some(workflow_id().into()),
            run_id: Some(run_id().into()),
            display_name: "   ".to_owned(),
        };

        let error = rename(&context.guard, &context.caller, request).await;

        let error = error
            .err()
            .ok_or_else(|| WireError::backend("expected error"))?;
        assert_eq!(error.code, WireErrorCode::InvalidInput);
        Ok(())
    }

    /// #211: rename scopes the namespace then resolves the run like
    /// signal/cancel — an absent workflow is `not_found`, and nothing is
    /// appended.
    #[tokio::test]
    async fn rename_handler_scopes_then_reports_absent_workflow()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = context().await?;
        context.ownership.record(workflow_id(), NAMESPACE)?;
        let request = aion_proto::ProtoRenameRequest {
            namespace: NAMESPACE.to_owned(),
            workflow_id: Some(workflow_id().into()),
            run_id: Some(run_id().into()),
            display_name: "Nightly settlement".to_owned(),
        };

        let error = rename(&context.guard, &context.caller, request).await;

        let error = error
            .err()
            .ok_or_else(|| WireError::backend("expected error"))?;
        assert_eq!(error.code, WireErrorCode::NotFound);
        assert_eq!(error.error_type.as_deref(), Some("WorkflowNotFound"));
        Ok(())
    }

    #[tokio::test]
    async fn signal_handler_scopes_then_invokes_engine_signal()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = context().await?;
        context.ownership.record(workflow_id(), NAMESPACE)?;

        let error = signal(&context.guard, &context.caller, signal_request()?).await;

        let error = error
            .err()
            .ok_or_else(|| WireError::backend("expected error"))?;
        assert_eq!(error.code, WireErrorCode::NotFound);
        assert_eq!(error.error_type.as_deref(), Some("WorkflowNotFound"));
        assert_eq!(
            error.message,
            format!("workflow {} not found", workflow_id())
        );
        Ok(())
    }

    #[tokio::test]
    async fn query_handler_scopes_then_invokes_engine_query()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = context().await?;
        context.ownership.record(workflow_id(), NAMESPACE)?;

        let error = query(&context.guard, &context.caller, query_request()).await;

        let error = error
            .err()
            .ok_or_else(|| WireError::backend("expected error"))?;
        assert_eq!(error.code, WireErrorCode::NotFound);
        assert_eq!(error.error_type.as_deref(), Some("WorkflowNotFound"));
        assert_eq!(
            error.message,
            format!("workflow {} not found", workflow_id())
        );
        Ok(())
    }

    #[tokio::test]
    async fn query_handler_returns_not_running_outcome_for_terminal_workflow()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = context().await?;
        context.ownership.record(workflow_id(), NAMESPACE)?;
        append_completed(context.store.as_ref()).await?;
        // Resolve the latest run from the chain: the completed history was
        // recorded for the started run, not the fixed test run id.
        let mut request = query_request();
        request.run_id = None;

        let response = query(&context.guard, &context.caller, request).await?;

        // A terminal workflow is a query-semantic outcome: the transport call
        // succeeds and the typed error rides the QueryResponse.error oneof.
        let Some(proto_query_response::Outcome::Error(error)) = response.outcome else {
            return Err("expected a QueryResponse.error outcome".into());
        };
        let error = WireError::try_from(error)?;
        assert_eq!(error.code, WireErrorCode::NotRunning);
        assert_eq!(error.error_type.as_deref(), Some("QueryNotRunning"));
        Ok(())
    }

    #[tokio::test]
    async fn query_handler_keeps_non_resident_non_terminal_workflow_as_transport_not_found()
    -> Result<(), Box<dyn std::error::Error>> {
        // A recorded but non-resident, non-terminal workflow misses the live
        // registry and has no terminal history, so Engine::query reports
        // WorkflowNotFound — a transport-level error, never an outcome.error.
        let context = context().await?;
        context.ownership.record(workflow_id(), NAMESPACE)?;
        append_started(context.store.as_ref()).await?;
        let mut request = query_request();
        request.run_id = None;

        let error = query(&context.guard, &context.caller, request).await;

        let error = error
            .err()
            .ok_or_else(|| WireError::backend("expected error"))?;
        assert_eq!(error.code, WireErrorCode::NotFound);
        assert_eq!(error.error_type.as_deref(), Some("WorkflowNotFound"));
        Ok(())
    }

    #[tokio::test]
    async fn cancel_handler_scopes_then_invokes_engine_cancel()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = context().await?;
        context.ownership.record(workflow_id(), NAMESPACE)?;

        let error = cancel(
            &context.state,
            &context.guard,
            &context.caller,
            cancel_request(),
        )
        .await;

        let error = error
            .err()
            .ok_or_else(|| WireError::backend("expected error"))?;
        assert_eq!(error.code, WireErrorCode::NotFound);
        assert_eq!(error.error_type.as_deref(), Some("WorkflowNotFound"));
        assert_eq!(
            error.message,
            format!("workflow {} not found", workflow_id())
        );
        Ok(())
    }

    #[tokio::test]
    async fn reopen_handler_maps_missing_workflow_to_not_found()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = context().await?;
        context.ownership.record(workflow_id(), NAMESPACE)?;

        let error = reopen(&context.guard, &context.caller, reopen_request()).await;

        let error = error
            .err()
            .ok_or_else(|| WireError::backend("expected error"))?;
        assert_eq!(error.code, WireErrorCode::NotFound);
        assert_eq!(error.error_type.as_deref(), Some("WorkflowNotFound"));
        Ok(())
    }

    #[tokio::test]
    async fn reopen_handler_rejects_completed_workflow_as_invalid_state()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = context().await?;
        context.ownership.record(workflow_id(), NAMESPACE)?;
        append_completed(context.store.as_ref()).await?;
        let mut request = reopen_request();
        request.run_id = None;

        let error = reopen(&context.guard, &context.caller, request).await;

        let error = error
            .err()
            .ok_or_else(|| WireError::backend("expected error"))?;
        assert_eq!(error.code, WireErrorCode::InvalidState);
        assert_eq!(error.error_type.as_deref(), Some("InvalidState"));
        Ok(())
    }

    #[tokio::test]
    async fn reopen_handler_rejects_timed_out_workflow_as_invalid_state()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = context().await?;
        context.ownership.record(workflow_id(), NAMESPACE)?;
        append_timed_out(context.store.as_ref()).await?;
        let mut request = reopen_request();
        request.run_id = None;

        let error = reopen(&context.guard, &context.caller, request).await;

        let error = error
            .err()
            .ok_or_else(|| WireError::backend("expected error"))?;
        // TimedOut is a non-reopenable terminal (only Failed and Cancelled reopen).
        assert_eq!(error.code, WireErrorCode::InvalidState);
        assert_eq!(error.error_type.as_deref(), Some("InvalidState"));
        Ok(())
    }

    #[tokio::test]
    async fn reopen_handler_maps_omitted_run_missing_workflow_to_not_found()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = context().await?;
        context.ownership.record(workflow_id(), NAMESPACE)?;
        let mut request = reopen_request();
        request.run_id = None;

        let error = reopen(&context.guard, &context.caller, request).await;

        assert_workflow_not_found(error)?;
        Ok(())
    }

    /// A caller WITHOUT a grant for the target namespace is denied reopen with
    /// the namespace-denied wire code — mirroring the signal denial test.
    #[tokio::test]
    async fn denied_reopen_is_namespace_denied_before_engine_check()
    -> Result<(), Box<dyn std::error::Error>> {
        let (guard, caller) = denied_guard();
        let request = ProtoReopenRequest {
            namespace: NAMESPACE.to_owned(),
            workflow_id: Some(workflow_id().into()),
            run_id: Some(run_id().into()),
        };

        let error = reopen(&guard, &caller, request).await;

        assert_eq!(
            error.err().map(|error| error.code),
            Some(WireErrorCode::NamespaceDenied)
        );
        Ok(())
    }

    #[tokio::test]
    async fn signal_handler_rejects_completed_workflow() -> Result<(), Box<dyn std::error::Error>> {
        let context = context().await?;
        context.ownership.record(workflow_id(), NAMESPACE)?;
        append_completed(context.store.as_ref()).await?;

        let error = signal(&context.guard, &context.caller, signal_request()?).await;

        let error = error
            .err()
            .ok_or_else(|| WireError::backend("expected error"))?;
        assert_eq!(error.code, WireErrorCode::NotRunning);
        assert_eq!(error.error_type.as_deref(), Some("WorkflowTerminal"));
        assert_eq!(
            error.message,
            format!(
                "workflow {} has already reached terminal state Completed",
                workflow_id()
            )
        );
        Ok(())
    }

    #[tokio::test]
    async fn signal_handler_rejects_failed_workflow() -> Result<(), Box<dyn std::error::Error>> {
        let context = context().await?;
        context.ownership.record(workflow_id(), NAMESPACE)?;
        append_failed(context.store.as_ref()).await?;

        let error = signal(&context.guard, &context.caller, signal_request()?).await;

        let error = error
            .err()
            .ok_or_else(|| WireError::backend("expected error"))?;
        assert_eq!(error.code, WireErrorCode::NotRunning);
        assert_eq!(error.error_type.as_deref(), Some("WorkflowTerminal"));
        assert_eq!(
            error.message,
            format!(
                "workflow {} has already reached terminal state Failed",
                workflow_id()
            )
        );
        Ok(())
    }

    #[tokio::test]
    async fn cancel_handler_rejects_completed_workflow() -> Result<(), Box<dyn std::error::Error>> {
        let context = context().await?;
        context.ownership.record(workflow_id(), NAMESPACE)?;
        append_completed(context.store.as_ref()).await?;

        let error = cancel(
            &context.state,
            &context.guard,
            &context.caller,
            cancel_request(),
        )
        .await;

        let error = error
            .err()
            .ok_or_else(|| WireError::backend("expected error"))?;
        assert_eq!(error.code, WireErrorCode::NotRunning);
        assert_eq!(error.error_type.as_deref(), Some("WorkflowTerminal"));
        assert_eq!(
            error.message,
            format!(
                "workflow {} has already completed with status Completed",
                workflow_id()
            )
        );
        assert!(!error.message.contains("process 0 is not live"));
        Ok(())
    }

    #[tokio::test]
    async fn cancel_handler_rejects_failed_workflow() -> Result<(), Box<dyn std::error::Error>> {
        let context = context().await?;
        context.ownership.record(workflow_id(), NAMESPACE)?;
        append_failed(context.store.as_ref()).await?;

        let error = cancel(
            &context.state,
            &context.guard,
            &context.caller,
            cancel_request(),
        )
        .await;

        let error = error
            .err()
            .ok_or_else(|| WireError::backend("expected error"))?;
        assert_eq!(error.code, WireErrorCode::NotRunning);
        assert_eq!(error.error_type.as_deref(), Some("WorkflowTerminal"));
        assert_eq!(
            error.message,
            format!(
                "workflow {} has already completed with status Failed",
                workflow_id()
            )
        );
        assert!(!error.message.contains("process 0 is not live"));
        Ok(())
    }

    #[tokio::test]
    async fn signal_handler_maps_omitted_run_missing_workflow_to_not_found()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = context().await?;
        context.ownership.record(workflow_id(), NAMESPACE)?;
        let mut request = signal_request()?;
        request.run_id = None;

        let error = signal(&context.guard, &context.caller, request).await;

        assert_workflow_not_found(error)?;
        Ok(())
    }

    #[tokio::test]
    async fn query_handler_maps_omitted_run_missing_workflow_to_not_found()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = context().await?;
        context.ownership.record(workflow_id(), NAMESPACE)?;
        let mut request = query_request();
        request.run_id = None;

        let error = query(&context.guard, &context.caller, request).await;

        assert_workflow_not_found(error)?;
        Ok(())
    }

    #[tokio::test]
    async fn cancel_handler_maps_omitted_run_missing_workflow_to_not_found()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = context().await?;
        context.ownership.record(workflow_id(), NAMESPACE)?;
        let mut request = cancel_request();
        request.run_id = None;

        let error = cancel(&context.state, &context.guard, &context.caller, request).await;

        assert_workflow_not_found(error)?;
        Ok(())
    }

    #[tokio::test]
    async fn denied_start_does_not_decode_missing_payload_before_namespace_check()
    -> Result<(), Box<dyn std::error::Error>> {
        let (guard, caller) = denied_guard();
        let request = ProtoStartWorkflowRequest {
            namespace: NAMESPACE.to_owned(),
            workflow_type: "fixture".to_owned(),
            input: None,
            routing_key: None,
            task_queue: None,
            display_name: None,
        };

        let error = start(&guard, &caller, request).await;

        assert_eq!(
            error.err().map(|error| error.code),
            Some(WireErrorCode::NamespaceDenied)
        );
        Ok(())
    }

    #[tokio::test]
    async fn denied_signal_does_not_decode_missing_payload_before_namespace_check()
    -> Result<(), Box<dyn std::error::Error>> {
        let (guard, caller) = denied_guard();
        let request = ProtoSignalRequest {
            namespace: NAMESPACE.to_owned(),
            workflow_id: Some(workflow_id().into()),
            run_id: Some(run_id().into()),
            signal_name: "poke".to_owned(),
            payload: None,
        };

        let error = signal(&guard, &caller, request).await;

        assert_eq!(
            error.err().map(|error| error.code),
            Some(WireErrorCode::NamespaceDenied)
        );
        Ok(())
    }

    // ---- Minted-on-use START safety net (Control-Plane Phase 1, S6) --------

    use std::sync::Arc;

    use aion_store::{NamespaceOrigin, NamespaceStore};

    use crate::config::AutoCreate;

    fn namespace_store() -> Arc<dyn NamespaceStore> {
        Arc::new(aion_store::InMemoryStore::default())
    }

    fn minter(store: &Arc<dyn NamespaceStore>, policy: AutoCreate) -> NamespaceMinter {
        NamespaceMinter::new(Arc::clone(store), policy)
    }

    fn fresh_start_request() -> Result<ProtoStartWorkflowRequest, aion_core::PayloadError> {
        Ok(ProtoStartWorkflowRequest {
            namespace: NAMESPACE.to_owned(),
            workflow_type: "missing-workflow".to_owned(),
            input: Some(proto_payload()?),
            routing_key: None,
            task_queue: None,
            display_name: None,
        })
    }

    /// A start into a never-before-seen namespace (no worker registered) mints a
    /// durable record under the open policy, even though the start itself fails
    /// at the engine (no such workflow type) — the mint runs strictly after
    /// authorization and before the engine call. A second start is idempotent:
    /// no duplicate row.
    #[tokio::test]
    async fn open_start_mints_durable_record_and_is_idempotent()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = context().await?;
        let store = namespace_store();
        let minter = minter(&store, AutoCreate::Open);

        // No worker ever registered, so the namespace has no row yet.
        assert!(store.get_namespace(NAMESPACE).await?.is_none());

        // The start fails at the engine (unknown workflow type) but the mint
        // already ran: a durable record exists afterwards.
        let first = start_with_placement(
            &context.guard,
            &context.caller,
            fresh_start_request()?,
            None,
            Some(&minter),
        )
        .await;
        assert!(
            first.is_err(),
            "the fixture start has no registered workflow type"
        );
        let record = store
            .get_namespace(NAMESPACE)
            .await?
            .ok_or("expected a durable record minted by the start")?;
        assert_eq!(record.name, NAMESPACE);
        assert_eq!(record.origin, NamespaceOrigin::StartMint);

        // A second start is idempotent: still exactly one row, no duplicate.
        let _second = start_with_placement(
            &context.guard,
            &context.caller,
            fresh_start_request()?,
            None,
            Some(&minter),
        )
        .await;
        let all = store.list_namespaces().await?;
        assert_eq!(
            all.iter().filter(|r| r.name == NAMESPACE).count(),
            1,
            "a second start must not create a duplicate namespace row"
        );
        Ok(())
    }

    /// Under the closed policy a start into an unknown namespace is rejected with
    /// the same namespace-denied error the worker-registration seam uses, and the
    /// namespace is not created.
    #[tokio::test]
    async fn closed_start_rejects_unknown_namespace_and_does_not_create_it()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = context().await?;
        let store = namespace_store();
        let minter = minter(&store, AutoCreate::Closed);

        let denied = start_with_placement(
            &context.guard,
            &context.caller,
            fresh_start_request()?,
            None,
            Some(&minter),
        )
        .await;

        let error = denied
            .err()
            .ok_or_else(|| WireError::backend("expected a namespace-denied error"))?;
        assert_eq!(error.code, WireErrorCode::NamespaceDenied);
        assert!(
            store.get_namespace(NAMESPACE).await?.is_none(),
            "closed policy must NOT create the namespace it rejected"
        );
        Ok(())
    }

    /// Under the closed policy a start into a namespace that already has a
    /// durable record (the `POST /namespaces` escape hatch's effect) is admitted
    /// — it proceeds to the engine exactly as the open path does.
    #[tokio::test]
    async fn closed_start_admits_a_known_namespace() -> Result<(), Box<dyn std::error::Error>> {
        let context = context().await?;
        let store = namespace_store();
        store
            .register_namespace(NAMESPACE, NamespaceOrigin::Explicit)
            .await?;
        let minter = minter(&store, AutoCreate::Closed);

        // The known namespace passes the gate, so the start reaches the engine
        // and fails only on the unknown workflow type — never on the namespace.
        let error = start_with_placement(
            &context.guard,
            &context.caller,
            fresh_start_request()?,
            None,
            Some(&minter),
        )
        .await
        .err()
        .ok_or_else(|| WireError::backend("expected the fixture workflow-type miss"))?;
        assert_eq!(
            error.code,
            WireErrorCode::NotFound,
            "a known namespace must pass the gate and fail only at the engine"
        );
        assert_eq!(error.error_type.as_deref(), Some("WorkflowTypeNotFound"));
        Ok(())
    }

    /// With no minter installed the start path is byte-identical to before S6:
    /// the namespace is never touched and the start reaches the engine as usual.
    #[tokio::test]
    async fn no_minter_leaves_start_untouched() -> Result<(), Box<dyn std::error::Error>> {
        let context = context().await?;
        let error = start_with_placement(
            &context.guard,
            &context.caller,
            fresh_start_request()?,
            None,
            None,
        )
        .await
        .err()
        .ok_or_else(|| WireError::backend("expected the fixture workflow-type miss"))?;
        assert_eq!(error.code, WireErrorCode::NotFound);
        assert_eq!(error.error_type.as_deref(), Some("WorkflowTypeNotFound"));
        Ok(())
    }
}
