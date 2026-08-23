//! The emitter: the `--` cure at the bytes, the emit-layer refusals, and the
//! determinism assertion.

mod command;

use aion_awl::command::{
    ArgumentValue, CommandCause, CommandDecl, CommandLayer, ParameterSet, check_command, emit,
    fire_specimen,
};

use command::specimens::{DIFF_CENSUS, GIT_TAG, document};

/// The one command a document declares, parsed and checked.
fn only_command(command_text: &str) -> CommandDecl {
    let source = document(command_text);
    let parsed = match aion_awl::parse(&source) {
        Ok(parsed) => parsed,
        Err(error) => unreachable!("the specimen must parse: {}", error.message),
    };
    let diagnostics = aion_awl::check_in(&parsed, std::env::temp_dir().as_path());
    assert!(
        diagnostics.is_empty(),
        "the specimen must check clean: {:#?}",
        diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
    match parsed.commands.into_iter().next() {
        Some(command) => command,
        None => unreachable!("the specimen declares one command"),
    }
}

/// Resolve a declaration against a parameter set.
fn argv(command_text: &str, parameters: &ParameterSet) -> Vec<String> {
    let decl = only_command(command_text);
    let checked = match check_command(&decl) {
        Ok(checked) => checked,
        Err(refusals) => unreachable!("must check: {:#?}", refusals),
    };
    match emit(checked, parameters) {
        Ok(resolved) => resolved.argv,
        Err(refusal) => unreachable!("must emit: {}", refusal.rendered()),
    }
}

/// THE CURE, BYTE-VERIFIED. `git tag -- <name> <sha>`: the wall-flight2
/// canonical fixture. The marker sits between the options and the operands
/// because the invocation declared that it admits it — not because the
/// operand happens to begin with a dash.
#[test]
fn an_admitting_invocation_carries_the_marker_before_its_operands() {
    let emitted = argv(
        GIT_TAG,
        &ParameterSet::new()
            .with("tag_name", ArgumentValue::scalar("-v2.0"))
            .with("commit", ArgumentValue::scalar("HEAD")),
    );
    assert_eq!(
        emitted,
        vec!["git", "tag", "--", "-v2.0", "HEAD"],
        "the leading-dash operand must sit behind the end-of-options marker"
    );
}

/// UNCONDITIONALLY. The marker is not placed because an operand looked
/// dangerous — it is placed because the invocation admits it. An ordinary
/// operand gets it too, and that is the whole point: the family closes by
/// construction, so there is no condition to get wrong.
#[test]
fn the_marker_is_placed_whatever_the_operands_turn_out_to_be() {
    let emitted = argv(
        GIT_TAG,
        &ParameterSet::new()
            .with("tag_name", ArgumentValue::scalar("v2.0"))
            .with("commit", ArgumentValue::scalar("HEAD")),
    );
    assert_eq!(emitted, vec!["git", "tag", "--", "v2.0", "HEAD"]);
}

/// A refusing invocation gets NO marker: `git diff -- <range>` would read
/// the range as a pathspec and silently change what the census measures.
#[test]
fn a_refusing_invocation_never_carries_the_marker() {
    let emitted = argv(
        DIFF_CENSUS,
        &ParameterSet::new().with(
            "base_pin",
            ArgumentValue::scalar("5e2fb5a3ed32b53d48dbc585aa914acd056458c6"),
        ),
    );
    assert_eq!(
        emitted,
        vec![
            "git",
            "diff",
            "--no-ext-diff",
            "--name-only",
            "5e2fb5a3ed32b53d48dbc585aa914acd056458c6..HEAD"
        ]
    );
    assert!(
        !emitted.contains(&"--".to_owned()),
        "a `--` here would reinterpret the revision range as a pathspec"
    );
}

/// RED, EMIT LAYER: a refusing invocation whose operand would begin with `-`
/// is refused. There is no marker to protect it, so nothing downstream could
/// tell it from an option.
#[test]
fn a_refusing_invocation_refuses_a_leading_dash_operand() {
    let decl = only_command(DIFF_CENSUS);
    let Ok(checked) = check_command(&decl) else {
        unreachable!("the census specimen checks clean");
    };
    let refusal = emit(
        checked,
        &ParameterSet::new().with("base_pin", ArgumentValue::scalar("-x")),
    );
    match refusal {
        Ok(resolved) => unreachable!("must refuse, emitted {:?}", resolved.argv),
        Err(refusal) => {
            assert_eq!(refusal.cause(), CommandCause::LeadingDashOperand);
            assert_eq!(refusal.layer(), CommandLayer::Emit);
            assert_eq!(refusal.observed_bytes(), "-x..HEAD");
        }
    }
}

/// RED, EMIT LAYER: the three parameter-set refusals.
#[test]
fn the_parameter_set_refusals_each_fire_with_their_own_cause() {
    let decl = only_command(GIT_TAG);
    let Ok(checked) = check_command(&decl) else {
        unreachable!("the tag specimen checks clean");
    };

    let missing = emit(
        checked,
        &ParameterSet::new().with("tag_name", ArgumentValue::scalar("v1")),
    );
    match missing {
        Err(refusal) => {
            assert_eq!(refusal.cause(), CommandCause::ArgumentMissing);
            assert_eq!(refusal.observed_bytes(), "commit");
        }
        Ok(resolved) => unreachable!("must refuse, emitted {:?}", resolved.argv),
    }

    let undeclared = emit(
        checked,
        &ParameterSet::new()
            .with("tag_name", ArgumentValue::scalar("v1"))
            .with("commit", ArgumentValue::scalar("HEAD"))
            .with("nowhere", ArgumentValue::scalar("x")),
    );
    match undeclared {
        Err(refusal) => {
            assert_eq!(refusal.cause(), CommandCause::ArgumentUndeclared);
            assert_eq!(refusal.observed_bytes(), "nowhere");
        }
        Ok(resolved) => unreachable!("must refuse, emitted {:?}", resolved.argv),
    }

    let mistyped = emit(
        checked,
        &ParameterSet::new()
            .with("tag_name", ArgumentValue::list(["a", "b"]))
            .with("commit", ArgumentValue::scalar("HEAD")),
    );
    match mistyped {
        Err(refusal) => {
            assert_eq!(refusal.cause(), CommandCause::ArgumentTypeMismatch);
            assert_eq!(refusal.observed_bytes(), "[a, b]");
        }
        Ok(resolved) => unreachable!("must refuse, emitted {:?}", resolved.argv),
    }
}

/// A value containing a space is ONE argv element. Nothing re-splits it,
/// which is the whole reason this surface exists.
#[test]
fn a_value_containing_a_space_stays_one_argv_element() {
    let emitted = argv(
        GIT_TAG,
        &ParameterSet::new()
            .with("tag_name", ArgumentValue::scalar("release two point oh"))
            .with("commit", ArgumentValue::scalar("HEAD")),
    );
    assert_eq!(
        emitted,
        vec!["git", "tag", "--", "release two point oh", "HEAD"]
    );
    assert_eq!(emitted.len(), 5, "a space must not become a new argument");
}

/// THE VARIADIC TAIL. A list-typed parameter standing alone in an operand
/// emits one argv element per item — the absorbed Just shape, typed.
#[test]
fn a_list_operand_emits_one_element_per_item() {
    let command = "command census(paths: [String])\n  program \"git\" \"diff\"\n  \
                   refuses \"--\"\n  specimen \"-x\"\n  \
                   operand rest: [String] = \"{{paths}}\"";
    let emitted = argv(
        command,
        &ParameterSet::new().with("paths", ArgumentValue::list(["a.rs", "b.rs", "c d.rs"])),
    );
    assert_eq!(emitted, vec!["git", "diff", "a.rs", "b.rs", "c d.rs"]);

    let empty = argv(
        command,
        &ParameterSet::new().with("paths", ArgumentValue::list(Vec::<String>::new())),
    );
    assert_eq!(
        empty,
        vec!["git", "diff"],
        "an empty tail contributes no elements, not one empty one"
    );
}

/// A parameter default fills a parameter the set omits — the absorbed Just
/// shape, typed.
#[test]
fn a_declared_default_fills_an_omitted_parameter() {
    let command = "command c(base: String, range: String = \"{{base}}..HEAD\")\n  \
                   program \"git\" \"diff\"\n  refuses \"--\"\n  specimen \"-x\"\n  \
                   operand r: String = \"{{range}}\"";
    let emitted = argv(
        command,
        &ParameterSet::new().with("base", ArgumentValue::scalar("main")),
    );
    assert_eq!(emitted, vec!["git", "diff", "main..HEAD"]);

    let overridden = argv(
        command,
        &ParameterSet::new()
            .with("base", ArgumentValue::scalar("main"))
            .with("range", ArgumentValue::scalar("v1..v2")),
    );
    assert_eq!(overridden, vec!["git", "diff", "v1..v2"]);
}

/// DETERMINISM. Same declaration + same parameters ⇒ byte-identical argv.
/// Asserted rather than assumed, because "deterministic" is the property the
/// whole determinism boundary rests on.
#[test]
fn the_same_declaration_and_parameters_emit_byte_identical_argv() {
    let parameters = || {
        ParameterSet::new()
            .with("tag_name", ArgumentValue::scalar("-v2.0"))
            .with("commit", ArgumentValue::scalar("HEAD"))
    };
    let first = argv(GIT_TAG, &parameters());
    for _ in 0..16 {
        assert_eq!(argv(GIT_TAG, &parameters()), first);
    }
}

/// ABSENCE IS A LEGAL DECLARED STATE. A command with no timeout emits no
/// timeout, and this surface substitutes nothing.
#[test]
fn a_declaration_with_no_timeout_emits_no_timeout() {
    let decl = only_command(GIT_TAG);
    let Ok(checked) = check_command(&decl) else {
        unreachable!("checks clean");
    };
    let resolved = match emit(
        checked,
        &ParameterSet::new()
            .with("tag_name", ArgumentValue::scalar("v1"))
            .with("commit", ArgumentValue::scalar("HEAD")),
    ) {
        Ok(resolved) => resolved,
        Err(refusal) => unreachable!("must emit: {}", refusal.rendered()),
    };
    assert_eq!(resolved.timeout_ms, None);
    assert_eq!(resolved.timeout_owner, None);
    assert_eq!(resolved.open_files, None);
    assert_eq!(resolved.hardened_path, None);
    assert_eq!(resolved.cwd, None);
    assert!(resolved.env.is_empty());
}

/// A declared timeout emits its own number and its owner, and nothing else.
#[test]
fn a_declared_timeout_emits_its_own_number_and_owner() {
    let decl = only_command(
        "command c\n  program \"git\"\n  admits \"--\"\n  timeout 10m owner \"waffles\"",
    );
    let Ok(checked) = check_command(&decl) else {
        unreachable!("checks clean");
    };
    let resolved = match emit(checked, &ParameterSet::new()) {
        Ok(resolved) => resolved,
        Err(refusal) => unreachable!("must emit: {}", refusal.rendered()),
    };
    assert_eq!(resolved.timeout_ms, Some(600_000));
    assert_eq!(resolved.timeout_owner.as_deref(), Some("waffles"));
}

/// The declared env, cwd and hardening reach the resolved form structurally —
/// never as a `K=V` prefix, and with `{workspace_root}` left for the
/// executing side to expand against ITS workspace.
#[test]
fn env_cwd_and_hardening_resolve_structurally() {
    let decl = only_command(
        "command c(bin: String)\n  program \"cargo\" \"nextest\"\n  admits \"--\"\n  \
         env GIT_PAGER = \"cat\"\n  env RUST_BACKTRACE = \"1\"\n  cwd \"{workspace_root}/repo\"\n  \
         hardening\n    path \"{{bin}}\"\n    open_files 65536",
    );
    let Ok(checked) = check_command(&decl) else {
        unreachable!("checks clean");
    };
    let resolved = match emit(
        checked,
        &ParameterSet::new().with("bin", ArgumentValue::scalar("/home/x/.cargo/bin")),
    ) {
        Ok(resolved) => resolved,
        Err(refusal) => unreachable!("must emit: {}", refusal.rendered()),
    };
    assert_eq!(
        resolved.env,
        vec![
            ("GIT_PAGER".to_owned(), "cat".to_owned()),
            ("RUST_BACKTRACE".to_owned(), "1".to_owned()),
        ]
    );
    assert_eq!(resolved.cwd.as_deref(), Some("{workspace_root}/repo"));
    assert_eq!(
        resolved.hardened_path.as_deref(),
        Some("/home/x/.cargo/bin")
    );
    assert_eq!(resolved.open_files, Some(65536));
    assert!(
        !resolved.argv.iter().any(|element| element.contains('=')),
        "an environment binding must never reach the argv"
    );
}

/// THE SPECIMEN LAYER. A refusing invocation's specimen is FIRED — run
/// through the real emitter — and must produce the leading-dash refusal.
#[test]
fn firing_a_declared_specimen_proves_the_refusal_is_load_bearing() {
    let decl = only_command(DIFF_CENSUS);
    let Ok(checked) = check_command(&decl) else {
        unreachable!("checks clean");
    };
    assert!(
        fire_specimen(checked).is_ok(),
        "the census specimen must refuse when fired"
    );

    // An admitting declaration carries no specimen, so there is nothing to
    // fire and firing is trivially satisfied.
    let admitting = only_command(GIT_TAG);
    let Ok(admitting) = check_command(&admitting) else {
        unreachable!("checks clean");
    };
    assert!(fire_specimen(admitting).is_ok());
}
