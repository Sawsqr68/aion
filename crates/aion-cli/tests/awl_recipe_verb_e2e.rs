//! The EXECUTING VERB, end to end: `aion awl recipe compose` and
//! `aion awl recipe fire` against a real tree.
//!
//! # Why this is an e2e and not a unit test
//!
//! Everything the front owns — the census comparison, the typed stage
//! compare, the receipt, every refusal — is already proven in
//! `aion-awl`'s own suite against stated bytes. What is NOT provable there
//! is the seam this crate exists to be: that the two ports really read a
//! real tree and really run a real process, and that the front's refusals
//! come out of the binary with their causes intact.
//!
//! RED FIRST: the refusing runs stand before the passing one.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn aion_binary() -> Option<PathBuf> {
    let mut path = std::env::current_exe().ok()?;
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    let candidate = path.join(if cfg!(windows) { "aion.exe" } else { "aion" });
    candidate.is_file().then_some(candidate)
}

/// A tree with a gate script, a ground file, and a declared assembly over
/// both, whose digests are MEASURED from the bytes just written.
struct Staged {
    root: PathBuf,
}

impl Staged {
    fn build(name: &str) -> Option<Self> {
        let root =
            std::env::temp_dir().join(format!("aion-recipe-verb-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).ok()?;

        let gate = "#!/bin/sh\necho \"GATE-RED: nothing under crates/aion-awl/ (named cause: \
                    deliverable_absent)\"\nexit 1\n";
        let ground = "# OPERATING GROUND\nThe box's measured record-run truth.\n";
        fs::write(root.join("gate.sh"), gate).ok()?;
        fs::write(root.join("OPERATING-GROUND.md"), ground).ok()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(root.join("gate.sh"), fs::Permissions::from_mode(0o755)).ok()?;
        }

        let document = format!(
            "//! Stage a flight against this tree.\n\
             workflow staging\n  outcome staged: type String, route success\n\n\
             command announce(branch: String)\n  program \"true\"\n  admits \"--\"\n  \
             operand target: String = \"{{{{branch}}}}\"\n\n\
             artifact gate_entry \"gate.sh\"\n  digest \"{}\"\n\n\
             artifact operating_ground \"OPERATING-GROUND.md\"\n  digest \"{}\"\n  \
             excludes \"RECORD-RUN-PENDING\"\n\n\
             recipe stage_the_flight\n  parameter branch: String per_item\n  \
             parameter ground_title: String re_derive derives \"OPERATING-GROUND.md\" extract \
             \"line 1\"\n  \
             step read_ground\n    captures ground: String of artifact operating_ground\n  \
             step self_test\n    after read_ground\n    requires \"workers serving the queue\"\n    \
             fires artifact gate_entry expecting \"deliverable_absent\"\n  \
             step announce\n    after self_test\n    runs command announce\n\n\
             step done\n  route staged(\"ok\")\n",
            digest_of(gate.as_bytes()),
            digest_of(ground.as_bytes())
        );
        fs::write(root.join("stage.awl"), document).ok()?;
        Some(Self { root })
    }

    fn awl(&self) -> PathBuf {
        self.root.join("stage.awl")
    }

    fn run(binary: &Path, arguments: &[&str]) -> Output {
        let Ok(output) = Command::new(binary).args(arguments).output() else {
            unreachable!("the aion binary beside this test must be runnable");
        };
        output
    }
}

impl Drop for Staged {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// The digest the front computes, computed the same way here so the fixture
/// and the census cannot disagree about what the bytes hash to.
fn digest_of(bytes: &[u8]) -> String {
    aion_awl::command::recipe::digest_of(bytes)
}

fn text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// An unmeasured run must be RED, never quietly green.
fn binary() -> PathBuf {
    let found = aion_binary();
    assert!(
        found.is_some(),
        "the `aion` binary is not built beside this test, so nothing was measured; run \
         `cargo test -p aion-cli`, which builds it"
    );
    found.unwrap_or_default()
}

// ─────────────────────────────────────────────────────────────────── RED

/// A slot nothing filled refuses at COMPOSE, before anything is measured and
/// before anything runs.
#[test]
fn compose_refuses_an_unfilled_slot_by_name() {
    let binary = binary();
    let Some(staged) = Staged::build("unfilled") else {
        unreachable!("the fixture tree must be creatable");
    };
    let output = Staged::run(
        &binary,
        &[
            "awl",
            "recipe",
            "compose",
            &staged.awl().display().to_string(),
            "--name",
            "stage_the_flight",
        ],
    );
    assert!(
        !output.status.success(),
        "an unfilled slot must exit non-zero"
    );
    let said = text(&output);
    assert!(
        said.contains("named cause: slot_unfilled") && said.contains("observed: \"branch\""),
        "the front's refusal must reach the operator whole; got:\n{said}"
    );
}

/// A declared at-fire precondition nobody answered stops the fire — AND the
/// receipt still prints, because a stage nobody can audit is the failure
/// receipt-first ordering exists to prevent.
#[test]
fn fire_refuses_an_unanswered_precondition_and_still_prints_its_receipt() {
    let binary = binary();
    let Some(staged) = Staged::build("precondition") else {
        unreachable!("the fixture tree must be creatable");
    };
    let output = Staged::run(
        &binary,
        &[
            "awl",
            "recipe",
            "fire",
            &staged.awl().display().to_string(),
            "--name",
            "stage_the_flight",
            "--fill",
            "branch=fleet/x",
        ],
    );
    assert!(!output.status.success());
    let said = text(&output);
    assert!(said.contains("named cause: precondition_unmet"), "{said}");
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("receipt: recipe stage_the_flight"),
        "the receipt prints on a REFUSED fire; got:\n{said}"
    );
}

/// A referenced artifact whose deployed bytes moved stops the fire, with
/// DECLARED AND OBSERVED hashes both, measured against the real tree.
#[test]
fn fire_refuses_a_census_mismatch_measured_against_the_real_tree() {
    let binary = binary();
    let Some(staged) = Staged::build("census") else {
        unreachable!("the fixture tree must be creatable");
    };
    let Ok(()) = fs::write(
        staged.root.join("OPERATING-GROUND.md"),
        "# OPERATING GROUND\nsomething else entirely\n",
    ) else {
        unreachable!("the fixture must be writable");
    };
    let output = Staged::run(
        &binary,
        &[
            "awl",
            "recipe",
            "fire",
            &staged.awl().display().to_string(),
            "--name",
            "stage_the_flight",
            "--fill",
            "branch=fleet/x",
            "--met",
            "workers serving the queue",
        ],
    );
    assert!(!output.status.success());
    let said = text(&output);
    assert!(said.contains("named cause: census_mismatch"), "{said}");
    assert!(
        said.contains("DISAGREES"),
        "the receipt must record the row that disagreed; got:\n{said}"
    );
}

// ───────────────────────────────────────────────────────────────── GREEN

/// The whole verb: compose, measure the census against the real tree,
/// re-derive at the fire minute, FIRE the declared artifact as a real
/// process, read the record its output names, compare TYPED, run the
/// declared command's emitted argv, and write a generated receipt.
#[test]
fn fire_measures_derives_fires_runs_and_writes_a_receipt() {
    let binary = binary();
    let Some(staged) = Staged::build("green") else {
        unreachable!("the fixture tree must be creatable");
    };
    let receipt = staged.root.join("receipt.txt");
    let output = Staged::run(
        &binary,
        &[
            "awl",
            "recipe",
            "fire",
            &staged.awl().display().to_string(),
            "--name",
            "stage_the_flight",
            "--fill",
            "branch=fleet/x",
            "--met",
            "workers serving the queue",
            "--receipt",
            &receipt.display().to_string(),
        ],
    );
    let said = text(&output);
    assert!(output.status.success(), "the fire must pass:\n{said}");

    let Ok(written) = fs::read_to_string(&receipt) else {
        unreachable!("the receipt must have been written");
    };
    // It names WHAT IT MEASURED …
    assert!(
        written.contains("census: 2 artifact(s) measured"),
        "{written}"
    );
    assert!(written.contains("(agrees)"), "{written}");
    // … it re-derived at the FIRE minute …
    assert!(
        written.contains("re-derived from `OPERATING-GROUND.md` at the fire minute"),
        "{written}"
    );
    // … it fired the declared artifact and compared TYPED …
    assert!(
        written.contains("compared: expected deliverable_absent at the gate layer")
            && written.contains("(matched)"),
        "{written}"
    );
    // … and it says what actually RAN, argv and status, never that a command
    // ran when nothing did.
    assert!(
        written.contains("ran declared command `announce` as \"true -- fleet/x\", exit 0"),
        "{written}"
    );
}

/// `--fill` that is not a binding is refused rather than guessed at.
#[test]
fn a_fill_that_is_not_a_binding_is_refused() {
    let binary = binary();
    let Some(staged) = Staged::build("badfill") else {
        unreachable!("the fixture tree must be creatable");
    };
    let output = Staged::run(
        &binary,
        &[
            "awl",
            "recipe",
            "compose",
            &staged.awl().display().to_string(),
            "--name",
            "stage_the_flight",
            "--fill",
            "branch",
        ],
    );
    assert!(!output.status.success());
    assert!(text(&output).contains("is not a `slot=value` binding"));
}

/// A recipe the document does not declare is named, with what it does
/// declare, rather than answered with nothing.
#[test]
fn an_undeclared_recipe_is_named_with_what_the_document_does_declare() {
    let binary = binary();
    let Some(staged) = Staged::build("noname") else {
        unreachable!("the fixture tree must be creatable");
    };
    let output = Staged::run(
        &binary,
        &[
            "awl",
            "recipe",
            "compose",
            &staged.awl().display().to_string(),
            "--name",
            "no_such_recipe",
        ],
    );
    assert!(!output.status.success());
    let said = text(&output);
    assert!(
        said.contains("declares no recipe `no_such_recipe`") && said.contains("stage_the_flight"),
        "{said}"
    );
}
