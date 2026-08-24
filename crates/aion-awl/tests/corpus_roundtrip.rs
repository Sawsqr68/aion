//! Every pre-existing AWL document parses to the same result before and after.
//!
//! ## The claim, and why the instrument has to be repo-wide
//!
//! A grammar change is safe only if the documents that already exist still
//! read the same way. That claim is repo-wide, so the instrument must be:
//! this gate DISCOVERS its corpus by extension (`**/*.awl`) over the whole
//! worktree at run time and never enumerates it by hand. Hand enumeration is
//! how a corpus claim comes to cover a fraction of its own population —
//! measured on this tree at the baseline commit, `examples/` and `workflows/`
//! together hold 35 of 431 documents, so a claim written over those two roots
//! would have missed 92% of what it said it covered.
//!
//! ## What is excluded, and why (three classes, never a file list)
//!
//! - `.git` and any other dot-directory: version-control and tool state, not
//!   documents of this tree;
//! - `target`: build output, which carries copies of documents that are
//!   already counted at their source;
//! - `node_modules`: vendored third-party output, same reasoning.
//!
//! NOTHING else is excluded. Fixture corpora, deliberately-invalid documents,
//! dated evidence and design records are all IN — a document that is supposed
//! to be refused must go on being refused with the same message, and that is
//! exactly the kind of regression a "living documents only" sweep cannot see.
//! The sibling gate `crates/aion-cli/tests/awl_corpus_gate.rs` asks a
//! different question (does every LIVING document still pass `aion awl
//! check`) and therefore excludes those classes for its own stated reasons;
//! the two are not substitutes.
//!
//! ## What is recorded
//!
//! For each document, its parse OUTCOME: on success the SHA-256 of the
//! canonical printing (`print(parse(src))`), on failure the exact error line,
//! column, and message. Recording the canonical printing rather than the
//! source means the pin also covers the printer — a grammar change that
//! parsed identically but printed differently would move `aion awl fmt`
//! underneath every document in the tree, and this fails on it.
//!
//! ## The manifest
//!
//! `tests/fixtures/corpus/manifest.txt` was generated at the BASELINE grammar
//! — before the command-declaration surface existed — which is the only order
//! in which a "before and after" claim means anything. Set
//! `AWL_CORPUS_BLESS=1` to rewrite it; never implicit, exactly as the BC2
//! beam goldens are re-blessed.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// Where the pinned baseline lives, relative to this crate's manifest dir.
const MANIFEST: &str = "tests/fixtures/corpus/manifest.txt";

/// The floor the discovery must clear before any comparison is believed.
///
/// A walk that finds almost nothing would agree with a manifest it never
/// read past. The baseline measured 431 documents; the floor sits below the
/// smallest root's contribution so that losing a whole area cannot leave the
/// count above it, and well above zero so a broken root reds instead of
/// passing vacuously.
const DISCOVERY_FLOOR: usize = 400;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map_or_else(
            || unreachable!("crates/aion-awl always has two ancestors"),
            Path::to_path_buf,
        )
}

/// Whether one path component puts its whole subtree outside the corpus.
fn component_excluded(component: &str) -> bool {
    component.starts_with('.') || matches!(component, "target" | "node_modules")
}

/// Every `.awl` document under `root`, repo-relative with `/` separators,
/// sorted.
fn discover(root: &Path) -> Vec<String> {
    let mut found = Vec::new();
    walk(root, root, &mut found);
    found.sort();
    found
}

fn walk(root: &Path, dir: &Path, found: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if path.is_dir() {
            if !component_excluded(name) {
                walk(root, &path, found);
            }
        } else if path.extension().is_some_and(|extension| extension == "awl")
            && let Ok(relative) = path.strip_prefix(root)
        {
            found.push(
                relative
                    .components()
                    .filter_map(|component| component.as_os_str().to_str())
                    .collect::<Vec<_>>()
                    .join("/"),
            );
        }
    }
}

/// What THIS tree's grammar makes of one document, as one manifest line's
/// value.
///
/// Unreadable bytes are an outcome too, and a named one: a document that
/// stops being readable must not look like a document that stopped existing.
fn outcome(source_bytes: Result<String, std::io::Error>) -> String {
    let source = match source_bytes {
        Ok(source) => source,
        Err(error) => return format!("unreadable:{}", one_line(&error.to_string())),
    };
    match aion_awl::parse(&source) {
        Err(error) => format!(
            "err:{}:{}:{}",
            error.span.line,
            error.span.column,
            one_line(&error.message)
        ),
        Ok(document) => {
            let mut hasher = Sha256::new();
            hasher.update(aion_awl::print(&document).as_bytes());
            let mut hex = String::new();
            for byte in hasher.finalize() {
                let _ = write!(hex, "{byte:02x}");
            }
            format!("ok:{hex}")
        }
    }
}

/// A message flattened to one line, so a manifest line is always one record.
fn one_line(text: &str) -> String {
    text.replace(['\n', '\r', '\t'], " ")
}

/// The top-level directory a repo-relative path sits under.
fn root_of(relative: &str) -> &str {
    relative.split('/').next().unwrap_or(relative)
}

/// The manifest as `path -> outcome`, in path order.
fn read_manifest(path: &Path) -> BTreeMap<String, String> {
    let Ok(text) = fs::read_to_string(path) else {
        return BTreeMap::new();
    };
    text.lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| line.split_once('\t'))
        .map(|(path, outcome)| (path.to_owned(), outcome.to_owned()))
        .collect()
}

/// THE GATE. Discover the corpus, read every document with this tree's
/// grammar, and hold the result against the baseline.
///
/// Three distinct failures, never merged into one: a baseline document that
/// now reads differently (a grammar regression), a baseline document that has
/// vanished from discovery (an instrument that stopped looking, or a document
/// deleted without the pin being moved), and a baseline ROOT that discovery no
/// longer reaches at all. Documents this cut ADDS are reported, not refused —
/// the claim is about pre-existing documents.
#[test]
fn every_pre_existing_document_reads_exactly_as_it_did_at_the_baseline() {
    let root = repo_root();
    let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(MANIFEST);
    let discovered = discover(&root);

    announce(&root, &discovered);

    let measured: BTreeMap<String, String> = discovered
        .iter()
        .map(|relative| {
            (
                relative.clone(),
                outcome(fs::read_to_string(root.join(relative))),
            )
        })
        .collect();

    if std::env::var("AWL_CORPUS_BLESS").is_ok() {
        bless(&manifest_path, &measured);
        return;
    }

    let baseline = read_manifest(&manifest_path);
    assert!(
        !baseline.is_empty(),
        "{MANIFEST} carries no records, so this gate would pass vacuously; regenerate it with \
         AWL_CORPUS_BLESS=1 at the grammar you mean to pin"
    );
    assert!(
        discovered.len() >= DISCOVERY_FLOOR,
        "discovery found only {} documents under {} — below the floor of {DISCOVERY_FLOOR}. The \
         walk is broken (wrong root, over-broad exclusion), not the corpus.",
        discovered.len(),
        root.display()
    );

    assert_roots_still_reached(&baseline, &discovered);

    let mut regressions = Vec::new();
    let mut vanished = Vec::new();
    for (relative, expected) in &baseline {
        match measured.get(relative) {
            None => vanished.push(relative.clone()),
            Some(actual) if actual != expected => regressions.push(format!(
                "  {relative}\n    baseline: {expected}\n    now:      {actual}"
            )),
            Some(_) => {}
        }
    }

    report_additions(&baseline, &measured);

    assert!(
        vanished.is_empty(),
        "documents in the baseline are no longer discovered: {vanished:#?}"
    );
    assert!(
        regressions.is_empty(),
        "{} pre-existing document(s) read differently than at the baseline:\n{}",
        regressions.len(),
        regressions.join("\n")
    );

    println!(
        "{} baseline documents re-read identically; {} discovered in total",
        baseline.len(),
        discovered.len()
    );
}

/// Print the discovered corpus: which roots, how many documents in each. The
/// completeness claim is PRINTED, not assumed — a reader of a green run can
/// see what was actually swept.
fn announce(root: &Path, discovered: &[String]) {
    let mut roots: BTreeMap<&str, usize> = BTreeMap::new();
    for relative in discovered {
        *roots.entry(root_of(relative)).or_default() += 1;
    }
    let census = roots
        .iter()
        .map(|(name, count)| format!("{name}/ {count}"))
        .collect::<Vec<_>>()
        .join(", ");
    println!(
        "corpus discovered by extension over {}: {} documents across {} roots — {census}",
        root.display(),
        discovered.len(),
        roots.len()
    );
}

/// Rewrite the pinned baseline. Never implicit: only under
/// `AWL_CORPUS_BLESS=1`, exactly as the BC2 beam goldens are re-blessed.
fn bless(manifest_path: &Path, measured: &BTreeMap<String, String>) {
    let mut text = String::from(
        "# AWL corpus baseline — every `.awl` document discovered repo-wide by extension.\n\
         # Generated by tests/corpus_roundtrip.rs with AWL_CORPUS_BLESS=1.\n\
         # One record per line: <repo-relative path>\\t<outcome>, where an outcome is\n\
         # `ok:<sha256 of print(parse(source))>`, `err:<line>:<column>:<message>`, or\n\
         # `unreadable:<io error>`. Path order.\n",
    );
    for (relative, value) in measured {
        let _ = writeln!(text, "{relative}\t{value}");
    }
    if let Some(parent) = manifest_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::write(manifest_path, text).unwrap_or_else(|error| {
        unreachable!("could not write {}: {error}", manifest_path.display())
    });
    println!("blessed {} records into {MANIFEST}", measured.len());
}

/// A baseline ROOT discovery no longer reaches is an instrument that stopped
/// looking, and it must not read green.
fn assert_roots_still_reached(baseline: &BTreeMap<String, String>, discovered: &[String]) {
    let baseline_roots: BTreeSet<&str> = baseline.keys().map(|path| root_of(path)).collect();
    let discovered_roots: BTreeSet<&str> = discovered.iter().map(|path| root_of(path)).collect();
    let lost: Vec<&&str> = baseline_roots.difference(&discovered_roots).collect();
    assert!(
        lost.is_empty(),
        "the baseline names roots discovery no longer reaches: {lost:?} — an instrument that \
         stopped looking must not read green"
    );
}

/// Documents added since the baseline are REPORTED, not refused: the claim is
/// about pre-existing documents, and a cut that adds fixtures is not a
/// regression in the ones that were already there.
fn report_additions(baseline: &BTreeMap<String, String>, measured: &BTreeMap<String, String>) {
    let added: Vec<&String> = measured
        .keys()
        .filter(|relative| !baseline.contains_key(*relative))
        .collect();
    if added.is_empty() {
        return;
    }
    println!(
        "documents added since the baseline (not part of the before-and-after claim): {}",
        added.len()
    );
    for relative in &added {
        println!("  + {relative}");
    }
}

/// The comparison must be able to FAIL. A manifest value the tree cannot
/// produce has to be rejected by the same code the real records run through,
/// or a green run says only that the loop executed.
#[test]
fn the_comparison_rejects_an_outcome_the_tree_does_not_produce() {
    let root = repo_root();
    let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(MANIFEST);
    let baseline = read_manifest(&manifest_path);
    let Some((relative, recorded)) = baseline.iter().next() else {
        unreachable!("{MANIFEST} must carry records for this control to mean anything");
    };
    let actual = outcome(fs::read_to_string(root.join(relative)));
    assert_eq!(
        &actual, recorded,
        "the control's own reference record must match before it can prove anything"
    );
    assert_ne!(
        actual, "ok:0000000000000000000000000000000000000000000000000000000000000000",
        "a fabricated outcome must not compare equal to a measured one"
    );
}

/// The discovery walk is the load-bearing half of the completeness claim, so
/// its exclusion rule is tested against known answers rather than trusted.
#[test]
fn the_walk_excludes_exactly_the_three_named_classes() {
    assert!(component_excluded(".git"));
    assert!(component_excluded(".claude"));
    assert!(component_excluded("target"));
    assert!(component_excluded("node_modules"));
    assert!(!component_excluded("fixtures"));
    assert!(!component_excluded("docs"));
    assert!(!component_excluded("gate-logs"));
    assert!(!component_excluded("examples"));
    assert_eq!(root_of("crates/aion-awl/x.awl"), "crates");
    assert_eq!(root_of("standalone.awl"), "standalone.awl");
}
