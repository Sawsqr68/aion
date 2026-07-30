//! `aion awl scaffold` against a REAL document, through the real compiler.
//!
//! The fixture is the shape that costs a first flight when it is wrong: two
//! node-pinned actions on one node, a third on another, an unpinned action
//! every connection owes, and a `run`-bodied action the server executes with
//! no worker at all.

use std::fs;
use std::path::{Path, PathBuf};

use super::command::{ScaffoldOutcome, scaffold};
use super::paths::{Link, link};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// The fixture document. `read_north`/`calibrate` are pinned to `north`,
/// `read_south` to `south`, `audit` is unpinned, and `stamp` carries a `run`
/// body the server executes itself.
const DOCUMENT: &str = r#"//! A fleet probe across two machines.
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

/// Stages the document inside a fresh temp tree and returns its path.
fn stage(directory: &Path) -> Result<PathBuf, std::io::Error> {
    let awl = directory.join("awl");
    fs::create_dir_all(&awl)?;
    let document = awl.join("fleet_probe.awl");
    fs::write(&document, DOCUMENT)?;
    Ok(document)
}

/// Scaffolds the fixture into `<temp>/workers` and returns the one outcome.
fn scaffold_fixture(directory: &Path) -> Result<ScaffoldOutcome, Box<dyn std::error::Error>> {
    let document = stage(directory)?;
    let out = directory.join("workers");
    let mut outcomes =
        scaffold(&document, &out, None, None).map_err(|failure| failure.message())?;
    assert_eq!(outcomes.len(), 1, "one crate per `worker` block");
    outcomes.pop().ok_or_else(|| "no outcome".into())
}

/// A `.awl` document with actions on two nodes yields ONE CONNECTION PER NODE,
/// each serving its own node's actions plus every unpinned one — the exact set
/// the server's admission gate demands of it.
#[test]
fn a_multi_node_document_yields_one_connection_per_node() -> TestResult {
    let directory = tempfile::tempdir()?;
    let outcome = scaffold_fixture(directory.path())?;

    let nodes: Vec<Option<&str>> = outcome
        .plan
        .connections
        .iter()
        .map(|connection| connection.node.as_deref())
        .collect();
    assert_eq!(nodes, vec![Some("north"), Some("south")]);
    assert_eq!(
        outcome.plan.connections[0].actions,
        ["read_north", "calibrate", "audit"]
    );
    assert_eq!(outcome.plan.connections[1].actions, ["read_south", "audit"]);
    Ok(())
}

/// The `run`-bodied action is executed by the SERVER, so it must appear in no
/// connection and in no generated file: advertising it would promise work this
/// worker does not do, and the admission gate never demands it.
#[test]
fn the_server_executed_action_reaches_no_generated_file() -> TestResult {
    let directory = tempfile::tempdir()?;
    let outcome = scaffold_fixture(directory.path())?;

    for relative in &outcome.written {
        let contents = fs::read_to_string(outcome.crate_root.join(relative))?;
        assert!(
            !contents.contains("stamp"),
            "`stamp` carries a `run` body and leaked into {relative}"
        );
    }
    Ok(())
}

/// The whole crate is written, and the paths it embeds RESOLVE: the
/// `include_str!` reaches the real document from `src/`, and the schema-import
/// root reaches its directory from the crate root.
#[test]
fn the_generated_crate_reaches_the_document_it_was_generated_from() -> TestResult {
    let directory = tempfile::tempdir()?;
    let outcome = scaffold_fixture(directory.path())?;

    assert_eq!(
        outcome.written,
        vec![
            "Cargo.toml".to_owned(),
            "src/main.rs".to_owned(),
            "src/declaration.rs".to_owned(),
            "src/handlers.rs".to_owned(),
        ]
    );
    assert!(outcome.preserved.is_empty());
    assert_eq!(
        outcome.crate_root,
        directory.path().join("workers").join("fleet-worker")
    );

    let declaration = fs::read_to_string(outcome.crate_root.join("src/declaration.rs"))?;
    assert!(
        declaration.contains("include_str!(\"../../../awl/fleet_probe.awl\")"),
        "{declaration}"
    );
    assert!(
        declaration.contains("concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/../../awl\")"),
        "{declaration}"
    );
    // The embedded paths must actually resolve, or the generated crate fails
    // to build with a message about a file it cannot find.
    assert!(
        outcome
            .crate_root
            .join("src")
            .join("../../../awl/fleet_probe.awl")
            .is_file()
    );
    assert!(outcome.crate_root.join("../../awl").is_dir());
    Ok(())
}

/// The generated worker registers a descriptor with every handler, taken from
/// the compiled document — never from a `schemars` derivation of the Rust
/// types, which would be a second rendering of the same shapes and exactly the
/// drift the server's contract gate exists to catch.
#[test]
fn every_registration_carries_a_descriptor() -> TestResult {
    let directory = tempfile::tempdir()?;
    let outcome = scaffold_fixture(directory.path())?;
    let main = fs::read_to_string(outcome.crate_root.join("src/main.rs"))?;

    assert!(main.contains("register_activity_with_descriptor::<Value, Value, _>"));
    assert!(main.contains("declaration.descriptor(action, connection.node)?"));
    assert!(!main.contains("schemars"));
    assert!(main.contains("node: Some(\"north\")"));
    assert!(main.contains("node: Some(\"south\")"));
    Ok(())
}

/// The author's file survives a re-run; the generated ones are rewritten.
/// Clobbering `handlers.rs` would delete the very work the scaffold exists to
/// start.
#[test]
fn a_second_run_rewrites_the_generated_files_and_keeps_the_handlers() -> TestResult {
    let directory = tempfile::tempdir()?;
    let outcome = scaffold_fixture(directory.path())?;
    let handlers = outcome.crate_root.join("src/handlers.rs");
    fs::write(&handlers, "// my work\n")?;
    fs::write(outcome.crate_root.join("src/main.rs"), "// hand edit\n")?;

    let document = directory.path().join("awl/fleet_probe.awl");
    let out = directory.path().join("workers");
    let second = scaffold(&document, &out, None, None).map_err(|failure| failure.message())?;

    assert_eq!(second.len(), 1);
    assert_eq!(second[0].preserved, vec!["src/handlers.rs".to_owned()]);
    assert_eq!(fs::read_to_string(&handlers)?, "// my work\n");
    assert!(
        fs::read_to_string(outcome.crate_root.join("src/main.rs"))?
            .contains("Generated by `aion awl scaffold`"),
        "a generated file must be regenerated over a hand edit"
    );
    Ok(())
}

/// A worker name the document does not declare is a typo, refused with what it
/// does declare.
#[test]
fn an_unknown_worker_name_is_refused() -> TestResult {
    let directory = tempfile::tempdir()?;
    let document = stage(directory.path())?;
    let out = directory.path().join("workers");

    let failure = scaffold(&document, &out, Some("fleeet"), None)
        .err()
        .ok_or("an unknown worker name must be refused")?;
    assert_eq!(
        failure.message(),
        "the document declares no `worker fleeet` block; it declares: [`fleet`]"
    );
    assert!(!out.exists(), "nothing is written when the selection fails");
    Ok(())
}

/// A directory that is not an aion checkout is refused before a crate is
/// written: a `path` dependency on nothing would fail at `cargo build` with a
/// message about a manifest, far from the flag that caused it.
#[test]
fn a_bogus_sdk_path_is_refused() -> TestResult {
    let directory = tempfile::tempdir()?;
    let document = stage(directory.path())?;
    let out = directory.path().join("workers");

    let failure = scaffold(&document, &out, None, Some(directory.path()))
        .err()
        .ok_or("a non-checkout SDK path must be refused")?;
    assert!(
        failure
            .message()
            .contains("is not an aion `crates/` directory"),
        "{}",
        failure.message()
    );
    Ok(())
}

/// A document that does not compile reports the compiler's own diagnostics and
/// writes nothing.
#[test]
fn a_document_that_does_not_compile_writes_nothing() -> TestResult {
    let directory = tempfile::tempdir()?;
    let document = directory.path().join("broken.awl");
    fs::write(&document, "workflow broken\n  input target: NoSuchType\n")?;
    let out = directory.path().join("workers");

    let failure = scaffold(&document, &out, None, None)
        .err()
        .ok_or("a broken document must be refused")?;
    assert!(
        failure.message().contains("error:"),
        "{}",
        failure.message()
    );
    assert!(!out.exists());
    Ok(())
}

/// Within one tree the link is RELATIVE, so the scaffold survives the tree
/// being moved or cloned: a sibling, a child, a parent, and the same
/// directory.
#[test]
fn a_link_within_one_tree_is_relative() -> TestResult {
    let root = Path::new("/tmp/tree");
    assert_eq!(
        link(root, &root.join("awl/doc.awl"))?,
        Link::Relative("awl/doc.awl".to_owned())
    );
    assert_eq!(
        link(&root.join("worker/src"), &root.join("awl/doc.awl"))?,
        Link::Relative("../../awl/doc.awl".to_owned())
    );
    assert_eq!(
        link(&root.join("worker"), root)?,
        Link::Relative("..".to_owned())
    );
    assert_eq!(link(root, root)?, Link::Relative(".".to_owned()));
    assert_eq!(
        link(&root.join("worker/./src/../src"), &root.join("awl"))?,
        Link::Relative("../../awl".to_owned())
    );
    Ok(())
}

/// Across trees that share nothing but the filesystem root the link is
/// ABSOLUTE. A relative path there would be a `../../../..` chain encoding the
/// depth of a directory nobody chose, and it would break the moment either
/// tree moved.
#[test]
fn a_link_across_unrelated_trees_is_absolute() -> TestResult {
    assert_eq!(
        link(
            Path::new("/tmp/out/worker/src"),
            Path::new("/Users/tom/repo/awl/doc.awl")
        )?,
        Link::Absolute("/Users/tom/repo/awl/doc.awl".to_owned())
    );
    // One shared directory is enough for a relative link to mean something.
    assert_eq!(
        link(Path::new("/Users/out/worker"), Path::new("/Users/repo/awl"))?,
        Link::Relative("../../repo/awl".to_owned())
    );
    Ok(())
}
