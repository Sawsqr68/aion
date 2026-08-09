//! Wire-error to tool-failure mapping.
//!
//! Every server-side refusal reaches the model as a `CallToolResult` with
//! `isError: true` rather than as a JSON-RPC error, so the model can read what
//! went wrong and correct itself. The stable `WireError` code travels in the
//! failure detail so a programmatic caller can branch without parsing prose,
//! and the message is rewritten into something a model can act on where the
//! bare wire message would not be.

use aion_mcp::tools::service::ToolFailure;
use aion_proto::{WireError, WireErrorCode};
use serde_json::json;

/// Map a wire error onto a tool-level failure.
pub(crate) fn tool_failure(error: &WireError) -> ToolFailure {
    let code = wire_code_label(error.code);
    let guidance = guidance_for(error.code);
    let message = match guidance {
        Some(guidance) => format!("{} — {guidance}", error.message),
        None => error.message.clone(),
    };
    ToolFailure::new(
        message,
        json!({
            "code": code,
            "message": error.message,
            "error_type": error.error_type,
        }),
    )
}

/// The stable, client-branchable label for a wire code.
fn wire_code_label(code: WireErrorCode) -> &'static str {
    match code {
        WireErrorCode::NotFound => "not_found",
        WireErrorCode::NamespaceDenied => "namespace_denied",
        WireErrorCode::SequenceConflict => "sequence_conflict",
        WireErrorCode::UnknownQuery => "unknown_query",
        WireErrorCode::QueryTimeout => "query_timeout",
        WireErrorCode::NotRunning => "not_running",
        WireErrorCode::Lagged => "lagged",
        WireErrorCode::InvalidInput => "invalid_input",
        WireErrorCode::Backend => "backend",
        WireErrorCode::QueryFailed => "query_failed",
        WireErrorCode::DeployDenied => "deploy_denied",
        WireErrorCode::VersionPinned => "version_pinned",
        WireErrorCode::NotOwner => "not_owner",
        WireErrorCode::InvalidState => "invalid_state",
    }
}

/// What the model should do about this class of refusal, when there is
/// something useful to say. Silence is the honest answer for the rest.
fn guidance_for(code: WireErrorCode) -> Option<&'static str> {
    match code {
        WireErrorCode::NotFound | WireErrorCode::NamespaceDenied => Some(
            "the workflow does not exist in that namespace, or you do not hold it. Both answer \
             the same way on purpose. Use list_runs to find a real id; do not retry with a \
             guessed one",
        ),
        WireErrorCode::NotRunning => Some(
            "the run has already reached a terminal status, so it cannot be signalled, \
             queried, or cancelled. Call describe_run to see which terminal it reached",
        ),
        WireErrorCode::UnknownQuery => Some(
            "the workflow registered no query by that name. Query names come from the \
             workflow's own code and cannot be invented",
        ),
        WireErrorCode::QueryTimeout => Some(
            "the workflow did not answer within its configured window; it may be busy or \
             parked. describe_run will say whether anything can currently serve it",
        ),
        WireErrorCode::NotOwner | WireErrorCode::SequenceConflict => Some(
            "this is a retryable routing or write race, not a mistake in your request. Retry \
             the same call",
        ),
        WireErrorCode::InvalidInput
        | WireErrorCode::Lagged
        | WireErrorCode::Backend
        | WireErrorCode::QueryFailed
        | WireErrorCode::DeployDenied
        | WireErrorCode::VersionPinned
        | WireErrorCode::InvalidState => None,
    }
}

#[cfg(test)]
mod tests {
    use aion_proto::{WireError, WireErrorCode};

    use super::{tool_failure, wire_code_label};

    #[test]
    fn a_not_found_carries_its_code_and_tells_the_model_not_to_guess() {
        let failure = tool_failure(&WireError::not_found("workflow x was not found"));
        assert_eq!(failure.detail["code"], "not_found");
        assert!(failure.message.contains("workflow x was not found"));
        assert!(failure.message.contains("do not retry with a guessed one"));
    }

    #[test]
    fn a_code_without_guidance_keeps_the_bare_message() {
        let failure = tool_failure(&WireError::invalid_input("attempt must be positive"));
        assert_eq!(failure.message, "attempt must be positive");
        assert_eq!(failure.detail["code"], "invalid_input");
    }

    #[test]
    fn every_wire_code_has_a_distinct_label() {
        let codes = [
            WireErrorCode::NotFound,
            WireErrorCode::NamespaceDenied,
            WireErrorCode::SequenceConflict,
            WireErrorCode::UnknownQuery,
            WireErrorCode::QueryTimeout,
            WireErrorCode::NotRunning,
            WireErrorCode::Lagged,
            WireErrorCode::InvalidInput,
            WireErrorCode::Backend,
            WireErrorCode::QueryFailed,
            WireErrorCode::DeployDenied,
            WireErrorCode::VersionPinned,
            WireErrorCode::NotOwner,
            WireErrorCode::InvalidState,
        ];
        let mut labels: Vec<&str> = codes.into_iter().map(wire_code_label).collect();
        let total = labels.len();
        labels.sort_unstable();
        labels.dedup();
        assert_eq!(labels.len(), total, "two wire codes share one label");
    }
}
