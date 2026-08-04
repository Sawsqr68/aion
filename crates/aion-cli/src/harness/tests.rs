//! Tests for harness selection and per-harness configuration.
//!
//! The selection tests are written against whatever this build actually carries, so
//! they hold under every feature combination rather than only the default one: a build
//! with one harness must DERIVE it, a build with several must refuse without a
//! selection, and a build with none must resolve to nothing without erroring. The
//! configuration tests are feature-gated, because a setting for an adapter this build
//! does not link has nothing to be parsed into.

use crate::settings::Environment;

use super::catalogue::{self};
// Named only by the tests that configure a specific adapter; a build with none carries
// no such test.
#[cfg(any(feature = "norn", feature = "acp"))]
use super::catalogue::HarnessKind;
use super::settings::HarnessArgs;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Arguments with nothing supplied — the shape `--check` is invoked with.
fn args() -> HarnessArgs {
    HarnessArgs::default()
}

/// The three-case selection rule, checked against what this build actually carries.
///
/// Written as one test over `catalogue::compiled()` rather than three feature-gated
/// ones, because the RULE is what matters and it must hold for every build: nothing to
/// choose is derived, a choice is refused until it is made.
#[test]
fn selection_derives_one_harness_and_refuses_an_ambiguous_choice() -> TestResult {
    let compiled = catalogue::compiled();
    let resolved = args().select(&Environment::default());
    match compiled.as_slice() {
        [] => {
            // No adapter compiled in is NOT an error: the action surface is still
            // reported, and the serve path refuses naming the build.
            assert_eq!(resolved?, None);
        }
        [only] => assert_eq!(
            resolved?,
            Some(*only),
            "one harness leaves nothing to choose"
        ),
        several => {
            let Err(error) = resolved else {
                return Err("several harnesses must not be resolved silently".into());
            };
            let message = error.to_string();
            for harness in several {
                assert!(
                    message.contains(harness.name()),
                    "the refusal must name every choice, missing `{}`: {message}",
                    harness.name()
                );
            }
            assert!(
                message.contains("--harness"),
                "the refusal must name the flag that selects one: {message}"
            );
        }
    }
    Ok(())
}

/// A harness name this build does not carry is refused naming what it does carry — a
/// harness is compiled in by a cargo feature, so it cannot be supplied at runtime and
/// the operator needs to know what the binary they are holding can do.
#[test]
fn an_uncompiled_harness_name_is_refused_naming_what_is_compiled_in() -> TestResult {
    let mut supplied = args();
    supplied.harness = Some(String::from("nobody"));
    let Err(error) = supplied.select(&Environment::default()) else {
        return Err("a harness this build does not carry must be refused".into());
    };
    let message = error.to_string();
    assert!(
        message.contains("nobody"),
        "the refusal names the selection: {message}"
    );
    for harness in catalogue::compiled() {
        assert!(
            message.contains(harness.name()),
            "the refusal names what the build carries, missing `{}`: {message}",
            harness.name()
        );
    }
    if catalogue::compiled().is_empty() {
        assert!(
            message.contains("none"),
            "a build with no harness says so rather than listing nothing: {message}"
        );
    }
    Ok(())
}

/// The selection reads its environment variable, and the flag wins over it — the same
/// contract every other worker setting follows.
#[test]
fn the_environment_selects_a_harness_and_the_flag_wins() -> TestResult {
    let Some(first) = catalogue::compiled().first().copied() else {
        // Nothing to select in this build; the no-harness case is covered above.
        return Ok(());
    };
    let environment = Environment::from_pairs([("AION_AGENT_HARNESS", first.name())]);
    assert_eq!(args().select(&environment)?, Some(first));

    // A blank variable is a deployment mistake, not a selection: it must fall through
    // to the derivation rather than becoming a harness named "".
    let blank = Environment::from_pairs([("AION_AGENT_HARNESS", "   ")]);
    let derived = args().select(&blank);
    match catalogue::compiled().as_slice() {
        [only] => assert_eq!(derived?, Some(*only)),
        _ => assert!(
            derived.is_err(),
            "a blank selection must not resolve an ambiguous build"
        ),
    }

    let mut supplied = args();
    supplied.harness = Some(first.name().to_owned());
    let other = Environment::from_pairs([("AION_AGENT_HARNESS", "nobody")]);
    assert_eq!(
        supplied.select(&other)?,
        Some(first),
        "the flag wins over the variable"
    );
    Ok(())
}

/// A flag configuring a harness that is not the resolved one is REFUSED, not ignored.
/// A flag that silently does nothing is how an operator comes to believe a policy is in
/// force when it is not.
#[cfg(all(feature = "norn", feature = "acp"))]
#[test]
fn a_flag_for_another_harness_is_refused_by_name() -> TestResult {
    let mut supplied = args();
    supplied.harness = Some(String::from("norn"));
    supplied.acp_command = Some(String::from("/usr/bin/agent"));
    supplied.acp_args = vec![String::from("--acp")];
    let Err(error) = supplied.select(&Environment::default()) else {
        return Err("an ACP flag on a norn worker must be refused".into());
    };
    let message = error.to_string();
    assert!(
        message.contains("--acp-command") && message.contains("--acp-arg"),
        "the refusal must name every offending flag: {message}"
    );
    assert!(
        message.contains("norn"),
        "the refusal must name the harness being served: {message}"
    );

    // And the other direction: every norn flag on an ACP worker — the scalar and
    // both repeatables, so a future edit that drops one from the attribution table
    // fails here rather than silently accepting it.
    let mut supplied = args();
    supplied.harness = Some(String::from("acp"));
    supplied.norn_binary = Some(String::from("/opt/norn"));
    supplied.norn_args = vec![String::from("--allow-degraded-session")];
    supplied.norn_unset_env = vec![String::from("OPENAI_API_KEY")];
    let Err(error) = supplied.select(&Environment::default()) else {
        return Err("a norn flag on an ACP worker must be refused".into());
    };
    let message = error.to_string();
    for flag in ["--norn-binary", "--norn-arg", "--norn-unset-env"] {
        assert!(
            message.contains(flag),
            "the refusal must name the offending flag `{flag}`: {message}"
        );
    }
    Ok(())
}

/// Each new repeatable norn flag is refused on its own when serving another harness —
/// not only in company with `--norn-binary`, which was already attributed. A flag that
/// is only refused alongside an older one would silently pass once supplied alone.
#[cfg(all(feature = "norn", feature = "acp"))]
#[test]
fn each_repeatable_norn_flag_is_refused_alone_on_a_foreign_harness() -> TestResult {
    let mut supplied = args();
    supplied.harness = Some(String::from("acp"));
    supplied.norn_args = vec![String::from("--fast")];
    let Err(error) = supplied.select(&Environment::default()) else {
        return Err("--norn-arg alone on an ACP worker must be refused".into());
    };
    assert!(
        error.to_string().contains("--norn-arg"),
        "the refusal must name --norn-arg: {error}"
    );

    let mut supplied = args();
    supplied.harness = Some(String::from("acp"));
    supplied.norn_unset_env = vec![String::from("OPENAI_API_KEY")];
    let Err(error) = supplied.select(&Environment::default()) else {
        return Err("--norn-unset-env alone on an ACP worker must be refused".into());
    };
    assert!(
        error.to_string().contains("--norn-unset-env"),
        "the refusal must name --norn-unset-env: {error}"
    );
    Ok(())
}

/// An ambient environment variable for another harness is NOT refused: a box that runs
/// both an ACP worker and a Norn worker legitimately exports both, and refusing on
/// ambient state would break a working deployment.
#[cfg(all(feature = "norn", feature = "acp"))]
#[test]
fn an_ambient_variable_for_another_harness_is_not_refused() -> TestResult {
    let mut supplied = args();
    supplied.harness = Some(String::from("norn"));
    let environment = Environment::from_pairs([
        ("AION_ACP_COMMAND", "/usr/bin/agent"),
        ("AION_ACP_PERMISSION", "deny"),
    ]);
    assert_eq!(supplied.select(&environment)?, Some(HarnessKind::Norn));
    Ok(())
}

/// Every ACP setting is demanded by name, and all of them at once: an operator fixing
/// this in a shell should need one correction, not four.
#[cfg(feature = "acp")]
#[test]
fn every_missing_acp_setting_is_reported_at_once() -> TestResult {
    let mut missing = Vec::new();
    let supplied = args().collect(
        HarnessKind::Acp.settings(),
        &Environment::default(),
        &mut missing,
    );
    let Err(error) = crate::settings::refuse_if_missing(missing) else {
        return Err("an unconfigured ACP harness must be refused".into());
    };
    let report = error.to_string();
    for expected in [
        "--acp-command or $AION_ACP_COMMAND",
        "--acp-cwd or $AION_ACP_CWD",
        "--acp-permission or $AION_ACP_PERMISSION",
        "--acp-exit-grace or $AION_ACP_EXIT_GRACE_SECONDS",
    ] {
        assert!(
            report.contains(expected),
            "the report must name `{expected}`, got:\n{report}"
        );
    }
    // The repeatable settings are optional: an agent may need no arguments at all
    // (Claude Code takes none), so their absence is not a miss.
    assert!(
        !report.contains("--acp-arg") && !report.contains("--acp-env"),
        "the repeatable settings are optional and must not be demanded:\n{report}"
    );
    // And the bag still parses to a refusal rather than a half-built configuration.
    assert!(
        supplied.parse().is_err(),
        "an incomplete ACP configuration must not parse"
    );
    Ok(())
}

/// A fully supplied ACP configuration parses to exactly what was supplied — the
/// command and its arguments verbatim, because ACP has no standard launch flag and a
/// wrong guess spawns the agent in a mode that speaks no JSON-RPC.
#[cfg(feature = "acp")]
#[test]
fn a_supplied_acp_configuration_parses_verbatim() -> TestResult {
    use super::settings::HarnessConfig;

    let mut supplied = args();
    supplied.acp_command = Some(String::from("npx"));
    supplied.acp_args = vec![
        String::from("@google/gemini-cli"),
        String::from("--acp"),
        String::from("--model=gemini-2.5-pro"),
    ];
    supplied.acp_cwd = Some(String::from("/srv/workspace"));
    supplied.acp_permission = Some(String::from("allow-once"));
    supplied.acp_env = vec![String::from("GEMINI_API_KEY=k=1"), String::from("QUIET=")];
    supplied.acp_exit_grace = Some(String::from("2.5"));

    let mut missing = Vec::new();
    let config = supplied
        .collect(
            HarnessKind::Acp.settings(),
            &Environment::default(),
            &mut missing,
        )
        .parse()?;
    assert!(missing.is_empty(), "nothing was missing: {missing:?}");
    let HarnessConfig::Acp(acp) = config else {
        return Err("an ACP selection must produce an ACP configuration".into());
    };
    assert_eq!(acp.command, std::path::Path::new("npx"));
    assert_eq!(
        acp.args,
        vec![
            String::from("@google/gemini-cli"),
            String::from("--acp"),
            String::from("--model=gemini-2.5-pro"),
        ],
        "the argv is exactly what was configured, in order"
    );
    assert_eq!(acp.cwd, std::path::Path::new("/srv/workspace"));
    assert_eq!(
        acp.permission,
        aion_integration_acp::PermissionPolicy::AllowOnce
    );
    // Split at the FIRST `=`, so a value containing `=` survives, and an empty value is
    // a real setting distinct from leaving the variable unset.
    assert_eq!(
        acp.env,
        vec![
            (String::from("GEMINI_API_KEY"), String::from("k=1")),
            (String::from("QUIET"), String::new()),
        ]
    );
    assert_eq!(acp.exit_grace, std::time::Duration::from_millis(2500));
    Ok(())
}

/// The environment supplies the ACP settings too, and a blank value counts as missing
/// rather than becoming an empty command nobody can spawn.
#[cfg(feature = "acp")]
#[test]
fn the_environment_supplies_acp_settings_and_blank_counts_as_missing() -> TestResult {
    let environment = Environment::from_pairs([
        ("AION_ACP_COMMAND", "opencode"),
        ("AION_ACP_CWD", "/srv/workspace"),
        ("AION_ACP_PERMISSION", "DENY"),
        ("AION_ACP_EXIT_GRACE_SECONDS", "0"),
    ]);
    let mut missing = Vec::new();
    let config = args()
        .collect(HarnessKind::Acp.settings(), &environment, &mut missing)
        .parse()?;
    assert!(missing.is_empty(), "nothing was missing: {missing:?}");
    let super::settings::HarnessConfig::Acp(acp) = config else {
        return Err("an ACP selection must produce an ACP configuration".into());
    };
    assert_eq!(acp.command, std::path::Path::new("opencode"));
    // Zero grace is a real choice: close stdin, then kill at once.
    assert_eq!(acp.exit_grace, std::time::Duration::ZERO);
    // Case-insensitive: an operator writing DENY in a unit file means deny.
    assert_eq!(acp.permission, aion_integration_acp::PermissionPolicy::Deny);

    let blank = Environment::from_pairs([
        ("AION_ACP_COMMAND", "   "),
        ("AION_ACP_CWD", "/srv/workspace"),
        ("AION_ACP_PERMISSION", "deny"),
        ("AION_ACP_EXIT_GRACE_SECONDS", "1"),
    ]);
    let mut missing = Vec::new();
    args().collect(HarnessKind::Acp.settings(), &blank, &mut missing);
    let Err(error) = crate::settings::refuse_if_missing(missing) else {
        return Err("a blank command must be reported missing".into());
    };
    assert!(
        error.to_string().contains("--acp-command"),
        "the refusal names the blank setting: {error}"
    );
    Ok(())
}

/// A relative `--acp-cwd` is refused rather than resolved against the worker's own
/// directory: ACP requires an absolute session root, and inventing one would root the
/// agent somewhere nobody chose.
#[cfg(feature = "acp")]
#[test]
fn a_relative_acp_cwd_is_refused() -> TestResult {
    let mut supplied = args();
    supplied.acp_command = Some(String::from("opencode"));
    supplied.acp_cwd = Some(String::from("workspace/agent"));
    supplied.acp_permission = Some(String::from("deny"));
    supplied.acp_exit_grace = Some(String::from("2"));
    let mut missing = Vec::new();
    let Err(error) = supplied
        .collect(
            HarnessKind::Acp.settings(),
            &Environment::default(),
            &mut missing,
        )
        .parse()
    else {
        return Err("a relative ACP working directory must be refused".into());
    };
    let message = error.to_string();
    assert!(
        message.contains("--acp-cwd") && message.contains("workspace/agent"),
        "the refusal names the setting and the value: {message}"
    );
    assert!(
        message.contains("ABSOLUTE"),
        "the refusal states the rule: {message}"
    );
    Ok(())
}

/// The permission policy has NO default and no third value: an unknown spelling is
/// refused naming both accepted ones, because guessing here is guessing at a security
/// decision.
#[cfg(feature = "acp")]
#[test]
fn an_unknown_permission_policy_is_refused_naming_both_values() -> TestResult {
    let mut supplied = args();
    supplied.acp_command = Some(String::from("opencode"));
    supplied.acp_cwd = Some(String::from("/srv/workspace"));
    supplied.acp_exit_grace = Some(String::from("2"));
    supplied.acp_permission = Some(String::from("allow-always"));
    let mut missing = Vec::new();
    let Err(error) = supplied
        .collect(
            HarnessKind::Acp.settings(),
            &Environment::default(),
            &mut missing,
        )
        .parse()
    else {
        return Err("an unknown permission policy must be refused".into());
    };
    let message = error.to_string();
    assert!(
        message.contains("deny") && message.contains("allow-once"),
        "the refusal names both accepted values: {message}"
    );
    assert!(
        message.contains("allow-always"),
        "the refusal names what was supplied: {message}"
    );
    Ok(())
}

/// A malformed `--acp-env` entry is refused rather than silently dropped: an operator
/// who believes an API key reached the agent when it did not gets a failing agent and
/// no explanation.
#[cfg(feature = "acp")]
#[test]
fn a_malformed_child_environment_entry_is_refused() -> TestResult {
    let base = || {
        let mut supplied = args();
        supplied.acp_command = Some(String::from("opencode"));
        supplied.acp_cwd = Some(String::from("/srv/workspace"));
        supplied.acp_permission = Some(String::from("deny"));
        supplied.acp_exit_grace = Some(String::from("2"));
        supplied
    };
    for malformed in ["GEMINI_API_KEY", "=orphan"] {
        let mut supplied = base();
        supplied.acp_env = vec![malformed.to_owned()];
        let mut missing = Vec::new();
        let Err(error) = supplied
            .collect(
                HarnessKind::Acp.settings(),
                &Environment::default(),
                &mut missing,
            )
            .parse()
        else {
            return Err(format!("`{malformed}` is not a KEY=VALUE pair").into());
        };
        assert!(
            error.to_string().contains("--acp-env"),
            "the refusal names the setting: {error}"
        );
    }
    Ok(())
}

/// The `norn` binary is the ONE optional harness setting, because its default lives in
/// the adapter (`norn` on `PATH`) rather than being invented at this boundary. Supplying
/// it is what lets an operator whose `norn` is installed elsewhere use the verb at all.
#[cfg(feature = "norn")]
#[test]
fn the_norn_binary_is_optional_and_overridable() -> TestResult {
    use super::settings::HarnessConfig;

    let mut missing = Vec::new();
    let derived = args()
        .collect(
            HarnessKind::Norn.settings(),
            &Environment::default(),
            &mut missing,
        )
        .parse()?;
    assert!(
        missing.is_empty(),
        "the adapter owns this default, so nothing is missing: {missing:?}"
    );
    assert_eq!(
        derived,
        HarnessConfig::Norn(super::settings::NornConfig {
            binary: None,
            args: Vec::new(),
            unset_env: Vec::new(),
        }),
        "no binary configured means the adapter's own `norn`-on-PATH default"
    );

    let environment = Environment::from_pairs([("AION_NORN_BINARY", "/opt/homebrew/bin/norn")]);
    let configured = args()
        .collect(HarnessKind::Norn.settings(), &environment, &mut missing)
        .parse()?;
    assert_eq!(
        configured,
        HarnessConfig::Norn(super::settings::NornConfig {
            binary: Some(std::path::PathBuf::from("/opt/homebrew/bin/norn")),
            args: Vec::new(),
            unset_env: Vec::new(),
        })
    );
    Ok(())
}

/// A `--norn-unset-env` entry that is not a plain variable name is refused rather
/// than reinterpreted: a `KEY=VALUE` pair is the shape of a SET (this flag removes),
/// and a blank entry is an unset-template mistake no environment can carry.
#[cfg(feature = "norn")]
#[test]
fn a_malformed_norn_unset_env_entry_is_refused() -> TestResult {
    for malformed in ["OPENAI_API_KEY=", "OPENAI_API_KEY=abc", "=", "   "] {
        let mut supplied = args();
        supplied.norn_unset_env = vec![malformed.to_owned()];
        let mut missing = Vec::new();
        let Err(error) = supplied
            .collect(
                HarnessKind::Norn.settings(),
                &Environment::default(),
                &mut missing,
            )
            .parse()
        else {
            return Err(format!("`{malformed}` is not a variable name and must be refused").into());
        };
        assert!(
            error.to_string().contains("--norn-unset-env"),
            "the refusal names the setting: {error}"
        );
    }
    Ok(())
}

/// THE ARGV-PARITY TEST: the reference session-pinned invocation the assistant's
/// hand-written worker composes (`examples/agent-dev/worker/src/norn_invocation.rs`)
/// is reproduced by `--norn-arg` pass-through, asserted on the COMPOSED child
/// invocation with the run-identity placeholders expanded. A flag only observed
/// through a live agent run is untested in the case that matters — which is exactly
/// how `--account` once went missing from a harness invocation.
#[cfg(all(feature = "norn", feature = "liminal-transport"))]
#[test]
fn the_norn_argv_pass_through_reaches_the_child_expanded_and_in_order() -> TestResult {
    use aion_core::{ActivityId, ContentType, Payload, WorkflowId};
    use aion_integrations::AgentRunSpec;

    // Exactly what the operator types, one `--norn-arg` per element.
    let mut supplied = args();
    supplied.norn_args = [
        "--workspace-root",
        "/tmp/{workflow_id}/repo",
        "-C",
        "/tmp/{workflow_id}/repo",
        "--session-id",
        "{workflow_id}-{activity_type}",
        "--resume-if-exists",
        "--account",
        "default",
        "--fast",
    ]
    .map(str::to_owned)
    .to_vec();
    supplied.norn_unset_env = vec![String::from("OPENAI_API_KEY")];

    let mut missing = Vec::new();
    let config = supplied
        .collect(
            HarnessKind::Norn.settings(),
            &Environment::default(),
            &mut missing,
        )
        .parse()?;
    assert!(missing.is_empty(), "nothing was missing: {missing:?}");
    let super::settings::HarnessConfig::Norn(norn) = config else {
        return Err("a norn selection must produce a norn configuration".into());
    };

    let harness = super::compose::build_norn(&norn);
    let workflow_id = WorkflowId::new_v4();
    let spec = AgentRunSpec::new(
        workflow_id.clone(),
        ActivityId::from_sequence_position(1),
        1,
        "assistant",
        Payload::new(ContentType::Json, b"\"prompt\"".to_vec()),
    );
    let invocation = harness.invocation(&spec);

    let clone = format!("/tmp/{workflow_id}/repo");
    assert_eq!(
        invocation.argv,
        vec![
            String::from("--protocol"),
            String::from("jsonrpc"),
            String::from("--workspace-root"),
            clone.clone(),
            String::from("-C"),
            clone,
            String::from("--session-id"),
            format!("{workflow_id}-assistant"),
            String::from("--resume-if-exists"),
            String::from("--account"),
            String::from("default"),
            String::from("--fast"),
        ],
        "the child argv must be the operator's arguments, in order, with the \
         run-identity placeholders expanded"
    );
    assert_eq!(
        invocation.env_removed,
        vec![String::from("OPENAI_API_KEY")],
        "--norn-unset-env must reach the child as an environment removal"
    );
    Ok(())
}

/// The name a harness is SELECTED by and the name its settings group is ATTRIBUTED to
/// must be the same string.
///
/// They are written in two places — [`HarnessKind::name`] for the selection, and the
/// settings group for the foreign-flag refusal — and the refusal compares them. If they
/// ever drifted, every flag for the renamed harness would silently be treated as
/// belonging to a different one, which is the exact failure the refusal exists to
/// prevent.
#[test]
fn a_harness_is_selected_and_attributed_by_the_same_name() {
    for harness in catalogue::compiled() {
        assert_eq!(
            harness.name(),
            harness.settings().harness_name(),
            "the selection name and the settings-group name must not drift"
        );
    }
}

/// Every compiled-in harness publishes a non-empty registration set, and it is the
/// ADAPTER's own published set — not a copy made here. A build with none advertises the
/// empty set, which is a first-class advertisement rather than a degenerate one.
#[test]
fn each_harness_advertises_its_adapters_published_capabilities() {
    assert!(
        catalogue::registration_capabilities(None).is_empty(),
        "a build with no harness supports no interventions"
    );
    for harness in catalogue::compiled() {
        let published = catalogue::registration_capabilities(Some(harness));
        assert!(
            !published.is_empty(),
            "`{}` must advertise what its adapter can honour",
            harness.name()
        );
        assert_eq!(
            published,
            harness.registration_capabilities(),
            "the set must come from the adapter, not from a second copy"
        );
    }
}

/// The ACP harness advertises exactly `Cancel` — the one primitive ACP v1 can deliver —
/// so the ops console offers that control and none of the four it cannot honour.
#[cfg(feature = "acp")]
#[test]
fn the_acp_harness_advertises_exactly_cancel() {
    use aion_core::InterventionPrimitive;

    let published = catalogue::registration_capabilities(Some(HarnessKind::Acp));
    assert!(published.supports_primitive(InterventionPrimitive::Cancel));
    for unsupported in [
        InterventionPrimitive::InjectMessage,
        InterventionPrimitive::PauseResume,
        InterventionPrimitive::UpdateBudget,
        InterventionPrimitive::RespondToApproval,
    ] {
        assert!(
            !published.supports_primitive(unsupported),
            "{unsupported:?} has no ACP mechanism and must not be advertised"
        );
    }
    assert_eq!(
        published,
        aion_integration_acp::registration_capabilities(),
        "the set is the adapter's, published by it"
    );
}

/// The Norn harness advertises what its adapter can translate — both primitives, not an
/// empty set, which would leave the console offering no controls for a Norn agent.
#[cfg(feature = "norn")]
#[test]
fn the_norn_harness_advertises_its_two_primitives() {
    use aion_core::InterventionPrimitive;

    let published = catalogue::registration_capabilities(Some(HarnessKind::Norn));
    assert!(
        published.supports_primitive(InterventionPrimitive::InjectMessage)
            && published.supports_primitive(InterventionPrimitive::Cancel),
        "got {published:?}"
    );
    assert_eq!(
        published,
        aion_integration_norn::registration_capabilities()
    );
}

/// Each accepted `--acp-permission` spelling resolves to the ADAPTER's own policy, and
/// the transcript label it will record is the one the flag's help promises — so an
/// operator can reconcile what they typed with what an audit shows.
#[cfg(feature = "acp")]
#[test]
fn each_permission_spelling_resolves_to_the_adapters_policy() -> TestResult {
    use aion_integration_acp::PermissionPolicy;

    use super::settings::HarnessConfig;

    for (spelling, expected, label) in [
        ("deny", PermissionPolicy::Deny, "deny"),
        ("allow-once", PermissionPolicy::AllowOnce, "allow_once"),
    ] {
        let mut supplied = args();
        supplied.acp_command = Some(String::from("opencode"));
        supplied.acp_cwd = Some(String::from("/srv/workspace"));
        supplied.acp_exit_grace = Some(String::from("2"));
        supplied.acp_permission = Some(spelling.to_owned());
        let mut missing = Vec::new();
        let config = supplied
            .collect(
                HarnessKind::Acp.settings(),
                &Environment::default(),
                &mut missing,
            )
            .parse()?;
        let HarnessConfig::Acp(acp) = config else {
            return Err("an ACP selection must produce an ACP configuration".into());
        };
        assert_eq!(
            acp.permission, expected,
            "`{spelling}` selects {expected:?}"
        );
        assert_eq!(
            acp.permission.label(),
            label,
            "`{spelling}` is recorded on the transcript as `{label}`"
        );
    }
    Ok(())
}

/// Composing a configured harness yields something the worker can drive through the
/// neutral seam — the out-of-box wiring, proven per compiled-in harness rather than
/// asserted for one.
///
/// Nothing is spawned: constructing an adapter only captures its configuration, which is
/// what makes this a composition-root test rather than an agent test.
#[cfg(all(any(feature = "norn", feature = "acp"), feature = "liminal-transport"))]
#[test]
fn every_compiled_harness_composes() {
    for harness in catalogue::compiled() {
        let config = harness.test_config();
        // The composed value is accepted by the worker SDK's own agent binding, which is
        // the only thing the serve path requires of it: an `Arc<dyn DynAgentHarness>`
        // the worker can drive without knowing which adapter it got.
        let advertised = aion_worker::AgentHarnessConfig::new(
            super::compose::build(&config),
            [String::from("review")],
            catalogue::registration_capabilities(Some(harness)),
        );
        assert!(
            advertised.agent_activity_types().contains("review"),
            "`{}` must compose into a harness the worker binds",
            harness.name()
        );
    }
}
