//! Execution-gated pins for value-position short-circuiting and the native
//! signed-millisecond duration boundary.

use std::error::Error;
use std::fs;

use aion_awl::mir::{LowerError, Value, lower, select};
use aion_awl::{check, parse};

use super::drivers::{Body, atom_ref, lit_ref};
use super::fork_generality::{echo_runner_ebin, lowered_at, parity_fixture, push_execute};
use super::harness::{build_vm, dispatch_echo_ebin, gleam_build, reference_module_at};

type TestResult = Result<(), Box<dyn Error>>;

const DURATION_DRIVER: &str = r#"
pub fn awl_rt_execute() {
  execute(DurationBoundaryInput(url: "https://example.test/report"))
}
"#;

#[test]
fn value_boolean_operators_skip_faulting_rhs_at_runtime() -> TestResult {
    let path = parity_fixture("value_short_circuit.awl");
    let mut direct = lowered_at(&path)?;
    let input_tag = atom_ref(&mut direct, "value_short_circuit_input");
    let mut body = Body::new();
    let empty = body.list(Vec::new());
    let input = body.record(input_tag, vec![Value::Var(empty)]);
    push_execute(&mut direct, "awl$rt_execute", body, Value::Var(input));

    let vm = build_vm(&[], &[select(&direct)?])?;
    let direct_result = vm.call0("value_short_circuit", "awl$rt_execute")?;
    assert_eq!(
        direct_result, "{ok, {completed_outcome, {decision, false, true}}}",
        "a skipped empty-list index executed or returned the wrong booleans"
    );
    Ok(())
}

#[test]
fn duration_boundary_executes_exactly_and_next_second_is_refused() -> TestResult {
    let path = parity_fixture("duration_boundary.awl");
    let source = fs::read_to_string(&path)?;
    let excessive = source.replace("9223372036854775s", "9223372036854776s");
    let document = parse(&excessive)?;
    let diagnostics = check(&document);
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message == "duration exceeds the native signed-millisecond range"
        }),
        "missing duration range diagnostic: {diagnostics:?}"
    );
    match lower(&document, path.parent()) {
        Err(LowerError::Message { message, .. }) => assert!(
            message.contains("duration exceeds the native signed-millisecond range"),
            "unexpected typed lowering refusal: {message}"
        ),
        Err(error) => return Err(format!("unexpected lowering refusal: {error}").into()),
        Ok(_) => return Err("out-of-range duration lowered successfully".into()),
    }

    if crate::gleam_test_support::skip_if_unavailable() {
        return Ok(());
    }
    let reference = reference_module_at(&path, DURATION_DRIVER)?;
    let mut ebins = gleam_build(&[("ref_duration_boundary", &reference)])?;
    ebins.push(dispatch_echo_ebin("duration_boundary")?);
    ebins.push(echo_runner_ebin(
        "duration_boundary",
        "duration_boundary",
        "ref_duration_boundary",
    )?);

    let mut direct = lowered_at(&path)?;
    let input_tag = atom_ref(&mut direct, "duration_boundary_input");
    let url = lit_ref(&mut direct, "https://example.test/report");
    let mut body = Body::new();
    let input = body.record(input_tag, vec![Value::Lit(url)]);
    push_execute(&mut direct, "awl$rt_execute", body, Value::Var(input));

    let vm = build_vm(&ebins, &[select(&direct)?])?;
    let direct_result = vm.call_echo("direct")?;
    let reference_result = vm.call_echo("ref")?;
    assert_eq!(direct_result, reference_result, "duration boundary parity");
    assert!(
        direct_result.contains("9223372036854775000"),
        "duration did not retain its declared millisecond value: {direct_result}"
    );
    assert!(
        !direct_result.contains("9223372036854775807"),
        "duration silently saturated to i64::MAX: {direct_result}"
    );
    Ok(())
}
