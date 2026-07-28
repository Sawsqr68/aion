//! End-to-end proof that an AWL `retry N` declaration buys N FURTHER attempts.
//!
//! The ratified meaning (`docs/design/aion-authoring/awl/`
//! `AWL-COMPLETION-COLD-READ.md` §2.3, and the `RetryContract` doc comments in
//! `aion-package`) is that `retry N` declares N further attempts AFTER the
//! first — N+1 total executions. The engine's dispatch loop is convention-free
//! (`attempt < max_attempts`), so the meaning is owned entirely by the two
//! places a declared count becomes the SDK's total-attempt budget: the
//! reference emitter and the direct MIR lowering.
//!
//! This gate runs ONE `retry 3` workflow through BOTH backends against a real
//! embedded engine and an always-failing dispatcher, and counts deliveries.
//! Four executions — the first plus three further — is the contract; anything
//! else is the engine disagreeing with its own documentation, or the two
//! backends disagreeing with each other.
//!
//! Both packages share the SDK closure and differ only in the entry module's
//! bytes (the BC-4 splice discipline), so a divergence can only be a backend
//! difference.

#[path = "test_support/gleam.rs"]
mod gleam_test_support;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use aion::EngineBuilder;
use aion::activity::bridge::{ActivityDispatch, ActivityDispatcher};
use aion_awl::mir::{lower, select};
use aion_awl::{emit_artifact, parse, schema_for_outcomes, schema_for_workflow};
use aion_core::Payload;
use aion_package::{
    BeamModule, BeamSet, ExtractionLimits, Package, PackageBuilder, PackageOptions, package_project,
};
use aion_store::{EventStore, InMemoryStore};
use serde_json::json;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// The declared budget under test: three FURTHER attempts.
const DECLARED_FURTHER_ATTEMPTS: u32 = 3;

/// Total executions the declaration must buy: the first plus the further ones.
const EXPECTED_EXECUTIONS: u32 = DECLARED_FURTHER_ATTEMPTS + 1;

/// One action, always failing retryably, under `retry 3 every 1s`.
const SOURCE: &str = r"//! Retry-counting e2e: `retry 3` buys three FURTHER attempts.
workflow awl_retry_counting
  input name: String
  outcome greeted: type Greeting, route success

type Greeting { text: String }

worker flaky
  action flaky_call(name: String) -> Greeting
    retry 3 every 1s

step call
  flaky_call(name: name) -> result
  route greeted(text: result.text)
";

/// A dispatcher whose every delivery is a retryable-class failure, counting
/// the deliveries it served.
struct AlwaysRetryableDispatcher {
    deliveries: AtomicU32,
}

impl AlwaysRetryableDispatcher {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            deliveries: AtomicU32::new(0),
        })
    }

    fn deliveries(&self) -> u32 {
        self.deliveries.load(Ordering::SeqCst)
    }
}

impl ActivityDispatcher for AlwaysRetryableDispatcher {
    fn dispatch(&self, request: ActivityDispatch) -> Result<String, String> {
        if request.name != "flaky_call" {
            return Err(format!("terminal:unknown activity {}", request.name));
        }
        self.deliveries.fetch_add(1, Ordering::SeqCst);
        Err(format!(
            "retryable:upstream refused (delivery attempt {})",
            request.attempt
        ))
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The reference package: AWL → emitted Gleam → `gleam build` → packaged.
fn reference_package() -> Result<(Package, String), Box<dyn std::error::Error>> {
    let document = parse(SOURCE)?;
    let diagnostics = aion_awl::check(&document);
    if !diagnostics.is_empty() {
        return Err(format!("the retry-counting source did not check: {diagnostics:?}").into());
    }
    let artifact = emit_artifact(&document)?;
    let entry = artifact.entry_module.clone();

    let project = repo_root().join("target/awl-test-scratch/retry-counting");
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
            "name = \"awl_retry_counting\"\nversion = \"0.1.0\"\ntarget = \"erlang\"\n\n\
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
             output_schema = \"schemas/output.json\"\nactivities = [\"flaky_call\"]\n"
        ),
    )?;
    run_gleam_build(&project)?;

    let report = package_project(&project, &PackageOptions::default())?;
    let packaged = report
        .packages
        .into_iter()
        .find(|packaged| packaged.workflow_type == entry)
        .ok_or_else(|| format!("no package produced for workflow type `{entry}`"))?;
    Ok((packaged.package, entry))
}

/// The direct package: the MIR backend's `.beam` bytes spliced in as the entry
/// module over the reference package's SDK closure.
fn direct_package(reference: &Package, entry: &str) -> Result<Package, Box<dyn std::error::Error>> {
    let document = parse(SOURCE)?;
    let module = lower(&document, None)?;
    let direct_bytes = select(&module)?;

    let reference_entry = reference
        .beams()
        .get(entry)
        .ok_or_else(|| format!("reference package has no entry beam `{entry}`"))?
        .to_vec();
    if reference_entry == direct_bytes {
        return Err(
            "the direct production is byte-identical to the reference entry — \
                    the splice would be a no-op"
                .into(),
        );
    }
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
    let spliced = Package::load_from_bytes(archive, ExtractionLimits::unbounded())?;
    let spliced_entry = spliced
        .beams()
        .get(entry)
        .ok_or_else(|| format!("spliced package lost its entry beam `{entry}`"))?;
    if spliced_entry != direct_bytes.as_slice() {
        return Err("the spliced entry beam is not the direct production".into());
    }
    Ok(spliced)
}

fn run_gleam_build(project: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("gleam")
        .arg("build")
        .current_dir(project)
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "the retry-counting reference project did not build\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(())
}

/// Runs `package` to its terminal against an always-retryable dispatcher and
/// returns the number of deliveries the declaration bought.
async fn executions(
    package: Package,
    workflow_type: &str,
) -> Result<u32, Box<dyn std::error::Error>> {
    let dispatcher = AlwaysRetryableDispatcher::new();
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    let engine = EngineBuilder::new()
        .in_process_activity_serving()
        .store_arc(Arc::clone(&store))
        .in_memory_visibility()
        .scheduler_threads(1)
        .activity_dispatcher(Arc::clone(&dispatcher) as Arc<dyn ActivityDispatcher>)
        .load_workflows(package)
        .build()
        .await?;
    let handle = engine
        .start_workflow(
            workflow_type,
            Payload::from_json(&json!({ "name": "Ada" }))?,
            HashMap::new(),
            String::from("default"),
        )
        .await?;
    let result = engine.result(handle.workflow_id(), handle.run_id()).await?;
    if let Ok(payload) = result {
        engine.shutdown()?;
        return Err(format!("an always-failing action must fail the run: {payload:?}").into());
    }
    engine.shutdown()?;
    Ok(dispatcher.deliveries())
}

/// The reference (emitted-Gleam) backend honours `retry 3` as three FURTHER
/// attempts, and the direct (MIR) backend agrees exactly.
#[tokio::test(flavor = "multi_thread")]
async fn retry_three_runs_the_action_four_times_on_both_backends() -> TestResult {
    if crate::gleam_test_support::skip_if_unavailable() {
        return Ok(());
    }
    let (reference, entry) = reference_package()?;
    let direct = direct_package(&reference, &entry)?;

    let reference_executions = executions(reference, &entry).await?;
    assert_eq!(
        reference_executions, EXPECTED_EXECUTIONS,
        "`retry {DECLARED_FURTHER_ATTEMPTS}` must buy {DECLARED_FURTHER_ATTEMPTS} FURTHER \
         attempts after the first — {EXPECTED_EXECUTIONS} executions on the reference backend"
    );

    let direct_executions = executions(direct, &entry).await?;
    assert_eq!(
        direct_executions, EXPECTED_EXECUTIONS,
        "`retry {DECLARED_FURTHER_ATTEMPTS}` must buy {EXPECTED_EXECUTIONS} executions on the \
         direct backend too"
    );
    assert_eq!(
        reference_executions, direct_executions,
        "the two lowerings must agree on the attempt budget"
    );
    Ok(())
}
