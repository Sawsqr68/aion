//! `default` belongs to the out-of-box workers (#200).
//!
//! # The defect this pins
//!
//! The built-in assistant used to ship on task queue `default`. `default` is
//! where a worker comes up when nobody has configured it otherwise — it is the
//! first queue a newcomer's worker lands on — and contract admission holds a
//! registering worker against EVERY reachable contract on its queue at once. So
//! on a fresh server the assistant's own contract demanded the `assistant`
//! action of every arriving worker, and a worker doing its whole job for its
//! own workflow was refused for omitting an action that was never its job. The
//! assistant starved the workers it exists to welcome.
//!
//! # What is proved here, and how it could fail
//!
//! Both arms run in ONE test against ONE engine and ONE worker advertisement,
//! and the only thing that varies is which queue the worker registers on:
//!
//! * on `default` the worker is ADMITTED — the fix,
//! * on the assistant's own queue the SAME worker is REFUSED, naming the
//!   `assistant` action.
//!
//! The second arm is what keeps the first honest. Admission really does demand
//! the assistant's action of a worker on the assistant's queue, so the green on
//! `default` is the absence of the assistant there and not a gate that stopped
//! demanding anything. A regression that put the assistant back on `default`
//! turns the first arm red; a change that quietly disabled admission turns the
//! second red.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

use aion::{Engine, EngineBuilder};
use aion_package::{ActivityDescriptor, ExtractionLimits, Package};
use aion_server::assistant::{
    AssistantInstall, EmbeddedAssistant, WorkerListenerAdvice, install_embedded_assistant,
    private_task_queue,
};
use aion_server::worker::AdmissionAudit;
use aion_server::worker::contracts::{WorkerAdvertisement, validate_worker_contracts};
use aion_store::{EventStore, InMemoryStore};

type TestError = Box<dyn std::error::Error>;

/// A plain workflow of the kind a newcomer writes first: one action, on
/// `default`, because that is the queue the getting-started worker serves.
const OUT_OF_BOX_DOCUMENT: &str = "//! The first workflow a newcomer writes, on the queue their \
                                   first worker serves.\n\
                                   workflow out_of_box\n  \
                                   input name: String\n  \
                                   outcome greeted: type Greeting, route success\n\n\
                                   type Greeting { greeting: String }\n\n\
                                   worker default\n  \
                                   action greet(name: String) -> Greeting\n\n\
                                   step greet_them\n  \
                                   name |> greet |> route greeted\n";

/// What it means if the defect reproduction is ADMITTED: there was never a
/// defect to fix, and the queue move bought nothing.
const ADMITTED_UNDER_THE_DEFECT: &str = concat!(
    "an assistant on `default` must refuse a worker that does not ",
    "advertise its `assistant` action — if this passes, #200 was never ",
    "a defect and moving the queue fixed nothing",
);

/// The schema root presented to the compiler. The documents here declare no
/// `schema(…)` imports, so it is never read; naming a path that does not exist
/// makes a future import fail loudly rather than resolve against the test's cwd.
const NO_SCHEMA_ROOT: &str = "<assistant-private-queue-test-has-no-schema-directory>";

async fn engine() -> Result<Arc<Engine>, TestError> {
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    Ok(Arc::new(
        EngineBuilder::new()
            .store_arc(store)
            .in_memory_visibility()
            .scheduler_threads(1)
            .build()
            .await?,
    ))
}

/// Compiles an AWL document into a validated package, exactly as a deploy does.
fn package_of(source: &str, filename: &str) -> Result<Package, TestError> {
    let prepared =
        aion_awl_package::compile_and_assemble_awl(source, Path::new(NO_SCHEMA_ROOT), filename)?;
    Ok(Package::load_from_bytes(
        &prepared.archive,
        ExtractionLimits::unbounded(),
    )?)
}

/// The advertisement a worker built from `package` would make: every action the
/// package's contract requires of an out-of-band worker, with the package's own
/// schemas.
///
/// Derived from the document rather than hand-written, so the worker under test
/// is one that does its whole job. A hand-written advertisement could be made to
/// pass by weakening it, which would prove nothing about the queue.
fn advertisement_of(package: &Package) -> Result<Vec<ActivityDescriptor>, TestError> {
    let contract = package.contract()?;
    Ok(contract
        .workers
        .iter()
        .flat_map(|worker| worker.actions.iter())
        .filter(|action| action.body.is_none())
        .map(|action| ActivityDescriptor {
            name: action.name.clone(),
            input_schema: action.input_schema.clone(),
            output_schema: action.output_schema.clone(),
        })
        .collect())
}

/// THE #200 REGRESSION. A fresh server with the assistant installed admits an
/// out-of-box worker on `default`, and the same worker on the assistant's queue
/// is refused — so the green above is the assistant's absence from `default`.
#[tokio::test]
async fn a_fresh_server_serves_default_workflows_with_no_interference_from_the_assistant()
-> Result<(), TestError> {
    let engine = engine().await?;
    let embedded = EmbeddedAssistant::load()?;

    let install = install_embedded_assistant(
        engine.as_ref(),
        WorkerListenerAdvice::without_boot_context(),
    )
    .await;
    assert_eq!(
        install,
        AssistantInstall::Installed {
            workflow_type: embedded.workflow_type().to_owned(),
            content_hash: embedded.content_hash().to_string(),
            task_queue: embedded.task_queue().to_owned(),
        },
        "the fresh-server arm needs the assistant actually installed and routed, or it proves \
         nothing about coexisting with it"
    );

    let out_of_box = package_of(OUT_OF_BOX_DOCUMENT, "out_of_box.awl")?;
    let descriptors = advertisement_of(&out_of_box)?;
    assert_eq!(
        descriptors
            .iter()
            .map(|d| d.name.as_str())
            .collect::<Vec<_>>(),
        vec!["greet"],
        "the newcomer's worker advertises its own one action and nothing else"
    );
    engine.load_package(out_of_box).await?;

    let activity_types: BTreeSet<String> = descriptors.iter().map(|d| d.name.clone()).collect();
    let advertised = WorkerAdvertisement {
        activity_types: &activity_types,
        contracts: &descriptors,
    };
    let audit = AdmissionAudit::default();

    validate_worker_contracts(
        engine.as_ref(),
        &audit,
        aion_core::DEFAULT_TASK_QUEUE,
        None,
        "out-of-box-worker",
        advertised,
    )
    .map_err(|error| {
        format!(
            "a worker serving its own `default` workflow was REFUSED on a fresh server — this is \
             #200: the built-in assistant is claiming `default` again. {error}"
        )
    })?;

    // THE CONTROL. The same worker, the same engine, the assistant's queue.
    match validate_worker_contracts(
        engine.as_ref(),
        &audit,
        embedded.task_queue(),
        None,
        "out-of-box-worker",
        advertised,
    ) {
        Ok(()) => Err(format!(
            "admission ADMITTED a worker that advertises no `assistant` action onto queue `{}`. \
             Admission must still demand the assistant's action there; if it does not, the pass \
             on `default` above measured a gate that demands nothing of anybody",
            embedded.task_queue()
        )
        .into()),
        Err(refusal) => {
            let refusal = refusal.to_string();
            assert!(
                refusal.contains("assistant"),
                "the control refusal must name the action it demanded; got: {refusal}"
            );
            Ok(())
        }
    }
}

/// THE DEFECT, REPRODUCED. The same fresh server, the same out-of-box worker,
/// and an assistant on `default` — and the worker is refused.
///
/// This is the arm that gives the two above their meaning. Without it "the
/// worker was admitted" is compatible with a server where nothing was ever
/// demanded of it, and the fix would look identical to never having had the
/// defect. Here the ONLY difference from the passing case is which queue the
/// assistant's contract declares, and the verdict flips.
///
/// The fixture is the shipped document with its `worker` block put back — the
/// single line the fix changed, which is also the only line of it that reaches
/// the compiled contract. The contract is checked before the arm runs, so a
/// fixture that failed to reconstruct the defect fails here rather than
/// reporting a green.
#[tokio::test]
async fn an_assistant_on_default_refuses_the_out_of_box_worker() -> Result<(), TestError> {
    let engine = engine().await?;

    let on_default: &str = &aion_server::assistant::EMBEDDED_ASSISTANT_DOCUMENT
        .replace("\nworker assistant\n", "\nworker default\n");
    assert_ne!(
        on_default,
        aion_server::assistant::EMBEDDED_ASSISTANT_DOCUMENT,
        "the fixture must differ from the shipped document, or it reproduces nothing"
    );
    let prior = package_of(on_default, "assistant.awl")?;
    let declared: Vec<&str> = prior
        .contract()?
        .workers
        .iter()
        .map(|worker| worker.task_queue.as_str())
        .collect();
    assert_eq!(
        declared,
        vec![aion_core::DEFAULT_TASK_QUEUE],
        "the fixture must actually put the assistant on `default`, or this test measures the fix"
    );
    engine.load_package(prior).await?;

    let out_of_box = package_of(OUT_OF_BOX_DOCUMENT, "out_of_box.awl")?;
    let descriptors = advertisement_of(&out_of_box)?;
    engine.load_package(out_of_box).await?;

    let activity_types: BTreeSet<String> = descriptors.iter().map(|d| d.name.clone()).collect();
    let advertised = WorkerAdvertisement {
        activity_types: &activity_types,
        contracts: &descriptors,
    };
    let audit = AdmissionAudit::default();

    match validate_worker_contracts(
        engine.as_ref(),
        &audit,
        aion_core::DEFAULT_TASK_QUEUE,
        None,
        "out-of-box-worker",
        advertised,
    ) {
        Ok(()) => Err(ADMITTED_UNDER_THE_DEFECT.into()),
        Err(refusal) => {
            let refusal = refusal.to_string();
            assert!(
                refusal.contains("assistant"),
                "the reproduction must be refused FOR the assistant's action; got: {refusal}"
            );
            Ok(())
        }
    }
}

/// A fresh catalog declares the assistant's own queue and does NOT declare
/// `default` — the assistant's absence from `default` read from the catalog
/// itself rather than inferred from an admission verdict.
#[tokio::test]
async fn a_fresh_catalog_declares_the_private_queue_and_not_default() -> Result<(), TestError> {
    let engine = engine().await?;
    let embedded = EmbeddedAssistant::load()?;
    let install = install_embedded_assistant(
        engine.as_ref(),
        WorkerListenerAdvice::without_boot_context(),
    )
    .await;
    assert!(matches!(install, AssistantInstall::Installed { .. }));

    let declared = engine.declared_task_queues()?;
    assert!(
        declared.covers_every_entry(),
        "an incomplete read cannot report an absence — its missing queues are unknown, not absent"
    );
    assert!(
        declared.declares(embedded.task_queue()),
        "the installed assistant must declare its own private queue"
    );
    assert!(
        !declared.declares(aion_core::DEFAULT_TASK_QUEUE),
        "a server whose only deploy is the built-in assistant must leave `default` undeclared — \
         it belongs to the workers an operator brings up (#200)"
    );
    assert_eq!(
        embedded.task_queue(),
        private_task_queue(embedded.workflow_type()),
        "the queue the catalog declares is the derivation, not a value chosen beside it"
    );
    Ok(())
}
