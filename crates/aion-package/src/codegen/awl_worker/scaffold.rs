//! The generator entry point: one compiled worker contract in, one Cargo
//! crate's worth of source out.
//!
//! Nothing here reads or writes the filesystem. The whole file set is
//! rendered before the caller touches a directory, so a refusal leaves the
//! author's tree exactly as it was.

use crate::contract::WorkerContract;

use super::error::AwlScaffoldError;
use super::plan::{ConnectionPlan, plan};
use super::{declaration_rs, handlers_rs, main_rs, manifest};

/// Where the generated crate takes the aion SDK crates from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AionDependency {
    /// The published crates at one exact version.
    ///
    /// The version is the generating `aion` binary's own: it is the only
    /// version whose SDK surface the emitted code was written against, so
    /// pinning anything else would be a guess about an API the generator
    /// never saw.
    Version(String),
    /// A local checkout's `crates/` directory, as the generated crate's
    /// `Cargo.toml` should spell it (relative to the crate root, or
    /// absolute).
    ///
    /// This is what an author working inside the aion tree needs, and what
    /// makes a generated crate compile against an SDK that is not published
    /// yet.
    Path(String),
}

/// The directory the generated worker resolves the document's `schema("…")`
/// imports against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentRoot {
    /// A path relative to the generated crate root, emitted as a
    /// `CARGO_MANIFEST_DIR` concatenation so the crate finds its document
    /// wherever the tree is checked out.
    InCrateTree(String),
    /// An absolute path, emitted verbatim — for a document in a tree the
    /// generated crate shares nothing but the filesystem root with, where no
    /// relative path is meaningful.
    Absolute(String),
}

impl DocumentRoot {
    /// The path text, whichever form it takes.
    #[must_use]
    pub fn text(&self) -> &str {
        match self {
            Self::InCrateTree(path) | Self::Absolute(path) => path,
        }
    }
}

/// Who owns a generated file once it exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileOwnership {
    /// Regenerated from the document every time; a hand-edit is overwritten.
    Generated,
    /// Written ONCE and never rewritten — the author's file from then on.
    Author,
}

/// One file of the scaffold: where it goes, what is in it, and who owns it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScaffoldedFile {
    /// Path relative to the generated crate's root.
    pub relative: String,
    /// The fully rendered contents.
    pub contents: String,
    /// Whether re-running the scaffold rewrites this file.
    pub ownership: FileOwnership,
}

/// What to generate: one queue's compiled contract, plus how the emitted
/// crate reaches the document and the SDK.
#[derive(Debug, Clone)]
pub struct AwlWorkerScaffold<'a> {
    /// The compiled `worker` contract — the ONE source of the schemas the
    /// generated worker advertises.
    pub contract: &'a WorkerContract,
    /// Cargo package (and binary) name for the generated crate.
    pub crate_name: &'a str,
    /// The document's path as `src/declaration.rs` must spell it for
    /// `include_str!` — relative to that file, using `/` separators.
    pub document_include: &'a str,
    /// The document's DIRECTORY, which the emitted code uses to resolve the
    /// document's `schema("…")` imports exactly as the deploy path does.
    pub document_directory: &'a DocumentRoot,
    /// The document's file name, for the generated documentation.
    pub document_name: &'a str,
    /// Where the generated `Cargo.toml` takes the aion crates from.
    pub dependencies: &'a AionDependency,
}

/// A rendered scaffold: the connection plan it was derived from, and the
/// files that realise it.
#[derive(Debug, Clone)]
pub struct WorkerScaffold {
    /// The plan — one connection per node — the emitted crate implements.
    pub plan: ConnectionPlan,
    /// Every file to write, in a deterministic order.
    pub files: Vec<ScaffoldedFile>,
}

/// Renders the worker scaffold for one compiled queue contract.
///
/// The emitted crate advertises a wire descriptor for every action it
/// registers, opens one connection per node, omits every server-executed
/// bodied action, and fails each un-implemented activity loudly. All four are
/// the difference between a skeleton that serves a queue and one that is
/// refused on the dial.
///
/// # Errors
///
/// Returns an [`AwlScaffoldError`] when the queue has no action for an
/// out-of-band worker, when an action cannot name a Rust function, when the
/// crate name or document paths are unusable, or when a declared schema
/// cannot be rendered into the handler documentation.
pub fn scaffold_awl_worker(
    request: &AwlWorkerScaffold<'_>,
) -> Result<WorkerScaffold, AwlScaffoldError> {
    validate_crate_name(request.crate_name)?;
    validate_document_path(request.document_include, "document")?;
    validate_document_path(request.document_directory.text(), "directory")?;
    let plan = plan(request.contract)?;
    let files = vec![
        ScaffoldedFile {
            relative: "Cargo.toml".to_owned(),
            contents: manifest::emit(request, &plan),
            ownership: FileOwnership::Generated,
        },
        ScaffoldedFile {
            relative: "src/main.rs".to_owned(),
            contents: main_rs::emit(request, &plan),
            ownership: FileOwnership::Generated,
        },
        ScaffoldedFile {
            relative: "src/declaration.rs".to_owned(),
            contents: declaration_rs::emit(request, &plan),
            ownership: FileOwnership::Generated,
        },
        ScaffoldedFile {
            relative: "src/handlers.rs".to_owned(),
            contents: handlers_rs::emit(&plan)?,
            ownership: FileOwnership::Author,
        },
    ];
    Ok(WorkerScaffold { plan, files })
}

/// Refuses a package name Cargo would not accept, before it reaches a
/// `Cargo.toml` that fails to parse.
fn validate_crate_name(crate_name: &str) -> Result<(), AwlScaffoldError> {
    let usable = !crate_name.is_empty()
        && crate_name.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        });
    if usable {
        return Ok(());
    }
    Err(AwlScaffoldError::CrateNameInvalid {
        crate_name: crate_name.to_owned(),
    })
}

/// Refuses an empty document path: `include_str!("")` and a manifest dir with
/// nothing appended both fail at build time, far from the cause.
fn validate_document_path(path: &str, role: &'static str) -> Result<(), AwlScaffoldError> {
    if path.is_empty() {
        return Err(AwlScaffoldError::DocumentPathEmpty { role });
    }
    Ok(())
}
