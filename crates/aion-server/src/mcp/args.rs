//! Argument extraction for the Aion tools.
//!
//! The catalog's JSON Schemas have already been enforced by the MCP dispatcher
//! before any of this runs, so a missing required argument here would be a
//! server defect rather than a caller mistake. These helpers still refuse
//! rather than default: a defaulted namespace is a defaulted authorization
//! scope, and there is no value for that which is safe to invent.

use aion_core::{ActivityId, RunId, WorkflowId};
use aion_mcp::tools::service::{ToolCall, ToolFailure};
use serde_json::{Value, json};
use uuid::Uuid;

/// A required string argument.
pub(crate) fn required_str(call: &ToolCall, key: &str) -> Result<String, ToolFailure> {
    call.arguments
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| missing(call, key, "a string"))
}

/// An optional string argument, treating an empty string as absent.
pub(crate) fn optional_str(call: &ToolCall, key: &str) -> Option<String> {
    call.arguments
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

/// A required UUID-shaped workflow identifier.
pub(crate) fn required_workflow_id(call: &ToolCall, key: &str) -> Result<WorkflowId, ToolFailure> {
    let raw = required_str(call, key)?;
    Uuid::parse_str(&raw).map(WorkflowId::new).map_err(|error| {
        ToolFailure::new(
            format!("`{key}` is not a valid workflow id: {error}"),
            json!({ "code": "invalid_argument", "argument": key, "value": raw }),
        )
    })
}

/// A required UUID-shaped run identifier.
pub(crate) fn required_run_id(call: &ToolCall, key: &str) -> Result<RunId, ToolFailure> {
    let raw = required_str(call, key)?;
    Uuid::parse_str(&raw).map(RunId::new).map_err(|error| {
        ToolFailure::new(
            format!("`{key}` is not a valid run id: {error}"),
            json!({ "code": "invalid_argument", "argument": key, "value": raw }),
        )
    })
}

/// An optional run identifier.
pub(crate) fn optional_run_id(call: &ToolCall, key: &str) -> Result<Option<RunId>, ToolFailure> {
    let Some(raw) = optional_str(call, key) else {
        return Ok(None);
    };
    Uuid::parse_str(&raw)
        .map(|id| Some(RunId::new(id)))
        .map_err(|error| {
            ToolFailure::new(
                format!("`{key}` is not a valid run id: {error}"),
                json!({ "code": "invalid_argument", "argument": key, "value": raw }),
            )
        })
}

/// A required non-negative integer argument.
pub(crate) fn required_u64(call: &ToolCall, key: &str) -> Result<u64, ToolFailure> {
    call.arguments
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| missing(call, key, "a non-negative integer"))
}

/// A required attempt number.
pub(crate) fn required_u32(call: &ToolCall, key: &str) -> Result<u32, ToolFailure> {
    let value = required_u64(call, key)?;
    u32::try_from(value).map_err(|error| {
        ToolFailure::new(
            format!("`{key}` is too large to be an attempt number: {error}"),
            json!({ "code": "invalid_argument", "argument": key, "value": value }),
        )
    })
}

/// An optional non-negative integer argument.
pub(crate) fn optional_u64(call: &ToolCall, key: &str) -> Option<u64> {
    call.arguments.get(key).and_then(Value::as_u64)
}

/// An optional `u32` argument, refusing a value that does not fit rather than
/// clamping it — a silently clamped page size is a silently wrong answer.
pub(crate) fn optional_u32(call: &ToolCall, key: &str) -> Result<Option<u32>, ToolFailure> {
    let Some(value) = optional_u64(call, key) else {
        return Ok(None);
    };
    u32::try_from(value).map(Some).map_err(|error| {
        ToolFailure::new(
            format!("`{key}` is too large: {error}"),
            json!({ "code": "invalid_argument", "argument": key, "value": value }),
        )
    })
}

/// An optional boolean argument.
pub(crate) fn optional_bool(call: &ToolCall, key: &str) -> bool {
    call.arguments
        .get(key)
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

/// An optional free-form JSON argument.
pub(crate) fn optional_json(call: &ToolCall, key: &str) -> Option<Value> {
    call.arguments
        .get(key)
        .filter(|value| !value.is_null())
        .cloned()
}

/// A required activity ordinal.
pub(crate) fn required_activity_id(call: &ToolCall, key: &str) -> Result<ActivityId, ToolFailure> {
    Ok(ActivityId::from_sequence_position(required_u64(call, key)?))
}

fn missing(call: &ToolCall, key: &str, expected: &str) -> ToolFailure {
    ToolFailure::new(
        format!(
            "`{key}` is required on `{}` and must be {expected}",
            call.name
        ),
        json!({ "code": "invalid_argument", "argument": key }),
    )
}

#[cfg(test)]
mod tests {
    use aion_mcp::tools::service::ToolCall;
    use serde_json::{Map, Value, json};

    use super::{optional_bool, optional_run_id, optional_u32, required_str, required_u32};

    fn call(arguments: &Value) -> Result<ToolCall, serde_json::Error> {
        let arguments: Map<String, Value> = serde_json::from_value(arguments.clone())?;
        Ok(ToolCall {
            name: "test".to_owned(),
            arguments,
        })
    }

    #[test]
    fn a_missing_required_argument_is_a_tool_failure() -> Result<(), serde_json::Error> {
        let call = call(&json!({}))?;
        let failure = required_str(&call, "namespace").err();
        assert!(failure.is_some_and(|failure| failure.message.contains("namespace")));
        Ok(())
    }

    #[test]
    fn an_oversize_integer_is_refused_rather_than_clamped() -> Result<(), serde_json::Error> {
        let call = call(&json!({ "attempt": u64::from(u32::MAX) + 1, "limit": 5 }))?;
        assert!(required_u32(&call, "attempt").is_err());
        assert_eq!(optional_u32(&call, "limit").ok().flatten(), Some(5));
        Ok(())
    }

    #[test]
    fn an_absent_or_empty_run_id_is_none() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(optional_run_id(&call(&json!({}))?, "run_id")?, None);
        assert_eq!(
            optional_run_id(&call(&json!({ "run_id": "" }))?, "run_id")?,
            None
        );
        assert!(optional_run_id(&call(&json!({ "run_id": "nope" }))?, "run_id").is_err());
        Ok(())
    }

    #[test]
    fn an_absent_boolean_is_false() -> Result<(), serde_json::Error> {
        assert!(!optional_bool(&call(&json!({}))?, "await_completion"));
        assert!(optional_bool(
            &call(&json!({ "await_completion": true }))?,
            "await_completion"
        ));
        Ok(())
    }
}
