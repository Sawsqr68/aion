//! Reachability tests for worker-admission contract selection.
//!
//! Every case here is built from REAL compiled AWL packages loaded into a real
//! engine, because the defect these guard was never in the comparison rule — it
//! was in which retained versions the comparison was handed.

use std::collections::HashMap;

use aion_core::{ContentType, Payload};
use aion_package::{ExtractionLimits, Package};

use super::AdmissionReason;
use crate::EngineBuilder;
use crate::engine::api::Engine;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// v1 of a payments package: `charge` takes an amount only.
const V1: &str = r"//! Contract-drift fixture, first deploy.
workflow contract_drift
  input amount: Int
  outcome completed: type Result, route success

type Result { approved: Bool }

worker payments
  action charge(amount: Int) -> Result

step run
  charge(amount: amount) -> result
  route completed(approved: result.approved)
";

/// v2 of the SAME workflow type: `charge` gained a required `currency`. No one
/// worker advertisement can satisfy both v1 and v2 — v1 sends no `currency`, so
/// a v2-shaped worker narrows its input; v2 sends one, so a v1-shaped worker
/// cannot accept it. That mutual exclusion is exactly the shape that made a
/// live queue unservable.
const V2: &str = r"//! Contract-drift fixture, second deploy.
workflow contract_drift
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

/// v1 of a workflow whose run parks on a signal forever, so the test owns a
/// live, non-terminal registry handle pinned to it with no timing window at
/// all.
const HELD_V1: &str = r"//! Contract-drift fixture whose run parks on a signal.
workflow contract_drift_held
  input amount: Int
  signal go: Ruling
  outcome completed: type Result, route success

type Result { approved: Bool }
type Ruling { proceed: Bool }

worker payments
  action charge(amount: Int) -> Result

step hold
  wait go -> decision

step run
  charge(amount: amount) -> result
  route completed(approved: result.approved)
";

/// v2 of that same workflow type, with the drifted `charge` shape. Deploying it
/// takes the route away from v1 while v1's parked run is still live.
const HELD_V2: &str = r"//! Contract-drift fixture, parked-run workflow, second deploy.
workflow contract_drift_held
  input amount: Int
  input currency: String
  signal go: Ruling
  outcome completed: type Result, route success

type Result { approved: Bool }
type Ruling { proceed: Bool }

worker payments
  action charge(amount: Int, currency: String) -> Result

step hold
  wait go -> decision

step run
  charge(amount: amount, currency: currency) -> result
  route completed(approved: result.approved)
";

async fn engine() -> TestResult<Engine> {
    Ok(EngineBuilder::new()
        .store(aion_store::InMemoryStore::default())
        .in_memory_visibility()
        .build()
        .await?)
}

async fn deploy(engine: &Engine, source: &str) -> TestResult<String> {
    let root = tempfile::tempdir()?;
    let prepared =
        aion_awl_package::compile_and_assemble_awl(source, root.path(), "contract_drift.awl")?;
    let package = Package::load_from_bytes(prepared.archive, ExtractionLimits::unbounded())?;
    let version = package.content_hash().to_string();
    engine.load_package(package).await?;
    Ok(version)
}

/// THE DEFECT. Two versions of one workflow coexist by design (content-hash
/// namespacing). Admission gathered BOTH and demanded one worker satisfy both,
/// which no worker can do once their action shapes differ — so a single stale
/// version made the queue permanently unservable. Only the reachable version
/// may be demanded.
#[tokio::test]
async fn a_superseded_version_with_no_live_run_binds_no_worker() -> TestResult {
    let engine = engine().await?;
    let stale = deploy(&engine, V1).await?;
    let current = deploy(&engine, V2).await?;

    let admission = engine.worker_contracts_for_admission("payments")?;

    assert_eq!(
        admission.required.len(),
        1,
        "only the routed version is reachable: {admission:?}"
    );
    assert_eq!(
        admission.required[0].contract.package_version.to_string(),
        current
    );
    assert_eq!(admission.required[0].reason, AdmissionReason::RouteActive);
    assert_eq!(
        admission
            .unreachable
            .iter()
            .map(|contract| contract.package_version.to_string())
            .collect::<Vec<_>>(),
        vec![stale],
        "the superseded version must be reported as reachable by nothing"
    );
    Ok(())
}

/// The gate keeps its teeth. A version a live run is pinned to is still
/// demanded of every worker, even after a newer version took the route — that
/// run's next activity dispatch lands on exactly this queue.
#[tokio::test]
async fn a_superseded_version_with_a_live_run_still_binds_every_worker() -> TestResult {
    let engine = engine().await?;
    let held = deploy(&engine, HELD_V1).await?;

    // Parks on `wait go` and never leaves it, so the handle is non-terminal for
    // the whole test with no sleeping or polling.
    let handle = engine
        .start_workflow(
            "contract_drift_held",
            Payload::new(ContentType::Json, br#"{"amount":1}"#.to_vec()),
            HashMap::new(),
            "default".to_owned(),
        )
        .await?;
    assert_eq!(handle.loaded_version().to_string(), held);

    // The SAME workflow type is redeployed, taking the route away from the
    // version the parked run is pinned to.
    let successor = deploy(&engine, HELD_V2).await?;
    assert_ne!(successor, held);

    let admission = engine.worker_contracts_for_admission("payments")?;
    let reasons = admission
        .required
        .iter()
        .map(|required| {
            (
                required.contract.package_version.to_string(),
                required.reason,
            )
        })
        .collect::<HashMap<_, _>>();

    assert_eq!(
        reasons.get(&held),
        Some(&AdmissionReason::LiveWorkflow),
        "a superseded version a live run is pinned to still binds every worker: {admission:?}"
    );
    assert_eq!(
        reasons.get(&successor),
        Some(&AdmissionReason::RouteActive),
        "the new route binds every worker: {admission:?}"
    );
    assert!(
        admission.unreachable.is_empty(),
        "one version routes, the other holds a live run: {admission:?}"
    );
    Ok(())
}

/// A rolled-back route is the reachable one. `POST /deploy/route` re-points a
/// workflow type at an already-loaded version, and admission must follow the
/// route pointer rather than a "newest deployed" reading of it — otherwise a
/// rollback would demand the shape of the version it just rolled away from.
#[tokio::test]
async fn a_rollback_moves_the_demanded_version_with_the_route() -> TestResult {
    let engine = engine().await?;
    let first = deploy(&engine, V1).await?;
    let second = deploy(&engine, V2).await?;
    let rolled_back = first.parse()?;

    engine
        .route_workflow_version("contract_drift", &rolled_back)
        .await?;

    let admission = engine.worker_contracts_for_admission("payments")?;
    assert_eq!(admission.required.len(), 1, "{admission:?}");
    assert_eq!(
        admission.required[0].contract.package_version.to_string(),
        first,
        "the rolled-back-to version is the one new starts resolve"
    );
    assert_eq!(
        admission
            .unreachable
            .iter()
            .map(|contract| contract.package_version.to_string())
            .collect::<Vec<_>>(),
        vec![second],
        "the rolled-away-from version binds nobody once nothing runs on it"
    );
    Ok(())
}

/// A queue no deployed package declares has nothing to hold a worker to. The
/// empty answer must be the empty answer, not an error and not a refusal.
#[tokio::test]
async fn an_undeclared_queue_demands_nothing() -> TestResult {
    let engine = engine().await?;
    drop(deploy(&engine, V1).await?);

    let admission = engine.worker_contracts_for_admission("no_such_queue")?;
    assert!(admission.required.is_empty(), "{admission:?}");
    assert!(admission.unreachable.is_empty(), "{admission:?}");
    Ok(())
}
