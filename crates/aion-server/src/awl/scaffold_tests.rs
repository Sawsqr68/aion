use std::collections::BTreeMap;

use tempfile::TempDir;

use super::scaffold::{ScaffoldRefusal, ScaffoldRequest, ScaffoldResponse, scaffold};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const ALL_BODIED: &str = r#"//! Every action is server-run.
workflow gate_flow
  input dir: String
  outcome done: type Report, route success

type Report { stdout: String, stderr: String }

worker gate
  action head(dir: String) -> Report
    run "git -C $dir rev-parse --short HEAD"
  action checks(dir: String) -> Report
    run "cargo clippy --manifest-path ${dir}/Cargo.toml"

step run
  head(dir: dir) -> at
  checks(dir: dir) -> checked
  route done(stdout: at.stdout, stderr: checked.stderr)
"#;

const BODYLESS: &str = r"//! One action is owed by an out-of-band worker.
workflow worker_flow
  input value: String
  outcome done: type String, route success

worker jobs
  action transform(value: String) -> String

step run
  transform(value: value) -> result
  route done(result)
";

const MIXED: &str = r#"//! One action is server-run and one is worker-owed.
workflow mixed_flow
  input value: String
  outcome done: type String, route success

type RunOutcome { stdout: String }

worker mixed
  action server_action(value: String) -> RunOutcome
    run "cmd"
  action worker_action(value: String) -> String

step run
  server_action(value: value) -> server_result
  worker_action(value: server_result.stdout) -> worker_result
  route done(worker_result)
"#;

const PINNED_ONLY: &str = r"//! The only worker-owed action is node-pinned.
workflow pinned_flow
  input value: String
  outcome done: type String, route success

worker pinned
  action pinned_action(value: String) -> String
    node gpu

step run
  pinned_action(value: value) -> result
  route done(result)
";

const PINNED_AND_UNPINNED: &str = r"//! A queue mixes pinned and unpinned worker-owed actions.
workflow hybrid_flow
  input value: String
  outcome done: type String, route success

worker hybrid
  action pinned_action(value: String) -> String
    node gpu
  action portable_action(value: String) -> String

step run
  pinned_action(value: value) -> pinned_result
  portable_action(value: pinned_result) -> portable_result
  route done(portable_result)
";

const IMPORTED_SCHEMA: &str = r#"//! The worker contract imports its payload schema from the workspace.
workflow imported_flow
  input payload: Payload
  outcome done: type Payload, route success

type Payload = schema("payload.schema.json")

worker imported
  action transform(payload: Payload) -> Payload

step run
  transform(payload: payload) -> result
  route done(result)
"#;

const PAYLOAD_SCHEMA: &str =
    r#"{"type":"object","properties":{"value":{"type":"string"}},"required":["value"]}"#;

fn response(source: &str, worker: &str, runtime: &str) -> TestResult<ScaffoldResponse> {
    let schema_root = TempDir::new()?;
    let request = ScaffoldRequest {
        source: source.to_owned(),
        worker: worker.to_owned(),
        runtime: runtime.to_owned(),
    };
    Ok(scaffold(&request, schema_root.path())?)
}

fn generated_files(response: ScaffoldResponse) -> TestResult<BTreeMap<String, String>> {
    let Some(files) = response.files else {
        return Err("scaffold was refused instead of generated".into());
    };
    Ok(files)
}

fn refusal(response: &ScaffoldResponse) -> TestResult<&ScaffoldRefusal> {
    let Some(refusal) = response.refusal.as_ref() else {
        return Err("scaffold was generated instead of refused".into());
    };
    Ok(refusal)
}

fn manifest_action_names(source: &str) -> TestResult<Vec<String>> {
    let document: toml::Value = toml::from_str(source)?;
    let actions = document
        .get("action")
        .and_then(toml::Value::as_array)
        .ok_or("manifest has no action array")?;
    actions
        .iter()
        .map(|action| {
            action
                .get("name")
                .and_then(toml::Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| "manifest action has no string name".into())
        })
        .collect()
}

#[test]
fn all_bodied_rust_document_is_refused_as_having_nothing_to_serve() -> TestResult {
    let response = response(ALL_BODIED, "gate", "rust")?;
    assert!(!response.ok);
    assert!(matches!(
        refusal(&response)?,
        ScaffoldRefusal::NoServableAction { reason }
            if reason.contains("nothing for a worker to serve")
    ));
    Ok(())
}

#[test]
fn bodyless_rust_document_uses_canonical_codegen_and_includes_document() -> TestResult {
    let source = BODYLESS.to_owned();
    let files = generated_files(response(&source, "jobs", "rust")?)?;
    let cargo = files.get("Cargo.toml").ok_or("missing Cargo.toml")?;
    assert!(cargo.contains(env!("CARGO_PKG_VERSION")));
    assert!(!cargo.contains("0.8.0"));

    let main = files.get("src/main.rs").ok_or("missing src/main.rs")?;
    assert!(main.contains("register_activity_with_descriptor"));
    assert!(!main.contains("todo!"));
    assert!(files.contains_key("src/handlers.rs"));
    assert_eq!(files.get("worker_flow.awl"), Some(&source));
    Ok(())
}

#[test]
fn mixed_rust_document_excludes_server_bodied_action() -> TestResult {
    let files = generated_files(response(MIXED, "mixed", "rust")?)?;
    let handlers = files
        .get("src/handlers.rs")
        .ok_or("missing src/handlers.rs")?;
    assert!(handlers.contains("pub fn worker_action"));
    assert!(!handlers.contains("pub fn server_action"));

    let main = files.get("src/main.rs").ok_or("missing src/main.rs")?;
    assert!(main.contains("\"worker_action\""));
    assert!(!main.contains("\"server_action\""));
    assert!(!main.lines().any(|line| {
        line.contains("register_activity_with_descriptor") && line.contains("server_action")
    }));
    Ok(())
}

#[test]
fn mixed_shell_document_wires_only_bodyless_action() -> TestResult {
    let files = generated_files(response(MIXED, "mixed", "shell")?)?;
    let manifest = files.get("worker.toml").ok_or("missing worker.toml")?;
    assert_eq!(manifest_action_names(manifest)?, ["worker_action"]);
    assert!(!manifest.contains("server_action"));
    assert!(manifest.contains(
        "echo 'aion scaffold stub: action worker_action has no command wired - edit worker.toml' >&2; exit 78"
    ));
    Ok(())
}

#[test]
fn shell_refuses_any_node_pinned_queue_with_actionable_code() -> TestResult {
    let response = response(PINNED_ONLY, "pinned", "shell")?;
    assert!(!response.ok);
    assert!(matches!(
        refusal(&response)?,
        ScaffoldRefusal::NodePinnedQueue { reason }
            if reason.contains("node-pinned")
                && reason.contains("no node flag")
                && reason.contains("whole worker-owed action set")
                && reason.contains("one connection per node")
    ));
    Ok(())
}

#[test]
fn shell_refuses_mixed_pinned_and_unpinned_queue_instead_of_wiring_partially() -> TestResult {
    let response = response(PINNED_AND_UNPINNED, "hybrid", "shell")?;
    assert!(!response.ok);
    assert!(response.files.is_none());
    assert!(matches!(
        refusal(&response)?,
        ScaffoldRefusal::NodePinnedQueue { reason }
            if reason.contains("node-pinned") && reason.contains("whole worker-owed action set")
    ));
    Ok(())
}

#[test]
fn unknown_worker_refusal_lists_declared_queues() -> TestResult {
    let response = response(BODYLESS, "absent", "rust")?;
    assert!(matches!(
        refusal(&response)?,
        ScaffoldRefusal::UnknownWorker { reason }
            if reason.contains("worker absent") && reason.contains("`jobs`")
    ));
    Ok(())
}

#[test]
fn unsupported_runtime_is_refused() -> TestResult {
    let response = response(BODYLESS, "jobs", "python")?;
    assert!(matches!(
        refusal(&response)?,
        ScaffoldRefusal::UnsupportedRuntime { reason } if reason.contains("python")
    ));
    Ok(())
}

#[test]
fn unparseable_source_is_an_invalid_document() -> TestResult {
    let response = response("not an awl document", "jobs", "rust")?;
    assert!(matches!(
        refusal(&response)?,
        ScaffoldRefusal::InvalidDocument { reason } if !reason.is_empty()
    ));
    Ok(())
}

#[test]
fn schema_imports_are_compiled_from_the_confined_staging_seam() -> TestResult {
    let schema_root = TempDir::new()?;
    std::fs::write(
        schema_root.path().join("payload.schema.json"),
        PAYLOAD_SCHEMA,
    )?;
    let request = ScaffoldRequest {
        source: IMPORTED_SCHEMA.to_owned(),
        worker: "imported".to_owned(),
        runtime: "rust".to_owned(),
    };

    let files = generated_files(scaffold(&request, schema_root.path())?)?;
    assert_eq!(
        files.get("imported_flow.awl").map(String::as_str),
        Some(IMPORTED_SCHEMA)
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn linked_schema_import_is_an_invalid_document_refusal() -> TestResult {
    use std::os::unix::fs::symlink;

    let schema_root = TempDir::new()?;
    let outside = TempDir::new()?;
    let outside_schema = outside.path().join("payload.schema.json");
    std::fs::write(&outside_schema, PAYLOAD_SCHEMA)?;
    symlink(
        &outside_schema,
        schema_root.path().join("payload.schema.json"),
    )?;
    let request = ScaffoldRequest {
        source: IMPORTED_SCHEMA.to_owned(),
        worker: "imported".to_owned(),
        runtime: "rust".to_owned(),
    };

    let response = scaffold(&request, schema_root.path())?;
    assert!(!response.ok);
    assert!(matches!(
        refusal(&response)?,
        ScaffoldRefusal::InvalidDocument { reason } if !reason.is_empty()
    ));
    Ok(())
}

#[test]
fn absent_workspace_scaffolds_an_import_free_document() -> TestResult {
    let parent = TempDir::new()?;
    let absent = parent.path().join("never-created");
    let request = ScaffoldRequest {
        source: BODYLESS.to_owned(),
        worker: "jobs".to_owned(),
        runtime: "rust".to_owned(),
    };

    let response = scaffold(&request, &absent)?;
    assert!(response.ok);
    let files = generated_files(response)?;
    assert_eq!(
        files.get("worker_flow.awl").map(String::as_str),
        Some(BODYLESS)
    );
    assert!(!absent.exists());
    Ok(())
}

#[test]
fn absent_workspace_reports_a_missing_import_as_the_documents_diagnostic() -> TestResult {
    let parent = TempDir::new()?;
    let absent = parent.path().join("never-created");
    let request = ScaffoldRequest {
        source: IMPORTED_SCHEMA.to_owned(),
        worker: "imported".to_owned(),
        runtime: "rust".to_owned(),
    };

    let response = scaffold(&request, &absent)?;
    assert!(matches!(
        refusal(&response)?,
        ScaffoldRefusal::InvalidDocument { reason }
            if reason.contains("payload.schema.json")
                && !reason.contains("AWL workspace I/O failed")
    ));
    assert!(!absent.exists());
    Ok(())
}

#[test]
fn missing_import_in_a_present_workspace_is_the_documents_diagnostic() -> TestResult {
    let schema_root = TempDir::new()?;
    let request = ScaffoldRequest {
        source: IMPORTED_SCHEMA.to_owned(),
        worker: "imported".to_owned(),
        runtime: "rust".to_owned(),
    };

    let response = scaffold(&request, schema_root.path())?;
    assert!(matches!(
        refusal(&response)?,
        ScaffoldRefusal::InvalidDocument { reason }
            if reason.contains("payload.schema.json")
                && !reason.contains("AWL workspace I/O failed")
    ));
    Ok(())
}

#[cfg(unix)]
#[test]
fn unopenable_workspace_is_the_servers_error_not_the_authors() -> TestResult {
    use std::os::unix::fs::PermissionsExt;

    let schema_root = TempDir::new()?;
    std::fs::set_permissions(schema_root.path(), std::fs::Permissions::from_mode(0o000))?;
    let request = ScaffoldRequest {
        source: BODYLESS.to_owned(),
        worker: "jobs".to_owned(),
        runtime: "rust".to_owned(),
    };

    let result = scaffold(&request, schema_root.path());
    std::fs::set_permissions(schema_root.path(), std::fs::Permissions::from_mode(0o700))?;
    assert!(matches!(
        result,
        Err(super::documents::DocumentError::Io(_))
    ));
    Ok(())
}

#[test]
fn non_directory_workspace_is_the_servers_error_not_an_absent_workspace() -> TestResult {
    let parent = TempDir::new()?;
    let schema_root = parent.path().join("workspace-file");
    std::fs::write(&schema_root, b"not a directory")?;
    let request = ScaffoldRequest {
        source: BODYLESS.to_owned(),
        worker: "jobs".to_owned(),
        runtime: "rust".to_owned(),
    };

    assert!(matches!(
        scaffold(&request, &schema_root),
        Err(super::documents::DocumentError::Io(_))
    ));
    Ok(())
}
