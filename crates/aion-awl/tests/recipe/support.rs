//! What every recipe test needs: a document wrapper, a parse-and-check door,
//! and the two ports the fire layer takes.
//!
//! The ports are in-memory ON PURPOSE. The language front opens no file and
//! spawns no process — [`ArtifactSource`] is the seam the executing verb in
//! `aion-cli` implements over a real tree — so every fire test here states
//! the bytes it measures, and a red is a fact about the census rather than
//! about a temporary directory.

#![allow(dead_code)]

use std::cell::RefCell;
use std::collections::BTreeMap;

use aion_awl::Document;
use aion_awl::command::Argv;
use aion_awl::command::recipe::{
    ArtifactSource, CommandRunner, SourceUnreadable, SpecimenFirer, VerdictRecord,
};

/// The narration every document opens with (the parser requires one).
pub const NARRATION: &str = "//! Recipe-surface specimen.\n";

/// A whole workflow document wrapping `declarations`.
#[must_use]
pub fn document(declarations: &str) -> String {
    format!(
        "{NARRATION}workflow specimen\n  outcome done: type String, route success\n\n\
         {declarations}\n\nstep go\n  route done(\"ok\")\n"
    )
}

/// Parse `source`, or say why it could not be parsed.
///
/// # Panics
///
/// Never: a parse failure is returned as the error arm.
pub fn parse(source: &str) -> Result<Document, String> {
    aion_awl::parse(source).map_err(|error| {
        format!(
            "{}:{}: {}",
            error.span.line, error.span.column, error.message
        )
    })
}

/// Every refusal the whole pipeline produced for a document.
#[must_use]
pub fn refusals(source: &str) -> Vec<String> {
    match aion_awl::parse(source) {
        Err(error) => vec![error.message],
        Ok(parsed) => aion_awl::check_in(&parsed, std::env::temp_dir().as_path())
            .into_iter()
            .map(|error| error.message)
            .collect(),
    }
}

/// Parse and check a document that must be clean, returning it.
///
/// # Panics
///
/// Panics when the document does not parse or does not check clean — a test
/// support helper that returned a broken document would make every assertion
/// downstream of it meaningless.
#[must_use]
pub fn checked(source: &str) -> Document {
    let parsed = match aion_awl::parse(source) {
        Ok(parsed) => parsed,
        Err(error) => unreachable!(
            "the fixture must parse — {}:{}: {}\n{source}",
            error.span.line, error.span.column, error.message
        ),
    };
    let complaints: Vec<String> = aion_awl::check_in(&parsed, std::env::temp_dir().as_path())
        .into_iter()
        .map(|error| error.message)
        .collect();
    assert!(
        complaints.is_empty(),
        "the fixture must check clean:\n{}\n{source}",
        complaints.join("\n")
    );
    parsed
}

/// A tree stated in memory, whose bytes a test controls exactly.
///
/// Mutable between reads on purpose: the fire-minute law is only provable by
/// a source that CAN change between compose and fire, and a source that
/// could not would make the proof vacuous.
#[derive(Debug, Default)]
pub struct StatedTree {
    bytes: RefCell<BTreeMap<String, Vec<u8>>>,
}

impl StatedTree {
    /// An empty tree.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// State what stands at `address`.
    #[must_use]
    pub fn with(self, address: &str, bytes: &str) -> Self {
        self.bytes
            .borrow_mut()
            .insert(address.to_owned(), bytes.as_bytes().to_vec());
        self
    }

    /// Change what stands at `address` after the tree was stated.
    pub fn rewrite(&self, address: &str, bytes: &str) {
        self.bytes
            .borrow_mut()
            .insert(address.to_owned(), bytes.as_bytes().to_vec());
    }
}

impl ArtifactSource for StatedTree {
    fn read(&self, address: &str) -> Result<Vec<u8>, SourceUnreadable> {
        self.bytes
            .borrow()
            .get(address)
            .cloned()
            .ok_or_else(|| SourceUnreadable::Absent {
                address: address.to_owned(),
            })
    }
}

/// A runner that records the argv it was handed and answers a stated status.
///
/// Recording rather than pretending: a receipt that said a command ran when
/// nothing did would be exactly the claim a generated record exists not to
/// make, so the tests assert on what the runner was ACTUALLY given.
#[derive(Debug)]
pub struct StatedRunner {
    status: i32,
    ran: RefCell<Vec<Vec<String>>>,
}

impl StatedRunner {
    /// A runner that answers `status` for everything it is handed.
    #[must_use]
    pub fn answering(status: i32) -> Self {
        Self {
            status,
            ran: RefCell::new(Vec::new()),
        }
    }

    /// Every argv this runner was handed, in order.
    #[must_use]
    pub fn ran(&self) -> Vec<Vec<String>> {
        self.ran.borrow().clone()
    }
}

impl Default for StatedRunner {
    fn default() -> Self {
        Self::answering(0)
    }
}

impl CommandRunner for StatedRunner {
    fn run(&self, command: &Argv) -> Result<i32, String> {
        self.ran.borrow_mut().push(command.argv.clone());
        Ok(self.status)
    }
}

/// A firer that answers with a record a test states.
pub struct StatedFirer {
    answers: BTreeMap<String, Result<VerdictRecord, String>>,
}

impl StatedFirer {
    /// A firer that answers nothing yet.
    #[must_use]
    pub fn new() -> Self {
        Self {
            answers: BTreeMap::new(),
        }
    }

    /// State what firing `address` answers.
    ///
    /// # Panics
    ///
    /// Panics when the stated record is not a record — a test that could not
    /// state its own answer would be asserting on the helper.
    #[must_use]
    pub fn answering(mut self, address: &str, code: &str, layer: &str, bytes: &str) -> Self {
        let record = VerdictRecord::new(code, layer, bytes)
            .unwrap_or_else(|error| unreachable!("the stated answer must be a record: {error}"));
        self.answers.insert(address.to_owned(), Ok(record));
        self
    }

    /// State that firing `address` fails.
    #[must_use]
    pub fn refusing(mut self, address: &str, detail: &str) -> Self {
        self.answers
            .insert(address.to_owned(), Err(detail.to_owned()));
        self
    }
}

impl Default for StatedFirer {
    fn default() -> Self {
        Self::new()
    }
}

impl SpecimenFirer for StatedFirer {
    fn fire(&self, address: &str) -> Result<VerdictRecord, String> {
        self.answers
            .get(address)
            .cloned()
            .unwrap_or_else(|| Err(format!("nothing was stated for `{address}`")))
    }
}

/// The canonical accepting assembly, in canonical printer order: the command
/// a step runs, the wall projected from one declaration, the two artifacts
/// the census measures, then the recipe that orders them.
pub const STAGING: &str = r#"command push_branch(branch: String)
  program "git" "push"
  admits "--"
  operand target: String = "{{branch}}"

wall changed_paths
  allows "crates/aion-awl"
  allows ".fleet/"
  excludes "mutation vocabulary"

artifact gate_entry ".fleet/gate-entry.sh"
  digest "1e1e3ad2ae66bb0bef5b82a8e154491032ee3d7598ac361a731a5f0917b47a6b"

artifact operating_ground "dispatch-rigs/sleepreap/OPERATING-GROUND.md"
  digest "032ee3d7598ac361a731a5f0917b47a6b1ea7c32c3cc3b9670b3c11c32db23b7"
  excludes "RECORD-RUN-PENDING"

recipe stage_the_flight
  parameter branch: String per_item
  parameter control_expect: String re_derive derives "origin/main:workflows/investigate/investigate_worker.awl" extract "line 22"
  step read_ground
    captures ground: String of artifact operating_ground
  step self_test
    after read_ground
    requires "workers serving the queue"
    fires artifact gate_entry expecting "deliverable_absent"
  step push
    after self_test
    runs command push_branch"#;
