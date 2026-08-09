//! What the verb composes, checked against the document's own contract.
//!
//! These are the composition proofs, and composition is the half that happens
//! BEFORE anything dials: every message `aion assistant` sends is built and
//! validated here against the schema the embedded document derives, so a
//! contract mismatch or a malformed invocation is named locally rather than
//! discovered as a server refusal. The transport round-trip is proven
//! separately, against a real server, in `tests/assistant_verb_e2e.rs`.

use super::*;

type TestResult = Result<()>;

const A_WORKFLOW_ID: &str = "d1cf6ad0-6d2b-4a0e-9a3e-1f4a2b3c4d5e";

fn embedded() -> Result<&'static EmbeddedAssistant> {
    embedded_assistant().map_err(|error| anyhow!("{error}"))
}

/// The start input the verb composes satisfies the document's own start
/// contract — with a repository and in scratch mode alike.
#[test]
fn the_composed_start_input_satisfies_the_document_contract() -> TestResult {
    let embedded = embedded()?;
    compose_start(embedded, "explain AWL loops", Some("/tmp/aion"), false)?;
    compose_start(embedded, "explain AWL loops", None, true)?;
    Ok(())
}

/// `--no-repo` composes the document's documented scratch mode: `repo_path` is
/// present and empty, never absent.
#[test]
fn no_repo_composes_the_documents_scratch_mode() -> TestResult {
    let embedded = embedded()?;
    let composed = compose_start(embedded, "explain AWL loops", None, true)?;
    let value: Value = serde_json::from_slice(composed.bytes())?;
    assert_eq!(value[REPO_PATH_INPUT], json!(""));
    assert_eq!(value[OBJECTIVE_INPUT], json!("explain AWL loops"));
    Ok(())
}

/// A blank objective is refused: there is nothing to open a session with, and
/// the refusal must not depend on a server having an opinion.
#[test]
fn a_blank_objective_is_refused() -> TestResult {
    let embedded = embedded()?;
    assert!(compose_start(embedded, "   ", None, true).is_err());
    Ok(())
}

/// The schema validation is not vacuous: an input missing the objective is
/// refused by the document's own contract. Without this, the tests above would
/// pass just as well against a schema that accepts anything.
#[test]
fn a_start_input_missing_the_objective_is_refused_by_the_schema() -> TestResult {
    let embedded = embedded()?;
    let input = json!({ REPO_PATH_INPUT: "" });
    assert!(
        validate(embedded.input_schema(), &input, "start input").is_err(),
        "a start input with no `{OBJECTIVE_INPUT}` must not satisfy the document's contract"
    );
    Ok(())
}

/// Both continuation payloads — the operator's next message, and the clean end
/// — compose and satisfy the signal's declared payload schema.
#[test]
fn both_continuation_payloads_satisfy_the_signal_contract() -> TestResult {
    let embedded = embedded()?;
    let carry_on = compose_continuation(embedded, A_WORKFLOW_ID, "next question", false, None)?;
    let finish = compose_continuation(embedded, A_WORKFLOW_ID, "", true, None)?;

    let carry_on: Value = serde_json::from_slice(carry_on.payload.bytes())?;
    assert_eq!(carry_on[CONTINUE_MESSAGE_FIELD], json!("next question"));
    assert_eq!(carry_on[CONTINUE_END_FIELD], json!(false));
    let finish: Value = serde_json::from_slice(finish.payload.bytes())?;
    assert_eq!(finish[CONTINUE_MESSAGE_FIELD], json!(""));
    assert_eq!(finish[CONTINUE_END_FIELD], json!(true));
    Ok(())
}

/// An empty message is refused at composition time — before any connection
/// exists — because the document treats a blank continuation as a no-op nudge.
#[test]
fn an_empty_message_is_refused_at_composition() -> TestResult {
    let embedded = embedded()?;
    let refusal = compose_continuation(embedded, A_WORKFLOW_ID, "", false, None)
        .err()
        .ok_or_else(|| anyhow!("an empty message must be refused"))?;
    assert!(
        refusal.to_string().contains("assistant end"),
        "the refusal must point at the verb that does finish a session; got: {refusal}"
    );
    Ok(())
}

/// The continuation schema really constrains: a payload missing `end` — the
/// field the document reads unguarded — is refused.
#[test]
fn a_continuation_missing_the_end_field_is_refused() -> TestResult {
    let embedded = embedded()?;
    let schema = embedded
        .continuation_schema()
        .map_err(|error| anyhow!("{error}"))?;
    let partial = json!({ CONTINUE_MESSAGE_FIELD: "next question" });
    assert!(
        validate(schema, &partial, CONTINUE_SIGNAL).is_err(),
        "the document requires both continuation fields on the wire"
    );
    Ok(())
}

/// A malformed workflow identifier is refused at composition, not by a server.
#[test]
fn a_malformed_workflow_id_is_refused_at_composition() -> TestResult {
    let embedded = embedded()?;
    assert!(compose_continuation(embedded, "not-a-uuid", "hello", false, None).is_err());
    assert!(compose_status("not-a-uuid", None).is_err());
    assert!(compose_status(A_WORKFLOW_ID, Some("not-a-uuid")).is_err());
    compose_status(A_WORKFLOW_ID, None)?;
    Ok(())
}

/// The document's status query is the one `aion assistant status` dispatches.
#[test]
fn the_status_verb_dispatches_a_query_the_document_declares() -> TestResult {
    let embedded = embedded()?;
    assert!(
        embedded.queries().iter().any(|name| name == STATUS_QUERY),
        "the document must declare `{STATUS_QUERY}`; got {:?}",
        embedded.queries()
    );
    Ok(())
}

/// `describe` reports the document's own identity and contract, from the
/// binary, with no server.
#[test]
fn describe_reports_the_embedded_identity_and_contract() -> TestResult {
    let embedded = embedded()?;
    let report = describe(embedded)?;
    assert_eq!(report["workflow_type"], embedded.workflow_type());
    assert_eq!(report["content_hash"], embedded.content_hash().to_string());
    assert_eq!(report["session"]["continue_signal"], CONTINUE_SIGNAL);
    assert_eq!(report["session"]["status_query"], STATUS_QUERY);
    assert_eq!(report["document_bytes"], json!(embedded.source().len()));
    Ok(())
}

/// `document --output` writes the embedded bytes, byte for byte — the file an
/// operator with no checkout hands to `aion worker agent`.
#[test]
fn document_writes_the_embedded_bytes() -> TestResult {
    let embedded = embedded()?;
    let scratch = tempfile::tempdir()?;
    let path = scratch.path().join("assistant.awl");
    let report = document(embedded, Some(&path))?;
    assert_eq!(report["written"], path.display().to_string());
    let written = std::fs::read_to_string(&path)?;
    assert_eq!(written, embedded.source());
    Ok(())
}
