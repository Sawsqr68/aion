//! Liminal registration contract admission tests on the real wire DTO.

use std::sync::Arc;

use aion_package::{ExtractionLimits, Package};
use liminal::protocol::{WorkerActivityDescriptor, WorkerRegistration};

use super::{ConnectedWorkerRegistry, LiminalConnectionNotifier};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const SOURCE: &str = r"//! Liminal worker contract fixture.
workflow liminal_contract_test
  input amount: Int
  outcome completed: type Result, route success

type Result { approved: Bool }

worker payments
  action charge(amount: Int) -> Result

step run
  charge(amount: amount) -> result
  route completed(approved: result.approved)
";

async fn notifier_with_contract() -> TestResult<(LiminalConnectionNotifier, String)> {
    let root = tempfile::tempdir()?;
    let prepared = aion_awl_package::compile_and_assemble_awl(SOURCE, root.path())?;
    let package = Package::load_from_bytes(prepared.archive, ExtractionLimits::unbounded())?;
    let version = package.content_hash().to_string();
    let engine = aion::EngineBuilder::new()
        .store(aion_store::InMemoryStore::default())
        .in_memory_visibility()
        .build()
        .await?;
    engine.load_package(package).await?;
    let notifier = LiminalConnectionNotifier::new(ConnectedWorkerRegistry::default())
        .with_contract_catalog(Arc::new(engine));
    Ok((notifier, version))
}

fn registration(input_type: &str) -> WorkerRegistration {
    WorkerRegistration {
        namespaces: vec!["default".to_owned()],
        task_queue: "payments".to_owned(),
        node: None,
        activity_types: vec!["charge".to_owned()],
        identity: "liminal-contract-test-worker".to_owned(),
        activities: vec![WorkerActivityDescriptor {
            name: "charge".to_owned(),
            input_schema_json: serde_json::json!({
                "type": "object",
                "properties": {"amount": {"type": input_type}},
                "required": ["amount"]
            })
            .to_string(),
            output_schema_json: serde_json::json!({
                "type": "object",
                "properties": {"approved": {"type": "boolean"}},
                "required": ["approved"]
            })
            .to_string(),
        }],
    }
}

#[tokio::test]
async fn matching_liminal_registration_passes_pre_insertion_admission() -> TestResult {
    let (notifier, _) = notifier_with_contract().await?;
    notifier.validate_registration_contract(&registration("integer"))?;
    Ok(())
}

#[tokio::test]
async fn mismatched_liminal_registration_names_field_and_exact_version() -> TestResult {
    let (notifier, version) = notifier_with_contract().await?;
    let Err(error) = notifier.validate_registration_contract(&registration("string")) else {
        return Err("a narrowed incompatible input must be refused".into());
    };
    let message = error.to_string();

    assert!(message.contains("WORKER_CONTRACT_MISMATCH"));
    assert!(message.contains("input_schema.properties.amount.type"));
    assert!(message.contains(&version));
    Ok(())
}
