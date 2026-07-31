//! The declaration seam and its two startup guards.
//!
//! The guards are driven against hand-built contracts (through
//! [`Declaration::from_worker_contract`]) so a deliberately wrong shape — an
//! undeclared action, a node pinned elsewhere, an unserved action, a
//! server-executed body — can actually be presented. The REAL document is
//! exercised by the composition-root tests (`main_advertisement_tests.rs`),
//! which pin the eleven served activities against the compiled contract.

use std::collections::BTreeSet;

use aion_package::{ActionBodyContract, ActionContract, PackageContract, WorkerContract};
use serde_json::json;

use super::{Declaration, TASK_QUEUE};

/// One declared action with the shape the guards read.
fn action(name: &str, node: Option<&str>, body: Option<&str>) -> ActionContract {
    ActionContract {
        name: name.to_owned(),
        input_schema: json!({"type": "object", "properties": {"item": {"type": "string"}}}),
        output_schema: json!({"type": "boolean"}),
        node: node.map(str::to_owned),
        timeout: None,
        retry: None,
        advisory: false,
        agent: false,
        body: body.map(|command| ActionBodyContract::Run {
            command: command.to_owned(),
        }),
    }
}

fn declaration(actions: Vec<ActionContract>) -> Declaration {
    Declaration::from_worker_contract(WorkerContract {
        task_queue: TASK_QUEUE.to_owned(),
        actions,
    })
}

fn served(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

/// The embedded document really does compile and really does carry the
/// `staged_rounds` worker — the whole advertisement depends on it.
#[test]
fn the_embedded_document_compiles_to_the_staged_rounds_worker_contract() -> anyhow::Result<()> {
    let declaration = Declaration::compile().map_err(anyhow::Error::msg)?;
    assert_eq!(declaration.contract().task_queue, TASK_QUEUE);
    assert_eq!(
        declaration.contract().actions.len(),
        11,
        "the document declares eleven actions; the worker serves them all"
    );
    Ok(())
}

/// A declared action's descriptor is the declaration's own schemas, verbatim.
#[test]
fn a_declared_action_yields_its_declared_schemas() -> anyhow::Result<()> {
    let declaration = declaration(vec![action("gate_item", Some("shell"), None)]);
    let descriptor = declaration
        .descriptor("gate_item", "shell")
        .map_err(anyhow::Error::msg)?;
    assert_eq!(descriptor.name, "gate_item");
    assert_eq!(
        descriptor.input_schema,
        declaration.contract().actions[0].input_schema
    );
    assert_eq!(
        descriptor.output_schema,
        declaration.contract().actions[0].output_schema
    );
    Ok(())
}

/// An UNPINNED declared action is reachable from every connection, so its
/// descriptor is available on any node.
#[test]
fn an_unpinned_action_is_advertised_from_any_node() -> anyhow::Result<()> {
    let declaration = declaration(vec![action("audit", None, None)]);
    for node in ["shell", "developer", "planner"] {
        assert_eq!(
            declaration
                .descriptor("audit", node)
                .map_err(anyhow::Error::msg)?
                .name,
            "audit"
        );
    }
    Ok(())
}

/// GUARD ONE. A handler the binary registers that the document declares no
/// action for is a loud startup failure naming the action — never a handler
/// registered with no descriptor, which is what made the worker invisible to
/// contract admission.
#[test]
fn a_handler_for_an_undeclared_action_is_refused_by_name() -> anyhow::Result<()> {
    let declaration = declaration(vec![action("gate_item", Some("shell"), None)]);
    let Err(error) = declaration.descriptor("gate_the_item", "shell") else {
        anyhow::bail!("an undeclared activity unexpectedly yielded a descriptor");
    };
    assert!(
        error.contains("gate_the_item"),
        "the refusal must name the offending action; got: {error}"
    );
    assert!(
        error.contains("does not declare"),
        "the refusal must say the document does not declare it; got: {error}"
    );
    Ok(())
}

/// GUARD ONE, the routing half: a handler registered on a node the action is
/// not pinned to could never be dispatched to, so it is refused naming the
/// action and BOTH nodes.
#[test]
fn a_handler_on_the_wrong_node_is_refused_naming_both_nodes() -> anyhow::Result<()> {
    let declaration = declaration(vec![action("dev_item", Some("developer"), None)]);
    let Err(error) = declaration.descriptor("dev_item", "shell") else {
        anyhow::bail!("a handler on the wrong node unexpectedly yielded a descriptor");
    };
    for fragment in ["dev_item", "`shell`", "`developer`"] {
        assert!(
            error.contains(fragment),
            "the refusal must contain {fragment}; got: {error}"
        );
    }
    Ok(())
}

/// GUARD TWO. A declared action with no body REQUIRES an out-of-band worker; a
/// binary that registers no handler for it refuses to start, naming the action
/// and the node that owed it.
#[test]
fn an_unserved_bodyless_action_is_refused_naming_the_action_and_its_node() -> anyhow::Result<()> {
    let declaration = declaration(vec![
        action("gate_item", Some("shell"), None),
        action("dev_item", Some("developer"), None),
    ]);
    let Err(error) = declaration.require_every_bodyless_action_served(&served(&["gate_item"]))
    else {
        anyhow::bail!("an unserved declared action was unexpectedly accepted");
    };
    assert!(
        error.contains("dev_item"),
        "the refusal must name the unserved action; got: {error}"
    );
    assert!(
        error.contains("`developer`"),
        "the refusal must name the unserved action's declared node; got: {error}"
    );
    assert!(
        !error.contains("gate_item"),
        "the served action must not be reported; got: {error}"
    );
    Ok(())
}

/// Every unserved action is named in ONE message — a guard that reported only
/// the first would send the operator round the loop once per action.
#[test]
fn every_unserved_action_is_named_at_once() -> anyhow::Result<()> {
    let declaration = declaration(vec![
        action("gate_item", Some("shell"), None),
        action("dev_item", Some("developer"), None),
        action("audit", None, None),
    ]);
    let Err(error) = declaration.require_every_bodyless_action_served(&BTreeSet::new()) else {
        anyhow::bail!("three unserved declared actions were unexpectedly accepted");
    };
    for fragment in ["gate_item", "dev_item", "audit", "unpinned"] {
        assert!(
            error.contains(fragment),
            "the refusal must contain {fragment}; got: {error}"
        );
    }
    Ok(())
}

/// GUARD TWO's exclusion. An action whose declaration carries a `body` is
/// executed by the SERVER, so no worker serves it and none may be demanded —
/// exactly the filter `aion_package::contract_diffs` applies. Demanding it
/// would make a queue that mixes a declared body with a bodyless action
/// unstartable for this worker.
#[test]
fn a_declared_body_is_not_demanded_of_this_worker() -> anyhow::Result<()> {
    let declaration = declaration(vec![
        action("gate_item", Some("shell"), None),
        action("snapshot", Some("shell"), Some("git rev-parse HEAD")),
    ]);
    declaration
        .require_every_bodyless_action_served(&served(&["gate_item"]))
        .map_err(anyhow::Error::msg)?;
    Ok(())
}

/// A compiled document declaring no `staged_rounds` worker leaves this binary
/// with nothing to advertise, so worker selection refuses and names what the
/// document DOES declare — a worker advertising nothing is exactly the defect
/// this module exists to prevent.
#[test]
fn a_document_without_the_worker_block_is_refused_naming_what_it_declares() -> anyhow::Result<()> {
    let contract = PackageContract {
        workers: vec![WorkerContract {
            task_queue: "some_other_queue".to_owned(),
            actions: vec![action("gate_item", Some("shell"), None)],
        }],
        ..PackageContract::default()
    };
    let Err(error) = super::select_worker(contract) else {
        anyhow::bail!("a contract without the staged_rounds worker was unexpectedly accepted");
    };
    assert!(
        error.contains(TASK_QUEUE),
        "the refusal must name the worker it wanted; got: {error}"
    );
    assert!(
        error.contains("some_other_queue"),
        "the refusal must name what the document declares; got: {error}"
    );
    Ok(())
}

/// The selection really does take the named worker when the document declares
/// several queues.
#[test]
fn the_named_worker_is_selected_from_several() -> anyhow::Result<()> {
    let contract = PackageContract {
        workers: vec![
            WorkerContract {
                task_queue: "some_other_queue".to_owned(),
                actions: vec![action("audit", None, None)],
            },
            WorkerContract {
                task_queue: TASK_QUEUE.to_owned(),
                actions: vec![action("gate_item", Some("shell"), None)],
            },
        ],
        ..PackageContract::default()
    };
    let selected = super::select_worker(contract).map_err(anyhow::Error::msg)?;
    assert_eq!(selected.task_queue, TASK_QUEUE);
    assert_eq!(selected.actions.len(), 1);
    assert_eq!(selected.actions[0].name, "gate_item");
    Ok(())
}
