//! Tests for harness resolution and per-harness configuration.
//!
//! The resolution tests are written against whatever this build actually carries, so
//! they hold under every feature combination rather than only the default one: a build
//! that carries the declared kind resolves it, a build that does not refuses naming what
//! it does, and a build with none resolves to nothing without erroring. The
//! configuration tests are feature-gated, because a setting for an adapter this build
//! does not link has nothing to be built into.
//!
//! Every configuration here comes from a `CompiledHarness` — the checked `harness`
//! section of a worker document. There is no flag left to test: the launch flags were
//! DELETED when the section shipped, and the tests that exercised them went with them
//! rather than being kept alongside as a second surface nothing serves.

use std::path::{Path, PathBuf};
use std::time::Duration;

use aion_awl::{CompiledHarness, CompiledHarnessKind, HarnessPermission};

use super::catalogue;
// Named only by the tests that configure a specific adapter; a build with none carries
// no such test.
#[cfg(any(feature = "norn", feature = "acp"))]
use super::catalogue::HarnessKind;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// The document path every refusal names. Never read: these tests build a
/// [`CompiledHarness`] directly, which is what the compile seam hands the resolver.
fn document() -> PathBuf {
    PathBuf::from("/srv/flows/assistant.awl")
}

/// A declared launch of `kind`, with the kind-independent settings filled in.
fn declared(kind: CompiledHarnessKind) -> CompiledHarness {
    CompiledHarness {
        concurrency: 4,
        reconnect_initial_backoff: Duration::from_secs(5),
        reconnect_max_backoff: Duration::from_secs(120),
        reconnect_max_attempts: 100,
        env_pass: vec![String::from("PATH"), String::from("HOME")],
        args: Vec::new(),
        kind,
    }
}

/// A declared ACP launch.
fn acp() -> CompiledHarness {
    declared(CompiledHarnessKind::Acp {
        command: PathBuf::from("/usr/local/bin/claude-code-acp"),
        cwd: PathBuf::from("/srv/agents/workspace"),
        permission: HarnessPermission::Deny,
        exit_grace: Duration::from_secs(10),
    })
}

/// A declared Norn launch.
fn norn() -> CompiledHarness {
    declared(CompiledHarnessKind::Norn { binary: None })
}

/// The declared launch for whichever kind this build carries first, so a test about the
/// RESOLUTION rule runs under every feature combination.
#[cfg(any(feature = "norn", feature = "acp"))]
fn declared_for(kind: HarnessKind) -> CompiledHarness {
    match kind {
        #[cfg(feature = "norn")]
        HarnessKind::Norn => norn(),
        #[cfg(feature = "acp")]
        HarnessKind::Acp => acp(),
    }
}

/// The resolution rule, checked against what this build actually carries: a declared
/// kind the build links resolves; a build with no adapter resolves to nothing without
/// erroring, because `--check` must still report a surface it could not serve.
#[test]
fn a_declared_kind_resolves_against_the_adapters_this_build_links() -> TestResult {
    let compiled = catalogue::compiled();
    if compiled.is_empty() {
        let resolved = catalogue::resolve("norn", &document())?;
        assert!(
            resolved.is_none(),
            "a build with no adapter resolves to nothing rather than erroring, so the \
             action surface is still reported"
        );
        return Ok(());
    }
    for kind in compiled {
        let resolved = catalogue::resolve(kind.name(), &document())?;
        assert_eq!(
            resolved,
            Some(kind),
            "a declared kind this build links must resolve to it"
        );
    }
    Ok(())
}

#[test]
fn an_uncompiled_harness_name_is_refused_naming_what_is_compiled_in() -> TestResult {
    let compiled = catalogue::compiled();
    if compiled.is_empty() {
        // Nothing to be refused AGAINST: a build with no adapter resolves to nothing for
        // every name, which the test above pins.
        return Ok(());
    }
    let Err(error) = catalogue::resolve("gemini-cli", &document()) else {
        return Err("a kind this build does not carry must be refused".into());
    };
    let message = error.to_string();
    assert!(
        message.contains("gemini-cli"),
        "the refusal names what was declared: {message}"
    );
    assert!(
        message.contains("assistant.awl"),
        "the refusal names the document that declared it: {message}"
    );
    for kind in compiled {
        assert!(
            message.contains(kind.name()),
            "the refusal names `{}`, which this build does carry: {message}",
            kind.name()
        );
    }
    Ok(())
}

/// The declared `env_pass` list becomes the child's WHOLE environment, and a launch
/// reads no setting from a flag or an ambient variable.
#[cfg(any(feature = "norn", feature = "acp"))]
#[test]
fn the_declared_env_pass_list_is_the_childs_whole_environment() -> TestResult {
    let Some(kind) = catalogue::compiled().first().copied() else {
        return Ok(());
    };
    let mut launch = declared_for(kind);
    launch.env_pass = vec![String::from("PATH")];
    let Some(resolved) = super::settings::resolve(&launch, &document())? else {
        return Err("a build carrying an adapter must resolve the declared kind".into());
    };
    let environment = environment_of(&resolved.config);

    // The PRESENCE twin. This resolution reads THIS process's environment, so the
    // absence assertion below is only worth something if the process really carries
    // something that did not cross — otherwise it would pass just as well against an
    // implementation that resolved nothing at all. Cargo sets `CARGO_PKG_NAME` on every
    // test process, and it is not declared.
    assert!(
        std::env::var_os("CARGO_PKG_NAME").is_some(),
        "this test needs a variable the launching process really carries, or the \
         absence assertion below proves nothing"
    );
    assert!(
        environment.carries("PATH"),
        "the declared variable IS carried, so an absent reading cannot come from an \
         environment that resolved nothing: {:?}",
        environment.pairs()
    );
    assert_eq!(
        environment.declared(),
        ["PATH"],
        "the declaration is exactly what the document wrote"
    );
    assert!(
        !environment.carries("CARGO_PKG_NAME"),
        "a variable this process really carries, and the document does not name, does \
         not cross: {:?}",
        environment.pairs()
    );
    assert!(
        !environment.carries("CLAUDECODE"),
        "a variable the document does not name is absent whatever this process carries"
    );
    Ok(())
}

/// The child environment a configuration carries, whichever adapter it configures.
#[cfg(any(feature = "norn", feature = "acp"))]
fn environment_of(config: &super::settings::HarnessConfig) -> &aion_integrations::ChildEnvironment {
    match config {
        #[cfg(feature = "norn")]
        super::settings::HarnessConfig::Norn(norn) => &norn.environment,
        #[cfg(feature = "acp")]
        super::settings::HarnessConfig::Acp(acp) => &acp.environment,
    }
}

/// An `env_pass` entry that is a `KEY=VALUE` pair rather than a name is refused at the
/// launch boundary too, not only by the checker: a `CompiledHarness` can also reach this
/// code from a document compiled by an older binary.
#[cfg(any(feature = "norn", feature = "acp"))]
#[test]
fn an_env_pass_entry_carrying_a_value_is_refused_at_the_launch_boundary() -> TestResult {
    let Some(kind) = catalogue::compiled().first().copied() else {
        return Ok(());
    };
    let mut launch = declared_for(kind);
    launch.env_pass = vec![String::from("ANTHROPIC_API_KEY=sk-live")];
    let Err(error) = super::settings::resolve(&launch, &document()) else {
        return Err("a KEY=VALUE entry is not a pass-through name".into());
    };
    let message = format!("{error:#}");
    assert!(
        message.contains("ANTHROPIC_API_KEY=sk-live"),
        "the refusal quotes the entry: {message}"
    );
    assert!(
        message.contains("assistant.awl"),
        "the refusal names the document: {message}"
    );
    Ok(())
}

#[cfg(feature = "acp")]
#[test]
fn a_declared_acp_launch_reaches_the_adapter_verbatim() -> TestResult {
    let mut launch = acp();
    launch.args = vec![String::from("--acp")];
    launch.kind = CompiledHarnessKind::Acp {
        command: PathBuf::from("/opt/gemini/bin/gemini"),
        cwd: PathBuf::from("/srv/agents/gemini"),
        permission: HarnessPermission::AllowOnce,
        exit_grace: Duration::from_secs(0),
    };
    let Some(resolved) = super::settings::resolve(&launch, &document())? else {
        return Err("this build links the acp adapter".into());
    };
    assert_eq!(resolved.kind, HarnessKind::Acp);
    let super::settings::HarnessConfig::Acp(config) = resolved.config else {
        return Err("an acp declaration must produce an acp configuration".into());
    };
    assert_eq!(config.command, Path::new("/opt/gemini/bin/gemini"));
    assert_eq!(config.args, vec![String::from("--acp")]);
    assert_eq!(config.cwd, Path::new("/srv/agents/gemini"));
    assert_eq!(
        config.permission,
        aion_integration_acp::PermissionPolicy::AllowOnce
    );
    assert_eq!(
        config.exit_grace,
        Duration::ZERO,
        "zero is a legitimate grace and means close stdin, then kill at once"
    );
    Ok(())
}

#[cfg(feature = "acp")]
#[test]
fn each_permission_spelling_resolves_to_the_adapters_policy() -> TestResult {
    for (declared, expected) in [
        (
            HarnessPermission::Deny,
            aion_integration_acp::PermissionPolicy::Deny,
        ),
        (
            HarnessPermission::AllowOnce,
            aion_integration_acp::PermissionPolicy::AllowOnce,
        ),
    ] {
        let mut launch = acp();
        launch.kind = CompiledHarnessKind::Acp {
            command: PathBuf::from("/usr/local/bin/claude-code-acp"),
            cwd: PathBuf::from("/srv/agents/workspace"),
            permission: declared,
            exit_grace: Duration::from_secs(1),
        };
        let Some(resolved) = super::settings::resolve(&launch, &document())? else {
            return Err("this build links the acp adapter".into());
        };
        let super::settings::HarnessConfig::Acp(config) = resolved.config else {
            return Err("an acp declaration must produce an acp configuration".into());
        };
        assert_eq!(
            config.permission, expected,
            "the document's policy and the adapter's must be one decision, not two"
        );
    }
    Ok(())
}

#[cfg(feature = "norn")]
#[test]
fn the_norn_binary_is_optional_and_the_absence_takes_the_adapters_own_default() -> TestResult {
    let Some(resolved) = super::settings::resolve(&norn(), &document())? else {
        return Err("this build links the norn adapter".into());
    };
    let super::settings::HarnessConfig::Norn(config) = resolved.config else {
        return Err("a norn declaration must produce a norn configuration".into());
    };
    assert_eq!(
        config.binary, None,
        "an omitted `binary` takes the adapter's own documented `norn`-on-PATH default, \
         which is the library's value rather than one invented at this boundary"
    );

    let mut declared_binary = norn();
    declared_binary.kind = CompiledHarnessKind::Norn {
        binary: Some(PathBuf::from("/opt/norn/bin/norn")),
    };
    let Some(resolved) = super::settings::resolve(&declared_binary, &document())? else {
        return Err("this build links the norn adapter".into());
    };
    let super::settings::HarnessConfig::Norn(config) = resolved.config else {
        return Err("a norn declaration must produce a norn configuration".into());
    };
    assert_eq!(
        config.binary.as_deref(),
        Some(Path::new("/opt/norn/bin/norn"))
    );
    Ok(())
}

#[cfg(feature = "norn")]
#[test]
fn the_declared_argv_reaches_the_child_expanded_and_in_order() -> TestResult {
    use aion_core::{ActivityId, ContentType, Payload, RunId, WorkflowId};
    use aion_integrations::AgentRunSpec;

    let mut launch = norn();
    // Exactly what the document's `args` list carries, one entry per element.
    launch.args = [
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
    launch.env_pass = vec![String::from("PATH")];

    let Some(resolved) = super::settings::resolve(&launch, &document())? else {
        return Err("this build links the norn adapter".into());
    };
    let super::settings::HarnessConfig::Norn(config) = resolved.config else {
        return Err("a norn declaration must produce a norn configuration".into());
    };

    let harness = super::compose::build_norn(&config);
    let workflow_id = WorkflowId::new_v4();
    let spec = AgentRunSpec::new(
        workflow_id.clone(),
        RunId::new_v4(),
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
        "the child argv must be the document's arguments, in order, with the \
         run-identity placeholders expanded"
    );
    let Some(environment) = invocation.declared_environment else {
        return Err("the document's env_pass must reach the child as a declaration".into());
    };
    assert_eq!(
        environment.declared(),
        ["PATH"],
        "the declaration reaches the previewable invocation, so what this root composed \
         is what the spawn performs"
    );
    Ok(())
}

/// The name a harness is RESOLVED by and the name a document DECLARES must be the same
/// string.
///
/// The AWL side spells the two kinds in `aion_awl::HARNESS_KIND_*`; this side spells
/// them in [`HarnessKind::name`]. If they ever drifted, a document declaring a kind this
/// binary carries would be refused as one it does not, which is the exact failure the
/// resolution exists to prevent.
#[test]
fn the_document_vocabulary_and_the_catalogue_vocabulary_are_one_table() {
    let language = [aion_awl::HARNESS_KIND_ACP, aion_awl::HARNESS_KIND_NORN];
    for harness in catalogue::compiled() {
        assert!(
            language.contains(&harness.name()),
            "the catalogue carries `{}`, which no document can declare",
            harness.name()
        );
    }
}

#[test]
fn each_harness_advertises_its_adapters_published_capabilities() {
    for harness in catalogue::compiled() {
        let advertised = harness.registration_capabilities();
        let published = match harness.name() {
            #[cfg(feature = "norn")]
            "norn" => aion_integration_norn::registration_capabilities(),
            #[cfg(feature = "acp")]
            "acp" => aion_integration_acp::registration_capabilities(),
            other => unreachable!("unknown harness `{other}`"),
        };
        assert_eq!(
            advertised, published,
            "the root must publish the ADAPTER's set, never one of its own"
        );
    }
}

#[test]
fn a_build_with_no_harness_advertises_the_empty_set() {
    assert_eq!(
        super::registration_capabilities(None),
        aion_core::InterventionCapabilities::none(),
        "an empty advertisement is first-class: a build with no harness supports no \
         interventions and the console offers none"
    );
}

/// Every compiled-in harness composes into something the worker SDK can bind.
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
