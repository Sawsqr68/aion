//! Regression pin for the reserved AWL and backend type-name boundary.

use std::error::Error;

use aion_awl::builtins::RESERVED_NAMES;
use aion_awl::{CheckError, EmitError, check, emit, parse};

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

fn synthesis_probe(name: &str) -> String {
    format!(
        "//! Emitter reservation probe.\n\
         workflow synthesis_probe\n\
         \x20 input envelope: Envelope\n\
         \x20 outcome done: type String, route success\n\
         \n\
         type Envelope = schema {{\n\
         \x20 \"type\": \"object\",\n\
         \x20 \"required\": [\"payload\"],\n\
         \x20 \"properties\": {{\n\
         \x20   \"payload\": {{ \"$ref\": \"#/$defs/{name}\" }}\n\
         \x20 }},\n\
         \x20 \"$defs\": {{\n\
         \x20   \"{name}\": {{\n\
         \x20     \"type\": \"object\",\n\
         \x20     \"required\": [\"value\"],\n\
         \x20     \"properties\": {{ \"value\": {{ \"type\": \"string\" }} }}\n\
         \x20   }}\n\
         \x20 }}\n\
         }}\n\
         \n\
         worker synthesis_probe\n\
         \x20 action render(envelope: Envelope) -> String\n\
         \n\
         step run\n\
         \x20 envelope |> render |> route done\n"
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
fn emitter_preclaims_every_name_the_checker_reserves() -> TestResult {
    for name in RESERVED_NAMES {
        let document = parse(&synthesis_probe(name))?;
        assert!(
            check(&document).is_empty(),
            "synthesis probe for `{name}` must check cleanly"
        );
        let emitted = emit(&document)?;
        assert!(
            emitted.contains(&format!("pub type {name}2 {{")),
            "emitter did not pre-claim reserved name `{name}`"
        );
    }
    let foo = emit(&parse(&synthesis_probe("Foo"))?)?;
    assert!(
        foo.contains("pub type Foo {"),
        "unreserved control must keep the unsuffixed name `Foo`"
    );
    Ok(())
}

#[test]
fn checker_and_emitter_acceptance_round_trip_for_reserved_names_and_control() -> TestResult {
    for name in RESERVED_NAMES.into_iter().chain(["Foo"]) {
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
        assert!(
            !check(&document).is_empty(),
            "schema of {name} must fail check"
        );
        assert!(emit(&document).is_err(), "schema of {name} must fail emit");
    }
    Ok(())
}
