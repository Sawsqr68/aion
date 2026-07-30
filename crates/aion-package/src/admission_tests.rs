//! Declared-contract admission tests.

use serde_json::{Value, json};

use super::{AdmissionError, admit_value, declares_nothing};
use crate::contract::{AdditionalWorkflowContract, PackageContract, SignalContract};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn continuation_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "message": { "type": "string" },
            "end": { "type": "boolean" }
        },
        "required": ["end", "message"]
    })
}

/// The violations text of a refusal, or an error naming what happened instead.
fn refusal(schema: &Value, value: &Value) -> Result<String, Box<dyn std::error::Error>> {
    match admit_value(schema, value) {
        Err(AdmissionError::Mismatch { violations }) => Ok(violations),
        Err(other) => Err(format!("expected a mismatch refusal, got: {other}").into()),
        Ok(()) => Err("the value was admitted when it should have been refused".into()),
    }
}

#[test]
fn a_conforming_value_is_admitted() -> TestResult {
    admit_value(
        &continuation_schema(),
        &json!({ "message": "carry on", "end": false }),
    )?;
    Ok(())
}

/// Extra fields ride through: the declaration says what must be present, not
/// what must be absent, and the workflow-side decoders ignore surplus keys.
#[test]
fn surplus_fields_are_admitted() -> TestResult {
    admit_value(
        &continuation_schema(),
        &json!({ "message": "carry on", "end": false, "note": "ignored" }),
    )?;
    Ok(())
}

#[test]
fn a_wrong_typed_field_is_refused_naming_its_location() -> TestResult {
    let violations = refusal(
        &continuation_schema(),
        &json!({ "message": "carry on", "end": "true" }),
    )?;
    assert!(
        violations.contains("/end"),
        "the refusal must name the field that failed: {violations}"
    );
    assert!(
        violations.contains("boolean"),
        "the refusal must state what was expected: {violations}"
    );
    Ok(())
}

#[test]
fn a_missing_required_field_is_refused_naming_it() -> TestResult {
    let violations = refusal(&continuation_schema(), &json!({ "end": true }))?;
    assert!(
        violations.contains("message"),
        "the refusal must name the absent field: {violations}"
    );
    Ok(())
}

/// Every violation is reported, not just the first: an operator who made two
/// mistakes must see both rather than rediscover the second on the retry.
#[test]
fn every_violation_is_reported() -> TestResult {
    let violations = refusal(
        &continuation_schema(),
        &json!({ "message": 7, "end": "true" }),
    )?;
    assert!(
        violations.contains("/message") && violations.contains("/end"),
        "both failing fields must be named: {violations}"
    );
    Ok(())
}

#[test]
fn a_non_object_payload_is_refused_at_the_root() -> TestResult {
    let violations = refusal(&continuation_schema(), &json!("just a string"))?;
    assert!(
        violations.contains("<root>"),
        "a whole-value mismatch reports the root: {violations}"
    );
    Ok(())
}

#[test]
fn an_uncompilable_schema_is_a_package_defect_not_a_caller_error() {
    let outcome = admit_value(&json!({ "type": "not-a-json-type" }), &json!({}));
    assert!(
        matches!(outcome, Err(AdmissionError::UnusableSchema { .. })),
        "an invalid schema must be reported as such, never as a caller mismatch: {outcome:?}"
    );
}

/// A `null` schema is what a manifest that never declared a shape carries —
/// `PackageContract::default()` has one — and it constrains nothing. Reading
/// it as a schema instead makes it uncompilable, which once turned every start
/// of every defaulted package into a caller refusal.
#[test]
fn the_undeclared_forms_declare_nothing() {
    for schema in [json!(null), json!({}), json!(true)] {
        assert!(
            declares_nothing(&schema),
            "an absent declaration must constrain nothing: {schema}"
        );
    }
    assert!(declares_nothing(&PackageContract::default().input_schema));
}

#[test]
fn a_real_schema_declares_something() {
    for schema in [
        continuation_schema(),
        json!({ "type": "object" }),
        json!(false),
    ] {
        assert!(
            !declares_nothing(&schema),
            "a schema that constrains values must be admitted against: {schema}"
        );
    }
}

fn contract_with_signals() -> PackageContract {
    PackageContract {
        input_schema: json!({ "type": "object", "required": ["objective"] }),
        signals: vec![
            SignalContract {
                name: "assistant_continue".to_owned(),
                input_schema: continuation_schema(),
            },
            SignalContract {
                name: "abort".to_owned(),
                input_schema: json!({ "type": "object" }),
            },
        ],
        ..PackageContract::default()
    }
}

#[test]
fn a_declared_signal_resolves_to_its_own_schema() -> TestResult {
    let contract = contract_with_signals();
    let declared = contract
        .declared_signal("assistant_continue")
        .ok_or("the contract declares this signal")?;
    assert_eq!(declared.input_schema, continuation_schema());
    assert!(contract.declared_signal("assistant_stop").is_none());
    Ok(())
}

#[test]
fn declared_signal_names_are_sorted_for_a_stable_refusal_message() {
    assert_eq!(
        contract_with_signals().declared_signal_names(),
        vec!["abort", "assistant_continue"]
    );
}

/// An additional entry's schema is its OWN, never the primary entry's: a
/// synthesized child validated against the parent's shape would refuse every
/// legitimate child start.
#[test]
fn an_additional_entry_resolves_to_its_own_input_schema() {
    let contract = PackageContract {
        input_schema: json!({ "type": "object", "required": ["objective"] }),
        additional_workflows: vec![AdditionalWorkflowContract {
            workflow_type: "assistant__round".to_owned(),
            input_schema: json!({ "type": "object", "required": ["prompt"] }),
            output_schema: json!({}),
        }],
        ..PackageContract::default()
    };

    assert_eq!(
        contract.entry_input_schema("assistant__round"),
        &json!({ "type": "object", "required": ["prompt"] })
    );
    assert_eq!(
        contract.entry_input_schema("assistant"),
        &json!({ "type": "object", "required": ["objective"] })
    );
}
