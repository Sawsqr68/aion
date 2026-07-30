//! The admission gate, run for real.
//!
//! These tests do not check that the descriptors "look right". They run the
//! SAME function the server runs to admit a worker —
//! `aion_package::contract_diffs` — against the contract compiled from the
//! shipped `.awl` documents, and require it to return nothing. Anything less
//! is a proxy for the gate, and a proxy is what let this worker ship a
//! registration that no contract would accept.

use aion_package::contract_diffs;
use serde_json::json;

use super::{ASSISTANT_DOCUMENT, advertised_surface};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Compile a shipped document and return its one worker contract.
fn contract_of(source: &str) -> Result<aion_package::WorkerContract, Box<dyn std::error::Error>> {
    let compiled = aion_awl::compile(source, std::path::Path::new("."))?;
    compiled
        .contract
        .workers
        .first()
        .cloned()
        .ok_or_else(|| "the document declares no worker block".into())
}

/// The full port — the document this worker exists to serve.
#[test]
fn the_advertised_surface_is_admitted_by_the_assistant_contract() -> TestResult {
    let surface = advertised_surface()?;
    let contract = contract_of(ASSISTANT_DOCUMENT)?;
    let diffs = contract_diffs("assistant-test", &contract, None, &surface.descriptors);
    assert!(
        diffs.is_empty(),
        "the worker would be refused by the contract it serves:\n{}",
        diffs
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    );
    Ok(())
}

/// The negative control the whole exercise turns on. Before this change the
/// worker advertised an EMPTY descriptor list while its name list was full, so
/// this asserts the gate really does refuse that — otherwise the test above
/// could pass for reasons unrelated to the descriptors.
#[test]
fn advertising_no_descriptors_is_refused_by_the_same_gate() -> TestResult {
    let contract = contract_of(ASSISTANT_DOCUMENT)?;
    let diffs = contract_diffs("assistant-test", &contract, None, &[]);
    assert!(
        !diffs.is_empty(),
        "an empty advertisement must be refused — if it is not, the gate is not \
         what admits this worker and these tests prove nothing"
    );
    let rendered = diffs
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    for action in ["assistant", "assistant_provision"] {
        assert!(
            rendered.contains(action),
            "the refusal must name `{action}`; got:\n{rendered}"
        );
    }
    Ok(())
}

/// A descriptor whose schema DRIFTS from the document is refused just as an
/// absent one is. This is the failure mode a hand-written schema would have
/// produced, and the reason the surface is derived instead.
#[test]
fn a_drifted_schema_is_refused() -> TestResult {
    let contract = contract_of(ASSISTANT_DOCUMENT)?;
    let mut descriptors = advertised_surface()?.descriptors;
    let drifted = descriptors
        .iter_mut()
        .find(|descriptor| descriptor.name == "assistant")
        .ok_or("the surface must advertise `assistant`")?;
    // Narrow the input: demand a field the workflow never sends.
    drifted.input_schema = json!({
        "type": "object",
        "properties": { "prompt": { "type": "string" }, "seed": { "type": "string" } },
        "required": ["prompt", "seed"]
    });
    let diffs = contract_diffs("assistant-test", &contract, None, &descriptors);
    assert!(
        !diffs.is_empty(),
        "a narrowed input schema must be refused, or drift would ship silently"
    );
    Ok(())
}

/// Every advertised name carries a schema, and vice versa. The registration bug
/// was exactly a name list and a descriptor list that disagreed, so the two can
/// only ever come from one source.
#[test]
fn names_and_descriptors_cannot_disagree() -> TestResult {
    let surface = advertised_surface()?;
    assert_eq!(
        surface.names().len(),
        surface.descriptors.len(),
        "names are derived from descriptors, so they cannot differ in count"
    );
    assert!(
        !surface.descriptors.is_empty(),
        "an empty surface is refused by every contract-bearing queue"
    );
    for descriptor in &surface.descriptors {
        assert!(
            !descriptor.name.is_empty(),
            "an unnamed descriptor cannot be matched to an action"
        );
        assert!(
            descriptor.input_schema.is_object() || descriptor.input_schema.is_boolean(),
            "action `{}` must advertise a real input schema, got {}",
            descriptor.name,
            descriptor.input_schema
        );
    }
    Ok(())
}

/// The queue comes from the document too, so the worker cannot serve one queue
/// while advertising another's actions.
#[test]
fn the_queue_comes_from_the_document() -> TestResult {
    let surface = advertised_surface()?;
    let contract = contract_of(ASSISTANT_DOCUMENT)?;
    assert_eq!(surface.task_queue, contract.task_queue);
    Ok(())
}

/// The surface must be deterministic: two derivations advertise identically, or
/// a reconnect could present a different surface than the first registration.
#[test]
fn the_surface_is_deterministic() -> TestResult {
    assert_eq!(advertised_surface()?, advertised_surface()?);
    Ok(())
}

/// The embedded document is the one in the tree, not a stale copy. `include_str!`
/// makes this true at build time; asserting it here makes the reason visible.
#[test]
fn the_embedded_document_is_the_shipped_one() -> TestResult {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assistant/awl/assistant.awl"
    );
    let on_disk = std::fs::read_to_string(path)?;
    assert_eq!(
        ASSISTANT_DOCUMENT, on_disk,
        "the embedded contract must be the shipped document"
    );
    Ok(())
}

/// THE INVARIANT, replacing the test that pinned the conflict.
///
/// Every AWL document deployed to this worker's queue must declare the shared
/// actions identically, because contract admission holds a registering worker
/// against EVERY reachable contract on that queue at once. One document that
/// disagrees does not degrade gracefully — the worker is refused outright and
/// NONE of the documents on the queue can run. That is what took `default`
/// down: the spike declared `assistant_provision(repo_path, run_id)` while the
/// full session added `objective`, and input compatibility is contravariant, so
/// no single worker could satisfy both.
///
/// This asserts the property that must hold FOREVER, not the accident that the
/// two once disagreed. A new `.awl` document sharing this queue belongs in the
/// list below, and if it disagrees this test says so before a deploy does.
///
/// Note the deliberate asymmetry with `assistant.awl`: a document may declare a
/// SUBSET of the queue's actions, but every action it does declare must match.
/// The worker advertises the union, so extra advertised actions are fine —
/// admission permits a worker to serve more than one contract needs.
#[test]
fn every_document_on_this_queue_agrees_with_the_advertised_surface() -> TestResult {
    // Every AWL document deployed to queue `default` and served by this worker.
    let documents = [
        ("assistant.awl", ASSISTANT_DOCUMENT),
        (
            "assistant_spike.awl",
            include_str!("../../../assistant/awl/assistant_spike.awl"),
        ),
    ];
    let surface = advertised_surface()?;
    for (name, source) in documents {
        let contract = contract_of(source)?;
        assert_eq!(
            contract.task_queue, surface.task_queue,
            "{name} declares queue `{}`, but this worker serves `{}`; a document on \
             another queue does not belong in this list",
            contract.task_queue, surface.task_queue
        );
        let diffs = contract_diffs(name, &contract, None, &surface.descriptors);
        assert!(
            diffs.is_empty(),
            "{name} disagrees with the advertised surface, so this worker would be \
             refused and NO document on queue `{}` could run:\n{}",
            surface.task_queue,
            diffs
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
    Ok(())
}

/// The non-vacuity guard for the invariant above: it must be checking more than
/// one document, or it has quietly become a restatement of
/// `the_advertised_surface_is_admitted_by_the_assistant_contract`.
#[test]
fn the_shared_queue_invariant_covers_more_than_one_document() -> TestResult {
    let spike = include_str!("../../../assistant/awl/assistant_spike.awl");
    assert_ne!(
        spike, ASSISTANT_DOCUMENT,
        "the two documents must be distinct for the shared-queue invariant to mean anything"
    );
    let spike_contract = contract_of(spike)?;
    assert!(
        spike_contract
            .actions
            .iter()
            .any(|action| action.name == "assistant_provision"),
        "the spike must still declare the action the two documents share, or it no \
         longer exercises the invariant"
    );
    Ok(())
}
