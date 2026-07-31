//! Compile gate for the worker crate `aion awl scaffold` generates.
//!
//! A skeleton that does not compile is worse than no skeleton: the author
//! spends their first hour fixing the generator's output instead of writing
//! the work. The emitter is otherwise exercised only by string-shape tests, so
//! a drift in the worker SDK's public surface — a renamed `WorkerConfig`
//! builder method, a changed `register_activity_with_descriptor` bound, a
//! moved `HandlerFuture` — would ship a generated crate that cannot build,
//! with green CI.
//!
//! This gate closes that end to end. It stages a REAL `.awl` document with the
//! shape that matters (two pinned nodes, an unpinned action every connection
//! owes, and a `run`-bodied action the server executes itself), generates the
//! crate against a `path` dependency on the in-tree SDK, and runs a nested
//! `cargo test` — not `cargo build`, because the generated tests compile the
//! embedded document through the real `aion_awl::compile` and assert that
//! every action the connection table serves is declared, reachable on its
//! connection's node, and handled. That is the whole contract the scaffold
//! exists to keep, proved against the actual compiler rather than against a
//! string.
//!
//! `cargo fmt --check` and `cargo clippy -- -D warnings` run first over the
//! same tree, because the generated crate carries the aion lint table at
//! `deny` and must satisfy it AS GENERATED: a pedantic lint or a formatting
//! diff in emitted code is a generator defect, never something the author
//! should have to fix in a file they were told not to edit.
//!
//! The nested build pulls the SDK's full dependency tree (tokio, tonic, the
//! AWL compiler) and is slow on a cold cache — that is the point, and it is
//! the same bargain `rust_worker_build.rs` already makes. It runs in an
//! isolated `CARGO_TARGET_DIR` so it never touches the workspace target
//! directory, and every temp directory drops with the test.

use std::path::{Path, PathBuf};
use std::process::Command;

use aion_package::{
    ActionBodyContract, ActionContract, AionDependency, AwlWorkerScaffold, DocumentRoot,
    WorkerContract, scaffold_awl_worker,
};
use serde_json::json;

type TestError = Box<dyn std::error::Error>;

/// The staged document. Its `worker fleet` block is the shape a generated
/// worker must get right: `read_north`/`calibrate` pinned to `north`,
/// `read_south` pinned to `south`, `audit` unpinned (so EVERY connection owes
/// it), and `stamp` carrying a `run` body the server executes with no worker
/// at all.
const DOCUMENT: &str = r#"//! A fleet probe across two machines: node-pinned readings, one unpinned
//! audit every connection owes, and one declarative `run` body the server
//! executes with no worker.
workflow fleet_probe
  input target: String
  outcome reported: type Report, route success

type Reading { host: String, celsius: Int }
type Stamp { exit_code: Int, stdout: String, stderr: String }
type Report { summary: String }

worker fleet
  action read_north(host: String) -> Reading
    node north
  action calibrate(host: String) -> Reading
    node north
  action read_south(host: String) -> Reading
    node south
  action audit(summary: String) -> Report
  action stamp(host: String) -> Stamp
    run "echo $host"

step probe_fleet
  target
    |> read_north
    |> .host
    |> calibrate
    |> .host
    |> read_south
    |> .host
    |> stamp
    |> .stdout
    |> audit
    |> route reported
"#;

/// The compiled `worker fleet` contract, as `aion_awl::compile` derives it.
///
/// The schemas here are only what the STUB DOCUMENTATION reproduces; every
/// schema the generated worker advertises is taken from the document at
/// startup, which is the property the nested `cargo test` proves. The names,
/// node pins, and bodies, though, are load-bearing — the generated tests fail
/// if any of them disagrees with the document.
fn fleet_contract() -> WorkerContract {
    let reading = json!({
        "type": "object",
        "properties": {"host": {"type": "string"}, "celsius": {"type": "integer"}},
        "required": ["celsius", "host"]
    });
    let host_input = json!({
        "type": "object",
        "properties": {"host": {"type": "string"}},
        "required": ["host"]
    });
    let action = |name: &str,
                  node: Option<&str>,
                  input: &serde_json::Value,
                  output: &serde_json::Value| ActionContract {
        name: name.to_owned(),
        input_schema: input.clone(),
        output_schema: output.clone(),
        node: node.map(str::to_owned),
        timeout: None,
        retry: None,
        advisory: false,
        agent: false,
        body: None,
    };
    let report = json!({
        "type": "object",
        "properties": {"summary": {"type": "string"}},
        "required": ["summary"]
    });
    WorkerContract {
        task_queue: "fleet".to_owned(),
        actions: vec![
            action("read_north", Some("north"), &host_input, &reading),
            action("calibrate", Some("north"), &host_input, &reading),
            action("read_south", Some("south"), &host_input, &reading),
            action(
                "audit",
                None,
                &json!({
                    "type": "object",
                    "properties": {"summary": {"type": "string"}},
                    "required": ["summary"]
                }),
                &report,
            ),
            ActionContract {
                name: "stamp".to_owned(),
                input_schema: host_input,
                output_schema: json!({
                    "type": "object",
                    "properties": {
                        "exit_code": {"type": "integer"},
                        "stdout": {"type": "string"},
                        "stderr": {"type": "string"}
                    },
                    "required": ["exit_code", "stderr", "stdout"]
                }),
                node: None,
                timeout: None,
                retry: None,
                advisory: false,
                agent: false,
                body: Some(ActionBodyContract::Run {
                    command: "echo $host".to_owned(),
                }),
            },
        ],
    }
}

/// Absolute path to the workspace `crates/` directory, for the generated
/// crate's `path` dependencies on the in-tree SDK.
fn crates_directory() -> Result<PathBuf, TestError> {
    Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()?)
}

/// Writes `relative` under `root`, creating parent directories first.
fn write_file(root: &Path, relative: &str, contents: &str) -> Result<(), TestError> {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, contents)?;
    Ok(())
}

/// Stages the document and generates the crate beside it, in the layout the
/// CLI produces: `<root>/awl/fleet_probe.awl` and `<root>/fleet-worker/`.
fn stage(root: &Path, crates: &Path) -> Result<PathBuf, TestError> {
    write_file(root, "awl/fleet_probe.awl", DOCUMENT)?;
    let contract = fleet_contract();
    let dependencies = AionDependency::Path(
        crates
            .to_str()
            .ok_or("the workspace crates directory is not valid UTF-8")?
            .to_owned(),
    );
    let scaffold = scaffold_awl_worker(&AwlWorkerScaffold {
        contract: &contract,
        crate_name: "fleet-worker",
        document_include: "../../awl/fleet_probe.awl",
        document_directory: &DocumentRoot::InCrateTree("../awl".to_owned()),
        document_name: "fleet_probe.awl",
        dependencies: &dependencies,
    })?;
    let crate_root = root.join("fleet-worker");
    for file in &scaffold.files {
        write_file(&crate_root, &file.relative, &file.contents)?;
    }
    Ok(crate_root)
}

/// Runs one cargo subcommand in the generated crate with an isolated target
/// directory, returning whether it succeeded and everything it printed.
fn cargo(
    crate_root: &Path,
    target: &Path,
    arguments: &[&str],
) -> Result<(bool, String), TestError> {
    let output = Command::new(env!("CARGO"))
        .current_dir(crate_root)
        .args(arguments)
        .env("CARGO_TARGET_DIR", target)
        .output()
        .map_err(|error| format!("failed to spawn `cargo {}`: {error}", arguments.join(" ")))?;
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok((output.status.success(), combined))
}

/// Generates the worker crate for a real document and proves it compiles,
/// lints clean, and keeps its own advertisement contract.
#[test]
fn the_generated_worker_compiles_lints_and_proves_its_advertisement() -> Result<(), TestError> {
    // `cargo` is provided by the test runner via `CARGO`; if it is somehow
    // unset the gate cannot run, so it skips loudly rather than panicking.
    if option_env!("CARGO").is_none() {
        eprintln!(
            "SKIP the_generated_worker_compiles_lints_and_proves_its_advertisement: \
             `CARGO` is not set, cannot run the nested build"
        );
        return Ok(());
    }

    let crates = crates_directory()?;
    let project = tempfile::tempdir()?;
    let crate_root = stage(project.path(), &crates)?;
    let target = tempfile::tempdir()?;

    // Format-clean AS GENERATED. The generated files carry a do-not-edit
    // banner, so an author whose `cargo fmt` rewrites them is being handed a
    // diff they must not keep — the emitter owes them formatted source.
    let (formatted, format_output) = cargo(&crate_root, target.path(), &["fmt", "--check"])?;
    if !formatted {
        return Err(format!(
            "the generated worker is not `cargo fmt` clean as generated — fix the emitter \
             under crates/aion-package/src/codegen/awl_worker/.\n\ncargo output:\n{format_output}"
        )
        .into());
    }

    // The lint table the generated manifest carries is at `deny`, so this
    // proves the emitted code satisfies the estate's own standard — a pedantic
    // lint in generated code is a generator defect, never something the author
    // should have to silence.
    let (linted, lint_output) = cargo(
        &crate_root,
        target.path(),
        &["clippy", "--all-targets", "--", "-D", "warnings"],
    )?;
    if !linted {
        return Err(format!(
            "the generated worker does not pass the lint table it carries — fix the emitter \
             under crates/aion-package/src/codegen/awl_worker/, never the generated file.\
             \n\ncargo output:\n{lint_output}"
        )
        .into());
    }

    // `cargo test`, not `cargo build`: the generated tests compile the embedded
    // document through the real `aion_awl::compile` and assert that every
    // action the connection table serves is declared, reachable on its node,
    // and handled — and that no server-executed action is served.
    let (tested, test_output) = cargo(&crate_root, target.path(), &["test"])?;
    if !tested {
        return Err(format!(
            "the generated worker failed its own advertisement tests — this is a generator \
             or SDK drift, not a test bug.\n\ncargo output:\n{test_output}"
        )
        .into());
    }
    if !test_output.contains("every_served_action_is_declared_reachable_and_handled ... ok") {
        return Err(format!(
            "cargo reported success but the generated advertisement test did not run\
             \n\ncargo output:\n{test_output}"
        )
        .into());
    }

    Ok(())
}
