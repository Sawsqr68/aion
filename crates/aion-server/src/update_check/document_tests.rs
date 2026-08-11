//! Proofs that the embedded document and the server half's constants cannot
//! silently diverge.
//!
//! `EmbeddedUpdateCheck::from_source` exists so every verification arm can be
//! shown to FAIL against a document that deliberately omits its surface — a
//! check nothing ever fails is a check nobody has measured.

use super::*;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// The embedded bytes compile, package, and carry every verified name. This is
/// the test that breaks first when the document and the constants drift.
#[test]
fn the_embedded_document_loads_and_verifies() -> TestResult {
    let embedded = EmbeddedUpdateCheck::load()?;
    assert_eq!(embedded.workflow_type(), UPDATE_CHECK_WORKFLOW_TYPE);
    assert_eq!(embedded.source(), EMBEDDED_UPDATE_CHECK_DOCUMENT);
    Ok(())
}

/// The compiled identity is deterministic: two loads of the same bytes are the
/// same content hash. The install rule's `AlreadyCurrent` case depends on it.
#[test]
fn the_compiled_identity_is_deterministic() -> TestResult {
    let first = EmbeddedUpdateCheck::load()?;
    let second = EmbeddedUpdateCheck::from_source(EMBEDDED_UPDATE_CHECK_DOCUMENT)?;
    assert_eq!(first.content_hash(), second.content_hash());
    Ok(())
}

/// A document whose command drifted from [`FETCH_COMMAND`] refuses to load,
/// naming the drifted body — the observer must never key on a command the
/// document no longer runs.
#[test]
fn a_drifted_command_refuses_by_name() -> TestResult {
    let drifted = EMBEDDED_UPDATE_CHECK_DOCUMENT.replace(
        FETCH_COMMAND,
        "curl -fsS https://example.com/somewhere-else",
    );
    match EmbeddedUpdateCheck::from_source(&drifted) {
        Err(EmbeddedUpdateCheckError::WrongBody { found: Some(found) }) => {
            assert_eq!(found, "curl -fsS https://example.com/somewhere-else");
            Ok(())
        }
        other => Err(format!("a drifted command must refuse as WrongBody: {other:?}").into()),
    }
}

/// A document that renamed the action refuses to load.
#[test]
fn a_renamed_action_refuses() -> TestResult {
    let renamed = EMBEDDED_UPDATE_CHECK_DOCUMENT.replace(FETCH_ACTION, "fetch_release_feed");
    match EmbeddedUpdateCheck::from_source(&renamed) {
        Err(EmbeddedUpdateCheckError::MissingAction) => Ok(()),
        other => Err(format!("a renamed action must refuse as MissingAction: {other:?}").into()),
    }
}

/// A document that renamed the workflow type refuses to load — the console
/// starts the check by this exact type name.
#[test]
fn a_renamed_workflow_type_refuses() -> TestResult {
    let renamed =
        EMBEDDED_UPDATE_CHECK_DOCUMENT.replace("workflow update_check", "workflow version_check");
    match EmbeddedUpdateCheck::from_source(&renamed) {
        Err(EmbeddedUpdateCheckError::WrongWorkflowType { found }) => {
            assert_eq!(found, "version_check");
            Ok(())
        }
        other => Err(format!("a renamed type must refuse as WrongWorkflowType: {other:?}").into()),
    }
}

/// The queue name is verified too: an action moved to another queue is an
/// action the observer would never match.
#[test]
fn a_moved_queue_refuses() -> TestResult {
    let moved = EMBEDDED_UPDATE_CHECK_DOCUMENT.replace("worker update_check", "worker maintenance");
    match EmbeddedUpdateCheck::from_source(&moved) {
        Err(EmbeddedUpdateCheckError::MissingAction) => Ok(()),
        other => Err(format!("a moved queue must refuse as MissingAction: {other:?}").into()),
    }
}

/// The command the document declares and the constant the observer trusts are
/// the same bytes — stated once here as a direct read, on top of the
/// load-time verification.
#[test]
fn the_document_authors_the_exact_constant_command() {
    assert!(
        EMBEDDED_UPDATE_CHECK_DOCUMENT.contains(&format!("run \"{FETCH_COMMAND}\"")),
        "the document must author `run \"{FETCH_COMMAND}\"` verbatim"
    );
}

/// The crate the fetched URL names is the crate the index parser demands of
/// every line: [`FETCH_COMMAND`] must end in `/{INDEX_CRATE_NAME}`, or the
/// check would fetch one crate's index and refuse it for naming that crate.
#[test]
fn the_fetched_url_and_the_accepted_crate_name_agree() {
    let expected_tail = format!("/{}", super::super::index::INDEX_CRATE_NAME);
    assert!(
        FETCH_COMMAND.ends_with(&expected_tail),
        "`{FETCH_COMMAND}` must end in `{expected_tail}` — the parser refuses lines naming any \
         other crate"
    );
}
