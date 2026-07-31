//! A package may not deploy declaring a type no validator can compile.
//!
//! WHY THIS EXISTS. An uncompilable declared schema does not fail loudly at the
//! boundary it is supposed to guard. `aion_package::admit_value` can only
//! answer `UnusableSchema` for it, and every admission site's answer to that is
//! to let the value through UNCHECKED with a log line. So a package carrying
//! one runs with that part of its declared contract switched off, and the only
//! evidence is a warning nobody is reading at the moment it matters. The
//! declaration is identity-bound, which makes it worse: the package's content
//! hash commits to a promise the engine can never hold it to.
//!
//! The door is the last place that can still be a diagnostic. These tests pin
//! that it is one — and, just as load-bearing, that RECOVERY is not a door: a
//! run parked for months must not be stranded because the package it is pinned
//! to declares something a validator cannot compile. Refusing there would turn
//! a validation gap into an availability outage, which for a durable execution
//! engine is the strictly worse failure.

use std::sync::Arc;

use aion::{Engine, EngineBuilder, EngineError};
use aion_package::{
    ActionContract, BeamModule, BeamSet, CURRENT_FORMAT_VERSION, ChildContract, ExtractionLimits,
    Manifest, ManifestVersion, Package, PackageBuilder, PackageContract, SignalContract,
    WorkerContract,
};
use aion_store::{EventStore, InMemoryStore, PackageRecord};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const FIXTURE_MODULE: &str = "aion_fixture_workflow";
const FIXTURE_BEAM: &[u8] = include_bytes!("fixtures/aion_fixture_workflow.beam");

/// A schema whose internal pointer resolves to nothing.
///
/// This is the exact shape of the real defect (#106): a schema door written on
/// its own terms, re-emitted somewhere its `#` no longer means what it meant.
fn dangling_reference() -> Value {
    json!({
        "type": "object",
        "properties": { "line": { "$ref": "#/$defs/Line" } },
        "required": ["line"]
    })
}

/// A schema whose `pattern` is legal ECMA-262 and is refused by the regex
/// engine the validator compiles with.
///
/// A second, unrelated way to be uncompilable, so these tests pin the PROPERTY
/// — "the validator cannot compile it" — rather than one library's quirk.
fn unsupported_pattern() -> Value {
    json!({
        "type": "object",
        "properties": { "code": { "type": "string", "pattern": "^{[0-9]+}$" } },
        "required": ["code"]
    })
}

/// A schema every validator compiles, and which genuinely constrains.
fn enforceable() -> Value {
    json!({
        "type": "object",
        "properties": { "objective": { "type": "string" } },
        "required": ["objective"]
    })
}

fn contract_with(input_schema: Value) -> PackageContract {
    PackageContract {
        input_schema,
        output_schema: json!({}),
        ..PackageContract::default()
    }
}

fn package_declaring(contract: PackageContract) -> Result<Package, Box<dyn std::error::Error>> {
    let beams = BeamSet::new(vec![BeamModule::new(FIXTURE_MODULE, FIXTURE_BEAM)])?;
    let manifest = Manifest {
        entry_module: FIXTURE_MODULE.to_owned(),
        entry_function: "wait".to_owned(),
        input_schema: contract.input_schema.clone(),
        output_schema: contract.output_schema.clone(),
        timeout: None,
        activities: Vec::new(),
        version: ManifestVersion::new("stamped-by-builder"),
        format_version: CURRENT_FORMAT_VERSION,
        additional_workflows: Vec::new(),
    };
    let archive = PackageBuilder::new(manifest, beams)
        .with_contract(contract)
        .write_to_bytes()?;
    Ok(Package::load_from_bytes(
        archive,
        ExtractionLimits::unbounded(),
    )?)
}

async fn engine_over(store: Arc<dyn EventStore>, package: Package) -> Result<Engine, EngineError> {
    EngineBuilder::new()
        .in_process_activity_serving()
        .store_arc(store)
        .in_memory_visibility()
        .scheduler_threads(1)
        .load_workflows(package)
        .build()
        .await
}

async fn engine_with_no_packages(store: Arc<dyn EventStore>) -> Result<Engine, EngineError> {
    EngineBuilder::new()
        .in_process_activity_serving()
        .store_arc(store)
        .in_memory_visibility()
        .scheduler_threads(1)
        .build()
        .await
}

fn refusal_detail(error: &EngineError) -> Result<(usize, String), Box<dyn std::error::Error>> {
    match error {
        EngineError::UnenforceableContract {
            workflow_type,
            count,
            detail,
        } => {
            assert_eq!(
                workflow_type, FIXTURE_MODULE,
                "the refusal must name the package it refused"
            );
            Ok((*count, detail.clone()))
        }
        other => Err(format!(
            "expected the deploy to be refused as an unenforceable contract, got: {other}"
        )
        .into()),
    }
}

/// THE DOOR. A package declaring an entry input type no validator can compile
/// does not deploy, and the refusal names the declaration rather than leaving
/// the operator to guess which of a package's declarations is at fault.
#[tokio::test]
async fn a_package_declaring_an_uncompilable_input_type_is_refused_at_the_door() -> TestResult {
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    let error = engine_over(
        store,
        package_declaring(contract_with(dangling_reference()))?,
    )
    .await
    .err()
    .ok_or("a package whose declared input type compiles into no validator must not deploy")?;

    let (count, detail) = refusal_detail(&error)?;
    assert_eq!(count, 1, "exactly one declaration is unenforceable here");
    assert!(
        detail.contains("the workflow input type"),
        "the refusal must name WHICH declaration cannot be enforced, got: {detail}"
    );
    assert!(
        detail.contains("$defs/Line"),
        "the refusal must carry the compiler's own reason so the author can act on it, got: {detail}"
    );
    Ok(())
}

/// The gate is not a `$ref` special case: an uncompilable `pattern` is refused
/// by the same door, for the same reason.
#[tokio::test]
async fn an_uncompilable_pattern_is_refused_by_the_same_door() -> TestResult {
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    let error = engine_over(
        store,
        package_declaring(contract_with(unsupported_pattern()))?,
    )
    .await
    .err()
    .ok_or("a declared `pattern` the validator cannot compile must not deploy")?;

    let (_, detail) = refusal_detail(&error)?;
    assert!(
        detail.contains("the workflow input type"),
        "the refusal must name the declaration, got: {detail}"
    );
    Ok(())
}

/// NON-VACUITY. The gate refuses only what it should: a contract every
/// validator compiles deploys exactly as before, and the workflow type is
/// routable afterwards.
///
/// Without this, every test above would still pass if the door refused
/// everything.
#[tokio::test]
async fn an_enforceable_contract_still_deploys_and_routes() -> TestResult {
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    let engine = engine_over(store, package_declaring(contract_with(enforceable()))?).await?;
    let versions = engine.list_workflow_versions()?;
    assert_eq!(
        versions.len(),
        1,
        "the enforceable package must be the one loaded version"
    );
    assert!(
        versions
            .iter()
            .any(|version| version.workflow_type == FIXTURE_MODULE && version.route_active),
        "an enforceable package must still win the route: {versions:?}"
    );
    engine.shutdown()?;
    Ok(())
}

/// THE WHOLE CONTRACT, not only the entry input. A declaration the start path
/// never reads is still identity-bound and still promises something, and every
/// one of them is measured at the door.
///
/// Each site gets its own uncompilable schema so a gate that checked only the
/// first, or only the ones some other boundary happens to read, fails here.
#[tokio::test]
async fn every_declared_type_is_measured_not_only_the_entry_input() -> TestResult {
    let contract = PackageContract {
        input_schema: enforceable(),
        output_schema: json!({}),
        signals: vec![SignalContract {
            name: "control".to_owned(),
            input_schema: dangling_reference(),
        }],
        children: vec![ChildContract {
            name: "sub".to_owned(),
            input_schema: enforceable(),
            output_schema: unsupported_pattern(),
        }],
        workers: vec![WorkerContract {
            task_queue: "default".to_owned(),
            actions: vec![ActionContract {
                name: "do_work".to_owned(),
                input_schema: dangling_reference(),
                output_schema: json!({}),
                node: None,
                timeout: None,
                retry: None,
                advisory: false,
                agent: false,
                body: None,
            }],
        }],
        ..PackageContract::default()
    };

    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    let error = engine_over(store, package_declaring(contract)?)
        .await
        .err()
        .ok_or(
            "a package declaring uncompilable signal, child and activity types must not deploy",
        )?;

    let (count, detail) = refusal_detail(&error)?;
    assert_eq!(
        count, 3,
        "every unenforceable declaration must be reported, not just the first: {detail}"
    );
    for named in [
        "the payload type of signal `control`",
        "the result type of child workflow `sub`",
        "the parameter types of activity `do_work` on queue `default`",
    ] {
        assert!(
            detail.contains(named),
            "the refusal must name `{named}`, got: {detail}"
        );
    }
    Ok(())
}

/// The RUNTIME deploy seam refuses it too, and registers nothing when it does.
///
/// The builder door and the live-deploy door are different call paths, and an
/// invariant enforced at one and not the other is the divergence family this
/// codebase keeps producing. A refusal that had already registered modules
/// would also be a lie: the operator is told the deploy failed while the
/// version sits in the catalog.
#[tokio::test]
async fn the_runtime_deploy_seam_refuses_it_and_leaves_the_catalog_untouched() -> TestResult {
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    let engine = engine_over(
        Arc::clone(&store),
        package_declaring(contract_with(enforceable()))?,
    )
    .await?;
    let before = engine.list_workflow_versions()?;
    let persisted_before = store.list_packages().await?;

    let error = engine
        .load_package(package_declaring(contract_with(dangling_reference()))?)
        .await
        .err()
        .ok_or("the runtime deploy seam must refuse an unenforceable contract")?;
    refusal_detail(&error)?;

    assert_eq!(
        engine.list_workflow_versions()?,
        before,
        "a refused deploy must register nothing and re-point nothing"
    );
    assert_eq!(
        store.list_packages().await?,
        persisted_before,
        "a refused deploy must not persist an archive"
    );
    engine.shutdown()?;
    Ok(())
}

/// RECOVERY IS NOT A DOOR. A package already persisted — accepted before this
/// gate existed, or by an older build — still reloads, so runs pinned to it
/// come back.
///
/// This is the deliberate asymmetry, and it is the reason the enforcement is a
/// parameter rather than a constant. An operator offering an archive can fix
/// it and must be told now; a run parked for months can fix nothing, reads no
/// declaration, and must not die for one. The absence is reported at `error`
/// level instead, so it is loud without being fatal.
#[tokio::test]
async fn recovery_still_loads_a_persisted_package_the_door_would_now_refuse() -> TestResult {
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    let package = package_declaring(contract_with(dangling_reference()))?;
    let archive = package.to_archive_bytes()?;
    store
        .put_package(PackageRecord {
            workflow_type: FIXTURE_MODULE.to_owned(),
            content_hash: package.content_hash().to_string(),
            archive,
            deployed_at: chrono::Utc::now(),
        })
        .await?;

    let engine = engine_with_no_packages(Arc::clone(&store)).await?;
    let versions = engine.list_workflow_versions()?;
    assert!(
        versions
            .iter()
            .any(|version| version.workflow_type == FIXTURE_MODULE && version.route_active),
        "a persisted package must recover and keep its route even though its contract is \
         unenforceable — refusing here strands every run pinned to it: {versions:?}"
    );
    engine.shutdown()?;
    Ok(())
}
