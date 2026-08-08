//! What a generated worker owes its queue, asserted on the rendered source.
//!
//! The fixture is the shape that costs a first flight when it is wrong: a
//! queue with actions pinned to two different nodes, an unpinned action every
//! connection owes, and a `run`-bodied action the server executes itself.

use serde_json::json;

use super::error::AwlScaffoldError;
use super::plan::plan;
use super::scaffold::{
    AionDependency, AwlWorkerScaffold, DocumentRoot, FileOwnership, ScaffoldedFile, WorkerScaffold,
    scaffold_awl_worker,
};
use super::shell_manifest::emit_shell_manifest;
use crate::canonical::CanonicalJson;
use crate::contract::{ActionBodyContract, ActionContract, WorkerContract};

/// One declared action of the fixture contract.
fn action(name: &str, node: Option<&str>, body: Option<ActionBodyContract>) -> ActionContract {
    ActionContract {
        name: name.to_owned(),
        input_schema: json!({
            "type": "object",
            "properties": {"host": {"type": "string"}},
            "required": ["host"]
        }),
        output_schema: json!({
            "type": "object",
            "properties": {"celsius": {"type": "integer"}},
            "required": ["celsius"]
        }),
        node: node.map(str::to_owned),
        timeout: None,
        retry: None,
        advisory: false,
        agent: false,
        body,
    }
}

/// The fixture: two nodes, one unpinned action, one server-executed body.
fn fleet() -> WorkerContract {
    WorkerContract {
        task_queue: "fleet".to_owned(),
        actions: vec![
            action("read_north", Some("north"), None),
            action("calibrate", Some("north"), None),
            action("read_south", Some("south"), None),
            action("audit", None, None),
            action(
                "stamp",
                None,
                Some(ActionBodyContract::Run {
                    command: "echo $host".to_owned(),
                }),
            ),
        ],
    }
}

/// A queue with no pins at all.
fn flat() -> WorkerContract {
    WorkerContract {
        task_queue: "flat".to_owned(),
        actions: vec![action("first", None, None), action("second", None, None)],
    }
}

/// Renders the fixture scaffold with version-sourced dependencies.
fn render(contract: &WorkerContract) -> Result<WorkerScaffold, AwlScaffoldError> {
    let dependencies = AionDependency::Version("0.10.0".to_owned());
    let document_directory = DocumentRoot::InCrateTree("..".to_owned());
    scaffold_awl_worker(&AwlWorkerScaffold {
        contract,
        crate_name: "fleet-worker",
        document_include: "../../fleet_probe.awl",
        document_directory: &document_directory,
        document_name: "fleet_probe.awl",
        dependencies: &dependencies,
    })
}

/// A file the scaffold was expected to render and did not.
#[derive(Debug, thiserror::Error)]
#[error("`{relative}` was not generated")]
struct MissingFile {
    relative: String,
}

/// One rendered file by path.
fn file<'a>(
    scaffold: &'a WorkerScaffold,
    relative: &str,
) -> Result<&'a ScaffoldedFile, MissingFile> {
    scaffold
        .files
        .iter()
        .find(|file| file.relative == relative)
        .ok_or_else(|| MissingFile {
            relative: relative.to_owned(),
        })
}

/// Every test that can fail on either a refusal or a missing file.
type TestResult = Result<(), Box<dyn std::error::Error>>;

/// One schema as the generated stub documents it: canonically encoded, then
/// prefixed line by line as a doc comment.
fn documented(schema: &serde_json::Value) -> Result<String, serde_json::Error> {
    let rendered = serde_json::to_string_pretty(&CanonicalJson::new(schema.clone()))?;
    Ok(rendered
        .lines()
        .map(|line| {
            if line.is_empty() {
                "///".to_owned()
            } else {
                format!("/// {line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n"))
}

/// THE ROUTING RULE. A worker serving actions pinned to several nodes must
/// open one connection PER NODE, each advertising the actions a dispatch could
/// reach it with — its own node's, plus every unpinned one, because an
/// unpinned dispatch reaches any worker in the pool.
#[test]
fn one_connection_per_node_serves_exactly_the_reachable_actions() -> Result<(), AwlScaffoldError> {
    let plan = plan(&fleet())?;

    let nodes: Vec<Option<&str>> = plan
        .connections
        .iter()
        .map(|connection| connection.node.as_deref())
        .collect();
    assert_eq!(nodes, vec![Some("north"), Some("south")]);
    assert_eq!(
        plan.connections[0].actions,
        ["read_north", "calibrate", "audit"]
    );
    assert_eq!(plan.connections[1].actions, ["read_south", "audit"]);
    Ok(())
}

/// A queue pinning nothing dials ONCE, carrying no locality: only unpinned
/// dispatches reach such a connection, which is then the whole queue.
#[test]
fn an_unpinned_queue_dials_one_connection_with_no_node() -> Result<(), AwlScaffoldError> {
    let plan = plan(&flat())?;

    assert_eq!(plan.connections.len(), 1);
    assert_eq!(plan.connections[0].node, None);
    assert_eq!(plan.connections[0].actions, ["first", "second"]);
    Ok(())
}

/// A `run`-bodied action is executed by the SERVER. No worker serves it, the
/// admission gate never demands it, and advertising one would promise work
/// this binary does not do — so it must appear in no connection, no handler
/// table, and no stub.
#[test]
fn a_server_executed_action_is_absent_everywhere() -> Result<(), AwlScaffoldError> {
    let scaffold = render(&fleet())?;

    assert!(
        !scaffold
            .plan
            .servable
            .iter()
            .any(|action| action.name == "stamp"),
        "a bodied action must not be planned for a worker"
    );
    for connection in &scaffold.plan.connections {
        assert!(!connection.actions.iter().any(|action| action == "stamp"));
    }
    for generated in &scaffold.files {
        assert!(
            !generated.contents.contains("stamp"),
            "`stamp` leaked into {}",
            generated.relative
        );
    }
    Ok(())
}

/// The connection table the emitted `main.rs` carries is the plan, verbatim:
/// one `Connection` per node, each listing its own actions and no others.
#[test]
fn the_emitted_connection_table_is_the_plan() -> TestResult {
    let scaffold = render(&fleet())?;
    let main = &file(&scaffold, "src/main.rs")?.contents;

    assert!(main.contains(
        "    Connection {\n        node: Some(\"north\"),\n        \
         actions: &[\"read_north\", \"calibrate\", \"audit\"],\n    },"
    ));
    assert!(main.contains(
        "    Connection {\n        node: Some(\"south\"),\n        \
         actions: &[\"read_south\", \"audit\"],\n    },"
    ));
    assert_eq!(main.matches("Connection {\n        node:").count(), 2);
    Ok(())
}

/// EVERY registered action is registered WITH a descriptor, and every
/// descriptor comes from the compiled contract. A handler registered with no
/// descriptor advertises nothing and the server refuses the whole connection;
/// a descriptor derived from Rust types with `schemars` would be a second
/// rendering of the same shapes, which is the drift the contract gate exists
/// to catch.
#[test]
fn every_registration_carries_a_descriptor_from_the_document() -> TestResult {
    let scaffold = render(&fleet())?;
    let main = &file(&scaffold, "src/main.rs")?.contents;

    assert!(main.contains(
        "builder = builder.register_activity_with_descriptor::<Value, Value, _>(\n\
         \x20           *action,\n\
         \x20           declaration.descriptor(action, connection.node)?,\n\
         \x20           handler,\n\
         \x20       )?;"
    ));
    assert!(
        !main.contains("register_activity_with_contract") && !main.contains("schemars"),
        "the advertisement must come from the document, never from a Rust-type derivation"
    );
    assert!(
        !main.contains(".register_activity("),
        "a descriptor-less registration advertises nothing"
    );
    Ok(())
}

/// The declaration module compiles the EMBEDDED document and takes its
/// schemas: the bytes advertised are the bytes deployed.
#[test]
fn the_declaration_embeds_the_document_and_takes_its_schemas() -> TestResult {
    let scaffold = render(&fleet())?;
    let declaration = &file(&scaffold, "src/declaration.rs")?.contents;

    assert!(
        declaration.contains("const DOCUMENT: &str = include_str!(\"../../fleet_probe.awl\");")
    );
    assert!(
        declaration
            .contains("const DOCUMENT_DIR: &str = concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/..\");")
    );
    assert!(declaration.contains("pub const TASK_QUEUE: &str = \"fleet\";"));
    assert!(declaration.contains("aion_awl::compile(DOCUMENT, Path::new(DOCUMENT_DIR))"));
    assert!(declaration.contains(
        "            input_schema: declared.input_schema.clone(),\n            \
         output_schema: declared.output_schema.clone(),"
    ));
    Ok(())
}

/// The declared schemas reach the author verbatim, in each stub's
/// documentation: it is what they need to fill the body in, and it is what
/// the server admits the advertisement against.
#[test]
fn every_stub_documents_the_declared_schemas() -> TestResult {
    let scaffold = render(&fleet())?;
    let handlers = &file(&scaffold, "src/handlers.rs")?.contents;

    let contract = fleet();
    for declared in &contract.actions {
        if declared.body.is_some() {
            continue;
        }
        for schema in [&declared.input_schema, &declared.output_schema] {
            assert!(
                handlers.contains(&documented(schema)?),
                "the declared schema of `{}` is not reproduced verbatim",
                declared.name
            );
        }
    }
    assert_eq!(handlers.matches("```json").count(), 8);
    Ok(())
}

/// A stub must be impossible to ship by accident: it fails the activity,
/// terminally, naming the action — never a `todo!()` or a `panic!()` (both
/// denied by the lint table the generated crate carries), and above all never
/// a silent success.
#[test]
fn every_stub_fails_loudly_and_names_its_action() -> TestResult {
    let scaffold = render(&fleet())?;
    let handlers = &file(&scaffold, "src/handlers.rs")?.contents;

    for name in ["read_north", "calibrate", "read_south", "audit"] {
        assert!(handlers.contains(&format!("pub fn {name}(input: Value")));
        assert!(handlers.contains(&format!("\"activity `{name}` is not implemented")));
    }
    assert_eq!(handlers.matches("ActivityFailure::terminal(").count(), 4);
    assert!(!handlers.contains("todo!"));
    assert!(!handlers.contains("panic!"));
    assert!(!handlers.contains("unimplemented!"));
    assert!(
        !handlers.contains("Ok(Value::"),
        "a stub must never report a result nobody computed"
    );
    Ok(())
}

/// The generated files are rewritten from the document; the handlers file is
/// the author's from the moment it exists.
#[test]
fn only_the_handlers_file_is_author_owned() -> Result<(), AwlScaffoldError> {
    let scaffold = render(&fleet())?;

    let owned: Vec<(&str, FileOwnership)> = scaffold
        .files
        .iter()
        .map(|file| (file.relative.as_str(), file.ownership))
        .collect();
    assert_eq!(
        owned,
        vec![
            ("Cargo.toml", FileOwnership::Generated),
            ("src/main.rs", FileOwnership::Generated),
            ("src/declaration.rs", FileOwnership::Generated),
            ("src/handlers.rs", FileOwnership::Author),
        ]
    );
    Ok(())
}

/// The manifest is standalone, carries the aion lint policy, and takes the SDK
/// from the requested source.
#[test]
fn the_manifest_is_standalone_and_strict() -> TestResult {
    let scaffold = render(&fleet())?;
    let manifest = &file(&scaffold, "Cargo.toml")?.contents;

    assert!(manifest.contains("name = \"fleet-worker\""));
    assert!(manifest.contains("\n[workspace]"));
    assert!(manifest.contains("unwrap_used = \"deny\""));
    assert!(manifest.contains("panic = \"deny\""));
    assert!(manifest.contains("todo = \"deny\""));
    assert!(manifest.contains("aion-worker = \"0.10.0\""));
    assert!(manifest.contains("aion-awl = \"0.10.0\""));

    let dependencies = AionDependency::Path("../../crates".to_owned());
    let document_directory = DocumentRoot::InCrateTree("..".to_owned());
    let by_path = scaffold_awl_worker(&AwlWorkerScaffold {
        contract: &fleet(),
        crate_name: "fleet-worker",
        document_include: "../../fleet_probe.awl",
        document_directory: &document_directory,
        document_name: "fleet_probe.awl",
        dependencies: &dependencies,
    })?;
    assert!(
        file(&by_path, "Cargo.toml")?
            .contents
            .contains("aion-worker = { path = \"../../crates/aion-worker\" }")
    );
    Ok(())
}

/// A queue whose every action carries a body is served entirely by the server.
/// Emitting a crate that registers nothing would be emitting a worker that
/// cannot start, so it is refused with the reason.
#[test]
fn a_queue_of_only_bodied_actions_is_refused() {
    let contract = WorkerContract {
        task_queue: "shell_box".to_owned(),
        actions: vec![action(
            "stamp",
            None,
            Some(ActionBodyContract::Run {
                command: "echo hi".to_owned(),
            }),
        )],
    };

    assert_eq!(
        plan(&contract).err(),
        Some(AwlScaffoldError::NoServableAction {
            task_queue: "shell_box".to_owned()
        })
    );
}

/// An action named after a Rust keyword still gets a handler, as a raw
/// identifier; one named after a keyword with no raw form is refused rather
/// than emitted into a crate that will not compile.
#[test]
fn keyword_action_names_are_escaped_or_refused() -> TestResult {
    let carries_its_own_escape = WorkerContract {
        task_queue: "edge".to_owned(),
        actions: vec![action("r#match", None, None)],
    };
    assert!(matches!(
        plan(&carries_its_own_escape),
        Err(AwlScaffoldError::ActionNameNotAnIdentifier { .. })
    ));

    let keyword = WorkerContract {
        task_queue: "edge".to_owned(),
        actions: vec![action("match", None, None)],
    };
    let scaffold = render(&keyword)?;
    assert!(
        file(&scaffold, "src/handlers.rs")?
            .contents
            .contains("pub fn r#match(")
    );
    assert!(
        file(&scaffold, "src/main.rs")?
            .contents
            .contains("\"match\" => Some(handlers::r#match),")
    );

    let unnameable = WorkerContract {
        task_queue: "edge".to_owned(),
        actions: vec![action("self", None, None)],
    };
    assert_eq!(
        plan(&unnameable).err(),
        Some(AwlScaffoldError::ActionNameUnnameable {
            task_queue: "edge".to_owned(),
            action: "self".to_owned()
        })
    );
    Ok(())
}

/// Generation is a pure function of the contract: the same document scaffolded
/// twice is byte-identical, so a regenerate-and-diff is a real drift check.
#[test]
fn generation_is_deterministic() -> Result<(), AwlScaffoldError> {
    let first = render(&fleet())?;
    let second = render(&fleet())?;

    assert_eq!(first.files, second.files);
    Ok(())
}

/// A crate name Cargo would refuse, and an empty document path, are refused
/// before a file is rendered — a manifest that does not parse and an
/// `include_str!("")` both fail far from their cause.
#[test]
fn unusable_names_and_paths_are_refused() {
    let contract = fleet();
    let dependencies = AionDependency::Version("0.10.0".to_owned());
    let request = |crate_name: &'static str, include: &'static str, directory: &'static str| {
        let document_directory = DocumentRoot::InCrateTree(directory.to_owned());
        scaffold_awl_worker(&AwlWorkerScaffold {
            contract: &contract,
            crate_name,
            document_include: include,
            document_directory: &document_directory,
            document_name: "fleet_probe.awl",
            dependencies: &dependencies,
        })
        .err()
    };

    assert_eq!(
        request("fleet worker", "../doc.awl", ".."),
        Some(AwlScaffoldError::CrateNameInvalid {
            crate_name: "fleet worker".to_owned()
        })
    );
    assert_eq!(
        request("fleet-worker", "", ".."),
        Some(AwlScaffoldError::DocumentPathEmpty { role: "document" })
    );
    assert_eq!(
        request("fleet-worker", "../doc.awl", ""),
        Some(AwlScaffoldError::DocumentPathEmpty { role: "directory" })
    );
}

/// Shell result encoding is derived from the declared output schema, and every
/// worker-owed action is wired exactly once so the runtime's whole-queue startup
/// check sees the complete set.
#[test]
fn shell_manifest_wires_the_whole_queue_with_declared_result_encodings() -> TestResult {
    let mut contract = flat();
    contract.actions[0].output_schema = json!({"type": "string"});
    let files = emit_shell_manifest(&contract, "flat_flow.awl")?;
    let manifest = files
        .iter()
        .find(|file| file.relative == "worker.toml")
        .ok_or("shell scaffold omitted worker.toml")?;
    let parsed: toml::Value = toml::from_str(&manifest.contents)?;
    let actions = parsed
        .get("action")
        .and_then(toml::Value::as_array)
        .ok_or("worker.toml omitted its action array")?;
    assert_eq!(actions.len(), 2);

    let encoding = |name: &str| {
        actions
            .iter()
            .find(|action| action.get("name").and_then(toml::Value::as_str) == Some(name))
            .and_then(|action| action.get("result"))
            .and_then(toml::Value::as_str)
    };
    assert_eq!(encoding("first"), Some("text"));
    assert_eq!(encoding("second"), Some("json"));
    Ok(())
}

/// A shell worker has no node flag and its startup reconciliation is
/// whole-queue, so one node pin makes the queue a Rust-scaffold topology.
#[test]
fn shell_manifest_refuses_any_node_pinned_queue() {
    assert_eq!(
        emit_shell_manifest(&fleet(), "fleet_probe.awl").err(),
        Some(AwlScaffoldError::NodePinnedQueue {
            task_queue: "fleet".to_owned()
        })
    );
}

/// The shell README carries the production invocation and the ownership rule:
/// wiring is local, while schemas and serving completeness come from the AWL
/// document.
#[test]
fn shell_readme_carries_the_production_invocation() -> TestResult {
    let files = emit_shell_manifest(&flat(), "flat_flow.awl")?;
    let readme = files
        .iter()
        .find(|file| file.relative == "README.md")
        .ok_or("shell scaffold omitted README.md")?;
    assert!(readme.contents.contains(
        "aion worker shell --manifest worker.toml --awl flat_flow.awl --endpoint <server>"
    ));
    assert!(readme.contents.contains("all-or-nothing per queue"));
    assert!(readme.contents.contains("complete worker-owed action set"));
    Ok(())
}
