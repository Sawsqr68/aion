//! R5: the absorbed Just subset, held against Just's own CC0 test suite.
//!
//! # Why an own parser needs someone else's tests
//!
//! AWL does not embed Just. It has its own parser over a SUBSET of Just's
//! authoring shapes, which is the right call and also the risk: the day
//! `bar='baz'` stops meaning what a justfile author expects, nothing in this
//! repository would notice. So the conformance cases come from Just's own
//! suite — CC0-1.0, so carrying its specimens is unencumbered — and a drift
//! against author expectation becomes a red test.
//!
//! Every fixture cites the suite by REPO, COMMIT, FILE and that file's
//! SHA-256, and quotes the specimen it was derived from. A fixture for a shape
//! the suite does not carry says so in its own header and gives the reason.
//!
//! Suite cases exercising a REFUSED form are carried as refusal fixtures, not
//! skipped: a refused list asserted only in prose is a list that quietly grows
//! shorter.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use aion_awl::command::{
    ABSORBED_PRODUCTIONS, CommandCause, REFUSED_FORMS, SUITE_COMMIT, SUITE_LICENSE, SUITE_REPO,
};

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/command/just-subset")
}

/// One fixture: its header claims and its document.
struct Fixture {
    name: String,
    header: Vec<String>,
    source: String,
}

impl Fixture {
    /// The value of a `//! KEY: …` header line, if present.
    fn claim(&self, key: &str) -> Option<&str> {
        let prefix = format!("{key}: ");
        self.header
            .iter()
            .find_map(|line| line.strip_prefix(&prefix))
    }

    /// The whole header as one string, for substring claims.
    fn header_text(&self) -> String {
        self.header.join("\n")
    }
}

/// Every fixture in the corpus, discovered rather than enumerated.
fn fixtures() -> Vec<Fixture> {
    let dir = fixture_dir();
    let Ok(entries) = fs::read_dir(&dir) else {
        unreachable!("{} is not readable", dir.display());
    };
    let mut found: Vec<Fixture> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let name = path.file_name()?.to_str()?.to_owned();
            if path.extension()? != "awl" {
                return None;
            }
            let source = fs::read_to_string(&path).ok()?;
            let header = source
                .lines()
                .take_while(|line| line.starts_with("//!"))
                .map(|line| line.trim_start_matches("//!").trim().to_owned())
                .collect();
            Some(Fixture {
                name,
                header,
                source,
            })
        })
        .collect();
    found.sort_by(|a, b| a.name.cmp(&b.name));
    found
}

/// Every refusal the surface produced for a document.
fn refusals(source: &str) -> Vec<String> {
    match aion_awl::parse(source) {
        Err(error) => vec![error.message],
        Ok(parsed) => aion_awl::check_in(&parsed, std::env::temp_dir().as_path())
            .into_iter()
            .map(|error| error.message)
            .collect(),
    }
}

/// The harness must find a corpus at all.
#[test]
fn the_fixture_corpus_is_discovered_and_not_empty() {
    let found = fixtures();
    assert!(
        found.len() >= ABSORBED_PRODUCTIONS.len() + REFUSED_FORMS.len(),
        "the corpus holds {} fixtures, fewer than the {} rows it must cover; the discovery is \
         broken or fixtures are missing",
        found.len(),
        ABSORBED_PRODUCTIONS.len() + REFUSED_FORMS.len()
    );
    println!(
        "just-subset corpus: {} fixtures for {} absorbed productions and {} refused forms",
        found.len(),
        ABSORBED_PRODUCTIONS.len(),
        REFUSED_FORMS.len()
    );
    for fixture in &found {
        println!("  {}", fixture.name);
    }
}

/// COVERAGE, both halves. Every absorbed production has at least one fixture,
/// and every refused form has at least one refusal fixture. The rows come
/// from the tables in `aion_awl::command`, not from a count written here, so a
/// production added without a fixture fails.
#[test]
fn every_absorbed_production_and_every_refused_form_has_a_fixture() {
    let found = fixtures();
    let headers: Vec<String> = found.iter().map(Fixture::header_text).collect();

    let mut uncovered = Vec::new();
    for production in ABSORBED_PRODUCTIONS {
        let marker = format!("absorbed production `{}`", production.id);
        if !headers.iter().any(|header| header.contains(&marker)) {
            uncovered.push(format!("absorbed production `{}`", production.id));
        }
    }
    for refused in REFUSED_FORMS {
        let marker = format!("refused form `{}`", refused.id);
        if !headers.iter().any(|header| header.contains(&marker)) {
            uncovered.push(format!("refused form `{}`", refused.id));
        }
    }
    assert!(
        uncovered.is_empty(),
        "these rows have no fixture, so nothing holds the parser to them: {uncovered:#?}"
    );
}

/// PROVENANCE. Every fixture says where its specimen came from, and every
/// suite-derived one cites the pinned commit and a file digest — so a
/// comparison is against cited bytes, never remembered ones.
#[test]
fn every_fixture_records_its_provenance() {
    let mut failures = Vec::new();
    for fixture in fixtures() {
        let Some(provenance) = fixture.claim("PROVENANCE") else {
            failures.push(format!("{} carries no PROVENANCE line", fixture.name));
            continue;
        };
        let header = fixture.header_text();
        if provenance.contains("AUTHORED") {
            // An authored fixture must say WHY the suite has no specimen.
            if !header.contains("ABSENT FROM SUITE") {
                failures.push(format!(
                    "{} is authored but does not name its absence from the suite",
                    fixture.name
                ));
            }
            continue;
        }
        for required in [
            SUITE_REPO.trim_start_matches("https://github.com/"),
            SUITE_COMMIT,
            SUITE_LICENSE,
        ] {
            if !header.contains(required) {
                failures.push(format!("{} does not cite `{required}`", fixture.name));
            }
        }
        let cites_digest = header.split_whitespace().any(|word| {
            let word = word.trim_matches(|c: char| !c.is_ascii_hexdigit());
            word.len() == 64 && word.chars().all(|c| c.is_ascii_hexdigit())
        });
        if !cites_digest {
            failures.push(format!(
                "{} cites a suite file with no sha256; the comparison would be against \
                 remembered bytes",
                fixture.name
            ));
        }
        if !header.contains("JUST-SPECIMEN:") {
            failures.push(format!(
                "{} quotes no JUST-SPECIMEN, so a reader cannot see what was absorbed",
                fixture.name
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// THE GATE. Every fixture does what its `EXPECT` line says: an accepting one
/// parses and checks clean, a refusing one refuses with the named cause.
#[test]
fn every_fixture_meets_its_declared_expectation() {
    let mut failures = Vec::new();
    for fixture in fixtures() {
        let Some(expect) = fixture.claim("EXPECT") else {
            failures.push(format!("{} carries no EXPECT line", fixture.name));
            continue;
        };
        let produced = refusals(&fixture.source);
        if expect == "accept" {
            if !produced.is_empty() {
                failures.push(format!(
                    "{} expects accept and was refused:\n{}",
                    fixture.name,
                    produced.join("\n")
                ));
            }
            continue;
        }
        let Some(code) = expect.strip_prefix("refuse ") else {
            failures.push(format!(
                "{} carries an EXPECT the harness cannot read: {expect:?}",
                fixture.name
            ));
            continue;
        };
        let Some(cause) = CommandCause::from_code(code.trim()) else {
            failures.push(format!(
                "{} expects `{code}`, which names no cause of the closed vocabulary",
                fixture.name
            ));
            continue;
        };
        if produced.is_empty() {
            failures.push(format!(
                "{} expects `{cause}` and was ACCEPTED",
                fixture.name
            ));
            continue;
        }
        let marker = format!("named cause: {cause}");
        if !produced.iter().any(|text| text.contains(&marker)) {
            failures.push(format!(
                "{} expects `{cause}` and was refused as:\n{}",
                fixture.name,
                produced.join("\n")
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// Every refused form's cause is one the fixtures actually exercise, so the
/// table and the corpus cannot drift into describing different languages.
#[test]
fn the_refused_table_and_the_refusal_fixtures_name_the_same_causes() {
    let declared: BTreeSet<CommandCause> = REFUSED_FORMS.iter().map(|form| form.cause).collect();
    let mut exercised = BTreeSet::new();
    for fixture in fixtures() {
        if let Some(expect) = fixture.claim("EXPECT") {
            if let Some(code) = expect.strip_prefix("refuse ") {
                if let Some(cause) = CommandCause::from_code(code.trim()) {
                    exercised.insert(cause);
                }
            }
        }
    }
    let missing: Vec<&str> = declared
        .difference(&exercised)
        .map(|cause| cause.as_str())
        .collect();
    assert!(
        missing.is_empty(),
        "the refused-form table names causes no fixture exercises: {missing:?}"
    );
}
