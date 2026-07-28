use std::time::Duration;

use serde_json::json;

use crate::{
    ActionContract, BeamModule, BeamSet, ChildContract, Manifest, ManifestVersion, PackageContract,
    PackageError, RetryContract, SignalContract, WorkerContract, content_hash_with_contract,
};

fn action(name: &str, field: &str) -> ActionContract {
    ActionContract {
        name: name.to_owned(),
        input_schema: json!({
            "required": [field],
            "properties": { field: { "type": "string" } },
            "type": "object"
        }),
        output_schema: json!({"type":"boolean"}),
        node: Some("shell".to_owned()),
        timeout: Some(Duration::from_secs(30)),
        retry: Some(RetryContract::Backoff {
            count: 2,
            min: Duration::from_secs(1),
            max: Duration::from_secs(8),
        }),
        advisory: false,
    }
}

fn contract(permuted: bool) -> PackageContract {
    let mut workers = vec![
        WorkerContract {
            task_queue: "payments".to_owned(),
            actions: vec![action("refund", "refund_id"), action("charge", "amount")],
        },
        WorkerContract {
            task_queue: "mail".to_owned(),
            actions: vec![action("send", "address")],
        },
    ];
    let mut children = vec![
        ChildContract {
            name: "receipt".to_owned(),
            input_schema: json!({"type":"string"}),
            output_schema: json!({"type":"boolean"}),
        },
        ChildContract {
            name: "audit".to_owned(),
            input_schema: json!({"type":"integer"}),
            output_schema: json!({"type":"null"}),
        },
    ];
    let mut signals = vec![
        SignalContract {
            name: "cancel".to_owned(),
            input_schema: json!({"type":"string"}),
        },
        SignalContract {
            name: "approve".to_owned(),
            input_schema: json!({"type":"boolean"}),
        },
    ];
    if permuted {
        workers.reverse();
        workers[1].actions.reverse();
        children.reverse();
        signals.reverse();
    }
    PackageContract {
        input_schema: json!({"required":["order_id","account_id"],"type":"object"}),
        output_schema: json!({"enum":["failed","paid"]}),
        workers,
        children,
        signals,
        additional_workflows: Vec::new(),
        unscoped_activities: Vec::new(),
    }
}

fn manifest() -> Manifest {
    Manifest {
        entry_module: "workflow/order".to_owned(),
        entry_function: "run".to_owned(),
        input_schema: json!({"type":"object"}),
        output_schema: json!({"type":"object"}),
        timeout: Some(Duration::from_secs(60)),
        activities: Vec::new(),
        version: ManifestVersion::new("unstamped"),
        format_version: crate::CURRENT_FORMAT_VERSION,
        additional_workflows: Vec::new(),
    }
}

#[test]
fn v4_identity_is_declaration_order_independent() -> Result<(), PackageError> {
    let beams = BeamSet::new(vec![BeamModule::new("workflow/order", vec![1, 2, 3])])?;
    let first = contract(false);
    let second = contract(true);

    assert_eq!(first.canonical_bytes(), second.canonical_bytes());
    assert_eq!(
        content_hash_with_contract(&beams, &manifest(), &first),
        content_hash_with_contract(&beams, &manifest(), &second),
    );
    Ok(())
}

#[test]
fn v4_identity_ignores_json_object_and_set_array_order() -> Result<(), Box<dyn std::error::Error>> {
    let first: serde_json::Value = serde_json::from_str(
        r#"{ "type": "object", "required": ["a", "b"], "properties": {"a":{"type":"string"},"b":{"type":"integer"}} }"#,
    )?;
    let second: serde_json::Value = serde_json::from_str(
        r#"{"properties":{"b":{"type":"integer"},"a":{"type":"string"}},"required":["b","a"],"type":"object"}"#,
    )?;
    let mut left = contract(false);
    let mut right = contract(false);
    left.input_schema = first;
    right.input_schema = second;

    assert_eq!(left.canonical_bytes(), right.canonical_bytes());
    Ok(())
}

/// The `.v4` identity of a contract WITHOUT advisory actions, frozen as a
/// committed golden.
///
/// Measured on the pre-advisory tree (`aion-package` at 55a07922, before
/// `ActionContract::advisory` existed) and asserted here against the
/// post-change encoder — the two agree byte-for-byte.
///
/// `ActionContract::canonical_bytes` appends the advisory marker only when
/// the flag is true, so every package already deployed keeps the identity it
/// was deployed under — adding the class mints no redeploy wave. This pin is
/// what makes that claim checkable rather than asserted.
const PRE_ADVISORY_V4_IDENTITY: &str =
    "eef121798ea576fbf700b50d3fd79c3f7a979b5fcc7c85b8a02c1a71cfef1672";

#[test]
fn advisory_free_contracts_keep_their_pre_advisory_v4_identity() -> Result<(), PackageError> {
    let beams = BeamSet::new(vec![BeamModule::new("workflow/order", vec![1, 2, 3])])?;
    let hash = content_hash_with_contract(&beams, &manifest(), &contract(false));
    assert_eq!(
        hash.to_string(),
        PRE_ADVISORY_V4_IDENTITY,
        "a contract with no advisory action must hash exactly as it did before the \
         advisory class existed"
    );
    Ok(())
}

#[test]
fn advisory_is_identity_bearing() -> Result<(), PackageError> {
    let beams = BeamSet::new(vec![BeamModule::new("workflow/order", vec![1, 2, 3])])?;
    let plain = contract(false);
    let mut advisory = contract(false);
    advisory.workers[0].actions[0].advisory = true;

    assert_ne!(
        plain.canonical_bytes(),
        advisory.canonical_bytes(),
        "advisory must be committed to the canonical record"
    );
    assert_ne!(
        content_hash_with_contract(&beams, &manifest(), &plain),
        content_hash_with_contract(&beams, &manifest(), &advisory),
        "flipping an action to advisory changes what the package promises, so it must \
         change the package identity"
    );
    Ok(())
}

#[test]
fn advisory_identity_is_declaration_order_independent() -> Result<(), PackageError> {
    let beams = BeamSet::new(vec![BeamModule::new("workflow/order", vec![1, 2, 3])])?;
    let mut first = contract(false);
    let mut second = contract(true);
    // The SAME action is advisory in both, reached through each contract's
    // own permuted position.
    mark_advisory(&mut first, "payments", "refund");
    mark_advisory(&mut second, "payments", "refund");

    assert_eq!(first.canonical_bytes(), second.canonical_bytes());
    assert_eq!(
        content_hash_with_contract(&beams, &manifest(), &first),
        content_hash_with_contract(&beams, &manifest(), &second),
    );
    Ok(())
}

/// Flip one named action of one named queue to advisory, wherever the
/// permutation put it.
fn mark_advisory(contract: &mut PackageContract, queue: &str, action: &str) {
    for worker in &mut contract.workers {
        if worker.task_queue != queue {
            continue;
        }
        for declared in &mut worker.actions {
            if declared.name == action {
                declared.advisory = true;
            }
        }
    }
}
