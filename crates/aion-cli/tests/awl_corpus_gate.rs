//! Every living `.awl` document in the tree must pass `aion awl check`.
//!
//! ## The defect class this closes
//!
//! Task #181 changed what the `agent` marker demands of an action's return
//! type, and four in-tree documents — two production workflows under
//! `workflows/`, a tutorial artifact, a deck example — kept teaching and
//! shipping the refused shape because NOTHING checked them: each dedicated
//! gate (fixtures, guide examples, the embedded assistant document, the doc
//! ```awl blocks) covered its own corpus, and the documents outside every
//! corpus failed silently. A hand-enumerated list of documents is exactly how
//! that happens, so this gate DERIVES its corpus from the tree: every `.awl`
//! file, minus excluded CLASSES, each excluded for a stated reason.
//!
//! ## What is excluded, and why (classes, never file lists)
//!
//! - hidden directories (`.git`, `.claude`, `.aion-*`): system state and
//!   worktrees, not documents of this tree;
//! - `target`, `node_modules`, `build`: build output;
//! - any `fixtures` or `golden` directory: test corpora swept by their own
//!   harnesses — including the explicitly-invalid fixture families, whose
//!   whole purpose is to be refused;
//! - `templates`: scaffolds carrying `{{name}}` placeholders, rendered and
//!   checked by the scaffold e2e tests, unparseable verbatim;
//! - `docs/design`: dated records of what was believed on a date (including
//!   the deliberately-historical `139-drafts` documents and the playtest
//!   probes, several of which exist to be refused) — rewriting them to keep a
//!   gate green would falsify the record;
//! - `gate-logs`: dated gate evidence, same reasoning.
//!
//! Everything else — `workflows/`, the deck and tutorial examples under
//! `docs/authoring/`, every `examples/` document, the embedded assistant
//! document — is a living document a reader is invited to copy or a server
//! actually serves, and it must check clean with THIS tree's checker.
//!
//! The check runs the real `aion` binary built beside this test with the real
//! production argv (`aion awl check FILE`), so both document families
//! (workflow and worker documents) resolve exactly as they do for an
//! operator, and the gate can never quietly diverge from the command it
//! stands in for.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The corpus floor. The tree carries ~37 living documents today; the floor
/// sits just under that so a whole area silently vanishing (the smallest,
/// `workflows/`, holds 3) cannot leave the count above it — a walk that finds
/// fewer has broken (wrong root, over-broad exclusion) and must be red rather
/// than vacuously green.
const CORPUS_FLOOR: usize = 35;

/// Documents that must be IN the corpus — presence anchors, not the corpus
/// itself. One per distinct area (the embedded product document, the
/// production workflows, the deck examples, the tutorial examples, the
/// examples tree), so an exclusion class that silently swallowed a whole
/// area fails here by name rather than hiding under the floor.
const ANCHORS: [&str; 5] = [
    "crates/aion-server/assistant-embed/assistant.awl",
    "workflows/norn-dev/norn_dev.awl",
    "docs/authoring/deck-examples/03-agent-seam.awl",
    "docs/authoring/tutorial-examples/repo_gate_judged.awl",
    "examples/awl-hello/awl_hello.awl",
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The `aion` binary built alongside this test — taken from the test
/// executable's own directory, never from `PATH`, so the gate reads THIS
/// tree's checker and can never quietly compare against an installed version.
fn aion_binary() -> Option<PathBuf> {
    let mut path = std::env::current_exe().ok()?;
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    let candidate = path.join(if cfg!(windows) { "aion.exe" } else { "aion" });
    candidate.is_file().then_some(candidate)
}

/// Whether one path component places its subtree outside the corpus.
fn component_excluded(component: &str) -> bool {
    component.starts_with('.')
        || matches!(
            component,
            "target" | "node_modules" | "build" | "fixtures" | "golden" | "templates"
        )
}

/// Whether a repo-relative path is outside the corpus for an area-level
/// reason (dated records and dated evidence).
fn area_excluded(relative: &Path) -> bool {
    relative.starts_with("docs/design") || relative.starts_with("gate-logs")
}

/// Every corpus document under `root`, repo-relative, sorted.
fn corpus(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    walk(root, root, &mut found);
    found.sort();
    found
}

fn walk(root: &Path, dir: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if path.is_dir() {
            if component_excluded(name) {
                continue;
            }
            let Ok(relative) = path.strip_prefix(root) else {
                continue;
            };
            if area_excluded(relative) {
                continue;
            }
            walk(root, &path, found);
        } else if path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("awl"))
            && let Ok(relative) = path.strip_prefix(root)
        {
            found.push(relative.to_path_buf());
        }
    }
}

/// Runs `aion awl check` on one document, returning `None` on a clean check
/// or the combined output on a refusal.
fn check_failure(binary: &Path, root: &Path, document: &Path) -> Option<String> {
    let output = Command::new(binary)
        .args(["awl", "check"])
        .arg(document)
        .current_dir(root)
        .output();
    let Ok(output) = output else {
        return Some("the aion binary beside this test must be runnable".to_owned());
    };
    if output.status.success() {
        return None;
    }
    Some(format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ))
}

/// The corpus is derived, not hand-listed — so first prove the derivation
/// found a real corpus: at least the floor, containing one known living
/// document from each distinct area. An over-broad exclusion class that
/// silently swallowed an area fails here by the anchor's name.
#[test]
fn the_derived_corpus_is_non_vacuous_and_covers_every_area() {
    let root = repo_root();
    let corpus = corpus(&root);
    assert!(
        corpus.len() >= CORPUS_FLOOR,
        "the corpus walk found only {} documents (floor {CORPUS_FLOOR}); the walk or an \
         exclusion class is broken:\n{corpus:#?}",
        corpus.len()
    );
    for anchor in ANCHORS {
        assert!(
            corpus.iter().any(|path| path == Path::new(anchor)),
            "the corpus must contain `{anchor}`; an exclusion class is swallowing its area"
        );
    }
}

/// THE GATE: every living document checks clean under this tree's checker.
#[test]
fn every_living_awl_document_checks_clean() {
    let root = repo_root();
    // An unmeasured run must be RED, never quietly green: the binary's
    // absence fails here. `cargo test -p aion-cli` builds it.
    let binary = aion_binary();
    assert!(
        binary.is_some(),
        "the `aion` binary is not built beside this test, so no document was checked; run \
         `cargo test -p aion-cli`, which builds it"
    );
    let Some(binary) = binary else {
        return;
    };

    let mut failures = Vec::new();
    let mut checked = 0_usize;
    for document in corpus(&root) {
        match check_failure(&binary, &root, &document) {
            None => checked += 1,
            Some(output) => failures.push(format!("{}:\n{output}", document.display())),
        }
    }
    assert!(
        failures.is_empty(),
        "{} living document(s) fail `aion awl check` — every living `.awl` in the tree must \
         check clean, or it teaches (or serves) a shape the language refuses:\n\n{}",
        failures.len(),
        failures.join("\n")
    );
    assert!(
        checked >= CORPUS_FLOOR,
        "only {checked} documents were checked; the corpus walk is broken"
    );
}

/// The negative control: the gate must be able to fail. A document carrying
/// the exact shape #181 retired (an `agent` action returning bare `String`)
/// is written to a scratch directory and must be REFUSED by the same binary
/// and argv the gate runs — proving the checking path can go red, so a green
/// sweep above is a measurement rather than a mood.
#[test]
fn the_gate_refuses_the_retired_agent_shape() -> Result<(), Box<dyn std::error::Error>> {
    let binary = aion_binary()
        .ok_or("the `aion` binary is not built beside this test; run `cargo test -p aion-cli`")?;
    let scratch = tempfile::tempdir()?;
    let document = scratch.path().join("retired_shape.awl");
    fs::write(
        &document,
        "//! The pre-#181 agent shape: refused, so the corpus gate can fail.\n\
         workflow retired\n  input question: String\n  outcome done: type String, route success\n\n\
         worker assistants\n  action ask(prompt: String) -> String\n    agent\n\n\
         step ask\n  ask(prompt: question) -> reply\n  route done(reply)\n",
    )?;
    let output = Command::new(&binary)
        .args(["awl", "check"])
        .arg(&document)
        .output()?;
    assert!(
        !output.status.success(),
        "the retired shape must be refused, or the corpus sweep cannot fail"
    );
    let rendered = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        rendered.contains("`text: String`, `stop_reason: String`, and `session_id: String`"),
        "the refusal names the canonical record: {rendered}"
    );
    Ok(())
}
