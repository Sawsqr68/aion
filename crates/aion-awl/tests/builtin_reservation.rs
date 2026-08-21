//! Regression pin for the reserved AWL and backend type-name boundary.

use std::error::Error;

use aion_awl::{CheckError, EmitError, check, emit, parse};

// Red-first inventory. The fix replaces this temporary test copy with the
// crate's single public authority so production and test readers cannot drift.
const RESERVED_NAMES: [&str; 8] = [
    "Bool", "Int", "Float", "String", "Nil", "Dir", "True", "False",
];
const ROUND_TRIP_NAMES: [&str; 9] = [
    "Bool", "Int", "Float", "String", "Nil", "Dir", "True", "False", "Foo",
];

type TestResult = Result<(), Box<dyn Error>>;

fn declaration_probe(name: &str) -> String {
    format!(
        "//! Reserved-name probe.\n\
         workflow reserved_probe\n\
         \x20 input value: String\n\
         \x20 outcome done: type {name}, route success\n\
         \n\
         type {name} {{ value: String }}\n\
         \n\
         worker reserved_probe\n\
         \x20 action make(value: String) -> {name}\n\
         \n\
         step run\n\
         \x20 value |> make |> route done\n"
    )
}

fn schema_probe(name: &str) -> String {
    format!(
        "//! Schema-of role probe.\n\
         workflow schema_probe\n\
         \x20 input task: String\n\
         \x20 outcome done: type String, route success\n\
         \n\
         const probe_schema = schema of {name}\n\
         \n\
         worker vocab\n\
         \x20 action stamp(text: String) -> String\n\
         \n\
         step run\n\
         \x20 task |> stamp |> route done\n"
    )
}

fn check_reason(errors: &[CheckError]) -> Option<&str> {
    errors.first().map(|error| error.message.as_str())
}

fn emit_reason(result: &Result<String, EmitError>) -> Option<&str> {
    result.as_ref().err().map(|error| error.message.as_str())
}

#[test]
fn every_reserved_name_is_refused_by_both_surfaces_for_the_same_reason() -> TestResult {
    for name in RESERVED_NAMES {
        let document = parse(&declaration_probe(name))?;
        let check_errors = check(&document);
        let emit_result = emit(&document);
        let Some(check_message) = check_reason(&check_errors) else {
            return Err(format!("checker accepted reserved name `{name}`").into());
        };
        let Some(emit_message) = emit_reason(&emit_result) else {
            return Err(format!("emitter accepted reserved name `{name}`").into());
        };
        assert!(
            emit_message.contains(check_message),
            "`{name}` refusal reasons differ: check={check_message:?}, emit={emit_message:?}"
        );
    }
    Ok(())
}

#[test]
fn checker_and_emitter_acceptance_round_trip_for_reserved_names_and_control() -> TestResult {
    for name in ROUND_TRIP_NAMES {
        let document = parse(&declaration_probe(name))?;
        let check_accepts = check(&document).is_empty();
        let emit_accepts = emit(&document).is_ok();
        assert_eq!(
            check_accepts, emit_accepts,
            "checker/emitter disagreement for `{name}`"
        );
    }
    Ok(())
}

#[test]
fn ordinary_name_passes_both_surfaces() -> TestResult {
    let document = parse(&declaration_probe("Foo"))?;
    assert!(check(&document).is_empty(), "checker must accept `Foo`");
    assert!(emit(&document).is_ok(), "emitter must accept `Foo`");
    Ok(())
}

#[test]
fn schema_of_keeps_referenceable_and_reserved_roles_distinct() -> TestResult {
    for name in ["Dir", "Int"] {
        let document = parse(&schema_probe(name))?;
        assert!(check(&document).is_empty(), "schema of {name} must check");
        assert!(emit(&document).is_ok(), "schema of {name} must emit");
    }
    for name in ["True", "Foo"] {
        let document = parse(&schema_probe(name))?;
        assert!(!check(&document).is_empty(), "schema of {name} must fail check");
        assert!(emit(&document).is_err(), "schema of {name} must fail emit");
    }
    Ok(())
}
