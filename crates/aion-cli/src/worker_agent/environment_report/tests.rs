//! Unit tests for the pass-through absence report.
//!
//! These prove the report's CONTENT. That it actually reaches an operator is proved
//! separately and end to end, by `tests/declared_environment_report_e2e.rs`, which runs
//! the real binary and reads its stderr — because a report that is only ever written to
//! a buffer inside its own test is exactly the defect this module was written to fix.

use std::collections::BTreeMap;
use std::path::Path;

use aion_integrations::{ChildEnvironment, EnvironmentDeclaration};

use super::report_absent_pass_through;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// A resolved environment over `declared`, against a context carrying `present`.
fn resolved(declared: &[&str], present: &[(&str, &str)]) -> Result<ChildEnvironment, String> {
    let context: BTreeMap<String, String> = present
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect();
    EnvironmentDeclaration::new(declared.iter().copied())
        .map(|declaration| declaration.resolve(&context))
        .map_err(|error| error.to_string())
}

/// The report the writer received, as one string.
fn report(environment: &ChildEnvironment) -> Result<String, Box<dyn std::error::Error>> {
    let mut written = Vec::new();
    report_absent_pass_through(
        &mut written,
        environment,
        Path::new("/srv/flows/assistant.awl"),
        "reviewers",
    )?;
    Ok(String::from_utf8(written)?)
}

#[test]
fn a_complete_environment_reports_nothing_at_all() -> TestResult {
    let environment = resolved(&["PATH", "HOME"], &[("PATH", "/usr/bin"), ("HOME", "/u/t")])?;
    assert_eq!(
        report(&environment)?,
        "",
        "a line saying nothing is missing on every start would train an operator to skip \
         the one that matters"
    );
    Ok(())
}

#[test]
fn every_absent_variable_is_named_beside_the_queue_and_the_document() -> TestResult {
    let environment = resolved(
        &["PATH", "ANTHROPIC_API_KEY", "AGENT_HOME"],
        &[("PATH", "/usr/bin")],
    )?;
    let report = report(&environment)?;
    for expected in [
        "ANTHROPIC_API_KEY",
        "AGENT_HOME",
        "reviewers",
        "/srv/flows/assistant.awl",
        "env_pass",
        "warning:",
    ] {
        assert!(
            report.contains(expected),
            "the report must name `{expected}`, got:\n{report}"
        );
    }
    assert!(
        !report.contains("PATH,") && !report.contains(" PATH\n"),
        "a variable that IS set is not an absence: {report}"
    );
    assert!(
        report.contains('2'),
        "the count is the operator's first read of how bad it is: {report}"
    );
    Ok(())
}

#[test]
fn a_declaration_whose_every_name_is_absent_says_so_in_words() -> TestResult {
    let environment = resolved(&["ANTHROPIC_API_KEY"], &[])?;
    let report = report(&environment)?;
    assert!(
        report.contains("no variable it names"),
        "an environment carrying nothing reads as words rather than as `0 of them`: \
         {report}"
    );
    Ok(())
}

#[test]
fn a_closed_writer_is_reported_rather_than_swallowed() -> TestResult {
    /// A writer that refuses everything — the closed-stderr case.
    struct Closed;

    impl std::io::Write for Closed {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("stderr is closed"))
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let environment = resolved(&["ANTHROPIC_API_KEY"], &[])?;
    let outcome = report_absent_pass_through(
        &mut Closed,
        &environment,
        Path::new("/srv/flows/assistant.awl"),
        "reviewers",
    );
    let Err(error) = outcome else {
        return Err("a report that could not be delivered is not a report".into());
    };
    assert!(
        format!("{error:#}").contains("environment pass-through report"),
        "the failure names what could not be delivered: {error:#}"
    );
    Ok(())
}
