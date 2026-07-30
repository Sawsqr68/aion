//! REALITY-WINS rewrites of agent output payloads.
//!
//! DOCTRINE: an agent's structured output is a report, not a record. Anything
//! in it the MACHINERY already knows as fact is overwritten with the fact
//! before the payload becomes a durable activity result:
//!
//! - [`assemble_dev_item_result`] (developer role): the report's `item_id`
//!   becomes the item the machinery served and `commits` becomes the ONE real
//!   branch head the mechanical commit produced — the agent never ran git, so
//!   an asserted hash is fabricated by construction. The report is then
//!   wrapped in the `DevItemResult` record the document's type table declares,
//!   with the worktree coordinates taken from the activity INPUT.
//! - [`stamp_item_verdict`] (reviewer role): the verdict's `item_id` and
//!   `lens` become the panel seat the machinery dispatched. The panel is
//!   MACHINE-generated (`plan_reviews` flattens item × lens), so an
//!   agent-asserted item or lens name could only ever be noise — and a
//!   mis-asserted lens would make `fold_phase`'s "every configured lens
//!   reported" check accept an item one lens never actually examined.
//!
//! Both preserve the double-encoded string envelope when the harness produced
//! one, so the rewrite is invisible to the transport.

use serde_json::Value;

use crate::types::ProvisionedItem;

/// Assemble the `dev_item` activity RESULT from the agent's report output.
///
/// # Errors
///
/// Returns a message when the output is not a JSON object document, when the
/// work item cannot be encoded, or when the result cannot be encoded.
pub fn assemble_dev_item_result(
    bytes: &[u8],
    work: &ProvisionedItem,
    head: &str,
) -> Result<Vec<u8>, String> {
    let (mut report, double_encoded) = output_json(bytes, "dev_item")?;
    let Value::Object(ref mut fields) = report else {
        return Err("dev_item output is not a JSON object; cannot assemble the result".to_owned());
    };
    fields.insert("item_id".to_owned(), Value::String(work.item.id.clone()));
    fields.insert(
        "commits".to_owned(),
        Value::Array(vec![Value::String(head.to_owned())]),
    );
    let item = serde_json::to_value(&work.item)
        .map_err(|error| format!("work item is not encodable: {error}"))?;
    let result = serde_json::json!({
        "item": item,
        "workspace_path": work.workspace_path,
        "branch": work.branch,
        "base_commit": work.base_commit,
        "report": report,
    });
    encode(&result, double_encoded, "assembled dev result")
}

/// Stamp the `review_item` verdict with the panel seat the machinery
/// dispatched: `item_id` and `lens` are overwritten, everything the reviewer
/// actually judged (`overall`, `findings`, `reject_reason`) is untouched.
///
/// # Errors
///
/// Returns a message when the output is not a JSON object document or when
/// the stamped verdict cannot be re-encoded.
pub fn stamp_item_verdict(bytes: &[u8], item_id: &str, lens: &str) -> Result<Vec<u8>, String> {
    let (mut verdict, double_encoded) = output_json(bytes, "review_item")?;
    let Value::Object(ref mut fields) = verdict else {
        return Err(
            "review_item output is not a JSON object; cannot stamp the verdict's item and lens"
                .to_owned(),
        );
    };
    fields.insert("item_id".to_owned(), Value::String(item_id.to_owned()));
    fields.insert("lens".to_owned(), Value::String(lens.to_owned()));
    encode(&verdict, double_encoded, "stamped verdict")
}

/// Re-encode a rewritten payload, preserving the double-encoded string
/// envelope the harness produced when there was one.
fn encode(value: &Value, double_encoded: bool, what: &str) -> Result<Vec<u8>, String> {
    let encoded = if double_encoded {
        let inner = serde_json::to_string(value)
            .map_err(|error| format!("{what} is not encodable: {error}"))?;
        serde_json::to_vec(&Value::String(inner))
    } else {
        serde_json::to_vec(value)
    };
    encoded.map_err(|error| format!("{what} is not encodable: {error}"))
}

/// Decode an agent output payload to its JSON value, unwrapping the
/// double-encoded string form when present. Returns the value and whether it
/// was double-encoded (so a rewrite can preserve the envelope).
fn output_json(bytes: &[u8], role: &str) -> Result<(Value, bool), String> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|error| format!("{role} output is not JSON: {error}"))?;
    match value {
        Value::String(inner) => {
            let unwrapped = serde_json::from_str(&inner).map_err(|error| {
                format!("{role} output string does not hold a JSON document: {error}")
            })?;
            Ok((unwrapped, true))
        }
        other => Ok((other, false)),
    }
}

#[cfg(test)]
#[path = "agent_output/tests.rs"]
mod tests;
