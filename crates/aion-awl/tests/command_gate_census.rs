//! R6: the diff-census gate-entry family, proven EXPRESSIBLE at the bytes.
//!
//! ⛔ EXPRESSIBILITY PROOF, NOT DEPLOYMENT. Nothing here retires a script,
//! converts a rig, or moves a deployed document. What it establishes is
//! narrower and load-bearing: that ONE declared command shape plus FOUR
//! parameter sets emit exactly the command lines the four primary rigs' gate
//! entries actually run — compared against the specimen bytes carried in this
//! tree, at the digests the rig's own table pins, never against remembered
//! text.
//!
//! # How a comparison is made honest
//!
//! Each fixture cites its source REPO, PATH and COMMIT, and the specimen file
//! carried under `.fleet/fixtures/gate-scripts/` with that file's SHA-256.
//! Before comparing, this gate RE-HASHES the carried specimen against the
//! cited digest and RE-READS the cited line — so a specimen edited after the
//! fixture was written fails here rather than passing against a stale copy.
//!
//! # Normalizations
//!
//! Declared one by one in each fixture's own header, never applied silently.
//! There are exactly two, and both are about the shell rather than about the
//! command: the specimen line is a `changed=$(…)` assignment, so the compared
//! text is the substitution's body; and `"${BASE}"` is one shell word whose
//! quotes exist only to survive word splitting, which this surface does not
//! perform.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use aion_awl::command::{ArgumentValue, ParameterSet, check_command, emit};
use sha2::{Digest, Sha256};

/// The four parameter sets, by fixture stem.
const FIXTURES: [&str; 4] = ["haem-wall", "haem-wall-r2", "seqlegs-impl", "speech-1"];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map_or_else(
            || unreachable!("crates/aion-awl always has two ancestors"),
            Path::to_path_buf,
        )
}

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/command/gate-census")
}

/// One parsed fixture: its cited provenance, its parameters, its expected argv.
struct Fixture {
    headers: BTreeMap<String, String>,
    params: Vec<(String, String)>,
    argv: Vec<String>,
}

fn read_fixture(stem: &str) -> Fixture {
    let path = fixture_dir().join(format!("{stem}.params"));
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => unreachable!("{} is not readable: {error}", path.display()),
    };
    let mut headers = BTreeMap::new();
    let mut params = Vec::new();
    let mut argv = Vec::new();
    for line in text.lines() {
        if let Some(header) = line.strip_prefix("# ") {
            if let Some((key, value)) = header.split_once(": ") {
                if key
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '-')
                {
                    headers.insert(key.to_owned(), value.trim().to_owned());
                }
            }
        } else if let Some(binding) = line.strip_prefix("param ") {
            if let Some((name, value)) = binding.split_once(" = ") {
                params.push((name.trim().to_owned(), value.trim().to_owned()));
            }
        } else if let Some(element) = line.strip_prefix("argv ") {
            argv.push(element.to_owned());
        }
    }
    Fixture {
        headers,
        params,
        argv,
    }
}

fn header<'a>(fixture: &'a Fixture, key: &str, stem: &str) -> &'a str {
    match fixture.headers.get(key) {
        Some(value) => value.as_str(),
        None => unreachable!("{stem}.params carries no `{key}:` header"),
    }
}

fn sha256_of(path: &Path) -> String {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) => unreachable!("{} is not readable: {error}", path.display()),
    };
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let mut hex = String::new();
    for byte in hasher.finalize() {
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

/// The declared shape, parsed and checked.
fn shape_source() -> String {
    let path = fixture_dir().join("shape.awl");
    match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => unreachable!("{} is not readable: {error}", path.display()),
    }
}

/// EVERY fixture cites bytes that are still there. The digest is re-computed
/// from the carried specimen, so a specimen edited after the fixture was
/// written fails here instead of passing against a stale copy.
#[test]
fn every_fixture_cites_a_specimen_that_is_still_at_its_pinned_digest() {
    let root = repo_root();
    for stem in FIXTURES {
        let fixture = read_fixture(stem);
        for required in [
            "SOURCE-REPO",
            "SOURCE-PATH",
            "SOURCE-COMMIT",
            "SOURCE-SHA256",
            "SPECIMEN-FILE",
            "SPECIMEN-LINE",
        ] {
            assert!(
                !header(&fixture, required, stem).is_empty(),
                "{stem}.params must cite `{required}`"
            );
        }
        let specimen = root.join(header(&fixture, "SPECIMEN-FILE", stem));
        assert!(
            specimen.is_file(),
            "{stem} cites {} which is not in the tree",
            specimen.display()
        );
        assert_eq!(
            sha256_of(&specimen),
            header(&fixture, "SOURCE-SHA256", stem),
            "{stem}'s carried specimen has drifted from the digest the fixture cites; the \
             comparison below would be against bytes nobody pinned"
        );
    }
}

/// The claim, at the bytes: the emitted argv IS the specimen's own command
/// line, with the fixture's declared normalizations and no others.
#[test]
fn one_shape_and_four_parameter_sets_emit_the_four_rigs_census_command_lines() {
    let root = repo_root();
    let source = shape_source();
    let parsed = match aion_awl::parse(&source) {
        Ok(parsed) => parsed,
        Err(error) => unreachable!("the shape must parse: {}", error.message),
    };
    let diagnostics = aion_awl::check_in(&parsed, &fixture_dir());
    assert!(
        diagnostics.is_empty(),
        "the shape must check clean: {:#?}",
        diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
    let Some(decl) = parsed.commands.first() else {
        unreachable!("shape.awl declares one command");
    };
    let checked = match check_command(decl) {
        Ok(checked) => checked,
        Err(refusals) => unreachable!("the shape must check: {refusals:#?}"),
    };

    let mut failures = Vec::new();
    for stem in FIXTURES {
        let fixture = read_fixture(stem);

        // Re-read the CITED line of the CITED specimen. The fixture's expected
        // argv is then held against bytes read from the tree on this run.
        let specimen = root.join(header(&fixture, "SPECIMEN-FILE", stem));
        let text = fs::read_to_string(&specimen).unwrap_or_default();
        let line_number: usize = header(&fixture, "SPECIMEN-LINE", stem).parse().unwrap_or(0);
        let Some(line) = text.lines().nth(line_number.saturating_sub(1)) else {
            failures.push(format!(
                "{stem}: {} has no line {line_number}",
                specimen.display()
            ));
            continue;
        };
        let Some(command_text) = command_substitution_body(line) else {
            failures.push(format!(
                "{stem}: line {line_number} is not a `changed=$(…)` census line: {line:?}"
            ));
            continue;
        };

        let mut parameters = ParameterSet::new();
        for (name, value) in &fixture.params {
            parameters = parameters.with(name, ArgumentValue::scalar(value));
        }
        let emitted = match emit(checked, &parameters) {
            Ok(resolved) => resolved.argv,
            Err(refusal) => {
                failures.push(format!("{stem}: emit refused — {}", refusal.rendered()));
                continue;
            }
        };

        if emitted != fixture.argv {
            failures.push(format!(
                "{stem}: the shape emitted {emitted:?}, the fixture expects {:?}",
                fixture.argv
            ));
            continue;
        }

        // The normalized specimen: shell quoting removed, `${BASE}` expanded
        // from the script's own assignment at the cited BASE-LINE.
        let base_line: usize = header(&fixture, "BASE-LINE", stem).parse().unwrap_or(0);
        let Some(pin) = base_assignment(&text, base_line) else {
            failures.push(format!(
                "{stem}: line {base_line} is not a `BASE=…` assignment"
            ));
            continue;
        };
        let normalized = normalize(command_text, &pin);
        if normalized != emitted {
            failures.push(format!(
                "{stem}: the specimen's own command line normalizes to {normalized:?}, the shape \
                 emits {emitted:?}\n  specimen line {line_number}: {line}\n  base line \
                 {base_line}: {pin}"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// THE CONTROL. The normalizer must be able to disagree, or the comparison
/// above says only that two functions ran.
#[test]
fn the_normalizer_rejects_a_command_line_that_is_not_the_census() {
    let normalized = normalize("git diff --stat \"${BASE}\"..HEAD", "abc");
    assert_eq!(normalized, vec!["git", "diff", "--stat", "abc..HEAD"]);
    assert_ne!(
        normalized,
        vec!["git", "diff", "--name-only", "abc..HEAD"],
        "a different option must not normalize to the same argv"
    );
    assert_eq!(command_substitution_body("no substitution here"), None);
    assert_eq!(
        command_substitution_body("changed=$(git diff --name-only \"${BASE}\"..HEAD)"),
        Some("git diff --name-only \"${BASE}\"..HEAD")
    );
    assert_eq!(
        base_assignment("BASE=abc  # note\n", 1),
        Some("abc".to_owned())
    );
    assert_eq!(base_assignment("changed=x\n", 1), None);
}

/// The body of a `changed=$(…)` census line — NORMALIZATION 1, declared in
/// every fixture header.
fn command_substitution_body(line: &str) -> Option<&str> {
    let rest = line.trim().strip_prefix("changed=$(")?;
    rest.strip_suffix(')')
}

/// The pin a `BASE=…` assignment carries, stopping at a trailing comment.
fn base_assignment(text: &str, line_number: usize) -> Option<String> {
    let line = text.lines().nth(line_number.saturating_sub(1))?;
    let value = line.trim().strip_prefix("BASE=")?;
    Some(
        value
            .split_once('#')
            .map_or(value, |(before, _)| before)
            .trim()
            .to_owned(),
    )
}

/// A shell command line as argv elements — NORMALIZATION 2, declared in every
/// fixture header.
///
/// Splits on unquoted whitespace, drops the quotes that exist only to survive
/// that splitting, and expands `${BASE}` / `$BASE` from the script's own
/// assignment. No other transformation: anything else the specimen carries
/// would show up as a difference, which is the point.
fn normalize(command: &str, base: &str) -> Vec<String> {
    let mut elements = Vec::new();
    let mut current = String::new();
    let mut started = false;
    let mut quote: Option<char> = None;
    let mut characters = command.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '"' | '\'' if quote.is_none() => {
                quote = Some(character);
                started = true;
            }
            character if Some(character) == quote => quote = None,
            character if character.is_whitespace() && quote.is_none() => {
                if started {
                    elements.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            '$' => {
                started = true;
                let name: String = if characters.peek() == Some(&'{') {
                    characters.next();
                    let mut name = String::new();
                    for inner in characters.by_ref() {
                        if inner == '}' {
                            break;
                        }
                        name.push(inner);
                    }
                    name
                } else {
                    let mut name = String::new();
                    while let Some(&inner) = characters.peek() {
                        if !inner.is_alphanumeric() && inner != '_' {
                            break;
                        }
                        name.push(inner);
                        characters.next();
                    }
                    name
                };
                if name == "BASE" {
                    current.push_str(base);
                } else {
                    current.push('$');
                    current.push_str(&name);
                }
            }
            character => {
                started = true;
                current.push(character);
            }
        }
    }
    if started {
        elements.push(current);
    }
    elements
}
