//! Payload and argument parsing helpers for the CLI.

use aion_core::{ContentType, Payload, RunId, WorkflowId, WorkflowStatus};
use anyhow::{Context, Result};
use serde_json::Value;
use uuid::Uuid;

pub(crate) fn json_payload(raw: &str) -> Result<Payload> {
    let value: Value = serde_json::from_str(raw)?;
    Payload::from_json(&value).context("failed to serialize JSON payload")
}

pub(crate) fn empty_query_payload() -> Payload {
    Payload::new(ContentType::Json, Vec::new())
}

pub(crate) fn payload_to_json(payload: &Payload) -> Result<Value> {
    payload
        .to_json()
        .context("query result was not a valid JSON payload")
}

pub(crate) fn parse_workflow_id(raw: &str) -> Result<WorkflowId> {
    let uuid = Uuid::parse_str(raw).with_context(|| format!("invalid workflow id '{raw}'"))?;
    Ok(WorkflowId::new(uuid))
}

pub(crate) fn parse_run_id(raw: &str) -> Result<RunId> {
    let uuid = Uuid::parse_str(raw).with_context(|| format!("invalid run id '{raw}'"))?;
    Ok(RunId::new(uuid))
}

/// Every status `parse_status` accepts, in the canonical spelling an operator
/// should type. `Paused` is one of them: it is a real, non-terminal status, and
/// omitting it from the refusal (and from the `aion list` help) told operators
/// the tool rejected a filter it has always accepted.
///
/// Named once and the refusal is BUILT from it, so the message can no longer
/// disagree with what the parser accepts. The pin in [`tests`] closes the other
/// direction: every entry here must actually parse, so adding a spelling to the
/// list without a match arm fails at the gate.
///
/// The `aion list --status` help in `main.rs` mirrors this list in prose and is
/// not derived from it — clap's help takes a literal. Change both together.
const ACCEPTED_STATUSES: [&str; 7] = [
    "running",
    "completed",
    "failed",
    "cancelled",
    "timed-out",
    "continued-as-new",
    "paused",
];

/// The refusal a bad `--status` produces, naming every accepted spelling.
fn accepted_statuses_message() -> String {
    format!("expected one of {}", ACCEPTED_STATUSES.join(", "))
}

pub(crate) fn parse_status(raw: &str) -> Result<WorkflowStatus, String> {
    match status_key(raw).as_str() {
        "running" => Ok(WorkflowStatus::Running),
        "completed" => Ok(WorkflowStatus::Completed),
        "failed" => Ok(WorkflowStatus::Failed),
        "cancelled" | "canceled" => Ok(WorkflowStatus::Cancelled),
        "timedout" | "timed-out" => Ok(WorkflowStatus::TimedOut),
        "continuedasnew" | "continued-as-new" => Ok(WorkflowStatus::ContinuedAsNew),
        "paused" => Ok(WorkflowStatus::Paused),
        _ => Err(accepted_statuses_message()),
    }
}

fn status_key(raw: &str) -> String {
    raw.trim()
        .chars()
        .map(|character| match character {
            '_' | ' ' => '-',
            other => other.to_ascii_lowercase(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        ACCEPTED_STATUSES, json_payload, parse_run_id, parse_status, parse_workflow_id,
        payload_to_json,
    };
    use aion_core::WorkflowStatus;

    #[test]
    fn json_payload_round_trips_to_json_value() -> anyhow::Result<()> {
        let payload = json_payload(r#"{"name":"Ada"}"#)?;

        assert_eq!(payload_to_json(&payload)?, json!({ "name": "Ada" }));
        Ok(())
    }

    #[test]
    fn parse_status_accepts_documented_and_serde_spellings() {
        assert_eq!(parse_status("running"), Ok(WorkflowStatus::Running));
        assert_eq!(parse_status("Completed"), Ok(WorkflowStatus::Completed));
        assert_eq!(parse_status("timed-out"), Ok(WorkflowStatus::TimedOut));
        assert_eq!(parse_status("TimedOut"), Ok(WorkflowStatus::TimedOut));
        assert_eq!(
            parse_status("continued_as_new"),
            Ok(WorkflowStatus::ContinuedAsNew)
        );
        assert_eq!(
            parse_status("ContinuedAsNew"),
            Ok(WorkflowStatus::ContinuedAsNew)
        );
    }

    /// The refusal and the match arms are two enumerations of one set, and they
    /// drifted: `paused` parsed fine while both the refusal and the `aion list`
    /// help denied it existed. This pins them together — every accepted spelling
    /// must parse AND be named in the refusal, so adding a status without
    /// mentioning it fails here rather than misinforming an operator.
    #[test]
    fn every_accepted_status_parses_and_is_named_in_the_refusal() -> anyhow::Result<()> {
        let Err(refusal) = parse_status("no-such-status") else {
            anyhow::bail!("an unknown status must be refused");
        };

        for status in ACCEPTED_STATUSES {
            assert!(
                parse_status(status).is_ok(),
                "`{status}` is advertised as accepted but the parser refuses it"
            );
            assert!(
                refusal.contains(status),
                "the refusal must name `{status}`; got: {refusal}"
            );
        }
        Ok(())
    }

    #[test]
    fn parse_workflow_id_rejects_invalid_uuid() {
        assert!(parse_workflow_id("not-a-uuid").is_err());
    }

    #[test]
    fn parse_run_id_rejects_invalid_uuid() {
        assert!(parse_run_id("not-a-uuid").is_err());
    }
}
