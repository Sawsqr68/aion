"""Move the agent-worker verb tests onto the document-declared launch."""

import pathlib

p = pathlib.Path("crates/aion-cli/src/worker_agent/tests.rs")
s = p.read_text()

s = s.replace("use crate::harness::HarnessArgs;\n", "")

s = s.replace(
    """fn args_for(document: &Path) -> AgentArgs {""",
    """fn args_for(document: &Path) -> AgentArgs {""",
)

# The constructor: only the surviving fields remain.
start = s.index("fn args_for(document: &Path) -> AgentArgs {")
end = s.index("\n}\n", start) + len("\n}\n")
s = (
    s[:start]
    + """fn args_for(document: &Path) -> AgentArgs {
    AgentArgs {
        awl: Some(document.to_path_buf()),
        liminal_addresses: Vec::new(),
        task_queue: None,
        identity: None,
        node: None,
        check: false,
    }
}
"""
    + s[end:]
)

# The missing-settings report now covers exactly the two settings that are still flags.
old = s[
    s.index(
        "/// Every connection setting must be supplied. A worker started from a shell should"
    ) : s.index("/// The environment supplies a setting the flag omits, and the flag WINS when both")
]
new = '''/// Every connection setting must be supplied. A worker started from a shell should
/// need ONE correction, not two — so every missing setting is named at once, by both
/// the flag and the variable that could supply it.
///
/// The set is deliberately SHORT now: concurrency and the three reconnect settings are
/// declared in the document\'s `harness` section and are no longer flags, so a missing
/// one is refused by `aion awl check` with the field named, long before a worker is
/// started. What is left is what genuinely belongs to this process on this box.
#[test]
fn missing_settings_are_reported_by_name_and_all_at_once() -> TestResult {
    let document = write_document("missing", DOCUMENT)?;
    let Err(error) = args::resolve(&args_for(&document), &Environment::default()) else {
        return Err("resolution with nothing supplied must be refused".into());
    };
    let report = error.to_string();
    for expected in [
        "--liminal-address or $AION_WORKER_LIMINAL_ADDRESSES",
        "--identity or $AION_WORKER_IDENTITY",
    ] {
        assert!(
            report.contains(expected),
            "the report must name `{expected}`, got:\\n{report}"
        );
    }
    for deleted in ["--concurrency", "--reconnect-"] {
        assert!(
            !report.contains(deleted),
            "`{deleted}` is declared in the document now, so the connection report must \\
             not ask for it: {report}"
        );
    }
    Ok(())
}

'''
s = s.replace(old, new, 1)

# The environment/flag-precedence test loses the migrated settings.
old = s[
    s.index("/// The environment supplies a setting the flag omits, and the flag WINS when both") : s.index(
        "/// An exported-but-blank variable is a deployment mistake"
    )
]
new = '''/// The environment supplies a setting the flag omits, and the flag WINS when both
/// are present. Nothing is defaulted except `node`, which falls through to the
/// worker SDK\'s own documented hostname default.
#[test]
fn the_environment_supplies_settings_and_the_flag_wins() -> TestResult {
    let document = write_document("environment", DOCUMENT)?;
    let environment = Environment::from_pairs([
        (
            "AION_WORKER_LIMINAL_ADDRESSES",
            "127.0.0.1:50061, 127.0.0.1:50062",
        ),
        ("AION_WORKER_IDENTITY", "from-environment"),
        ("AION_TASK_QUEUE", "reviewers"),
    ]);

    let resolved = args::resolve(&args_for(&document), &environment)?;
    assert_eq!(
        resolved.addresses,
        vec![
            String::from("127.0.0.1:50061"),
            String::from("127.0.0.1:50062")
        ],
        "a comma-separated list keeps its order and drops blank entries"
    );
    assert_eq!(resolved.identity, "from-environment");
    assert_eq!(resolved.node, None, "node is the SDK\'s default, not ours");

    let mut arguments = args_for(&document);
    arguments.identity = Some(String::from("from-flag"));
    arguments.liminal_addresses = vec![String::from("127.0.0.1:50099")];
    arguments.task_queue = Some(String::from("from-flag-queue"));
    let resolved = args::resolve(&arguments, &environment)?;
    assert_eq!(resolved.identity, "from-flag");
    assert_eq!(resolved.addresses, vec![String::from("127.0.0.1:50099")]);
    assert_eq!(
        arguments.requested_queue(&environment).as_deref(),
        Some("from-flag-queue")
    );
    Ok(())
}

'''
s = s.replace(old, new, 1)

# The blank-value test drops its harness argument.
s = s.replace(
    """    let Err(error) = args::resolve(&args_for(&document), &environment, None) else {
        return Err("blank values must be refused as missing".into());
    };""",
    """    let Err(error) = args::resolve(&args_for(&document), &environment) else {
        return Err("blank values must be refused as missing".into());
    };""",
)

# The unusable-value test tested settings that are now document fields, and the
# missing-harness-setting test tested a shared refusal that no longer exists. Both are
# replaced by the launch-source tests below.
old = s[
    s.index("/// An unusable value is refused naming the setting and the value, never coerced.") : s.index(
        "/// `--check` prints the exact surface"
    )
]
new = '''/// The LAUNCH comes from the document and from nowhere else.
///
/// The four settings that moved — concurrency and the reconnect trio — are read off a
/// compiled `harness` section here. The pin is that the values reaching the worker are
/// the DOCUMENT\'s, so a future edit that reintroduced an environment fallback for any of
/// them would be caught even though the flags are gone.
#[test]
fn the_launch_settings_come_from_the_documents_harness_section() -> TestResult {
    let document = write_document("launch", &harness_document())?;
    let Some(launch) = crate::worker_surface::compile_harness(&document, "reviewers")? else {
        return Err("the document declares a harness section".into());
    };
    assert_eq!(launch.concurrency, 7);
    assert_eq!(launch.reconnect_initial_backoff, Duration::from_secs(5));
    assert_eq!(launch.reconnect_max_backoff, Duration::from_secs(120));
    assert_eq!(launch.reconnect_max_attempts, 42);
    assert_eq!(
        launch.env_pass,
        vec![String::from("PATH"), String::from("HOME")],
        "the pass-through list is the agent child\'s WHOLE environment"
    );
    Ok(())
}

/// A document with no `harness` section declares no launch, and that is a first-class
/// answer rather than an error: a queue served by a worker SDK build or by declared
/// bodies has no agent to start.
#[test]
fn a_document_without_a_harness_section_declares_no_launch() -> TestResult {
    let document = write_document("nolaunch", DOCUMENT)?;
    let launch = crate::worker_surface::compile_harness(&document, "reviewers")?;
    assert!(
        launch.is_none(),
        "no section means no launch, not an empty one"
    );
    Ok(())
}

/// MIGRATION RIDER: a document that declares a queue but no launch is refused LOUDLY,
/// naming where the settings live now.
///
/// This is the refusal an operator meets after the flags were deleted, so it carries the
/// whole shape rather than one missing field: they have almost certainly arrived with
/// the old flags in their fingers.
#[test]
fn serving_a_document_with_no_harness_section_refuses_naming_the_section() -> TestResult {
    let document = write_document("refusal", DOCUMENT)?;
    let surface = surface::derive(&document, None)?;
    let error = super::serve::no_harness_section(&surface, &document);
    let message = error.to_string();
    for expected in [
        "declares no `harness` section",
        "There is no flag for any of it",
        "aion awl check",
        "reviewers",
    ] {
        assert!(
            message.contains(expected),
            "the refusal must name `{expected}`, got:\\n{message}"
        );
    }
    Ok(())
}

'''
s = s.replace(old, new, 1)

old = s[
    s.index(
        "/// A missing HARNESS setting appears in the SAME refusal as a missing connection"
    ) : s.index(
        "/// The composed harness's ADVERTISED set — the union `AgentHarnessConfig` computes"
    )
]
s = s.replace(old, "", 1)

# A document carrying a complete harness section, for the launch-source tests.
s += '''
/// The queue document with a complete `norn` launch declared on it.
fn harness_document() -> String {
    DOCUMENT.replace(
        "worker reviewers\\n",
        "worker reviewers\\n  harness\\n    kind norn\\n    concurrency 7\\n    \\
         reconnect_initial_backoff 5s\\n    reconnect_max_backoff 2m\\n    \\
         reconnect_max_attempts 42\\n    env_pass \\"PATH\\", \\"HOME\\"\\n",
    )
}
'''

p.write_text(s)
print("ok")
