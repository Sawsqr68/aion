//! The checker and the executor must agree about declared commands.
//!
//! `aion-awl` rules on a `run` command at CHECK time; `aion-worker`'s
//! `CommandTemplate` parses the same text into an argv at registration
//! time. If the two disagreed, a document could check clean and then fail
//! on the way to the process — exactly the gap a declarative body exists to
//! close. These tests pin the contract in both useful directions:
//!
//! * every command the checker ACCEPTS, the executor also accepts;
//! * for those commands, both bind exactly the same parameter names.
//!
//! The checker is deliberately STRICTER in places (it demands `snake_case`
//! AWL names and refuses a parameter anywhere in the program word, not just
//! a bare one). Strictness in that direction is safe and is asserted too.

use std::error::Error;

use aion_awl::{compile, scan_command};
use aion_worker::shell::CommandTemplate;

type TestResult = Result<(), Box<dyn Error>>;

/// Commands the checker accepts today. Each is also an executor input.
const ACCEPTED: &[&str] = &[
    "echo hello",
    "echo $name",
    "cp $source $target",
    "cp $source $target $source",
    "echo ${name}_snapshot",
    "echo x$name",
    "echo $one$two",
    "echo \"$first $second\"",
    "echo '$not_a_reference'",
    // Single-quoted throughout, so BOTH read a program literally named
    // `$program` and neither substitutes anything. Accepted, and identical.
    "'$program' argument",
    "echo $$HOME",
    "echo $${HOME}",
    "'my prog' $argument",
    "printf %s%s ${left} $right",
    "cargo test --workspace -- --nocapture",
    "sh -c 'echo hi'",
];

/// Commands the checker refuses. Some the executor refuses too; the rest
/// are where the checker is deliberately stricter.
const REFUSED: &[&str] = &[
    // Empty: no program to run.
    "",
    "   ",
    // Program position — the executor refuses the bare form, and the
    // checker refuses the spliced form it would otherwise let through.
    "$program argument",
    "${program} argument",
    "echo$suffix",
    // Not an AWL parameter name.
    "echo $HOME",
    "echo ${NOT_A_NAME}",
    "echo ${two words}",
    "echo $naïve",
    // Malformed.
    "echo $",
    "echo $-x",
    "echo ${name",
    "echo ${}",
    "echo 'unterminated",
    "echo \"unterminated",
];

/// The checker's verdict on a command, as `checker::bodies` reaches it:
/// `None` when it is refused, `Some(parameters)` when it is accepted, with
/// the parameter binding the compile seam would record.
fn checker_verdict(command: &str) -> Option<Vec<String>> {
    if command.trim().is_empty() {
        return None;
    }
    let scanned = scan_command(command).ok()?;
    if scanned
        .references
        .iter()
        .any(|reference| reference.offset < scanned.program_end)
    {
        return None;
    }
    Some(scanned.parameters())
}

#[test]
fn every_command_the_checker_accepts_the_executor_also_accepts() -> TestResult {
    for command in ACCEPTED.iter().chain(REFUSED) {
        let Some(parameters) = checker_verdict(command) else {
            continue;
        };
        let template = CommandTemplate::parse(command).map_err(|error| {
            format!("checker accepted {command:?} but the executor refuses it: {error}")
        })?;
        assert_eq!(
            parameters,
            template.referenced_parameters(),
            "checker and executor bind different parameters for {command:?}"
        );
    }
    Ok(())
}

#[test]
fn the_accepted_table_is_actually_accepted() {
    for command in ACCEPTED {
        assert!(
            checker_verdict(command).is_some(),
            "{command:?} is listed as accepted but the checker refuses it"
        );
    }
}

#[test]
fn the_refused_table_is_actually_refused() {
    for command in REFUSED {
        assert!(
            checker_verdict(command).is_none(),
            "{command:?} is listed as refused but the checker accepts it"
        );
    }
}

#[test]
fn a_parameter_in_program_position_is_refused_by_both() {
    // The one refusal that MUST be shared, because it is a security
    // property: a caller-supplied value can never choose the program.
    assert!(checker_verdict("$program argument").is_none());
    assert!(
        CommandTemplate::parse("$program argument").is_err(),
        "the executor must refuse a parameterized program"
    );
}

const DOCUMENT: &str = "\
//! A declared command carried all the way to the compiled artifact.
workflow declared_command
  input source: String
  outcome copied: type Receipt, route success

type Receipt { exit_code: Int, stdout: String, stderr: String }

worker files
  action copy_file(source: String, target: String) -> Receipt
    run \"cp -- $source ${target}\"
  action verify(path: String) -> Receipt

step copy
  copy_file(source: source, target: source) -> receipt

step verify after copy
  verify(path: receipt.stdout) -> verified
  route copied(exit_code: verified.exit_code, stdout: verified.stdout, stderr: verified.stderr)
";

#[test]
fn the_compiled_binding_matches_what_the_executor_would_parse() -> TestResult {
    let compiled = compile(DOCUMENT, std::path::Path::new("."))?;
    let implementation = compiled
        .action_bodies
        .first()
        .ok_or("the compiled workflow must carry the declared command")?;
    assert_eq!(
        compiled.action_bodies.len(),
        1,
        "only one action has a body"
    );
    assert_eq!(implementation.task_queue, "files");
    assert_eq!(implementation.action, "copy_file");
    let aion_awl::CompiledActionBody::Run {
        command,
        parameters,
    } = &implementation.body;
    assert_eq!(command, "cp -- $source ${target}");
    let template = CommandTemplate::parse(command)?;
    assert_eq!(
        parameters,
        &template.referenced_parameters(),
        "the compiled binding must be what the executor will ask for"
    );
    assert_eq!(parameters, &["source".to_owned(), "target".to_owned()]);
    Ok(())
}
