//! No living markdown may teach the retired agent-seam doctrine.
//!
//! ## The defect class this closes
//!
//! Task #181 replaced the `agent` marker's bare-`String` return with the
//! canonical agent-outcome record and killed the "parse it downstream"
//! doctrine — and then the R2 review found two production workflow READMEs
//! (`workflows/norn-fleet`, `workflows/norn-dev`) still teaching the retired
//! shape, citing the superseded rule as "enforced by the checker". Prose has
//! no compiler: a sentence stating a dead rule stays green forever unless
//! something reads it. This gate reads it — the retired phrases themselves,
//! swept over every living markdown file, with the scope DERIVED from the
//! tree the same way the sibling `awl_corpus_gate` derives its corpus.
//!
//! ## What is excluded, and why (classes, never file lists)
//!
//! - hidden directories, `target`, `node_modules`, `build`: system and build
//!   output;
//! - `fixtures` and `golden` directories: harness-owned corpora — the fixture
//!   MANIFEST legitimately QUOTES retired text as the history of a refusal;
//! - `templates`: scaffolds with their own e2e coverage;
//! - `docs/design`, `docs/archive`, `docs/evidence`, `gate-logs`: dated
//!   records of what was believed or measured on a date — rewriting them to
//!   keep a gate green would falsify the record (the #98 ruling document,
//!   for example, must keep stating the rule it ruled).
//!
//! ## What is banned
//!
//! Matching is over NORMALIZED text (lowercased, backticks stripped) so
//! `` one `String` in `` and `one String in` are one phrase. Each banned
//! phrase is a spelling of the retired doctrine, not a word that could occur
//! innocently — the sweep proves the whole living set clean of these
//! spellings on every run.

use std::fs;
use std::path::{Path, PathBuf};

/// The retired-doctrine spellings, over normalized text. Each entry names a
/// way the pre-#181 shape was actually taught in this repository.
const BANNED_PHRASES: [&str; 4] = [
    // The dead rule itself — the checker's pre-#181 remedy text.
    "parse it downstream",
    // The seam shape as both workflow READMEs and the deck taught it.
    "one string in, one string out",
    // The variant with the parameter spelled out.
    "string reply out",
    // The worker-doc variant ("ONE prompt string in, one terminal-text
    // string out").
    "terminal-text string out",
];

/// The scope floor. The tree carries ~143 living markdown files today; a walk
/// that finds far fewer has broken and must be red, not vacuously green.
const SCOPE_FLOOR: usize = 145;

/// Files that must be IN scope — presence anchors from distinct areas,
/// including the two READMEs this gate exists because of.
const ANCHORS: [&str; 5] = [
    "workflows/norn-fleet/README.md",
    "workflows/norn-dev/README.md",
    "docs/workers/AGENT-WORKERS.md",
    "docs/authoring/AWL.md",
    "examples/assistant/resources/AWL-REFERENCE.md",
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Lowercase with backticks stripped, so markdown code spans cannot hide a
/// phrase from the sweep.
fn normalize(text: &str) -> String {
    text.to_lowercase().replace('`', "")
}

fn component_excluded(component: &str) -> bool {
    component.starts_with('.')
        || matches!(
            component,
            "target" | "node_modules" | "build" | "fixtures" | "golden" | "templates"
        )
}

fn area_excluded(relative: &Path) -> bool {
    relative.starts_with("docs/design")
        || relative.starts_with("docs/archive")
        || relative.starts_with("docs/evidence")
        || relative.starts_with("gate-logs")
}

/// Every living markdown file under `root`, repo-relative, sorted.
fn scope(root: &Path) -> Vec<PathBuf> {
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
            .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
            && let Ok(relative) = path.strip_prefix(root)
        {
            found.push(relative.to_path_buf());
        }
    }
}

/// The scope is derived, not hand-listed — prove the derivation found the
/// real living set, including the two documents this gate exists because of.
#[test]
fn the_derived_scope_is_non_vacuous_and_covers_the_documents_that_slipped() {
    let root = repo_root();
    let scope = scope(&root);
    assert!(
        scope.len() >= SCOPE_FLOOR,
        "the markdown walk found only {} files (floor {SCOPE_FLOOR}); the walk or an \
         exclusion class is broken",
        scope.len()
    );
    for anchor in ANCHORS {
        assert!(
            scope.iter().any(|path| path == Path::new(anchor)),
            "the scope must contain `{anchor}`; an exclusion class is swallowing its area"
        );
    }
}

/// THE GATE: no living markdown teaches a retired-doctrine phrase.
#[test]
fn no_living_markdown_teaches_the_retired_doctrine() -> Result<(), Box<dyn std::error::Error>> {
    let root = repo_root();
    let mut violations = Vec::new();
    let mut swept = 0_usize;
    for relative in scope(&root) {
        let text = normalize(&fs::read_to_string(root.join(&relative))?);
        for phrase in BANNED_PHRASES {
            if text.contains(phrase) {
                violations.push(format!("{}: contains {phrase:?}", relative.display()));
            }
        }
        swept += 1;
    }
    assert!(
        violations.is_empty(),
        "living documentation still teaches the retired agent-seam doctrine — every phrase \
         below was superseded by #181's canonical agent-outcome record:\n{}",
        violations.join("\n")
    );
    assert!(
        swept >= SCOPE_FLOOR,
        "only {swept} files were swept; the walk is broken"
    );
    Ok(())
}

/// The negative control: the matcher must be able to fail, through the same
/// normalization the sweep uses — a backticked, mixed-case spelling of each
/// banned phrase must still match.
#[test]
fn the_matcher_finds_each_banned_phrase_through_markdown_disguises() {
    let disguises = [
        "…so you can Parse It Downstream if needed",
        "the seam is one `String` in, one `String` out",
        "one String prompt in, one String reply out",
        "ONE prompt string in, one terminal-text String out",
    ];
    for (phrase, disguise) in BANNED_PHRASES.iter().zip(disguises) {
        assert!(
            normalize(disguise).contains(phrase),
            "the normalizer must surface {phrase:?} inside {disguise:?}"
        );
    }
    let clean = normalize(
        "an agent action returns the canonical agent-outcome record: text, stop_reason, and session_id",
    );
    assert!(
        BANNED_PHRASES.iter().all(|phrase| !clean.contains(phrase)),
        "the current doctrine must not trip the matcher"
    );
}
