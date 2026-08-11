//! Unit tests for the declared harness environment.
//!
//! Every failure is REPORTED, never panicked: this crate's lints treat a panic in a test
//! exactly as they treat one in production code, and a test that cannot say what it
//! expected is worth less than one that can.

use std::collections::BTreeMap;
use std::path::Path;

use super::{ChildEnvironment, EnvironmentDeclaration, EnvironmentError};

/// Failures carry the expectation they broke.
type TestResult = Result<(), String>;

/// A launching context carrying `entries`.
fn context(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
    entries
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect()
}

/// A declaration over `names`, or the reason it is not one.
fn declaration(names: &[&str]) -> Result<EnvironmentDeclaration, String> {
    EnvironmentDeclaration::new(names.iter().copied())
        .map_err(|error| format!("declaration {names:?} should be valid: {error}"))
}

#[test]
fn a_variable_the_declaration_does_not_name_is_absent_in_the_child() -> TestResult {
    let parent = context(&[("PATH", "/usr/bin"), ("CLAUDECODE", "1"), ("HOME", "/u/t")]);
    let resolved = declaration(&["PATH", "HOME"])?.resolve(&parent);

    assert!(resolved.carries("PATH"), "PATH was declared");
    assert!(resolved.carries("HOME"), "HOME was declared");
    assert!(
        !resolved.carries("CLAUDECODE"),
        "CLAUDECODE was NOT declared, so no parent can put it in the child: {:?}",
        resolved.pairs()
    );
    assert_eq!(resolved.pairs().len(), 2, "the child's WHOLE environment");
    Ok(())
}

#[test]
fn the_resolved_pairs_are_the_childs_entire_environment_not_an_overlay() -> TestResult {
    let parent = context(&[("A", "1"), ("B", "2"), ("C", "3")]);
    let resolved = declaration(&["B"])?.resolve(&parent);
    assert_eq!(resolved.pairs(), [("B".to_owned(), "2".to_owned())]);
    Ok(())
}

#[test]
fn a_declared_name_the_context_does_not_carry_is_reported_absent_not_invented() -> TestResult {
    let resolved =
        declaration(&["PATH", "ANTHROPIC_API_KEY"])?.resolve(&context(&[("PATH", "/usr/bin")]));
    assert_eq!(resolved.absent(), ["ANTHROPIC_API_KEY".to_owned()]);
    assert!(
        !resolved.carries("ANTHROPIC_API_KEY"),
        "an unset variable must not become an empty-string variable"
    );
    Ok(())
}

#[test]
fn declaration_order_is_the_operators_and_duplicates_collapse_once() -> TestResult {
    let parent = context(&[("A", "1"), ("B", "2")]);
    let resolved = declaration(&["B", "A", "B"])?.resolve(&parent);
    assert_eq!(
        resolved.pairs(),
        [
            ("B".to_owned(), "2".to_owned()),
            ("A".to_owned(), "1".to_owned())
        ]
    );
    assert_eq!(resolved.declared(), ["B", "A", "B"], "written as authored");
    Ok(())
}

#[test]
fn a_blank_entry_is_refused_because_no_environment_can_carry_one() {
    assert_eq!(
        EnvironmentDeclaration::new(["PATH", "  "]),
        Err(EnvironmentError::EmptyName)
    );
}

#[test]
fn a_key_value_pair_is_refused_rather_than_split() -> TestResult {
    let Err(error) = EnvironmentDeclaration::new(["PATH=/usr/bin"]) else {
        return Err("a KEY=VALUE entry is not a pass-through name".to_owned());
    };
    assert_eq!(
        error,
        EnvironmentError::NameContainsEquals {
            entry: "PATH=/usr/bin".to_owned()
        }
    );
    assert!(
        error.to_string().contains("PATH=/usr/bin"),
        "the refusal quotes the entry: {error}"
    );
    Ok(())
}

#[test]
fn a_bare_program_without_path_in_the_declaration_is_refused_naming_path() -> TestResult {
    let resolved = declaration(&["HOME"])?.resolve(&context(&[("HOME", "/u/t")]));
    let Err(error) = resolved.require_for_program(Path::new("claude-code-acp")) else {
        return Err("a bare program name cannot be found without PATH".to_owned());
    };
    let message = error.to_string();
    assert!(message.contains("PATH"), "names the variable: {message}");
    assert!(
        message.contains("claude-code-acp"),
        "names the program: {message}"
    );
    assert!(
        message.contains("HOME"),
        "names what IS declared: {message}"
    );
    assert!(
        message.contains("env_pass"),
        "names the declaration to correct: {message}"
    );
    Ok(())
}

#[test]
fn an_absolute_program_needs_no_path_so_a_hermetic_declaration_stays_legitimate() -> TestResult {
    let resolved = declaration(&["HOME"])?.resolve(&context(&[("HOME", "/u/t")]));
    assert_eq!(
        resolved.require_for_program(Path::new("/usr/local/bin/claude-code-acp")),
        Ok(())
    );
    Ok(())
}

#[test]
fn a_bare_program_with_path_declared_is_permitted() -> TestResult {
    let resolved = declaration(&["PATH"])?.resolve(&context(&[("PATH", "/usr/bin")]));
    assert_eq!(resolved.require_for_program(Path::new("norn")), Ok(()));
    Ok(())
}

#[test]
fn a_relative_multi_component_program_is_not_gated_on_path() -> TestResult {
    let resolved = declaration(&["HOME"])?.resolve(&context(&[("HOME", "/u/t")]));
    assert_eq!(
        resolved.require_for_program(Path::new("./bin/agent")),
        Ok(())
    );
    Ok(())
}

#[test]
fn an_environment_fault_reaches_the_seam_as_a_retryable_transport_failure() {
    let error = EnvironmentError::MissingForExec {
        variable: "PATH",
        program: "norn".to_owned(),
        declared: "HOME".to_owned(),
    };
    let mapped: crate::HarnessError = error.into();
    assert!(
        matches!(mapped, crate::HarnessError::Transport { .. }),
        "a corrected declaration makes the same run succeed, so it stays retryable: {mapped:?}"
    );
    assert!(mapped.to_string().contains("PATH"), "{mapped}");
}

#[test]
fn an_empty_declaration_renders_as_words_in_a_refusal() -> TestResult {
    let empty = ChildEnvironment {
        declared: Vec::new(),
        pairs: Vec::new(),
        absent: Vec::new(),
    };
    let Err(error) = empty.require_for_program(Path::new("agent")) else {
        return Err("an empty environment cannot resolve a bare program name".to_owned());
    };
    assert!(
        error.to_string().contains("no variables"),
        "an empty declaration reads as words, never as an empty list: {error}"
    );
    Ok(())
}

#[test]
fn the_process_snapshot_really_reads_this_process() {
    let snapshot = super::process_environment();
    assert!(
        snapshot.contains_key("PATH"),
        "a process with no PATH could not have run this test at all"
    );
}
