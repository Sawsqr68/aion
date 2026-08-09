//! `read_history`: a cursor page of the authoritative event history.
//!
//! A `CallToolResult` is not a `CacheableResult` in the 2026-07-28 revision, so
//! the immutability of a history page cannot be expressed as `ttlMs`. It is
//! expressed where it is actually true instead: `page_is_immutable` is set when
//! every event on the page sits strictly below the head, which for an
//! append-only history means the page can never change. That is an honest
//! statement about the DATA; a protocol cache hint on a result the revision
//! declares uncacheable would not be.

use aion_core::{Event, PayloadElision};
use aion_mcp::tools::service::{ToolCall, ToolFailure, ToolOutcome};
use serde_json::{Map, Value, json};

use crate::mcp::args::{optional_u32, optional_u64};
use crate::{CallerIdentity, ServerState};

use super::run_view::RunView;

/// Run `read_history`.
///
/// # Errors
///
/// [`ToolFailure`] when the arguments are malformed, the caller does not hold
/// the namespace, or the workflow does not exist.
pub(crate) async fn read_history(
    state: &ServerState,
    caller: &CallerIdentity,
    call: &ToolCall,
) -> Result<ToolOutcome, ToolFailure> {
    let view = RunView::read(state, caller, call).await?;
    let from_seq = optional_u64(call, "from_seq").unwrap_or(0);
    let limit = optional_u32(call, "limit")?;
    let payload_limit_bytes = optional_u64(call, "payload_limit_bytes");
    let head_seq = view.history_head_seq();

    let mut tail: Vec<Event> = view
        .history
        .iter()
        .filter(|event| event.seq() >= from_seq)
        .cloned()
        .collect();
    let next_from_seq = limit
        .and_then(|limit| usize::try_from(limit).ok())
        .and_then(|limit| tail.get(limit).map(Event::seq));
    if let Some(limit) = limit.and_then(|limit| usize::try_from(limit).ok()) {
        tail.truncate(limit);
    }
    let page_is_immutable = tail.last().is_none_or(|event| event.seq() < head_seq);
    let events = tail
        .into_iter()
        .map(|event| project_event(&event, payload_limit_bytes))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(ToolOutcome {
        summary: format!(
            "{} history event(s) of workflow {} (head seq {head_seq})",
            events.len(),
            view.workflow_id
        ),
        structured: json!({
            "workflow_id": view.workflow_id.to_string(),
            "events": events,
            "next_from_seq": next_from_seq,
            "head_seq": head_seq,
            "page_is_immutable": page_is_immutable,
        }),
    })
}

/// Encode one event, eliding any payload larger than the caller's ceiling.
///
/// Elision replaces the bytes with an explicit size marker rather than
/// truncating them: a truncated payload looks like a payload, and a model would
/// read it as one.
fn project_event(event: &Event, payload_limit_bytes: Option<u64>) -> Result<Value, ToolFailure> {
    let mut value = serde_json::to_value(event).map_err(|error| {
        ToolFailure::new(
            format!("a history event could not be encoded: {error}"),
            json!({ "code": "backend" }),
        )
    })?;
    if let Some(limit) = payload_limit_bytes.filter(|limit| *limit > 0) {
        elide_payloads(&mut value, limit)?;
    }
    Ok(value)
}

fn elide_payloads(value: &mut Value, limit: u64) -> Result<(), ToolFailure> {
    match value {
        Value::Array(values) => {
            for value in values {
                elide_payloads(value, limit)?;
            }
        }
        Value::Object(object) => {
            if is_payload_object(object) {
                elide_one(object, limit)?;
            } else {
                for value in object.values_mut() {
                    elide_payloads(value, limit)?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn elide_one(object: &mut Map<String, Value>, limit: u64) -> Result<(), ToolFailure> {
    let Some(bytes) = object.get_mut("bytes") else {
        return Ok(());
    };
    let Some(size) = byte_length(bytes).filter(|size| *size > limit) else {
        return Ok(());
    };
    *bytes = serde_json::to_value(PayloadElision::new(size)).map_err(|error| {
        ToolFailure::new(
            format!("a payload elision marker could not be encoded: {error}"),
            json!({ "code": "backend" }),
        )
    })?;
    Ok(())
}

fn is_payload_object(object: &Map<String, Value>) -> bool {
    object.get("content_type").is_some_and(Value::is_string) && object.contains_key("bytes")
}

fn byte_length(value: &Value) -> Option<u64> {
    value
        .as_array()
        .and_then(|bytes| u64::try_from(bytes.len()).ok())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{elide_payloads, is_payload_object};

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn an_oversize_payload_is_replaced_by_a_size_marker_not_truncated() -> TestResult {
        let mut value = json!({
            "WorkflowStarted": {
                "input": { "content_type": "application/json", "bytes": [1, 2, 3, 4, 5] },
            }
        });
        elide_payloads(&mut value, 2)?;
        let bytes = &value["WorkflowStarted"]["input"]["bytes"];
        assert!(
            bytes.as_array().is_none(),
            "an elided payload must not still look like bytes: {bytes}"
        );
        assert!(
            bytes.to_string().contains('5'),
            "the marker must state the ORIGINAL size, which was five bytes: {bytes}"
        );
        Ok(())
    }

    #[test]
    fn a_payload_within_the_ceiling_is_untouched() -> TestResult {
        let mut value = json!({
            "input": { "content_type": "application/json", "bytes": [1, 2] },
        });
        let before = value.clone();
        elide_payloads(&mut value, 8)?;
        assert_eq!(value, before);
        Ok(())
    }

    #[test]
    fn only_payload_shaped_objects_are_elided() -> TestResult {
        let object = json!({ "content_type": "application/json", "bytes": [] });
        let map = object.as_object().ok_or("expected an object")?;
        assert!(is_payload_object(map));
        let other = json!({ "bytes": [1, 2, 3] });
        let map = other.as_object().ok_or("expected an object")?;
        assert!(!is_payload_object(map));
        Ok(())
    }
}
