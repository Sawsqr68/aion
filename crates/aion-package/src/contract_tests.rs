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
        agent: false,
        body: None,
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
/// The identity this exact contract hashed to under the superseded `.v4`
/// domain, recorded when `.v5` replaced it. Kept as the negative pin below.
const SUPERSEDED_V4_IDENTITY: &str =
    "eef121798ea576fbf700b50d3fd79c3f7a979b5fcc7c85b8a02c1a71cfef1672";

#[test]
fn the_v5_domain_supersedes_every_v4_identity() -> Result<(), PackageError> {
    // Deliberate, not incidental: the `.v5` domain always encodes the action
    // body block, so no `.v4`-minted identity can verify again and every
    // deployed package re-mints on redeploy. If this hash ever matches, the
    // domain separation has been lost and a `.v4` archive could load with an
    // unverified body.
    let beams = BeamSet::new(vec![BeamModule::new("workflow/order", vec![1, 2, 3])])?;
    let hash = content_hash_with_contract(&beams, &manifest(), &contract(false));
    assert_ne!(
        hash.to_string(),
        SUPERSEDED_V4_IDENTITY,
        "a `.v5` identity must never collide with the superseded `.v4` domain"
    );
    Ok(())
}

#[test]
fn a_declared_body_is_identity_bearing() -> Result<(), PackageError> {
    let beams = BeamSet::new(vec![BeamModule::new("workflow/order", vec![1, 2, 3])])?;
    let plain = contract(false);
    let mut bodied = contract(false);
    bodied.workers[0].actions[0].body = Some(crate::ActionBodyContract::Run {
        command: "echo $amount".to_owned(),
    });

    assert_ne!(
        plain.canonical_bytes(),
        bodied.canonical_bytes(),
        "a declared body is executable authority and must be committed to the record"
    );
    assert_ne!(
        content_hash_with_contract(&beams, &manifest(), &plain),
        content_hash_with_contract(&beams, &manifest(), &bodied),
        "declaring a body changes what the package IS, so it must change identity"
    );
    Ok(())
}

#[test]
fn editing_a_declared_command_changes_identity() -> Result<(), PackageError> {
    // The tamper case the binding exists for: same names, same schemas, one
    // byte of command text different — the package identity must move.
    let beams = BeamSet::new(vec![BeamModule::new("workflow/order", vec![1, 2, 3])])?;
    let mut first = contract(false);
    first.workers[0].actions[0].body = Some(crate::ActionBodyContract::Run {
        command: "echo safe".to_owned(),
    });
    let mut second = contract(false);
    second.workers[0].actions[0].body = Some(crate::ActionBodyContract::Run {
        command: "echo saf3".to_owned(),
    });

    assert_ne!(
        content_hash_with_contract(&beams, &manifest(), &first),
        content_hash_with_contract(&beams, &manifest(), &second),
        "a rewritten command under an unchanged identity would execute unvouched"
    );
    Ok(())
}

#[test]
fn advisory_and_body_cannot_be_confused_in_the_record() -> Result<(), PackageError> {
    // The advisory marker is conditional and the body block follows it
    // unconditionally; these four corners must all be distinct records.
    let beams = BeamSet::new(vec![BeamModule::new("workflow/order", vec![1, 2, 3])])?;
    let mut corners = Vec::new();
    for advisory in [false, true] {
        for body in [
            None,
            Some(crate::ActionBodyContract::Run {
                command: "echo x".to_owned(),
            }),
        ] {
            let mut candidate = contract(false);
            candidate.workers[0].actions[0].advisory = advisory;
            candidate.workers[0].actions[0].body = body;
            corners.push(content_hash_with_contract(&beams, &manifest(), &candidate));
        }
    }
    for (left_index, left) in corners.iter().enumerate() {
        for right in &corners[left_index + 1..] {
            assert_ne!(
                left, right,
                "every advisory/body corner must have its own identity"
            );
        }
    }
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
