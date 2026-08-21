//! JSON output helpers for the CLI.

use std::io::{self, Write};

use aion_client::{PauseOutcome, ReopenOutcome, ResumeOutcome, StartOutcome};
use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::{Map, Value, json};

/// What `aion start` prints.
///
/// [`StartOutcome::display_name_not_applied`] is deliberately NOT rendered
/// here. The SDK raises that report only on an idempotency-key replay, and this
/// binary cannot produce one: no CLI verb takes an idempotency key, and the
/// SDK's replay cache is per-`Client`, so a key could not survive between
/// invocations even if a verb took one. A field that can never carry a value is
/// a promise the output does not keep, so it is absent rather than always-null.
/// A verb that grows an idempotency key grows this output with it.
#[derive(Serialize)]
pub(crate) struct StartOutput {
    workflow_id: String,
    run_id: String,
}

#[derive(Serialize)]
pub(crate) struct AcknowledgementOutput<'a> {
    pub(crate) workflow_id: &'a str,
    pub(crate) accepted: bool,
}

#[derive(Serialize)]
pub(crate) struct QueryOutput {
    pub(crate) result: Value,
}

#[derive(Serialize)]
pub(crate) struct ReopenOutput {
    pub(crate) workflow_id: String,
    pub(crate) run_id: String,
    pub(crate) status: String,
    pub(crate) reopened: bool,
}

#[derive(Serialize)]
pub(crate) struct DescribeOutput<TSummary, TRunId, TTerminal> {
    pub(crate) summary: TSummary,
    pub(crate) run_id: TRunId,
    pub(crate) history_head_seq: u64,
    pub(crate) terminal_event: Option<TTerminal>,
}

pub(crate) fn start_output(outcome: &StartOutcome) -> StartOutput {
    StartOutput {
        workflow_id: outcome.handle.workflow_id().to_string(),
        run_id: outcome.handle.run_id().to_string(),
    }
}

pub(crate) fn reopen_output(workflow_id: &str, outcome: &ReopenOutcome) -> ReopenOutput {
    ReopenOutput {
        workflow_id: workflow_id.to_owned(),
        run_id: outcome.run_id.to_string(),
        status: format!("{:?}", outcome.status),
        reopened: true,
    }
}

/// Output of a pause request (#204).
#[derive(Serialize)]
pub(crate) struct PauseOutput {
    pub(crate) workflow_id: String,
    pub(crate) run_id: String,
    pub(crate) status: String,
    pub(crate) paused: bool,
}

pub(crate) fn pause_output(workflow_id: &str, outcome: &PauseOutcome) -> PauseOutput {
    PauseOutput {
        workflow_id: workflow_id.to_owned(),
        run_id: outcome.run_id.to_string(),
        status: format!("{:?}", outcome.status),
        paused: true,
    }
}

/// Output of a resume request (#204).
#[derive(Serialize)]
pub(crate) struct ResumeOutput {
    pub(crate) workflow_id: String,
    pub(crate) run_id: String,
    pub(crate) status: String,
    pub(crate) resumed: bool,
}

pub(crate) fn resume_output(workflow_id: &str, outcome: &ResumeOutcome) -> ResumeOutput {
    ResumeOutput {
        workflow_id: workflow_id.to_owned(),
        run_id: outcome.run_id.to_string(),
        status: format!("{:?}", outcome.status),
        resumed: true,
    }
}

pub(crate) fn to_value<T>(value: T) -> Result<Value>
where
    T: Serialize,
{
    serde_json::to_value(value).context("failed to encode command output")
}

pub(crate) fn describe_output<TSummary, TRunId, TTerminal>(
    summary: TSummary,
    run_id: TRunId,
    history_head_seq: u64,
    terminal_event: Option<TTerminal>,
    raw: bool,
) -> Result<Value>
where
    TSummary: Serialize,
    TRunId: Serialize,
    TTerminal: Serialize,
{
    let mut value = to_value(DescribeOutput {
        summary,
        run_id,
        history_head_seq,
        terminal_event,
    })?;
    if !raw
        && let Value::Object(object) = &mut value
        && let Some(terminal_event) = object.get_mut("terminal_event")
    {
        decode_payloads_in_value(terminal_event);
    }
    Ok(value)
}

fn decode_payloads_in_value(value: &mut Value) {
    match value {
        Value::Array(items) => {
            for item in items {
                decode_payloads_in_value(item);
            }
        }
        Value::Object(object) => {
            if let Some(display_value) = payload_display_value(object) {
                *value = display_value;
            } else {
                for item in object.values_mut() {
                    decode_payloads_in_value(item);
                }
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn payload_display_value(object: &Map<String, Value>) -> Option<Value> {
    let content_type = object.get("content_type")?.as_str()?;
    let bytes = payload_bytes(object.get("bytes")?)?;

    if bytes.is_empty() {
        return Some(json!({
            "content_type": content_type,
            "empty": true
        }));
    }

    if content_type == "Json"
        && let Ok(decoded) = serde_json::from_slice::<Value>(&bytes)
    {
        return Some(decoded);
    }

    Some(json!({
        "content_type": content_type,
        "encoding": "hex",
        "data": hex_encode(&bytes)
    }))
}

fn payload_bytes(value: &Value) -> Option<Vec<u8>> {
    value
        .as_array()?
        .iter()
        .map(|item| item.as_u64().and_then(|byte| u8::try_from(byte).ok()))
        .collect()
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

pub(crate) fn print_json(value: &Value, pretty: bool) -> Result<()> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    if pretty {
        serde_json::to_writer_pretty(&mut handle, value).context("failed to write JSON output")?;
    } else {
        serde_json::to_writer(&mut handle, value).context("failed to write JSON output")?;
    }
    writeln!(handle).context("failed to write trailing newline")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{decode_payloads_in_value, describe_output};

    #[test]
    fn describe_output_decodes_terminal_payload_and_carries_identity() -> anyhow::Result<()> {
        let terminal =
            json!({"event_type":"WorkflowCompleted","result":payload(r#"{"done":true}"#)});
        let output = describe_output(
            json!({"workflow_id":"wf"}),
            "run",
            42,
            Some(terminal),
            false,
        )?;
        assert_eq!(output["summary"]["workflow_id"], json!("wf"));
        assert_eq!(output["run_id"], json!("run"));
        assert_eq!(output["history_head_seq"], json!(42));
        assert_eq!(output["terminal_event"]["result"], json!({"done":true}));
        assert!(output.get("history").is_none());
        Ok(())
    }

    #[test]
    fn raw_describe_output_preserves_terminal_payload_bytes() -> anyhow::Result<()> {
        let terminal = json!({"result":payload(r#"{\"value\":1}"#)});
        let output = describe_output(json!({"workflow_id":"wf"}), "run", 2, Some(terminal), true)?;
        assert_eq!(
            output["terminal_event"]["result"],
            payload(r#"{\"value\":1}"#)
        );
        Ok(())
    }

    #[test]
    fn describe_output_does_not_decode_summary_payload_shapes() -> anyhow::Result<()> {
        let summary = json!({"workflow_id":"wf","metadata":payload(r#"{\"raw\":true}"#)});
        let output = describe_output(summary, "run", 1, Option::<serde_json::Value>::None, false)?;
        assert_eq!(output["summary"]["metadata"], payload(r#"{\"raw\":true}"#));
        Ok(())
    }

    #[test]
    fn malformed_json_payload_falls_back_to_hex() {
        let mut value = json!({
            "payload": {
                "content_type": "Json",
                "bytes": [123, 110, 111, 116]
            }
        });

        decode_payloads_in_value(&mut value);

        assert_eq!(
            value["payload"],
            json!({
                "content_type": "Json",
                "encoding": "hex",
                "data": "7b6e6f74"
            })
        );
    }

    #[test]
    fn invalid_utf8_json_payload_falls_back_to_hex() {
        let mut value = json!({
            "payload": {
                "content_type": "Json",
                "bytes": [255, 254, 253]
            }
        });

        decode_payloads_in_value(&mut value);

        assert_eq!(
            value["payload"],
            json!({
                "content_type": "Json",
                "encoding": "hex",
                "data": "fffefd"
            })
        );
    }

    #[test]
    fn non_json_payload_falls_back_to_hex_with_content_type() {
        let mut value = json!({
            "payload": {
                "content_type": "Binary",
                "bytes": [0, 15, 16, 255]
            }
        });

        decode_payloads_in_value(&mut value);

        assert_eq!(
            value["payload"],
            json!({
                "content_type": "Binary",
                "encoding": "hex",
                "data": "000f10ff"
            })
        );
    }

    #[test]
    fn empty_payload_uses_clear_empty_indicator() {
        let mut value = json!({
            "payload": {
                "content_type": "Json",
                "bytes": []
            }
        });

        decode_payloads_in_value(&mut value);

        assert_eq!(
            value["payload"],
            json!({
                "content_type": "Json",
                "empty": true
            })
        );
    }

    #[test]
    fn invalid_payload_shape_is_left_unchanged() {
        let original = json!({
            "content_type": "Json",
            "bytes": [256]
        });
        let mut value = original.clone();

        decode_payloads_in_value(&mut value);

        assert_eq!(value, original);
    }

    fn payload(json: &str) -> serde_json::Value {
        json!({
            "content_type": "Json",
            "bytes": json.as_bytes()
        })
    }
}
