//! THE ADMISSION PROOF for this worker's advertisement.
//!
//! On 2026-07-30 every one of the five connections was refused at registration
//! with `WORKER_CONTRACT_MISMATCH ... but worker advertised <missing>` for each
//! of its own node's actions: the handlers were committed with no wire
//! descriptor, so each connection advertised a set of activity NAMES and no
//! schemas at all. These tests pin the fix at the two places it can regress:
//!
//! 1. Every activity the binary serves advertises a descriptor, and each
//!    advertised schema EQUALS the one the compiled document declares for that
//!    action. The comparison is against the compiled contract — never a
//!    hand-copied literal, which by construction cannot catch drift.
//! 2. `aion_package::contract_diffs` — the very function the server admits
//!    with — returns EMPTY for each connection, given the compiled worker
//!    contract, that connection's node, and that connection's advertised
//!    descriptors. A green result here is an admission proof, not a hope.
//!
//! Plus both startup guards, fired through the production code paths.

use std::collections::BTreeMap;

use aion_package::{ActivityDescriptor, contract_diffs};
use staged_rounds_worker::harness::AgentActivity;
use staged_rounds_worker::prompts;

use super::tests::{declaration, shell};
use super::{
    COORDINATOR_CONCURRENCY, Connection, Declaration, SHELL_NODE, agent_advertisements,
    connection_advertisement, connections, require_every_declared_action_served, roles,
};
use staged_rounds_worker::harness;
use staged_rounds_worker::profiles::Profiles;

/// A `.v4` package identity stands in for the deployed package's; `contract_diffs`
/// only reports it back inside a diff, and a green run reports nothing.
const PACKAGE_VERSION: &str = "staged_rounds$test";

/// The eleven activities the binary serves, in the document's action order.
const SERVED_ACTIVITIES: [&str; 11] = [
    "planner",
    "provision_item",
    "dev_item",
    "gate_item",
    "plan_reviews",
    "review_item",
    "fold_phase",
    "merge_branches",
    "remediate",
    "verify_integration",
    "fix_integration",
];

fn profiles() -> Profiles {
    Profiles {
        planner: "plan".to_owned(),
        developer: "dev".to_owned(),
        reviewer: "rev".to_owned(),
        remediator: "rem".to_owned(),
    }
}

/// The production connection table.
fn connection_table() -> anyhow::Result<Vec<Connection>> {
    connections(roles(profiles(), 4, COORDINATOR_CONCURRENCY)).map_err(anyhow::Error::msg)
}

/// Every descriptor the whole binary advertises, keyed by activity type: the
/// four agent connections' declared descriptors plus the shell registry's
/// committed ones.
fn every_advertised_descriptor(
    declaration: &Declaration,
) -> anyhow::Result<BTreeMap<String, ActivityDescriptor>> {
    let mut advertised: BTreeMap<String, ActivityDescriptor> = BTreeMap::new();
    let connections = connection_table()?;
    let mut lists = agent_advertisements(declaration, &connections)?;
    lists.push(shell(declaration)?.activity_descriptors());
    for descriptor in lists.into_iter().flatten() {
        let name = descriptor.name.clone();
        assert!(
            advertised.insert(name.clone(), descriptor).is_none(),
            "activity `{name}` is advertised by two connections"
        );
    }
    Ok(advertised)
}

/// THE FIRST PIN. Every one of the eleven activities advertises a descriptor,
/// and every advertised schema is byte-equal to the compiled document's
/// declaration for that action. Compared against the compiled contract, so a
/// document edit that changes a schema either flows through or fails here.
#[test]
fn every_served_activity_advertises_the_documents_declared_schemas() -> anyhow::Result<()> {
    let declaration = declaration()?;
    let advertised = every_advertised_descriptor(&declaration)?;

    assert_eq!(
        advertised.keys().map(String::as_str).collect::<Vec<_>>(),
        {
            let mut expected = SERVED_ACTIVITIES.to_vec();
            expected.sort_unstable();
            expected
        },
        "every activity the binary serves must advertise a descriptor — a \
         registered handler with no descriptor advertises nothing and the \
         connection is refused"
    );

    for action in &declaration.contract().actions {
        let descriptor = advertised.get(&action.name).ok_or_else(|| {
            anyhow::anyhow!("declared action `{}` advertises no descriptor", action.name)
        })?;
        assert_eq!(
            descriptor.input_schema, action.input_schema,
            "action `{}` advertises an input schema that is not the declared one",
            action.name
        );
        assert_eq!(
            descriptor.output_schema, action.output_schema,
            "action `{}` advertises an output schema that is not the declared one",
            action.name
        );
    }
    Ok(())
}

/// Every shell handler is committed WITH a descriptor: the registry's handler
/// count and descriptor count must match, or some handler took the
/// descriptor-less registration path again.
#[test]
fn no_shell_handler_is_registered_without_a_descriptor() -> anyhow::Result<()> {
    let registry = shell(&declaration()?)?;
    let types = registry.activity_types();
    let descriptors = registry.activity_descriptors();
    assert_eq!(
        descriptors.len(),
        types.len(),
        "{} shell handlers are registered but only {} advertise a descriptor",
        types.len(),
        descriptors.len()
    );
    for descriptor in &descriptors {
        assert!(
            types.contains(&descriptor.name),
            "descriptor `{}` names no registered handler",
            descriptor.name
        );
    }
    Ok(())
}

/// THE SECOND PIN, and the real admission proof: the server's own comparison
/// function returns no diff for ANY of the five connections. This is the exact
/// call `crates/aion-server/src/worker/contracts.rs` admits a registering
/// connection with, so green here means the worker is admissible.
#[test]
fn every_connection_is_admitted_by_the_servers_own_contract_gate() -> anyhow::Result<()> {
    let declaration = declaration()?;
    let contract = declaration.contract();
    let connections = connection_table()?;
    let advertisements = agent_advertisements(&declaration, &connections)?;

    for (connection, descriptors) in connections.iter().zip(&advertisements) {
        // Non-vacuity first: the gate must genuinely DEMAND actions of this
        // connection, or a green result below would prove nothing at all.
        assert!(
            !contract_diffs(PACKAGE_VERSION, contract, Some(connection.node), &[]).is_empty(),
            "the contract gate demands nothing of the `{}` connection — this \
             admission proof would be vacuous",
            connection.node
        );
        let diffs = contract_diffs(
            PACKAGE_VERSION,
            contract,
            Some(connection.node),
            descriptors,
        );
        assert!(
            diffs.is_empty(),
            "the `{}` connection would be REFUSED at registration: {}",
            connection.node,
            diffs
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; ")
        );
    }

    let shell_descriptors = shell(&declaration)?.activity_descriptors();
    assert!(
        !contract_diffs(PACKAGE_VERSION, contract, Some(SHELL_NODE), &[]).is_empty(),
        "the contract gate demands nothing of the shell connection — this \
         admission proof would be vacuous"
    );
    let diffs = contract_diffs(
        PACKAGE_VERSION,
        contract,
        Some(SHELL_NODE),
        &shell_descriptors,
    );
    assert!(
        diffs.is_empty(),
        "the shell connection would be REFUSED at registration: {}",
        diffs
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("; ")
    );
    Ok(())
}

/// GUARD ONE through the production path: a connection carrying an activity the
/// document declares no action for refuses at startup, naming the activity.
#[test]
fn an_activity_the_document_does_not_declare_refuses_at_startup() -> anyhow::Result<()> {
    let declaration = declaration()?;
    let connection = Connection {
        node: "developer",
        output_schema: "{}",
        profile: "doctrine".to_owned(),
        disallowed_tools: None,
        max_concurrency: 4,
        activities: vec![AgentActivity {
            activity_type: "dev_the_item",
            assemble: prompts::dev_item,
            extract: harness::dev_item_context,
        }],
    };
    let Err(error) = connection_advertisement(&declaration, &connection) else {
        anyhow::bail!("an undeclared activity was unexpectedly advertised");
    };
    let error = error.to_string();
    assert!(
        error.contains("dev_the_item"),
        "the startup refusal must name the activity; got: {error}"
    );
    Ok(())
}

/// GUARD ONE's routing half through the production path: an activity registered
/// on a node the document pins elsewhere could never be dispatched to, so it
/// refuses at startup naming the activity and both nodes.
#[test]
fn an_activity_registered_on_the_wrong_node_refuses_at_startup() -> anyhow::Result<()> {
    let declaration = declaration()?;
    let connection = Connection {
        node: "reviewer",
        output_schema: "{}",
        profile: "doctrine".to_owned(),
        disallowed_tools: None,
        max_concurrency: 4,
        activities: vec![AgentActivity {
            activity_type: "dev_item",
            assemble: prompts::dev_item,
            extract: harness::dev_item_context,
        }],
    };
    let Err(error) = connection_advertisement(&declaration, &connection) else {
        anyhow::bail!("an activity on the wrong node was unexpectedly advertised");
    };
    let error = error.to_string();
    for fragment in ["dev_item", "`reviewer`", "`developer`"] {
        assert!(
            error.contains(fragment),
            "the startup refusal must contain {fragment}; got: {error}"
        );
    }
    Ok(())
}

/// GUARD TWO through the production path: the real connection and shell tables
/// together serve every declared action, and dropping one activity from the
/// table makes the binary refuse to start, naming the action AND its declared
/// node.
#[test]
fn a_declared_action_no_handler_serves_refuses_at_startup() -> anyhow::Result<()> {
    let declaration = declaration()?;
    let connections = connection_table()?;
    let shell = shell(&declaration)?;
    require_every_declared_action_served(&declaration, &connections, &shell)?;

    let short: Vec<Connection> = connections
        .into_iter()
        .filter(|connection| connection.node != "reviewer")
        .collect();
    let Err(error) = require_every_declared_action_served(&declaration, &short, &shell) else {
        anyhow::bail!("a declared action with no handler was unexpectedly accepted");
    };
    let error = error.to_string();
    assert!(
        error.contains("review_item"),
        "the startup refusal must name the unserved action; got: {error}"
    );
    assert!(
        error.contains("`reviewer`"),
        "the startup refusal must name the unserved action's declared node; got: {error}"
    );
    Ok(())
}
