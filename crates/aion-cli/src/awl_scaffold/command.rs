//! `aion awl scaffold`: a `.awl` document in, a compiling worker crate out.
//!
//! The document is compiled through the SAME `aion_awl::compile` seam the
//! server's deploy path uses, and the crate is generated from the contract it
//! returns. That is the whole point: the schemas the generated worker
//! advertises are the bytes the server deployed and admits against, so no
//! second rendering of the same shapes exists to drift from the first.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use aion_package::{
    AionDependency, AwlWorkerScaffold, ConnectionPlan, DocumentRoot, FileOwnership, WorkerContract,
    WorkerScaffold, scaffold_awl_worker,
};

use super::paths::{Link, link};
use crate::awl::{compile_diagnostics, document_root, read_source, report};

/// The version of the aion SDK crates a generated `Cargo.toml` depends on
/// when no local checkout is supplied: this binary's own.
///
/// It is not a guess. The emitted code is written against the SDK surface
/// THIS binary was built with, so any other version is a version the
/// generator never saw.
const SDK_VERSION: &str = env!("CARGO_PKG_VERSION");

/// What one scaffolded queue produced.
#[derive(Debug)]
pub(super) struct ScaffoldOutcome {
    /// The generated crate's root directory.
    pub(super) crate_root: PathBuf,
    /// The connection plan the crate implements.
    pub(super) plan: ConnectionPlan,
    /// Files written, relative to the crate root.
    pub(super) written: Vec<String>,
    /// Author-owned files left exactly as they were.
    pub(super) preserved: Vec<String>,
}

/// Why a scaffold run stopped.
#[derive(Debug)]
pub(super) enum ScaffoldFailure {
    /// The document did not compile; these are its compiler diagnostics.
    Document(Vec<String>),
    /// Everything else, as one operator-facing line.
    Message(String),
}

impl ScaffoldFailure {
    /// The single line an operator reads, for the cases that have one.
    #[cfg(test)]
    pub(super) fn message(&self) -> String {
        match self {
            Self::Document(diagnostics) => diagnostics.join("\n"),
            Self::Message(message) => message.clone(),
        }
    }
}

/// Runs `aion awl scaffold`, reporting to the terminal.
pub(crate) fn run(
    file: &Path,
    out: &Path,
    worker: Option<&str>,
    aion_crates: Option<&Path>,
) -> ExitCode {
    match scaffold(file, out, worker, aion_crates) {
        Ok(outcomes) => {
            for outcome in &outcomes {
                print_report(outcome);
            }
            ExitCode::SUCCESS
        }
        Err(ScaffoldFailure::Document(diagnostics)) => report(&diagnostics),
        Err(ScaffoldFailure::Message(message)) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

/// Compiles the document and generates a crate for each selected worker.
pub(super) fn scaffold(
    file: &Path,
    out: &Path,
    worker: Option<&str>,
    aion_crates: Option<&Path>,
) -> Result<Vec<ScaffoldOutcome>, ScaffoldFailure> {
    let source = read_source(file)
        .ok_or_else(|| ScaffoldFailure::Message(format!("could not read {}", file.display())))?;
    let compiled = aion_awl::compile(&source, document_root(file))
        .map_err(|error| ScaffoldFailure::Document(compile_diagnostics(file, &error)))?;
    let selected = select_workers(&compiled.contract.workers, worker)?;
    selected
        .into_iter()
        .map(|contract| scaffold_one(file, out, contract, aion_crates))
        .collect()
}

/// The worker blocks to scaffold: the one named, or all of them.
///
/// A document with no `worker` block declares no action for anyone to serve,
/// and a name that matches nothing is a typo — both refuse, naming what the
/// document does declare.
fn select_workers<'a>(
    workers: &'a [WorkerContract],
    requested: Option<&str>,
) -> Result<Vec<&'a WorkerContract>, ScaffoldFailure> {
    if workers.is_empty() {
        return Err(ScaffoldFailure::Message(
            "the document declares no `worker` block, so it requires no out-of-band worker \
             and there is nothing to scaffold"
                .to_owned(),
        ));
    }
    let Some(requested) = requested else {
        return Ok(workers.iter().collect());
    };
    workers
        .iter()
        .find(|worker| worker.task_queue == requested)
        .map(|worker| vec![worker])
        .ok_or_else(|| {
            ScaffoldFailure::Message(format!(
                "the document declares no `worker {requested}` block; it declares: [{}]",
                workers
                    .iter()
                    .map(|worker| format!("`{}`", worker.task_queue))
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        })
}

/// Generates and writes one queue's crate.
fn scaffold_one(
    file: &Path,
    out: &Path,
    contract: &WorkerContract,
    aion_crates: Option<&Path>,
) -> Result<ScaffoldOutcome, ScaffoldFailure> {
    let crate_name = crate_name(&contract.task_queue);
    let crate_root = out.join(&crate_name);
    let document_name = file.file_name().map_or_else(
        || file.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    let document_include = path(&crate_root.join("src"), file)?;
    let document_directory = document_directory(&crate_root, document_root(file))?;
    let dependencies = dependencies(&crate_root, aion_crates)?;
    let request = AwlWorkerScaffold {
        contract,
        crate_name: &crate_name,
        document_include: &document_include,
        document_directory: &document_directory,
        document_name: &document_name,
        dependencies: &dependencies,
    };
    let scaffold = scaffold_awl_worker(&request)
        .map_err(|error| ScaffoldFailure::Message(error.to_string()))?;
    write(crate_root, &scaffold)
}

/// The Cargo package name for a queue: `<queue>-worker`, with the queue's
/// `snake_case` underscores spelled the way Cargo package names conventionally
/// are.
fn crate_name(task_queue: &str) -> String {
    format!("{}-worker", task_queue.replace('_', "-"))
}

/// Where the generated crate takes the aion SDK from.
///
/// A local checkout is expressed RELATIVE to the generated crate, so the
/// scaffold and the checkout move together.
fn dependencies(
    crate_root: &Path,
    aion_crates: Option<&Path>,
) -> Result<AionDependency, ScaffoldFailure> {
    let Some(directory) = aion_crates else {
        return Ok(AionDependency::Version(SDK_VERSION.to_owned()));
    };
    if !directory.join("aion-worker").join("Cargo.toml").is_file() {
        return Err(ScaffoldFailure::Message(format!(
            "`{}` is not an aion `crates/` directory: it holds no `aion-worker/Cargo.toml`",
            directory.display()
        )));
    }
    reach(crate_root, directory).map(|link| AionDependency::Path(link.text().to_owned()))
}

/// A path the generated crate needs, or the refusal that names it.
fn path(from_directory: &Path, to: &Path) -> Result<String, ScaffoldFailure> {
    reach(from_directory, to).map(|link| link.text().to_owned())
}

/// The schema-import root the generated crate resolves the document against.
fn document_directory(
    crate_root: &Path,
    directory: &Path,
) -> Result<DocumentRoot, ScaffoldFailure> {
    Ok(match reach(crate_root, directory)? {
        Link::Relative(path) => DocumentRoot::InCrateTree(path),
        Link::Absolute(path) => DocumentRoot::Absolute(path),
    })
}

/// How a file in `from_directory` reaches `to`, or the refusal that names it.
fn reach(from_directory: &Path, to: &Path) -> Result<Link, ScaffoldFailure> {
    link(from_directory, to).map_err(|error| {
        ScaffoldFailure::Message(format!(
            "the generated crate could not reach the document: {error:#}"
        ))
    })
}

/// Writes the rendered crate, preserving the author-owned file.
///
/// The generated files are rewritten every time — they carry a do-not-edit
/// banner and are a pure function of the document. `src/handlers.rs` is
/// written ONCE: it is where the author's work lives, and clobbering it to
/// re-emit stubs would delete the very thing the scaffold exists to start.
fn write(
    crate_root: PathBuf,
    scaffold: &WorkerScaffold,
) -> Result<ScaffoldOutcome, ScaffoldFailure> {
    let mut written = Vec::new();
    let mut preserved = Vec::new();
    for file in &scaffold.files {
        let path = crate_root.join(&file.relative);
        if file.ownership == FileOwnership::Author && path.exists() {
            preserved.push(file.relative.clone());
            continue;
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                ScaffoldFailure::Message(format!("failed to create {}: {error}", parent.display()))
            })?;
        }
        fs::write(&path, &file.contents).map_err(|error| {
            ScaffoldFailure::Message(format!("failed to write {}: {error}", path.display()))
        })?;
        written.push(file.relative.clone());
    }
    Ok(ScaffoldOutcome {
        crate_root,
        plan: scaffold.plan.clone(),
        written,
        preserved,
    })
}

/// The operator-facing summary: the topology the crate will dial with, what
/// was written, and what was kept.
fn print_report(outcome: &ScaffoldOutcome) {
    println!(
        "scaffolded: {} ({} action(s) across {} connection(s))",
        outcome.crate_root.display(),
        outcome.plan.servable.len(),
        outcome.plan.connections.len()
    );
    for connection in &outcome.plan.connections {
        let node = connection.node.as_deref().map_or_else(
            || "no node (unpinned actions only)".to_owned(),
            |node| format!("node `{node}`"),
        );
        println!("  connection {node}: {}", connection.actions.join(", "));
    }
    for relative in &outcome.written {
        println!("  wrote     {relative}");
    }
    for relative in &outcome.preserved {
        println!("  preserved {relative} (yours — a re-run never rewrites it)");
    }
    if outcome.preserved.is_empty() {
        println!(
            "next: implement each handler in src/handlers.rs — every one fails loudly until you do."
        );
    } else {
        println!(
            "note: an action added to the document since that file was written has no handler \
             in it; the build will name the function you owe."
        );
    }
}
