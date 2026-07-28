//! Thin `aion` CLI client helpers for OS-process gates: run a remote command and
//! parse its JSON, start a workflow, describe a run, and read the observables the
//! failover gate asserts on (status + per-ordinal terminal count).

use std::process::Command;

use serde_json::Value;

/// Error type used across the OS-process harness and its tests (the same alias
/// `common/osproc.rs` exports; this module is included alongside it).
type TestError = Box<dyn std::error::Error>;

/// Fan-out arity of the `collect_four` fixture (one `ActivityCompleted` terminal
/// per ordinal on success).
const FAN_OUT: usize = 4;

/// How many trailing stderr lines a failed CLI invocation quotes back.
const STDERR_TAIL_LINES: usize = 20;

/// Run `aion <args>` and return parsed JSON stdout, or the REASON there is none
/// (the process could not be run, it exited non-zero, or its stdout was not
/// JSON). Poll loops built on this must be able to say what they kept hitting —
/// a bare `None` turned a fenced start and an unreachable endpoint into the same
/// silence.
pub fn aion_json(args: &[&str]) -> Result<Value, String> {
    let rendered = args.join(" ");
    let output = Command::new(env!("CARGO_BIN_EXE_aion"))
        .args(args)
        .output()
        .map_err(|error| format!("could not run `aion {rendered}`: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "`aion {rendered}` exited {status}; stderr:\n{stderr}",
            status = output.status,
            stderr = last_lines(&String::from_utf8_lossy(&output.stderr)),
        ));
    }
    serde_json::from_slice(&output.stdout).map_err(|error| {
        format!(
            "`aion {rendered}` succeeded but its stdout was not JSON ({error}):\n{stdout}",
            stdout = last_lines(&String::from_utf8_lossy(&output.stdout)),
        )
    })
}

/// The last [`STDERR_TAIL_LINES`] lines of `text`, or an explicit note when
/// there are none — empty quoted output must never read as "it said nothing"
/// when it may mean "nobody captured it".
fn last_lines(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return "<empty>".to_owned();
    }
    lines[lines.len().saturating_sub(STDERR_TAIL_LINES)..].join("\n")
}

/// Start the `collect_four` fan-out (workflow type = the fixture module name)
/// against `endpoint`, returning its workflow id, or the reason the attempt did
/// not land.
pub fn try_start(endpoint: &str) -> Result<String, String> {
    let value = aion_json(&[
        "start",
        "aion_outbox_fixture",
        "--input",
        "{\"fixture\":\"input\"}",
        "--endpoint",
        endpoint,
    ])?;
    value
        .get("workflow_id")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("start reply carried no workflow_id: {value}"))
}

/// Describe `workflow_id` over `endpoint`, returning the parsed JSON, or `None`
/// while the endpoint is unreachable or the run is not visible there yet (both
/// are EXPECTED states for a poll that spans a `kill -9`).
pub fn describe(endpoint: &str, workflow_id: &str) -> Option<Value> {
    aion_json(&["describe", workflow_id, "--endpoint", endpoint]).ok()
}

/// The `summary.status` string from a describe payload, if present.
pub fn status_of(description: &Value) -> Option<&str> {
    description
        .get("summary")
        .and_then(|summary| summary.get("status"))
        .and_then(Value::as_str)
}

/// Assert the completed run recorded EXACTLY ONE terminal per ordinal.
pub fn check_exactly_once(description: Option<&Value>) -> Result<(), TestError> {
    let description =
        description.ok_or_else(|| TestError::from("missing completed describe payload"))?;
    let completions = activity_completed_count(description);
    if completions == FAN_OUT {
        Ok(())
    } else {
        Err(format!(
            "exactly-once violated: expected {FAN_OUT} ActivityCompleted terminals \
             (one per ordinal), found {completions}"
        )
        .into())
    }
}

/// Count `ActivityCompleted` terminal events in a describe payload's history —
/// the exactly-once observable. Events serialize as `{"type": "...", ...}`.
fn activity_completed_count(description: &Value) -> usize {
    description
        .get("history")
        .and_then(Value::as_array)
        .map_or(0, |events| {
            events
                .iter()
                .filter(|event| {
                    event.get("type").and_then(Value::as_str) == Some("ActivityCompleted")
                })
                .count()
        })
}
