//! Reality-wins rewrite tests: the dev result is assembled around the
//! agent's report with the MACHINERY's coordinates, and the verdict is
//! stamped with the panel seat the machinery dispatched.

use super::{assemble_dev_item_result, note_in_summary, stamp_item_verdict};
use crate::types::{ProvisionedItem, WorkItem};

fn work() -> ProvisionedItem {
    ProvisionedItem {
        item: WorkItem {
            id: "it-1".to_owned(),
            title: "t".to_owned(),
            goal: "g".to_owned(),
            scope_in: vec!["src/".to_owned()],
            scope_out: vec![],
            phase: 1,
            depends_on: vec![],
            feedback: String::new(),
        },
        workspace_path: "/repo/.staged-rounds/wf/items/it-1".to_owned(),
        branch: "staged/wf/it-1".to_owned(),
        base_commit: "basehead".to_owned(),
    }
}

#[test]
fn assembly_wraps_the_report_with_real_coordinates() -> anyhow::Result<()> {
    let assembled = assemble_dev_item_result(
        b"{\"item_id\":\"fabricated\",\"summary\":\"s\",\"commits\":[\"fake\"],\"claims\":[]}",
        &work(),
        "realhead",
        None,
    )
    .map_err(anyhow::Error::msg)?;
    let value: serde_json::Value = serde_json::from_slice(&assembled)?;
    assert_eq!(value["item"]["id"], "it-1");
    assert_eq!(
        value["workspace_path"],
        "/repo/.staged-rounds/wf/items/it-1"
    );
    assert_eq!(value["branch"], "staged/wf/it-1");
    assert_eq!(value["base_commit"], "basehead");
    assert_eq!(value["report"]["item_id"], "it-1");
    assert_eq!(value["report"]["commits"], serde_json::json!(["realhead"]));
    assert_eq!(value["report"]["summary"], "s");
    Ok(())
}

#[test]
fn assembly_preserves_the_double_encoded_envelope() -> anyhow::Result<()> {
    let inner = "{\"item_id\":\"it-1\",\"summary\":\"s\",\"commits\":[],\"claims\":[]}";
    let outer = serde_json::to_vec(&serde_json::Value::String(inner.to_owned()))?;
    let assembled =
        assemble_dev_item_result(&outer, &work(), "realhead", None).map_err(anyhow::Error::msg)?;
    let value: serde_json::Value = serde_json::from_slice(&assembled)?;
    let serde_json::Value::String(inner_encoded) = value else {
        anyhow::bail!("the double-encoded envelope was not preserved");
    };
    let inner_value: serde_json::Value = serde_json::from_str(&inner_encoded)?;
    assert_eq!(
        inner_value["report"]["commits"],
        serde_json::json!(["realhead"])
    );
    Ok(())
}

#[test]
fn a_non_object_report_is_a_loud_fault() {
    let result = assemble_dev_item_result(b"[1,2]", &work(), "head", None);
    assert!(result.is_err());
}

/// The machinery's own record reaches the PANEL, not just the worker log: the
/// excluded-path note is appended to the report summary the reviewers read,
/// under a heading that says it is not the agent's words.
#[test]
fn the_machinery_note_is_appended_to_the_report_summary() -> anyhow::Result<()> {
    let assembled = assemble_dev_item_result(
        b"{\"item_id\":\"it-1\",\"summary\":\"added the parser\",\"commits\":[],\"claims\":[]}",
        &work(),
        "realhead",
        Some("2 untracked path(s) were NOT staged: target/a, target/b"),
    )
    .map_err(anyhow::Error::msg)?;
    let value: serde_json::Value = serde_json::from_slice(&assembled)?;
    let summary = value["report"]["summary"].as_str().unwrap_or_default();
    assert!(
        summary.starts_with("added the parser"),
        "the agent's own summary must survive intact: {summary}"
    );
    assert!(
        summary.contains("MACHINERY RECORD"),
        "the note must be attributed to the machinery: {summary}"
    );
    assert!(summary.contains("target/a"), "{summary}");
    Ok(())
}

/// The remediation-side payloads pass through untouched except for the same
/// record, and the double-encoded envelope survives it.
#[test]
fn a_remediation_report_carries_the_machinery_note_through_the_envelope() -> anyhow::Result<()> {
    let inner = "{\"summary\":\"resolved both sides\",\"resolved\":[\"src/a.rs\"]}";
    let outer = serde_json::to_vec(&serde_json::Value::String(inner.to_owned()))?;
    let noted = note_in_summary(&outer, "remediate", "left target/junk untracked")
        .map_err(anyhow::Error::msg)?;
    let value: serde_json::Value = serde_json::from_slice(&noted)?;
    let serde_json::Value::String(inner_encoded) = value else {
        anyhow::bail!("the double-encoded envelope was not preserved");
    };
    let inner_value: serde_json::Value = serde_json::from_str(&inner_encoded)?;
    let summary = inner_value["summary"].as_str().unwrap_or_default();
    assert!(summary.starts_with("resolved both sides"), "{summary}");
    assert!(summary.contains("left target/junk untracked"), "{summary}");
    assert_eq!(inner_value["resolved"], serde_json::json!(["src/a.rs"]));
    Ok(())
}

/// A payload with no `summary` at all is already off-schema; the note is still
/// recorded rather than dropped on top of a fault.
#[test]
fn a_report_without_a_summary_still_gets_the_note() -> anyhow::Result<()> {
    let noted = note_in_summary(b"{\"resolved\":[]}", "remediate", "nothing was adopted")
        .map_err(anyhow::Error::msg)?;
    let value: serde_json::Value = serde_json::from_slice(&noted)?;
    let summary = value["summary"].as_str().unwrap_or_default();
    assert!(summary.contains("MACHINERY RECORD"), "{summary}");
    assert!(summary.contains("nothing was adopted"), "{summary}");
    Ok(())
}

#[test]
fn a_non_object_remediation_report_is_a_loud_fault() {
    assert!(note_in_summary(b"[1,2]", "remediate", "note").is_err());
    assert!(note_in_summary(b"not json", "remediate", "note").is_err());
}

#[test]
fn the_verdict_is_stamped_with_the_dispatched_seat_not_the_agent_claim() -> anyhow::Result<()> {
    let stamped = stamp_item_verdict(
        b"{\"item_id\":\"wrong-item\",\"lens\":\"wrong-lens\",\"overall\":\"accept\",\
          \"findings\":[]}",
        "it-1",
        "correctness",
    )
    .map_err(anyhow::Error::msg)?;
    let value: serde_json::Value = serde_json::from_slice(&stamped)?;
    assert_eq!(value["item_id"], "it-1");
    assert_eq!(value["lens"], "correctness");
    // The judgement itself is untouched — only identity is stamped.
    assert_eq!(value["overall"], "accept");
    assert_eq!(value["findings"], serde_json::json!([]));
    Ok(())
}

#[test]
fn a_verdict_omitting_the_lens_gets_one_stamped_in() -> anyhow::Result<()> {
    let stamped = stamp_item_verdict(
        b"{\"item_id\":\"it-1\",\"overall\":\"reject\",\
          \"findings\":[{\"severity\":\"blocking\",\"title\":\"t\",\"evidence\":\"e\"}],\
          \"reject_reason\":\"r\"}",
        "it-1",
        "regressions",
    )
    .map_err(anyhow::Error::msg)?;
    let value: serde_json::Value = serde_json::from_slice(&stamped)?;
    assert_eq!(value["lens"], "regressions");
    assert_eq!(value["reject_reason"], "r");
    Ok(())
}

#[test]
fn the_verdict_stamp_preserves_the_double_encoded_envelope() -> anyhow::Result<()> {
    let inner = "{\"item_id\":\"x\",\"overall\":\"accept\",\"findings\":[]}";
    let outer = serde_json::to_vec(&serde_json::Value::String(inner.to_owned()))?;
    let stamped =
        stamp_item_verdict(&outer, "it-7", "spec_fidelity").map_err(anyhow::Error::msg)?;
    let value: serde_json::Value = serde_json::from_slice(&stamped)?;
    let serde_json::Value::String(inner_encoded) = value else {
        anyhow::bail!("the double-encoded envelope was not preserved");
    };
    let inner_value: serde_json::Value = serde_json::from_str(&inner_encoded)?;
    assert_eq!(inner_value["item_id"], "it-7");
    assert_eq!(inner_value["lens"], "spec_fidelity");
    Ok(())
}

#[test]
fn a_non_object_verdict_is_a_loud_fault() {
    assert!(stamp_item_verdict(b"\"just a string document\"", "it-1", "correctness").is_err());
    assert!(stamp_item_verdict(b"not json at all", "it-1", "correctness").is_err());
}
