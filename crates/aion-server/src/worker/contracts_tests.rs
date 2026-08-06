//! Worker-admission tests at the shared gate every registration transport uses.
//!
//! These are built from REAL compiled AWL packages loaded into a REAL engine,
//! because the incident these guard was never a comparison bug — the comparison
//! was right and was handed the wrong set of contracts.

use aion_package::{ActivityDescriptor, ExtractionLimits, Package};
use serde_json::json;

use super::super::admission_audit::AdmissionAudit;
use super::{ContractAdmissionError, WorkerAdvertisement, validate_worker_contracts};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// First deploy: `charge` takes an amount only.
const V1: &str = r"//! Worker-admission fixture, first deploy.
workflow admission_drift
  input amount: Int
  outcome completed: type Result, route success

type Result { approved: Bool }

worker payments
  action charge(amount: Int) -> Result

step run
  charge(amount: amount) -> result
  route completed(approved: result.approved)
";

/// Second deploy of the SAME workflow type: `charge` gained a required
/// `currency` — EXACTLY the live incident, where the stale package's
/// `verify_integration` lacked the `base_branch` field the current worker
/// advertises. Input compatibility is contravariant, so a worker built for v2
/// (which requires `currency`) cannot serve v1 (which never sends one): it
/// would narrow the input v1 dispatches. Under the old gate that single stale
/// version made the queue permanently unservable.
const V2: &str = r"//! Worker-admission fixture, second deploy.
workflow admission_drift
  input amount: Int
  input currency: String
  outcome completed: type Result, route success

type Result { approved: Bool }

worker payments
  action charge(amount: Int, currency: String) -> Result

step run
  charge(amount: amount, currency: currency) -> result
  route completed(approved: result.approved)
";

/// A third deploy that RETYPES the amount. Incompatible in both directions, so
/// it is the fixture for proving the gate still has teeth against the version
/// that actually routes new starts.
const V3: &str = r"//! Worker-admission fixture, retyped third deploy.
workflow admission_drift
  input amount: String
  outcome completed: type Result, route success

type Result { approved: Bool }

worker payments
  action charge(amount: String) -> Result

step run
  charge(amount: amount) -> result
  route completed(approved: result.approved)
";

async fn engine() -> TestResult<aion::Engine> {
    Ok(aion::EngineBuilder::new()
        .store(aion_store::InMemoryStore::default())
        .in_memory_visibility()
        .build()
        .await?)
}

async fn deploy(engine: &aion::Engine, source: &str) -> TestResult<String> {
    let root = tempfile::tempdir()?;
    let prepared =
        aion_awl_package::compile_and_assemble_awl(source, root.path(), "admission_drift.awl")?;
    let package = Package::load_from_bytes(prepared.archive, ExtractionLimits::unbounded())?;
    let version = package.content_hash().to_string();
    engine.load_package(package).await?;
    Ok(version)
}

/// A worker advertising exactly the v1 shape.
fn v1_worker() -> Vec<ActivityDescriptor> {
    vec![ActivityDescriptor {
        name: "charge".to_owned(),
        input_schema: json!({
            "type": "object",
            "properties": {"amount": {"type": "integer"}},
            "required": ["amount"]
        }),
        output_schema: json!({
            "type": "object",
            "properties": {"approved": {"type": "boolean"}},
            "required": ["approved"]
        }),
    }]
}

/// A worker advertising exactly the v2 shape.
fn v2_worker() -> Vec<ActivityDescriptor> {
    vec![ActivityDescriptor {
        name: "charge".to_owned(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "amount": {"type": "integer"},
                "currency": {"type": "string"}
            },
            "required": ["amount", "currency"]
        }),
        output_schema: json!({
            "type": "object",
            "properties": {"approved": {"type": "boolean"}},
            "required": ["approved"]
        }),
    }]
}

/// The activity-type NAME set a worker advertises.
fn names(names: &[&str]) -> std::collections::BTreeSet<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

/// Pair the two advertised forms for one registration.
fn advertisement<'a>(
    activity_types: &'a std::collections::BTreeSet<String>,
    contracts: &'a [ActivityDescriptor],
) -> WorkerAdvertisement<'a> {
    WorkerAdvertisement {
        activity_types,
        contracts,
    }
}

/// THE INCIDENT, at the gate that refused it. A queue carries two deployed
/// versions whose contracts differ; the older one no longer routes and has no
/// live run. A worker built for the current version — doing its whole job —
/// must be ADMITTED. On 2026-07-30 it was refused on ~20 consecutive dials with
/// `WORKER_CONTRACT_MISMATCH`, and the only way out was manually unloading the
/// stale package.
#[tokio::test]
async fn a_worker_matching_the_current_version_is_admitted_despite_a_stale_one() -> TestResult {
    let engine = engine().await?;
    let stale = deploy(&engine, V1).await?;
    let current = deploy(&engine, V2).await?;
    assert_ne!(stale, current);

    // The test is only worth anything if the stale contract genuinely refuses
    // this worker: pin that first, so an admission gate that had simply stopped
    // comparing could never make this test pass.
    let stale_contract = engine
        .worker_contracts_for_queue("payments")?
        .into_iter()
        .find(|deployed| deployed.package_version.to_string() == stale)
        .ok_or("the stale version is not retained")?;
    assert!(
        !aion_package::contract_diffs(&stale, &stale_contract.contract, None, &v2_worker())
            .is_empty(),
        "the fixture must reproduce the incident: the stale version has to be unsatisfiable by \
         the worker built for the current one"
    );

    validate_worker_contracts(
        &engine,
        &AdmissionAudit::new(),
        "payments",
        None,
        "drift-worker",
        advertisement(&names(&["charge"]), &v2_worker()),
    )?;
    Ok(())
}

/// The gate keeps its teeth. Narrowing the set of versions a worker is held to
/// must not narrow the check itself: a worker that cannot serve the version
/// routing new starts is still refused, and the refusal now says WHICH version,
/// WHY that version was held, WHICH field disagreed, which retained versions
/// were ignored, and what to do next.
#[tokio::test]
async fn a_worker_that_cannot_serve_the_routed_version_is_still_refused() -> TestResult {
    let engine = engine().await?;
    let stale = deploy(&engine, V1).await?;
    let current = deploy(&engine, V3).await?;

    let Err(error) = validate_worker_contracts(
        &engine,
        &AdmissionAudit::new(),
        "payments",
        None,
        "stale-worker",
        advertisement(&names(&["charge"]), &v1_worker()),
    ) else {
        return Err("a worker that cannot accept the routed version's input was admitted".into());
    };
    let message = error.to_string();

    assert!(message.contains("WORKER_CONTRACT_MISMATCH"), "{message}");
    assert!(
        message.contains(&current),
        "the refusal must name the version that was held against the worker: {message}"
    );
    assert!(
        message.contains("held because it currently routes new starts"),
        "the refusal must say WHY that version was held: {message}"
    );
    assert!(
        message.contains("input_schema.properties.amount.type"),
        "the refusal must name the field that disagreed: {message}"
    );
    assert!(
        message.contains(&stale),
        "the refusal must name the retained version it did NOT hold against the worker, so an \
         operator is not left wondering which deploys were in play: {message}"
    );
    assert!(
        message.contains("ignored 1 unreachable version"),
        "the ignored version must be marked as ignored, not silently mixed in: {message}"
    );
    assert!(
        message.contains("REMEDY"),
        "the refusal must carry the operator's next move: {message}"
    );
    assert!(
        message.contains("POST /deploy/route"),
        "a route-active version cannot be unloaded, so the remedy must name the calls that DO \
         make it removable: {message}"
    );
    Ok(())
}

/// Unloading is never offered for a version named in a disagreement, because
/// every reason admission demands a version is a reason unload refuses it — so
/// that advice would send an operator into a second refusal. The unreachable
/// versions ARE offered, with the exact request body (there is no `aion
/// undeploy` verb), and labelled as hygiene that will not change the refusal.
#[tokio::test]
async fn the_remedy_offers_unload_only_for_the_versions_that_can_be_unloaded() -> TestResult {
    let engine = engine().await?;
    let stale = deploy(&engine, V1).await?;
    let current = deploy(&engine, V3).await?;

    let Err(error) = validate_worker_contracts(
        &engine,
        &AdmissionAudit::new(),
        "payments",
        None,
        "stale-worker",
        advertisement(&names(&["charge"]), &v1_worker()),
    ) else {
        return Err("a worker that cannot accept the routed version's input was admitted".into());
    };
    let message = error.to_string();

    assert!(
        message.contains(&format!(
            r#"POST /deploy/unload {{"workflow_type":"admission_drift","content_hash":"{stale}"}}"#
        )),
        "the ignored version's exact unload body must be in the message: {message}"
    );
    assert!(
        message.contains("will NOT change this refusal"),
        "clearing an ignored version cannot fix a mismatch, and the message must say so: {message}"
    );
    assert!(
        !message.contains(&format!(r#""content_hash":"{current}""#)),
        "the demanded version can never be unloaded and must not be offered for it: {message}"
    );
    Ok(())
}

/// A queue that no deployed package declares holds a worker to nothing. This is
/// the pre-deploy boot order (workers up before the first package), and it must
/// admit rather than refuse.
#[tokio::test]
async fn a_queue_no_package_declares_admits_any_worker() -> TestResult {
    let engine = engine().await?;
    drop(deploy(&engine, V1).await?);

    validate_worker_contracts(
        &engine,
        &AdmissionAudit::new(),
        "other_queue",
        None,
        "unrelated-worker",
        advertisement(&names(&[]), &[]),
    )?;
    Ok(())
}

/// THE 2026-07-30 LIVE REFUSAL LOOP, both sides of it.
///
/// A worker advertises the action's NAME — so the dispatcher would select it —
/// but registers no typed contract for it. Admission compares contracts, so the
/// action is `<missing>` and the worker is refused; the refusal record used to
/// print the name set beside that verdict and so appeared to contradict itself.
///
/// The refused half asserts the refusal now RECONCILES the two sets by naming
/// the gap. The admitted half is the control on the same fixture: add the
/// contract, change nothing else, and the worker is admitted — which is what
/// proves the refusal is about the contract and not about the name.
#[tokio::test]
async fn a_name_without_a_contract_is_refused_with_a_message_that_names_the_gap() -> TestResult {
    let engine = engine().await?;
    drop(deploy(&engine, V1).await?);
    let advertised_names = names(&["charge"]);

    // Refused half: the name is advertised, the contract is not.
    let Err(error) = validate_worker_contracts(
        &engine,
        &AdmissionAudit::new(),
        "payments",
        None,
        "name-only-worker",
        advertisement(&advertised_names, &[]),
    ) else {
        return Err("a worker advertising no contract for a required action was admitted".into());
    };
    let message = error.to_string();

    assert!(message.contains("WORKER_CONTRACT_MISMATCH"), "{message}");
    assert!(
        message.contains("action `charge` field `action`"),
        "the refusal must still name the missing action: {message}"
    );
    assert!(
        message.contains("worker advertised <missing>"),
        "the refusal must still report the action as advertised-missing: {message}"
    );
    // The reconciliation: the same message that says `<missing>` states that
    // the name WAS advertised and that admission compares the other set.
    assert!(
        message.contains("admission compares CONTRACTS"),
        "the refusal must say which of the two advertised sets it compared: {message}"
    );
    assert!(
        message.contains("1 action advertised by name with NO contract: `charge`"),
        "the refusal must name the action that is advertised by name but not by contract, or it \
         contradicts the name set the log prints beside it: {message}"
    );
    assert!(
        message.contains("worker advertised 1 activity-type name and 0 typed contracts"),
        "the refusal must state both advertised counts: {message}"
    );
    assert!(
        message.contains("must announce an input and output schema at registration"),
        "the remedy must say what to do about the gap it just named: {message}"
    );

    // Admitted half — the control. Same engine, same queue, same name set; the
    // ONLY difference is that the contract is now advertised.
    validate_worker_contracts(
        &engine,
        &AdmissionAudit::new(),
        "payments",
        None,
        "contract-worker",
        advertisement(&advertised_names, &v1_worker()),
    )?;
    Ok(())
}

/// The reconciling clause must not fire when there is nothing to reconcile: a
/// worker whose every advertised name carries a contract is refused (if at all)
/// on the schemas alone, with no gap sentence to send an operator hunting for a
/// missing descriptor that is not missing.
#[tokio::test]
async fn a_schema_mismatch_alone_never_reports_a_missing_contract() -> TestResult {
    let engine = engine().await?;
    drop(deploy(&engine, V1).await?);
    drop(deploy(&engine, V3).await?);

    let Err(error) = validate_worker_contracts(
        &engine,
        &AdmissionAudit::new(),
        "payments",
        None,
        "stale-worker",
        advertisement(&names(&["charge"]), &v1_worker()),
    ) else {
        return Err("a worker that cannot accept the routed version's input was admitted".into());
    };
    let message = error.to_string();

    assert!(
        message.contains("input_schema.properties.amount.type"),
        "the fixture must really be a schema disagreement: {message}"
    );
    assert!(
        !message.contains("advertised by name with NO contract"),
        "every advertised name carries a contract here, so the gap clause must stay silent: \
         {message}"
    );
    assert!(
        !message.contains("must announce an input and output schema at registration"),
        "the gap remedy must stay silent when there is no gap: {message}"
    );
    Ok(())
}

/// The gap is computed from the two sets, never from a hardcoded list: a name
/// with a contract is not in it, a name without one is, and the answer is
/// independent of how many of each there are.
#[test]
fn the_gap_is_exactly_the_advertised_names_carrying_no_contract() {
    let advertised_names = names(&["charge", "refund", "settle"]);
    let contracts = v1_worker();
    let advertisement = advertisement(&advertised_names, &contracts);

    let gap = advertisement.names_without_contracts();

    let described = contracts
        .iter()
        .map(|contract| contract.name.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let expected = advertised_names
        .iter()
        .filter(|name| !described.contains(*name))
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(gap, expected);
    // Non-vacuity: the fixture really does contain both kinds of name.
    assert!(
        !gap.is_empty(),
        "the fixture must contain an undescribed name"
    );
    assert!(
        gap.len() < advertised_names.len(),
        "the fixture must also contain a described name, or the filter is untested"
    );
}

/// A contract advertised for an action the worker did NOT name is not a gap —
/// the gap is one-directional, and reporting it both ways would invent a fault.
#[test]
fn a_contract_without_a_matching_advertised_name_is_not_a_gap() {
    let advertised_names = names(&[]);
    let contracts = v1_worker();

    assert!(
        advertisement(&advertised_names, &contracts)
            .names_without_contracts()
            .is_empty()
    );
}

/// A catalog read that cannot answer must refuse, never admit. The typed
/// classification is what makes that visible at the transport, so it must stay
/// distinct from a mismatch.
#[test]
fn a_catalog_failure_is_classified_apart_from_a_mismatch() {
    let error = ContractAdmissionError::Catalog {
        source: aion::EngineError::CatalogPoisoned,
    };
    let message = error.to_string();
    assert!(
        message.contains("contract catalog lookup failed"),
        "{message}"
    );
    assert!(
        !message.contains("WORKER_CONTRACT_MISMATCH"),
        "an unreadable catalog is not a contract disagreement: {message}"
    );
}
