//! The queue-declaration read over a MIXED catalog — the state the handshake
//! migration creates: retained pre-`.v4` packages beside deployed `.v4` ones.

use aion_package::{ContentHash, PackageContract, WorkerContract};

use super::DeclaredQueues;
use crate::loader::WorkflowCatalog;
use crate::log_capture::LogCapture;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn contract_declaring(task_queue: &str) -> PackageContract {
    PackageContract {
        workers: vec![WorkerContract {
            task_queue: task_queue.to_owned(),
            actions: Vec::new(),
        }],
        ..PackageContract::default()
    }
}

#[test]
fn a_read_of_only_v4_entries_covers_every_entry() -> TestResult {
    let catalog = WorkflowCatalog::new();
    catalog.note_loaded_workflow_with_contract_for_test(
        "orders",
        "orders__v4",
        "run",
        ContentHash::from_bytes([1; 32]),
        Some(contract_declaring("orders")),
    );

    let read = catalog.declared_task_queues()?;

    assert!(read.declares("orders"));
    assert!(read.covers_every_entry());
    assert!(read.undecodable_identities().is_empty());
    Ok(())
}

/// The blocker pin. A catalog holding one `.v4` package and one pre-`.v4`
/// package answers with a NON-EMPTY queue set that silently omits whatever the
/// pre-`.v4` package serves. The read must say so: the queue it found is
/// found, and the identity it could not decode is named — otherwise the
/// classifier reads the omission as "declared by nobody" and terminally
/// refuses a dispatch a live compatible worker could serve.
#[test]
fn a_mixed_catalog_names_the_entry_it_could_not_decode() -> TestResult {
    let catalog = WorkflowCatalog::new();
    catalog.note_loaded_workflow_with_contract_for_test(
        "orders",
        "orders__v4",
        "run",
        ContentHash::from_bytes([1; 32]),
        Some(contract_declaring("orders")),
    );
    let legacy_version = ContentHash::from_bytes([2; 32]);
    catalog.note_loaded_workflow_with_contract_for_test(
        "checkout",
        "checkout__legacy",
        "run",
        legacy_version.clone(),
        None,
    );

    let read = catalog.declared_task_queues()?;

    assert!(read.declares("orders"), "the decoded queue is still found");
    assert!(
        !read.covers_every_entry(),
        "a pre-.v4 entry means this read saw only part of the catalog"
    );
    assert_eq!(
        read.undecodable_identities(),
        [legacy_version.to_string()].as_slice(),
        "the undecodable stored identity must be named, not skipped"
    );
    Ok(())
}

/// The operator half of the fix: an entry that could not be decoded is not
/// only carried in the answer, it is SAID — at WARN, naming the identity to
/// re-deploy. A silent downgrade to `Unknown` would leave the fleet reading
/// as merely unserved forever.
#[test]
fn an_undecodable_entry_is_named_at_warn() -> TestResult {
    let (captured, subscriber) = LogCapture::new()?;
    let legacy_version = ContentHash::from_bytes([3; 32]);

    tracing::subscriber::with_default(subscriber, || {
        let catalog = WorkflowCatalog::new();
        catalog.note_loaded_workflow_with_contract_for_test(
            "checkout",
            "checkout__legacy",
            "run",
            legacy_version.clone(),
            None,
        );
        catalog.declared_task_queues().map(|_| ())
    })?;

    let warnings = captured.at_level("WARN")?;
    assert_eq!(warnings.len(), 1, "captured: {:?}", captured.events()?);
    assert!(
        warnings[0].mentions(&legacy_version.to_string()),
        "the WARN must name the stored identity to re-deploy: {}",
        warnings[0]
    );
    Ok(())
}

#[test]
fn a_read_reports_exactly_what_it_was_given() {
    let read = DeclaredQueues::new(
        ["orders".to_owned()].into_iter().collect(),
        vec!["sha256:legacy".to_owned()],
    );

    assert!(read.declares("orders"));
    assert!(!read.declares("checkout"));
    assert!(!read.found_no_declaration());
    assert!(!read.covers_every_entry());
    assert_eq!(read.declared().len(), 1);
}
