//! The shipped `examples/shell-run/shell_run.awl` demo is the end-to-end
//! proof that an `.awl` file alone makes an action RUN: no Rust crate, no
//! connected worker. It is a shipped artifact, so it is gated like one —
//! it must check clean, print canonically, and compile to a package
//! contract that actually carries the command the server will execute.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use aion_awl::{CompiledActionBody, check_in, compile, parse, print};
use aion_package::ActionBodyContract;

type TestResult = Result<(), Box<dyn Error>>;

fn example() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/shell-run/shell_run.awl")
}

#[test]
fn the_shell_run_example_checks_clean_and_is_canonically_formatted() -> TestResult {
    let path = example();
    let source = fs::read_to_string(&path)?;
    let document = parse(&source)?;
    let root = path.parent().ok_or("the example has a directory")?;
    let errors = check_in(&document, root);
    assert!(errors.is_empty(), "{errors:#?}");
    assert_eq!(
        print(&document),
        source,
        "the shipped example must already be in canonical form"
    );
    Ok(())
}

#[test]
fn the_shell_run_example_compiles_with_its_command_in_the_package_contract() -> TestResult {
    let path = example();
    let source = fs::read_to_string(&path)?;
    let root = path.parent().ok_or("the example has a directory")?;
    let compiled = compile(&source, root)?;

    // The compile seam's own view of the body.
    let implementation = compiled
        .action_bodies
        .first()
        .ok_or("the example declares one bodied action")?;
    assert_eq!(compiled.action_bodies.len(), 1);
    assert_eq!(implementation.task_queue, "shell_box");
    assert_eq!(implementation.action, "stamp");
    let CompiledActionBody::Run {
        command,
        parameters,
    } = &implementation.body;
    assert_eq!(command, "echo $text");
    assert_eq!(parameters, &["text".to_owned()]);

    // The form that is sealed into package identity and read at dispatch.
    let worker = compiled
        .contract
        .workers
        .first()
        .ok_or("the contract declares the queue")?;
    let action = worker
        .actions
        .first()
        .ok_or("the contract declares the action")?;
    assert_eq!(
        action.body,
        Some(ActionBodyContract::Run {
            command: "echo $text".to_owned()
        }),
        "the deployed contract must carry the command the server executes"
    );
    Ok(())
}

#[test]
fn the_example_command_is_identity_bound() -> TestResult {
    // Editing the command must mint a different package: a body is
    // executable authority, and authority that could be swapped without
    // changing the package hash would be a spoofable deployment.
    let path = example();
    let source = fs::read_to_string(&path)?;
    let root = path.parent().ok_or("the example has a directory")?;
    let original = compile(&source, root)?.contract.canonical_bytes();
    let edited = source.replace(r#"run "echo $text""#, r#"run "printf $text""#);
    assert_ne!(edited, source, "the substitution must have happened");
    let changed = compile(&edited, root)?.contract.canonical_bytes();
    assert_ne!(
        original, changed,
        "a command edit must move the contract identity"
    );
    Ok(())
}
