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
//! - [`note_in_summary`] (remediator and integration fixer): the machinery
//!   appends its OWN record to the report's summary. Today that record is the
//!   list of untracked paths the tracked-only staging fence declined to commit
//!   (see [`crate::commit`]).
//!
//! All three preserve the double-encoded string envelope when the harness
//! produced one, so the rewrite is invisible to the transport.
//!
//! WHY A SUMMARY APPEND AND NOT A NEW FIELD. The wire shapes are derived from
//! `../awl/staged_rounds.awl` and every derived record schema is CLOSED
//! (`additionalProperties: false`), so an extra key on `DevItemResult` or
//! `RemediationReport` would need a document change, a new package identity
//! and a redeploy. The summary is the field the panel and the operator
//! actually read, so the machinery's record goes there, under a heading that
//! says plainly it is not the agent's words.

use serde_json::{Map, Value};

use crate::types::ProvisionedItem;

/// The heading every machinery-appended note sits under. An agent's summary is
/// its own report; anything below this line was written by the worker.
const MACHINERY_HEADING: &str = "--- MACHINERY RECORD (not the agent's words) ---";

/// Assemble the `dev_item` activity RESULT from the agent's report output.
///
/// `machinery_note`, when present, is appended to the report's `summary` — it
/// is how the untracked paths the dev commit's scope fence excluded reach the
/// review panel, which is the audience that has to be able to tell "the agent
/// wrote no new file there" from "the machinery declined to commit one".
///
/// # Errors
///
/// Returns a message when the output is not a JSON object document, when the
/// work item cannot be encoded, or when the result cannot be encoded.
pub fn assemble_dev_item_result(
    bytes: &[u8],
    work: &ProvisionedItem,
    head: &str,
    machinery_note: Option<&str>,
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
    if let Some(note) = machinery_note {
        append_to_summary(fields, note);
    }
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

/// Append the machinery's own record to a report payload's `summary`.
///
/// Used by the remediator and the integration fixer, whose reports pass
/// through otherwise untouched: their mechanical step has no identity to
/// rewrite, but it still owes the run an account of anything it declined to
/// commit.
///
/// # Errors
///
/// Returns a message when the output is not a JSON object document or when the
/// annotated report cannot be re-encoded.
pub fn note_in_summary(bytes: &[u8], role: &str, note: &str) -> Result<Vec<u8>, String> {
    let (mut report, double_encoded) = output_json(bytes, role)?;
    let Value::Object(ref mut fields) = report else {
        return Err(format!(
            "{role} output is not a JSON object; cannot record the machinery note"
        ));
    };
    append_to_summary(fields, note);
    encode(&report, double_encoded, "annotated report")
}

/// Append a machinery note under [`MACHINERY_HEADING`], creating `summary`
/// when the payload has none (an output that arrived without one is already
/// off-schema; dropping the note on top of that would compound a fault with a
/// silence).
fn append_to_summary(fields: &mut Map<String, Value>, note: &str) {
    let existing = fields
        .get("summary")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim_end()
        .to_owned();
    let annotated = if existing.is_empty() {
        format!("{MACHINERY_HEADING}\n{note}")
    } else {
        format!("{existing}\n\n{MACHINERY_HEADING}\n{note}")
    };
    fields.insert("summary".to_owned(), Value::String(annotated));
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
