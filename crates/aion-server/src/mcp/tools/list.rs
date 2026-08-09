//! `list_runs`: filtered enumeration within one namespace.

use aion_mcp::tools::service::{ToolCall, ToolFailure, ToolOutcome};
use aion_proto::{ProtoListWorkflowsRequest, WireError};
use aion_store::visibility::ListWorkflowsFilter;
use serde_json::json;

use crate::mcp::args::{optional_str, optional_u32, required_str};
use crate::{CallerIdentity, ServerState, api::handlers};

use super::errors::tool_failure;

/// Run `list_runs`.
///
/// The filter is narrowed to the caller's authorized namespace by the shared
/// visibility handler, exactly as the HTTP and gRPC list surfaces are. There is
/// no second scoping path here to drift from that one.
///
/// # Errors
///
/// [`ToolFailure`] when the arguments are malformed, the caller does not hold
/// the namespace, or the visibility store fails.
pub(crate) async fn list_runs(
    state: &ServerState,
    caller: &CallerIdentity,
    call: &ToolCall,
) -> Result<ToolOutcome, ToolFailure> {
    let namespace = required_str(call, "namespace")?;
    let filter = ListWorkflowsFilter {
        workflow_type: optional_str(call, "workflow_type"),
        status: parse_status(call)?,
        started_after: parse_instant(call, "started_after")?,
        started_before: parse_instant(call, "started_before")?,
        closed_after: None,
        closed_before: None,
        search_attributes: Vec::new(),
        limit: optional_u32(call, "limit")?,
        offset: optional_u32(call, "offset")?,
    };
    let encoded = aion_proto::encode_core_value(namespace.clone(), None, &filter)
        .map_err(|error| tool_failure(&error))?;
    let response = handlers::list(
        state.namespace_guard(),
        caller,
        ProtoListWorkflowsRequest {
            namespace: namespace.clone(),
            filter: Some(encoded),
        },
    )
    .await
    .map_err(|error| tool_failure(&error))?;

    let runs = response
        .summaries
        .iter()
        .map(aion_proto::decode_core_value::<aion_core::WorkflowSummary>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| tool_failure(&error))?;
    let runs = serde_json::to_value(&runs).map_err(|error| {
        tool_failure(&WireError::backend(format!(
            "run summaries could not be encoded: {error}"
        )))
    })?;
    let count = runs.as_array().map_or(0, Vec::len);

    Ok(ToolOutcome {
        summary: format!("{count} run(s) in namespace {namespace}"),
        structured: json!({
            "namespace": namespace,
            "runs": runs,
            "count": count,
        }),
    })
}

/// Parse the optional status filter.
///
/// An unrecognised status is refused with the accepted set named, rather than
/// dropped: a silently ignored filter returns rows the caller believes were
/// filtered, which is worse than an error.
fn parse_status(call: &ToolCall) -> Result<Option<aion_core::WorkflowStatus>, ToolFailure> {
    let Some(raw) = optional_str(call, "status") else {
        return Ok(None);
    };
    serde_json::from_value(json!(raw))
        .map(Some)
        .map_err(|_error| {
            ToolFailure::new(
                format!(
                    "`status` must be one of Running, Completed, Failed, Cancelled, TimedOut, \
                     ContinuedAsNew, Paused — not `{raw}`"
                ),
                json!({ "code": "invalid_argument", "argument": "status", "value": raw }),
            )
        })
}

/// Parse an optional RFC 3339 instant.
fn parse_instant(
    call: &ToolCall,
    key: &str,
) -> Result<Option<chrono::DateTime<chrono::Utc>>, ToolFailure> {
    let Some(raw) = optional_str(call, key) else {
        return Ok(None);
    };
    chrono::DateTime::parse_from_rfc3339(&raw)
        .map(|instant| Some(instant.with_timezone(&chrono::Utc)))
        .map_err(|error| {
            ToolFailure::new(
                format!("`{key}` must be an RFC 3339 instant: {error}"),
                json!({ "code": "invalid_argument", "argument": key, "value": raw }),
            )
        })
}

#[cfg(test)]
mod tests {
    use aion_core::WorkflowStatus;
    use aion_mcp::tools::service::ToolCall;
    use serde_json::{Map, Value, json};

    use super::{parse_instant, parse_status};

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn call(arguments: &Value) -> Result<ToolCall, serde_json::Error> {
        Ok(ToolCall {
            name: "list_runs".to_owned(),
            arguments: serde_json::from_value::<Map<String, Value>>(arguments.clone())?,
        })
    }

    #[test]
    fn a_recognised_status_parses_and_an_unrecognised_one_is_refused() -> TestResult {
        assert_eq!(
            parse_status(&call(&json!({ "status": "Running" }))?)?,
            Some(WorkflowStatus::Running)
        );
        assert_eq!(parse_status(&call(&json!({}))?)?, None);
        let failure = parse_status(&call(&json!({ "status": "running" }))?).err();
        assert!(
            failure.is_some_and(|failure| failure.message.contains("must be one of")),
            "a lowercase status is not the projection's spelling and must be refused, \
             not silently dropped"
        );
        Ok(())
    }

    #[test]
    fn an_instant_must_be_rfc_3339() -> TestResult {
        assert!(
            parse_instant(
                &call(&json!({ "started_after": "2026-08-09T00:00:00Z" }))?,
                "started_after"
            )?
            .is_some()
        );
        assert!(
            parse_instant(
                &call(&json!({ "started_after": "yesterday" }))?,
                "started_after"
            )
            .is_err()
        );
        Ok(())
    }
}
