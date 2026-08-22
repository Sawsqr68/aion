//! End-to-end proof of authored policy-refusal fallback through both AWL backends.

#[path = "test_support/gleam.rs"]
mod gleam_test_support;

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use aion::EngineBuilder;
use aion::activity::bridge::{ActivityDispatch, ActivityDispatcher};
use aion_awl::mir::{lower, select};
use aion_awl::{emit_artifact, parse, schema_for_outcomes, schema_for_workflow};
use aion_core::{ActivityErrorKind, Event, Payload, WorkflowStatus};
use aion_package::{
    BeamModule, BeamSet, ExtractionLimits, Package, PackageBuilder, PackageOptions, package_project,
};
use aion_store::{EventStore, InMemoryStore};
use serde_json::json;

type TestResult = Result<(), Box<dyn std::error::Error>>;

const CHAIN_SOURCE: &str = r"//! Route a provider refusal to the declared fallback queue.
workflow awl_fallback_chain
  input name: String
  outcome greeted: type Greeting, route success

type Greeting { text: String }

worker agents
  action greet(name: String) -> Greeting
    fallback claude

step call
  greet(name: name) -> result
  route greeted(text: result.text)
";

const NO_CHAIN_SOURCE: &str = r"//! Preserve configured retry when no fallback chain is declared.
workflow awl_fallback_no_chain
  input name: String
  outcome greeted: type Greeting, route success

type Greeting { text: String }

worker agents
  action greet(name: String) -> Greeting
    retry 1 every 1s

step call
  greet(name: name) -> result
  route greeted(text: result.text)
";

#[derive(Clone, Debug, PartialEq, Eq)]
struct HistorySummary {
    refused: usize,
    hops: Vec<(String, String, u32)>,
    starts: Vec<u32>,
    completed: bool,
    workflow_status: WorkflowStatus,
}

struct RefusingDispatcher {
    succeed_on: Option<String>,
}

impl RefusingDispatcher {
    fn chain() -> Arc<Self> {
        Arc::new(Self {
            succeed_on: Some("claude".to_owned()),
        })
    }

    fn always() -> Arc<Self> {
        Arc::new(Self { succeed_on: None })
    }
}

impl ActivityDispatcher for RefusingDispatcher {
    fn dispatch(&self, request: ActivityDispatch) -> Result<String, String> {
        if request.name != "greet" {
            return Err(format!("terminal:unknown activity {}", request.name));
        }
        if self.succeed_on.as_deref() == Some(request.task_queue.as_str()) {
            let input: serde_json::Value = serde_json::from_str(&request.input)
                .map_err(|error| format!("terminal:bad input: {error}"))?;
            return Ok(json!({ "text": format!("hello {}", input["name"].as_str().unwrap_or("stranger")) }).to_string());
        }
        Err(format!(
            "policy_refused:provider refused queue {}",
            request.task_queue
        ))
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn reference_package(
    source: &str,
    scratch_name: &str,
) -> Result<(Package, String), Box<dyn std::error::Error>> {
    let document = parse(source)?;
    let diagnostics = aion_awl::check(&document);
    if !diagnostics.is_empty() {
        return Err(format!("fallback source did not check: {diagnostics:?}").into());
    }
    let artifact = emit_artifact(&document)?;
    let entry = artifact.entry_module.clone();
    let project = repo_root()
        .join("target/awl-test-scratch")
        .join(scratch_name);
    let src = project.join("src");
    let schemas = project.join("schemas");
    let _ = fs::remove_dir_all(&src);
    let _ = fs::remove_dir_all(&schemas);
    fs::create_dir_all(&src)?;
    fs::create_dir_all(&schemas)?;
    fs::write(src.join(format!("{entry}.gleam")), &artifact.source)?;
    fs::write(
        src.join(format!("{entry}.awl.json")),
        serde_json::to_vec_pretty(&artifact.project_metadata())?,
    )?;
    fs::write(
        schemas.join("input.json"),
        serde_json::to_vec(&schema_for_workflow(&document)?)?,
    )?;
    fs::write(
        schemas.join("output.json"),
        serde_json::to_vec(&schema_for_outcomes(&document)?)?,
    )?;
    fs::write(
        project.join("gleam.toml"),
        format!(
            "name = \"{scratch_name}\"\nversion = \"0.1.0\"\ntarget = \"erlang\"\n\n\
             [dependencies]\naion_flow = {{ path = \"{}\" }}\n\
             gleam_stdlib = \">= 0.34.0 and < 2.0.0\"\n\
             gleam_json = \">= 2.0.0 and < 4.0.0\"\n",
            repo_root().join("gleam/aion_flow").display()
        ),
    )?;
    fs::write(
        project.join("workflow.toml"),
        format!(
            "[[workflow]]\nentry_module = \"{entry}\"\nentry_function = \"run\"\n\
             timeout_seconds = 60\ninput_schema = \"schemas/input.json\"\n\
             output_schema = \"schemas/output.json\"\nactivities = [\"greet\"]\n"
        ),
    )?;
    gleam_test_support::compile_whole_project(&project)?;
    let report = package_project(&project, &PackageOptions::default())?;
    let packaged = report
        .packages
        .into_iter()
        .find(|packaged| packaged.workflow_type == entry)
        .ok_or_else(|| format!("no package produced for `{entry}`"))?;
    Ok((packaged.package, entry))
}

fn direct_package(
    source: &str,
    reference: &Package,
    entry: &str,
) -> Result<Package, Box<dyn std::error::Error>> {
    let document = parse(source)?;
    let direct_bytes = select(&lower(&document, None)?)?;
    let mut modules = Vec::new();
    for beam in reference.beams().iter() {
        if beam.name() == entry {
            modules.push(BeamModule::new(entry, direct_bytes.clone()));
        } else {
            modules.push(BeamModule::new(beam.name(), beam.bytes()));
        }
    }
    let archive = PackageBuilder::new(reference.manifest().clone(), BeamSet::new(modules)?)
        .write_to_bytes()?;
    Ok(Package::load_from_bytes(
        archive,
        ExtractionLimits::unbounded(),
    )?)
}

async fn run_chain(
    package: Package,
    workflow_type: &str,
) -> Result<HistorySummary, Box<dyn std::error::Error>> {
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    let dispatcher = RefusingDispatcher::chain();
    let engine = EngineBuilder::new()
        .in_process_activity_serving()
        .store_arc(Arc::clone(&store))
        .in_memory_visibility()
        .scheduler_threads(1)
        .activity_dispatcher(dispatcher as Arc<dyn ActivityDispatcher>)
        .load_workflows(package)
        .build()
        .await?;
    let handle = engine
        .start_workflow(
            workflow_type,
            Payload::from_json(&json!({ "name": "Ada" }))?,
            HashMap::new(),
            "default".to_owned(),
        )
        .await?;
    let payload = engine
        .result(handle.workflow_id(), handle.run_id())
        .await?
        .map_err(|error| format!("fallback workflow failed: {error:?}"))?;
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(payload.bytes())?,
        json!("hello Ada")
    );
    let history = store.read_history(handle.workflow_id()).await?;
    let summary = summarize(&history);
    engine.shutdown()?;
    Ok(summary)
}

fn summarize(history: &[Event]) -> HistorySummary {
    HistorySummary {
        refused: history
            .iter()
            .filter(|event| matches!(event, Event::ActivityFailed { error, .. } if error.kind == ActivityErrorKind::PolicyRefused))
            .count(),
        hops: history
            .iter()
            .filter_map(|event| match event {
                Event::ActivityFallbackRouted {
                    from_task_queue,
                    to_task_queue,
                    fallback_index,
                    ..
                } => Some((
                    from_task_queue.clone(),
                    to_task_queue.clone(),
                    *fallback_index,
                )),
                _ => None,
            })
            .collect(),
        starts: history
            .iter()
            .filter_map(|event| match event {
                Event::ActivityStarted { attempt, .. } => Some(*attempt),
                _ => None,
            })
            .collect(),
        completed: history
            .iter()
            .any(|event| matches!(event, Event::ActivityCompleted { .. })),
        workflow_status: aion_core::status_from_events(history),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn declared_chain_is_auditable_and_both_lowerings_agree() -> TestResult {
    if gleam_test_support::skip_if_unavailable() {
        return Ok(());
    }
    let (reference, entry) = reference_package(CHAIN_SOURCE, "fallback-chain")?;
    let direct = direct_package(CHAIN_SOURCE, &reference, &entry)?;
    let reference_summary = run_chain(reference, &entry).await?;
    let direct_summary = run_chain(direct, &entry).await?;

    assert_eq!(reference_summary, direct_summary);
    assert_eq!(reference_summary.refused, 1);
    assert_eq!(
        reference_summary.hops,
        vec![("default".to_owned(), "claude".to_owned(), 0)]
    );
    assert_eq!(reference_summary.starts, vec![1, 2]);
    assert!(reference_summary.completed);
    assert_eq!(reference_summary.workflow_status, WorkflowStatus::Completed);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn no_chain_retries_without_a_hop_and_finishes_policy_refused() -> TestResult {
    if gleam_test_support::skip_if_unavailable() {
        return Ok(());
    }
    let (package, entry) = reference_package(NO_CHAIN_SOURCE, "fallback-no-chain")?;
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    let dispatcher = RefusingDispatcher::always();
    let engine = EngineBuilder::new()
        .in_process_activity_serving()
        .store_arc(Arc::clone(&store))
        .in_memory_visibility()
        .scheduler_threads(1)
        .activity_dispatcher(dispatcher as Arc<dyn ActivityDispatcher>)
        .load_workflows(package)
        .build()
        .await?;
    let handle = engine
        .start_workflow(
            &entry,
            Payload::from_json(&json!({ "name": "Ada" }))?,
            HashMap::new(),
            "default".to_owned(),
        )
        .await?;
    let result = engine.result(handle.workflow_id(), handle.run_id()).await?;
    assert!(result.is_err());
    let history = store.read_history(handle.workflow_id()).await?;
    assert!(
        !history
            .iter()
            .any(|event| matches!(event, Event::ActivityFallbackRouted { .. }))
    );
    let refused = history
        .iter()
        .filter(|event| matches!(event, Event::ActivityFailed { error, .. } if error.kind == ActivityErrorKind::PolicyRefused))
        .count();
    assert!(refused >= 1);
    let terminal = history.iter().rev().find_map(|event| match event {
        Event::ActivityFailed { error, .. } if error.kind == ActivityErrorKind::Terminal => {
            Some(error.message.as_str())
        }
        _ => None,
    });
    assert!(
        matches!(terminal, Some(reason) if reason.starts_with("policy_refused:") && !reason.starts_with("exhausted:"))
    );
    assert!(
        history
            .iter()
            .filter(|event| matches!(event, Event::ActivityStarted { .. }))
            .count()
            >= 2
    );
    engine.shutdown()?;
    Ok(())
}
