//! Tests for the agent worker's derived surface and its configuration contract.
//!
//! Every fixture is written to a path carrying this process's id, so concurrent
//! tests in this binary never read each other's document.

use std::collections::BTreeSet;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{Value, json};

use super::args::{self, AgentArgs};
use super::surface;
use crate::harness::HarnessArgs;
use crate::settings::Environment;
use crate::worker_surface::{self, ServingSource};

type TestResult = Result<(), Box<dyn Error>>;

/// A document declaring one queue with one bodyless agent action. The narration
/// lines before the `workflow` header are required by the checker.
const DOCUMENT: &str = r"//! reviewer: the document an agent worker derives its advertised surface from.
workflow review_flow
  input brief: String

  outcome reviewed: type Review, route success

/// What the agent reported back.
type Review {
  verdict: String,
  detail: String,
}

/// The queue an agent worker serves.
worker reviewers
  action review(brief: String) -> Review

step do_review
  brief |> review |> route reviewed
";

/// Writes `source` to a uniquely named document so concurrent tests never read
/// each other's fixture.
fn write_document(label: &str, source: &str) -> Result<PathBuf, Box<dyn Error>> {
    let directory =
        std::env::temp_dir().join(format!("aion-agent-worker-{}-{label}", std::process::id()));
    std::fs::create_dir_all(&directory)?;
    let path = directory.join("document.awl");
    std::fs::write(&path, source)?;
    Ok(path)
}

/// Arguments with only the document set — the shape `--check` is invoked with.
fn args_for(document: &Path) -> AgentArgs {
    AgentArgs {
        awl: document.to_path_buf(),
        liminal_addresses: Vec::new(),
        task_queue: None,
        identity: None,
        concurrency: None,
        reconnect_initial_backoff: None,
        reconnect_max_backoff: None,
        reconnect_max_attempts: None,
        node: None,
        check: false,
        harness: HarnessArgs::default(),
    }
}

/// THE REGRESSION THAT MATTERS. A worker advertising an empty action surface is
/// refused by contract admission on every queue carrying a deployed contract — and
/// AWL emits a contract for every `worker` block, so that is every queue this verb
/// exists to serve. The surface must be non-empty and must carry the DOCUMENT's own
/// schemas, not a permissive stand-in.
#[test]
fn the_advertised_surface_is_the_documents_and_is_not_empty() -> TestResult {
    let document = write_document("advertises", DOCUMENT)?;
    let surface = surface::derive(&document, None)?;

    assert_eq!(surface.task_queue, "reviewers");
    assert_eq!(
        surface.descriptors.len(),
        1,
        "the queue's one bodyless action must be advertised, got {:?}",
        surface.descriptors
    );
    assert_eq!(surface.descriptors[0].name, "review");
    // The document's own types: `review` takes a named `brief` parameter and
    // returns a declared record, advertised in the `$ref` + `$defs` form the
    // server's subset check resolves.
    assert_eq!(
        surface.descriptors[0].input_schema["properties"]["brief"]["type"],
        json!("string")
    );
    assert_eq!(
        surface.descriptors[0].output_schema["$ref"],
        json!("#/$defs/Review")
    );
    assert_eq!(
        surface.descriptors[0].output_schema["$defs"]["Review"]["properties"]["verdict"]["type"],
        json!("string")
    );
    Ok(())
}

/// A document declaring exactly one `worker` block leaves nothing to choose, so the
/// queue is DERIVED — that is unambiguous derivation, not an assumed default.
#[test]
fn a_single_worker_block_derives_the_queue_without_a_flag() -> TestResult {
    let document = write_document("derived", DOCUMENT)?;
    assert_eq!(surface::derive(&document, None)?.task_queue, "reviewers");
    // An explicit selection of the same queue agrees with the derivation.
    assert_eq!(
        surface::derive(&document, Some("reviewers"))?.task_queue,
        "reviewers"
    );
    Ok(())
}

/// Two worker blocks make the queue genuinely ambiguous. Guessing one would serve
/// the wrong queue silently, so the verb refuses and names every choice.
#[test]
fn a_multi_worker_document_refuses_and_names_every_choice() -> TestResult {
    let document = write_document("ambiguous", &two_worker_document())?;
    let Err(error) = surface::derive(&document, None) else {
        return Err("a document declaring two queues must not pick one".into());
    };
    let error = error.to_string();
    assert!(
        error.contains("reviewers") && error.contains("summarisers"),
        "the refusal must name every choice, got: {error}"
    );
    assert!(
        error.contains("--task-queue"),
        "the refusal must name the flag that selects one, got: {error}"
    );

    // And the flag resolves it, in either direction.
    assert_eq!(
        surface::derive(&document, Some("summarisers"))?.task_queue,
        "summarisers"
    );
    assert_eq!(
        surface::derive(&document, Some("reviewers"))?.task_queue,
        "reviewers"
    );
    Ok(())
}

/// A selected queue the document does not declare is refused naming both the
/// selection and what the document actually declares.
#[test]
fn a_queue_the_document_does_not_declare_is_refused() -> TestResult {
    let document = write_document("wrongqueue", DOCUMENT)?;
    let Err(error) = surface::derive(&document, Some("nobody")) else {
        return Err("a queue absent from the document must be refused".into());
    };
    let error = error.to_string();
    assert!(
        error.contains("nobody") && error.contains("reviewers"),
        "the refusal must name the selection and the declaration, got: {error}"
    );
    Ok(())
}

/// An action whose body the document carries is executed by the SERVER from the
/// deployed contract. It is not this worker's to run, so it must not appear in the
/// advertisement either — a worker claiming it would shadow the declared body.
#[test]
fn a_body_carrying_action_is_excluded_from_the_surface() -> TestResult {
    let document = write_document("mixed", MIXED_BODY_DOCUMENT)?;
    let surface = surface::derive(&document, None)?;

    assert_eq!(
        surface.names(),
        BTreeSet::from([String::from("review")]),
        "only the bodyless action is the worker's to serve"
    );
    Ok(())
}

/// A queue whose EVERY action carries a declared body leaves an agent worker with
/// nothing to advertise, and an empty advertisement is refused by every
/// contract-bearing queue. Refuse here, where the reason is readable, rather than
/// remotely.
#[test]
fn a_queue_of_only_bodied_actions_is_refused_by_name() -> TestResult {
    let document = write_document("allbodied", ALL_BODIED_DOCUMENT)?;
    let Err(error) = surface::derive(&document, None) else {
        return Err("a queue with nothing for a worker to serve must be refused".into());
    };
    let error = error.to_string();
    assert!(
        error.contains("declared body") && error.contains("run"),
        "the refusal must name the rule and the action, got: {error}"
    );
    Ok(())
}

/// Serving is all-or-nothing across a queue: a worker admitted on some of a queue's
/// actions is then dispatched one it never declared. The agent worker derives its
/// served set from the document and runs it through this shared rule, so the rule is
/// pinned here on a deliberately narrowed set.
#[test]
fn a_partially_served_queue_is_refused_by_name() -> TestResult {
    let document = write_document("partial", &two_action_document())?;
    let contract = worker_surface::compile_contract(&document)?;
    let worker = worker_surface::select_worker(&document, &contract, None, "--task-queue")?;
    // The whole queue reconciles.
    let whole = worker_surface::serviceable_action_names(worker);
    assert_eq!(
        whole,
        BTreeSet::from([String::from("review"), String::from("summarise")])
    );
    assert_eq!(
        worker_surface::reconcile(&document, worker, &whole, AGENT_SOURCE)?.len(),
        2
    );

    // Dropping one is refused, by name, with the rule stated.
    let partial = BTreeSet::from([String::from("review")]);
    let Err(error) = worker_surface::reconcile(&document, worker, &partial, AGENT_SOURCE) else {
        return Err("a queue must be served whole or not at all".into());
    };
    let error = error.to_string();
    assert!(
        error.contains("summarise") && error.contains("whole queue or none"),
        "the refusal must name the unserved action and the rule, got: {error}"
    );
    Ok(())
}

/// The wording the agent worker's refusals use, mirroring the verb's own source.
const AGENT_SOURCE: ServingSource = ServingSource {
    subject: "the agent worker",
    serves: "serves",
    omits: "does not serve",
};

/// A document that does not compile has no surface to derive, and saying so beats
/// advertising nothing and being refused remotely.
#[test]
fn a_document_that_does_not_compile_is_refused_with_its_diagnostics() -> TestResult {
    let document = write_document("broken", "//! broken\nworkflow \n")?;
    let Err(error) = surface::derive(&document, None) else {
        return Err("an uncompilable document must be refused".into());
    };
    assert!(
        error.to_string().contains("does not compile"),
        "the refusal must say the document does not compile, got: {error}"
    );
    Ok(())
}

/// Every connection setting must be supplied. A worker started from a shell should
/// need ONE correction, not seven — so every missing setting is named at once, by
/// both the flag and the variable that could supply it.
///
/// Resolved with NO harness, which isolates the connection contract: a build compiling
/// no adapter in demands no harness configuration, and the harness settings have their
/// own coverage in [`crate::harness`]. That the two groups share ONE refusal is pinned
/// by `a_missing_harness_setting_joins_the_connection_report` below.
#[test]
fn missing_settings_are_reported_by_name_and_all_at_once() -> TestResult {
    let document = write_document("missing", DOCUMENT)?;
    let Err(error) = args::resolve(&args_for(&document), &Environment::default(), None) else {
        return Err("resolution with nothing supplied must be refused".into());
    };
    let report = error.to_string();
    for expected in [
        "--liminal-address or $AION_WORKER_LIMINAL_ADDRESSES",
        "--identity or $AION_WORKER_IDENTITY",
        "--concurrency or $AION_WORKER_CONCURRENCY",
        "--reconnect-initial-backoff or $AION_RECONNECT_INITIAL_BACKOFF_SECONDS",
        "--reconnect-max-backoff or $AION_RECONNECT_MAX_BACKOFF_SECONDS",
        "--reconnect-max-attempts or $AION_RECONNECT_MAX_ATTEMPTS",
    ] {
        assert!(
            report.contains(expected),
            "the report must name `{expected}`, got:\n{report}"
        );
    }
    Ok(())
}

/// The environment supplies a setting the flag omits, and the flag WINS when both
/// are present. Nothing is defaulted except `node`, which falls through to the
/// worker SDK's own documented hostname default.
#[test]
fn the_environment_supplies_settings_and_the_flag_wins() -> TestResult {
    let document = write_document("environment", DOCUMENT)?;
    let environment = Environment::from_pairs([
        (
            "AION_WORKER_LIMINAL_ADDRESSES",
            "127.0.0.1:50061, 127.0.0.1:50062",
        ),
        ("AION_WORKER_IDENTITY", "from-environment"),
        ("AION_WORKER_CONCURRENCY", "3"),
        ("AION_RECONNECT_INITIAL_BACKOFF_SECONDS", "0.25"),
        ("AION_RECONNECT_MAX_BACKOFF_SECONDS", "5"),
        ("AION_RECONNECT_MAX_ATTEMPTS", "9"),
        ("AION_TASK_QUEUE", "reviewers"),
    ]);

    let (resolved, harness) = args::resolve(&args_for(&document), &environment, None)?;
    assert_eq!(
        harness, None,
        "no harness resolved means no harness configuration"
    );
    assert_eq!(
        resolved.addresses,
        vec![
            String::from("127.0.0.1:50061"),
            String::from("127.0.0.1:50062")
        ],
        "a comma-separated list keeps its order and drops blank entries"
    );
    assert_eq!(resolved.identity, "from-environment");
    assert_eq!(resolved.concurrency, 3);
    assert_eq!(resolved.initial_backoff, Duration::from_millis(250));
    assert_eq!(resolved.max_backoff, Duration::from_secs(5));
    assert_eq!(resolved.max_attempts, 9);
    assert_eq!(resolved.node, None, "node is the SDK's default, not ours");

    let mut arguments = args_for(&document);
    arguments.identity = Some(String::from("from-flag"));
    arguments.liminal_addresses = vec![String::from("127.0.0.1:50099")];
    arguments.task_queue = Some(String::from("from-flag-queue"));
    let (resolved, _) = args::resolve(&arguments, &environment, None)?;
    assert_eq!(resolved.identity, "from-flag");
    assert_eq!(resolved.addresses, vec![String::from("127.0.0.1:50099")]);
    assert_eq!(
        arguments.requested_queue(&environment).as_deref(),
        Some("from-flag-queue")
    );
    Ok(())
}

/// An exported-but-blank variable is a deployment mistake, not a setting: it must be
/// reported missing rather than silently becoming an empty identity or no address.
#[test]
fn a_blank_environment_value_counts_as_missing() -> TestResult {
    let document = write_document("blank", DOCUMENT)?;
    let environment = Environment::from_pairs([
        ("AION_WORKER_LIMINAL_ADDRESSES", "  ,  "),
        ("AION_WORKER_IDENTITY", "   "),
    ]);
    let Err(error) = args::resolve(&args_for(&document), &environment, None) else {
        return Err("blank values must be refused as missing".into());
    };
    let report = error.to_string();
    assert!(report.contains("--liminal-address"));
    assert!(report.contains("--identity"));
    Ok(())
}

/// An unusable value is refused naming the setting and the value, never coerced.
#[test]
fn an_unusable_setting_value_is_refused_by_name() -> TestResult {
    let document = write_document("unusable", DOCUMENT)?;
    let supplied = |concurrency: &str, backoff: &str| {
        Environment::from_pairs([
            ("AION_WORKER_LIMINAL_ADDRESSES", "127.0.0.1:50061"),
            ("AION_WORKER_IDENTITY", "reviewer-worker"),
            ("AION_WORKER_CONCURRENCY", concurrency),
            ("AION_RECONNECT_INITIAL_BACKOFF_SECONDS", backoff),
            ("AION_RECONNECT_MAX_BACKOFF_SECONDS", "5"),
            ("AION_RECONNECT_MAX_ATTEMPTS", "9"),
        ])
    };
    let arguments = args_for(&document);

    let Err(error) = args::resolve(&arguments, &supplied("0", "1"), None) else {
        return Err("a zero concurrency must be refused".into());
    };
    assert!(
        error.to_string().contains("--concurrency"),
        "the refusal must name the setting, got: {error}"
    );

    let Err(error) = args::resolve(&arguments, &supplied("4", "nonsense"), None) else {
        return Err("a non-numeric backoff must be refused".into());
    };
    let message = error.to_string();
    assert!(
        message.contains("--reconnect-initial-backoff") && message.contains("nonsense"),
        "the refusal must name the setting and the value, got: {message}"
    );
    Ok(())
}

/// `--check` prints the exact surface — every action, both schemas — plus the harness
/// that would drive it, and connects to nothing. It is written to an injected writer
/// here, which is the same call the verb makes against stdout.
///
/// Driven over EVERY harness this build carries, plus the no-harness report, because the
/// report is what an operator reads instead of a remote refusal: the harness name and
/// the capability set beside it are what the server routes operator commands on.
#[test]
fn check_prints_the_surface_without_connecting() -> TestResult {
    let document = write_document("check", DOCUMENT)?;
    let surface = surface::derive(&document, None)?;
    let mut rendered = Vec::new();
    surface::print(&mut rendered, &surface, None)?;

    let report: Value = serde_json::from_slice(&rendered)?;
    assert_eq!(report["task_queue"], json!("reviewers"));
    let Some(actions) = report["actions"].as_array() else {
        return Err("the report must carry an actions array".into());
    };
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0]["name"], json!("review"));
    assert_eq!(
        actions[0]["input_schema"]["properties"]["brief"]["type"],
        json!("string")
    );
    assert_eq!(
        actions[0]["output_schema"]["$defs"]["Review"]["properties"]["detail"]["type"],
        json!("string")
    );
    // With no harness resolved the report says so rather than naming one: that is the
    // `--no-default-features` build, and it must be visible BEFORE a dial.
    assert_eq!(report["harness"], Value::Null);
    assert_eq!(
        report["intervention_capabilities"]["supported"],
        json!([]),
        "a build with no harness advertises no controls"
    );

    // And once a harness is resolved, the report names it and carries the ADAPTER's own
    // published registration set — not an empty set, which would leave the ops console
    // offering no controls at all for a live agent.
    for harness in crate::harness::catalogue::compiled() {
        let mut rendered = Vec::new();
        surface::print(&mut rendered, &surface, Some(harness))?;
        let report: Value = serde_json::from_slice(&rendered)?;
        assert_eq!(report["harness"], json!(harness.name()));
        assert_eq!(
            report["intervention_capabilities"],
            serde_json::to_value(crate::harness::registration_capabilities(Some(harness)))?
        );
        let Some(supported) = report["intervention_capabilities"]["supported"].as_array() else {
            return Err("the capability set must carry a supported array".into());
        };
        assert!(
            !supported.is_empty(),
            "`{}` must advertise the controls its adapter can honour",
            harness.name()
        );
    }
    Ok(())
}

/// The ACP harness's `--check` report names `cancel` and nothing else — the one
/// intervention primitive ACP v1 can actually deliver. This is the operator-visible end
/// of the adapter's published ceiling.
#[cfg(feature = "acp")]
#[test]
fn the_acp_check_report_advertises_exactly_cancel() -> TestResult {
    let document = write_document("acpcheck", DOCUMENT)?;
    let surface = surface::derive(&document, None)?;
    let mut rendered = Vec::new();
    surface::print(
        &mut rendered,
        &surface,
        Some(crate::harness::HarnessKind::Acp),
    )?;
    let report: Value = serde_json::from_slice(&rendered)?;
    assert_eq!(report["harness"], json!("acp"));
    assert_eq!(
        report["intervention_capabilities"]["supported"],
        json!([{ "primitive": "Cancel" }]),
        "ACP can deliver exactly one primitive, so exactly one is advertised"
    );
    Ok(())
}

/// A missing HARNESS setting appears in the SAME refusal as a missing connection
/// setting. One shell invocation with two mistakes is one mistake to correct, and this
/// is the acceptance the whole shared-resolution contract exists for.
#[cfg(feature = "acp")]
#[test]
fn a_missing_harness_setting_joins_the_connection_report() -> TestResult {
    let document = write_document("bothmissing", DOCUMENT)?;
    let mut arguments = args_for(&document);
    // Everything the ACP harness needs EXCEPT the permission policy, which has no
    // default because it is a security decision.
    arguments.harness.acp_command = Some(String::from("opencode"));
    arguments.harness.acp_cwd = Some(String::from("/srv/workspace"));
    arguments.harness.acp_exit_grace = Some(String::from("2"));
    // ... and a connection setting supplied, so the report has to carry both halves.
    arguments.identity = Some(String::from("reviewer-worker"));

    let Err(error) = args::resolve(
        &arguments,
        &Environment::default(),
        Some(crate::harness::HarnessKind::Acp),
    ) else {
        return Err("an ACP worker with no permission policy must be refused".into());
    };
    let report = error.to_string();
    assert!(
        report.contains("--acp-permission or $AION_ACP_PERMISSION"),
        "the report must name the missing security decision, got:\n{report}"
    );
    assert!(
        report.contains("--liminal-address or $AION_WORKER_LIMINAL_ADDRESSES")
            && report.contains("--concurrency or $AION_WORKER_CONCURRENCY"),
        "the report must name the missing connection settings in the SAME message, got:\n{report}"
    );
    assert!(
        !report.contains("--identity"),
        "a setting that WAS supplied must not be reported missing, got:\n{report}"
    );
    Ok(())
}

/// The composed harness's ADVERTISED set — the union `AgentHarnessConfig` computes
/// from its activity types and its descriptor names — must be exactly the document's
/// serviceable actions. A divergence would register a worker for a type it will not
/// route, or route one it never announced.
#[cfg(all(any(feature = "norn", feature = "acp"), feature = "liminal-transport"))]
#[test]
fn the_composed_config_advertises_exactly_the_documents_surface() -> TestResult {
    let document = write_document("composed", &two_action_document())?;
    let surface = surface::derive(&document, None)?;
    let advertised = surface.names();
    let Some(harness) = crate::harness::catalogue::compiled().first().copied() else {
        return Err("a build with liminal transport and a harness feature must carry one".into());
    };
    let config = harness.test_config();
    let agent = aion_worker::AgentHarnessConfig::new(
        crate::harness::compose::build(&config),
        advertised.iter().cloned(),
        crate::harness::registration_capabilities(Some(harness)),
    )
    .with_activity_descriptors(surface.descriptors.clone());

    assert_eq!(agent.agent_activity_types(), &advertised);
    assert_eq!(
        agent
            .activity_descriptors()
            .iter()
            .map(|descriptor| descriptor.name.clone())
            .collect::<BTreeSet<_>>(),
        advertised,
        "a descriptor must exist for every advertised action"
    );
    assert!(
        !agent.activity_descriptors().is_empty(),
        "an empty descriptor set is refused by every contract-bearing queue"
    );
    Ok(())
}

/// A document declaring two queues, so queue selection is genuinely ambiguous.
fn two_worker_document() -> String {
    DOCUMENT.replace(
        "step do_review",
        "/// A second queue, so the choice is the operator's.\nworker summarisers\n  \
         action summarise(brief: String) -> Review\n\nstep do_review",
    )
}

/// A document whose one queue declares two bodyless actions.
fn two_action_document() -> String {
    DOCUMENT.replace(
        "  action review(brief: String) -> Review",
        "  action review(brief: String) -> Review\n  action summarise(brief: String) -> Review",
    )
}

/// One queue carrying one bodyless action and one the document implements itself. A
/// `run` body's outcome members are fixed by the language to `exit_code`, `stdout`,
/// and `stderr`, so the bodied action returns that shape.
const MIXED_BODY_DOCUMENT: &str = r#"//! mixed: one action for a worker, one the server runs from the contract.
workflow mixed_flow
  input brief: String

  outcome reviewed: type Review, route success

/// What the agent reported back.
type Review {
  verdict: String,
  detail: String,
}

/// What a declared command reported.
type RunOutcome { exit_code: Int, stdout: String, stderr: String }

/// One action a worker serves, one the server runs.
worker reviewers
  action review(brief: String) -> Review
  action stamp(brief: String) -> RunOutcome
    run "printf %s $brief"

step do_review
  brief |> review |> route reviewed
"#;

/// A queue whose every action carries a declared body, leaving a worker nothing.
const ALL_BODIED_DOCUMENT: &str = r#"//! bodied: every action on the queue is the server's to run.
workflow bodied_flow
  input brief: String

  outcome stamped: type RunOutcome, route success

/// What a declared command reported.
type RunOutcome { exit_code: Int, stdout: String, stderr: String }

/// The server runs every action here; no worker serves any of them.
worker reviewers
  action stamp(brief: String) -> RunOutcome
    run "printf %s $brief"

step do_stamp
  brief |> stamp |> route stamped
"#;
