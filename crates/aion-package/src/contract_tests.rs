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
