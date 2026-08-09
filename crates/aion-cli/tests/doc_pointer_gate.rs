//! Every repository path the user-facing documentation points at must exist.
//!
//! ## What this gate exists to catch
//!
//! `doc_command_gate.py` reads the commands the documentation names and asks
//! the binary whether they resolve. It reads no LINKS, so a documentation
//! tree that was re-organised leaves every sentence pointing at the old home
//! silently false — and that is not hypothetical: when this gate was first
//! run it found twenty-four dead pointers, most of them at a `docs/guides/`
//! directory that has not existed for some time, in the README files
//! `aion new` writes into a new operator's project.
//!
//! A dead pointer in a template is worse than a dead pointer in a design
//! note, because the operator meets it on their first day and has no way to
//! know the document moved rather than never existed.
//!
//! ## What it establishes, and what it does not
//!
//! ESTABLISHED: for every file in the scanned set, every
//! `https://github.com/ablative-io/aion/blob/main/<path>` link and every
//! relative markdown link names a path that exists in this repository.
//!
//! NOT ESTABLISHED: that the document found there says what the sentence
//! around the link claims. A gate that claimed otherwise would be a mood, not
//! a measurement.
//!
//! ## What is scanned, and what is deliberately not
//!
//! Scanned: everything `aion new` writes into a project, the authoring,
//! worker, and operations guides, the assistant's resource documents, the
//! crate READMEs published to crates.io, and the example READMEs.
//!
//! NOT scanned: `docs/design/**` and `docs/archive/**`. Those are records of
//! what was believed on a date. A design note from June that cites a path
//! since moved is an accurate record of June; rewriting it to keep a link
//! gate green would falsify the record to protect the gate.

use std::fs;
use std::path::{Path, PathBuf};

/// The GitHub form the documentation uses for repository files.
const BLOB_PREFIX: &str = "https://github.com/ablative-io/aion/blob/main/";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map_or_else(
            || unreachable!("crates/aion-cli always has two ancestors"),
            Path::to_path_buf,
        )
}

/// One pointer, and where it was written.
#[derive(Debug, PartialEq, Eq)]
struct Pointer {
    /// The file the pointer was written in, relative to the repository root.
    source: String,
    /// The target exactly as written.
    written: String,
    /// The repository path it resolves to.
    resolved: PathBuf,
}

/// Files under `directory` (recursively) whose extension is in `extensions`.
fn files_under(directory: &Path, extensions: &[&str]) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = fs::read_dir(directory) else {
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            found.extend(files_under(&path, extensions));
        } else if path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extensions.contains(&extension))
        {
            found.push(path);
        }
    }
    found.sort();
    found
}

/// Files matching `<root>/<child>/<name>` for every immediate child of
/// `root` — the crate and example README case.
fn files_in_each_child(root: &Path, name: &str) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = fs::read_dir(root) else {
        return found;
    };
    for entry in entries.flatten() {
        let candidate = entry.path().join(name);
        if candidate.is_file() {
            found.push(candidate);
        }
    }
    found.sort();
    found
}

/// The scanned set. See the module note for what is excluded and why.
fn scanned(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    files.extend(files_under(
        &root.join("crates/aion-cli/templates"),
        &["md", "toml", "gleam", "awl"],
    ));
    files.extend(files_under(&root.join("docs/authoring"), &["md"]));
    files.extend(files_under(&root.join("docs/workers"), &["md"]));
    files.extend(files_under(&root.join("docs/operations"), &["md"]));
    files.extend(files_under(
        &root.join("examples/assistant/resources"),
        &["md"],
    ));
    files.extend(files_in_each_child(&root.join("crates"), "README.md"));
    files.extend(files_in_each_child(&root.join("examples"), "README.md"));
    for named in [
        "README.md",
        "docs/README.md",
        "docs/GETTING-STARTED.md",
        "gleam/aion_flow/README.md",
    ] {
        let path = root.join(named);
        if path.is_file() {
            files.push(path);
        }
    }
    files
}

/// A character that may appear in a repository path inside a bare URL.
fn path_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '/' | '.' | '-' | '_')
}

/// Whether a markdown link target names something in this repository at all.
///
/// A scheme, an anchor-only link, and a template placeholder are each a real
/// thing that is not a repository path; treating any of them as one would
/// make the gate fail over correct documents, and a gate that cries wolf gets
/// deleted.
fn is_repository_target(target: &str) -> bool {
    !(target.contains("://")
        || target.starts_with('#')
        || target.starts_with("mailto:")
        || target.contains("{{"))
}

/// Every pointer written in `text`, resolved against `source`'s directory.
fn pointers_in(root: &Path, source: &Path, text: &str) -> Vec<Pointer> {
    let relative = source
        .strip_prefix(root)
        .unwrap_or(source)
        .to_string_lossy()
        .into_owned();
    let directory = source.parent().unwrap_or(root);
    let mut pointers = Vec::new();

    let mut rest = text;
    while let Some(index) = rest.find(BLOB_PREFIX) {
        let tail = &rest[index + BLOB_PREFIX.len()..];
        let end = tail
            .find(|character| !path_character(character))
            .unwrap_or(tail.len());
        let written = &tail[..end];
        if !written.is_empty() {
            pointers.push(Pointer {
                source: relative.clone(),
                written: format!("{BLOB_PREFIX}{written}"),
                resolved: root.join(written),
            });
        }
        rest = &tail[end..];
    }

    let mut rest = text;
    while let Some(index) = rest.find("](") {
        let tail = &rest[index + 2..];
        let Some(end) = tail.find(')') else {
            break;
        };
        let written = tail[..end].trim();
        if is_repository_target(written) && !written.is_empty() {
            let target = written.split('#').next().unwrap_or_default();
            if !target.is_empty() {
                pointers.push(Pointer {
                    source: relative.clone(),
                    written: written.to_owned(),
                    resolved: directory.join(target),
                });
            }
        }
        rest = &tail[end..];
    }

    pointers
}

fn all_pointers(root: &Path) -> Vec<Pointer> {
    let mut pointers = Vec::new();
    for file in scanned(root) {
        let Ok(text) = fs::read_to_string(&file) else {
            continue;
        };
        pointers.extend(pointers_in(root, &file, &text));
    }
    pointers
}

#[test]
fn every_documentation_pointer_names_a_file_that_exists() {
    let root = repo_root();
    let pointers = all_pointers(&root);
    assert!(
        pointers.len() > 100,
        "only {} pointers were read; the scanned set has collapsed and a green run would mean \
         nothing",
        pointers.len()
    );

    let mut dead: Vec<String> = pointers
        .iter()
        .filter(|pointer| !pointer.resolved.exists())
        .map(|pointer| format!("  {} -> {}", pointer.source, pointer.written))
        .collect();
    dead.sort();
    assert!(
        dead.is_empty(),
        "{} documentation pointer(s) name a path that does not exist:\n{}",
        dead.len(),
        dead.join("\n")
    );
}

/// The templates `aion new` writes are the first documentation an operator
/// reads, so they are asserted to carry pointers at all. Without this, a
/// template whose links were all deleted would pass the gate above by having
/// nothing left to check.
#[test]
fn the_project_templates_still_point_somewhere() {
    let root = repo_root();
    let templates = root.join("crates/aion-cli/templates");
    let pointers: Vec<Pointer> = all_pointers(&root)
        .into_iter()
        .filter(|pointer| pointer.source.starts_with("crates/aion-cli/templates"))
        .collect();
    assert!(
        templates.is_dir(),
        "the template directory must exist for this gate to mean anything"
    );
    assert!(
        pointers.len() > 5,
        "the project templates carry only {} documentation pointer(s)",
        pointers.len()
    );
}

/// The extractor is the load-bearing half of this gate, so it is measured
/// against known answers rather than trusted. An extractor that quietly
/// returned nothing would make the gate above unable to fail.
#[test]
fn the_extractor_reads_both_pointer_forms_and_skips_what_is_not_a_path() {
    let root = repo_root();
    let source = root.join("docs/authoring/AWL.md");
    let text = "\
See [the guide](../workers/DECLARED-COMMANDS.md) and
<https://github.com/ablative-io/aion/blob/main/docs/authoring/AWL.md>, plus
[the site](https://example.com/x), [a section](#anchor), and
[a placeholder]({{name}}.awl).
";
    let pointers = pointers_in(&root, &source, text);
    let written: Vec<&str> = pointers
        .iter()
        .map(|pointer| pointer.written.as_str())
        .collect();
    assert_eq!(
        written,
        vec![
            "https://github.com/ablative-io/aion/blob/main/docs/authoring/AWL.md",
            "../workers/DECLARED-COMMANDS.md",
        ],
        "the extractor must read both real forms and skip a scheme, an anchor, and a placeholder"
    );
    assert!(
        pointers.iter().all(|pointer| pointer.resolved.exists()),
        "both real pointers in the fixture resolve"
    );
}

/// The gate must be able to fail. A pointer at a path that cannot exist has
/// to be reported by the same resolution the real pointers run through.
#[test]
fn the_gate_rejects_a_pointer_at_a_path_that_is_not_there() {
    let root = repo_root();
    let source = root.join("docs/authoring/AWL.md");
    let text = "[gone](../guides/workflows.md) and \
                <https://github.com/ablative-io/aion/blob/main/docs/guides/workflows.md>\n";
    let pointers = pointers_in(&root, &source, text);
    assert_eq!(pointers.len(), 2, "both forms must be extracted");
    assert!(
        pointers.iter().all(|pointer| !pointer.resolved.exists()),
        "a pointer into the departed docs/guides/ directory must resolve to nothing"
    );
}
