//! Real gRPC registration contract-admission regressions for matching and
//! field-level mismatched activity schemas.
//!
//! This binary asserts on the `Status` the WORKER receives. Its sibling
//! `grpc_admission_refusal_logs` asserts on what the SERVER said about the same
//! refusal, and needs its own binary for the global subscriber. Both build the
//! same world from `test_support/grpc_admission.rs` so the two halves of one
//! refusal are never measured against two different servers.

use aion_proto::generated;

#[path = "test_support/grpc_admission.rs"]
mod grpc_admission;

use grpc_admission::{
    PARTITIONED_SOURCE, TestResult, action_named, descriptor_for, register, register_on_node,
    state_from, state_with_contract,
};

#[tokio::test]
async fn mismatched_worker_is_refused_with_field_level_diff() -> TestResult {
    let (state, action) = state_with_contract().await?;
    let result = register(
        state,
        generated::ActivityDescriptor {
            name: "fetch".to_owned(),
            input_schema_json: serde_json::json!({"type":"string"}).to_string(),
            output_schema_json: action.output_schema.to_string(),
        },
    )
    .await?;
    let status = result.err().ok_or("mismatched registration was accepted")?;

    assert_eq!(status.code(), tonic::Code::FailedPrecondition);
    assert!(status.message().contains("WORKER_CONTRACT_MISMATCH"));
    assert!(status.message().contains("input_schema"));
    assert!(status.message().contains("expected"));
    assert!(status.message().contains("worker advertised"));
    Ok(())
}

/// THE DEFECT, on the real gRPC registration path. Admission checked every
/// bodyless action of the queue against every connection with no regard for the
/// node dimension, so a worker serving `netbox` was refused for omitting the
/// `shell` node's action and vice versa: BOTH connections rejected on every
/// dial, the queue unservable, and the refused process looking merely asleep.
///
/// This test lives at the transport rather than only over `contract_diffs`
/// because the unit-level rule can be perfectly right while the server hands it
/// an empty node forever — the threading is the part that was missing.
#[tokio::test]
async fn a_connection_serving_one_node_is_admitted_without_the_other_nodes_action() -> TestResult {
    let (state, actions) = state_from(PARTITIONED_SOURCE).await?;
    let fetch = action_named(&actions, "fetch")?;

    let result = register_on_node(state, "netbox".to_owned(), vec![descriptor_for(&fetch)]).await?;

    assert!(
        result.is_ok(),
        "the netbox connection serves its whole job and must be admitted without \
         advertising the shell node's action: {:?}",
        result.err().map(|status| status.message().to_owned())
    );
    Ok(())
}

/// The gate keeps its teeth on the actions that CAN reach the connection: the
/// `shell` node's own action, omitted, is still refused — and the refusal names
/// the node that owed it so the operator knows which connection to look at.
#[tokio::test]
async fn a_connection_omitting_its_own_nodes_action_is_still_refused() -> TestResult {
    let (state, actions) = state_from(PARTITIONED_SOURCE).await?;
    let fetch = action_named(&actions, "fetch")?;

    // On the `shell` node, advertising only the netbox node's action.
    let result = register_on_node(state, "shell".to_owned(), vec![descriptor_for(&fetch)]).await?;
    let status = result
        .err()
        .ok_or("a connection omitting its own node's action was accepted")?;

    assert_eq!(status.code(), tonic::Code::FailedPrecondition);
    assert!(status.message().contains("WORKER_CONTRACT_MISMATCH"));
    assert!(
        status.message().contains("grade"),
        "the refusal must name the unserved action: {}",
        status.message()
    );
    assert!(
        status.message().contains("pinned to node `shell`"),
        "the refusal must name the node that owed the action: {}",
        status.message()
    );
    Ok(())
}

#[tokio::test]
async fn matching_worker_receives_register_ack() -> TestResult {
    let (state, action) = state_with_contract().await?;
    let result = register(
        state,
        generated::ActivityDescriptor {
            name: action.name,
            input_schema_json: action.input_schema.to_string(),
            output_schema_json: action.output_schema.to_string(),
        },
    )
    .await?;

    assert!(result.is_ok());
    Ok(())
}
