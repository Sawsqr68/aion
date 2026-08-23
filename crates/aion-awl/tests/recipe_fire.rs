//! BRIEF-R5 (DESIGN-R5 + R7) and BRIEF-R6 (DESIGN-R8, builder's half):
//! mechanical specimen firing, the vocabulary-generic compare, generated
//! receipts, the re-read discipline, and the reference census.
//!
//! RED FIRST: every fire-layer refusal stands before the passing path.

mod recipe;

use std::fmt::Write as _;

use aion_awl::command::recipe::{
    Composition, MeasuredTree, Ports, Preconditions, Receipt, ReceiptError, claim, compose,
    digest_of, fire, measure, referenced_artifacts,
};
use aion_awl::command::{CommandCause, CommandLayer};

use recipe::support::{StatedFirer, StatedRunner, StatedTree, checked, document};

/// A firer that answers correctly AND spoils the restoration while it runs —
/// exactly the case a byte-compare exists to catch, since a hazard-adjacent
/// step that leaves the tree changed is not one a green may be reported over.
struct SpoilingFirer<'a>(&'a StatedTree);

impl aion_awl::command::recipe::SpecimenFirer for SpoilingFirer<'_> {
    fn fire(&self, _address: &str) -> Result<aion_awl::command::recipe::VerdictRecord, String> {
        self.0.rewrite(
            ".fleet/gate-entry.sh",
            "#!/bin/sh\n# a line the hazard left behind\n",
        );
        aion_awl::command::recipe::VerdictRecord::new("restored", "gate", "ok")
            .map_err(|error| error.to_string())
    }
}

/// The gate script's bytes and their digest, stated so a test's census is a
/// fact about bytes this file carries rather than about a directory.
const GATE_BYTES: &str = "#!/bin/sh\nred() { echo \"GATE-RED: $1\"; exit 1; }\n";
const GROUND_BYTES: &str = "# OPERATING GROUND\nThe box's measured record-run truth.\n";

fn gate_digest() -> String {
    digest_of(GATE_BYTES.as_bytes())
}

fn ground_digest() -> String {
    digest_of(GROUND_BYTES.as_bytes())
}

/// A staging recipe whose census rows carry the digests this file measures.
fn staging(gate: &str, ground: &str) -> String {
    document(&format!(
        "command push_branch(branch: String)\n  program \"git\" \"push\"\n  admits \"--\"\n  \
         operand target: String = \"{{{{branch}}}}\"\n\n\
         artifact gate_entry \".fleet/gate-entry.sh\"\n  digest \"{gate}\"\n\n\
         artifact operating_ground \"OPERATING-GROUND.md\"\n  digest \"{ground}\"\n  \
         excludes \"RECORD-RUN-PENDING\"\n\n\
         recipe stage_the_flight\n  parameter branch: String per_item\n  \
         step read_ground\n    captures ground: String of artifact operating_ground\n  \
         step self_test\n    after read_ground\n    requires \"workers serving the queue\"\n    \
         fires artifact gate_entry expecting \"deliverable_absent\"\n  \
         step push\n    after self_test\n    runs command push_branch"
    ))
}

fn stated_tree() -> StatedTree {
    StatedTree::new()
        .with(".fleet/gate-entry.sh", GATE_BYTES)
        .with("OPERATING-GROUND.md", GROUND_BYTES)
}

fn passing_firer() -> StatedFirer {
    StatedFirer::new().answering(
        ".fleet/gate-entry.sh",
        "deliverable_absent",
        "gate",
        "deliverable sdks/liminal-py/pyproject.toml absent from the diff",
    )
}

fn met() -> Preconditions {
    Preconditions::new().answered("workers serving the queue", true)
}

fn tree() -> MeasuredTree {
    MeasuredTree::new(
        "7c3dc0b4c0aa508c9baf127a628fb814e152c15a",
        "the flight tree",
        true,
    )
    .unwrap_or_else(|error| unreachable!("a stated tree names what it measured: {error}"))
}

fn composed_branch(parsed: &aion_awl::Document) -> aion_awl::command::recipe::Composed {
    compose(
        &parsed.recipes[0],
        &Composition::new().with("branch", "fleet/awl-cut2-result"),
    )
    .unwrap_or_else(|error| unreachable!("the composition fills every slot: {error:?}"))
}

// ───────────────────────────────────── RED: a receipt that cannot say what
// it measured is UNREPRESENTABLE

/// The tree-naming fields are mandatory AT CONSTRUCTION. A receipt that
/// cannot say which tree it read is a claim about no tree at all, and it is
/// not a value this language can build.
#[test]
fn a_receipt_that_cannot_say_what_it_measured_is_unrepresentable() {
    assert_eq!(
        MeasuredTree::new("", "the flight tree", true),
        Err(ReceiptError::HeadUnstated)
    );
    assert_eq!(
        MeasuredTree::new("   ", "the flight tree", true),
        Err(ReceiptError::HeadUnstated)
    );
    assert_eq!(
        MeasuredTree::new("7c3dc0b4", "", true),
        Err(ReceiptError::TreeUnstated {
            head: "7c3dc0b4".to_owned()
        })
    );
    assert_eq!(
        Receipt::new(tree(), "  ").err(),
        Some(ReceiptError::RecipeUnstated)
    );
}

/// The re-read discipline, red: the object MOVES between the write and the
/// claim, and the claim refuses rather than being made.
///
/// `fleet-fanin.sh`'s discipline generalized — a builder claims what the
/// committed state says.
#[test]
fn an_object_that_moved_between_the_write_and_the_claim_refuses() {
    let tree = StatedTree::new().with(".fleet/reports/build.md", "the report as written");
    tree.rewrite(".fleet/reports/build.md", "the report as it now stands");
    let Err(refusal) = claim(
        &tree,
        ".fleet/reports/build.md",
        b"the report as written",
        aion_awl::Span {
            start: 0,
            end: 1,
            line: 1,
            column: 1,
        },
    ) else {
        unreachable!("an object that moved must refuse the claim");
    };
    assert_eq!(refusal.cause(), CommandCause::ObjectMovedBeforeClaim);
    assert_eq!(refusal.layer(), CommandLayer::Fire);
    assert!(
        refusal
            .message()
            .contains(&digest_of(b"the report as written")),
        "both digests must ride in the refusal; got: {}",
        refusal.message()
    );
}

/// The re-read discipline must be able to pass, or the red above says only
/// that it always refuses.
#[test]
fn an_object_that_stayed_put_is_claimed() {
    let tree = StatedTree::new().with(".fleet/reports/build.md", "the report");
    let digest = claim(
        &tree,
        ".fleet/reports/build.md",
        b"the report",
        aion_awl::Span {
            start: 0,
            end: 1,
            line: 1,
            column: 1,
        },
    )
    .unwrap_or_else(|error| unreachable!("an object that did not move is claimable: {error}"));
    assert_eq!(digest, digest_of(b"the report"));
}

/// A CENSUS MISMATCH refuses the fire with DECLARED AND OBSERVED HASHES
/// both. The five-stale-scripts / one-load-bearing family, mechanized.
#[test]
fn a_census_mismatch_refuses_the_fire_with_both_hashes() {
    let stale = "0000000000000000000000000000000000000000000000000000000000000000";
    let parsed = checked(&staging(stale, &ground_digest()));
    let fired = fire(
        &parsed.recipes[0],
        &parsed.commands,
        &parsed.artifacts,
        &composed_branch(&parsed),
        &met(),
        tree(),
        &Ports {
            source: &stated_tree(),
            firer: &passing_firer(),
            runner: &StatedRunner::default(),
        },
    )
    .unwrap_or_else(|error| unreachable!("the receipt itself is buildable: {error}"));
    assert!(!fired.passed());
    let receipt = fired.receipt.rendered();
    let refusal = fired
        .refusal
        .unwrap_or_else(|| unreachable!("a stale census row must refuse"));
    assert_eq!(refusal.cause(), CommandCause::CensusMismatch);
    assert_eq!(refusal.layer(), CommandLayer::Fire);
    assert!(
        refusal.message().contains(stale) && refusal.message().contains(&gate_digest()),
        "declared and observed hashes must both ride in the refusal; got: {}",
        refusal.message()
    );
    // RECEIPT FIRST, as law: a refused fire STILL yields its receipt.
    assert!(
        receipt.contains("DISAGREES"),
        "the receipt must record what it measured even when that is what stopped the fire"
    );
}

/// An artifact whose bytes cannot be read at all stops the fire, rather than
/// the fire discovering it.
#[test]
fn an_unmeasurable_census_row_refuses_the_fire() {
    let parsed = checked(&staging(&gate_digest(), &ground_digest()));
    let empty = StatedTree::new();
    let fired = fire(
        &parsed.recipes[0],
        &parsed.commands,
        &parsed.artifacts,
        &composed_branch(&parsed),
        &met(),
        tree(),
        &Ports {
            source: &empty,
            firer: &passing_firer(),
            runner: &StatedRunner::default(),
        },
    )
    .unwrap_or_else(|error| unreachable!("the receipt is buildable: {error}"));
    let refusal = fired
        .refusal
        .unwrap_or_else(|| unreachable!("an unreadable artifact must refuse"));
    assert_eq!(refusal.cause(), CommandCause::ArtifactUnreadable);
}

/// An artifact a recipe fires but declares NO digest for is a reference the
/// census cannot measure — which is the stale-script family with the
/// measurement removed.
#[test]
fn an_artifact_with_no_declared_digest_cannot_pass_the_census() {
    let parsed = checked(&document(
        "artifact gate_entry \".fleet/gate-entry.sh\"\n  excludes \"RECORD-RUN-PENDING\"\n\n\
         recipe stage\n  step self_test\n    fires artifact gate_entry expecting \"x\"",
    ));
    let (rows, refusal) = measure(&parsed.recipes[0], &parsed.artifacts, &stated_tree());
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].declared, None);
    assert!(!rows[0].agrees());
    assert_eq!(
        refusal.map(|refusal| refusal.cause()),
        Some(CommandCause::CensusMismatch)
    );
}

/// A DECLARED PRECONDITION nobody answered stops the fire. An unanswered
/// precondition is not a met one.
///
/// The exhibit is the checklist's own three-arm re-check at the pinned
/// bytes: docs repo `3c7cbb6`,
/// `dispatch-rigs/speech-1-python-wire/STAGING.md`, sha256
/// `1150e466b0c3264da1ac6ce93d9301287d89a492c60615b88b4b9b420f7e5f4a`,
/// step 9 — "idle-enough, workers serving dev, disk ≥ 40G".
#[test]
fn an_unanswered_precondition_stops_the_fire() {
    let parsed = checked(&staging(&gate_digest(), &ground_digest()));
    for (label, preconditions) in [
        ("never answered", Preconditions::new()),
        (
            "answered NOT MET",
            Preconditions::new().answered("workers serving the queue", false),
        ),
    ] {
        let fired = fire(
            &parsed.recipes[0],
            &parsed.commands,
            &parsed.artifacts,
            &composed_branch(&parsed),
            &preconditions,
            tree(),
            &Ports {
                source: &stated_tree(),
                firer: &passing_firer(),
                runner: &StatedRunner::default(),
            },
        )
        .unwrap_or_else(|error| unreachable!("the receipt is buildable: {error}"));
        let refusal = fired
            .refusal
            .unwrap_or_else(|| unreachable!("a precondition {label} must stop the fire"));
        assert_eq!(refusal.cause(), CommandCause::PreconditionUnmet);
        assert_eq!(refusal.observed_bytes(), "workers serving the queue");
    }
}

/// A STAGE COMPARE MISMATCH refuses the stage WITH BOTH SIDES' BYTES.
#[test]
fn a_stage_compare_mismatch_refuses_with_both_sides_bytes() {
    let parsed = checked(&staging(&gate_digest(), &ground_digest()));
    let wrong = StatedFirer::new().answering(
        ".fleet/gate-entry.sh",
        "suite_red",
        "gate",
        "cargo nextest run --workspace rc=101",
    );
    let fired = fire(
        &parsed.recipes[0],
        &parsed.commands,
        &parsed.artifacts,
        &composed_branch(&parsed),
        &met(),
        tree(),
        &Ports {
            source: &stated_tree(),
            firer: &wrong,
            runner: &StatedRunner::default(),
        },
    )
    .unwrap_or_else(|error| unreachable!("the receipt is buildable: {error}"));
    let recorded = fired.receipt.rendered();
    let refusal = fired
        .refusal
        .unwrap_or_else(|| unreachable!("a wrong answer must refuse the stage"));
    assert_eq!(refusal.cause(), CommandCause::StageCompareMismatch);
    assert!(
        refusal.message().contains("deliverable_absent") && refusal.message().contains("suite_red"),
        "both sides' bytes must ride in the refusal; got: {}",
        refusal.message()
    );
    assert!(
        recorded.contains("MISMATCH") && recorded.contains("suite_red"),
        "the receipt must carry the typed comparison that stopped the stage; got:\n{recorded}"
    );
}

/// A ground still carrying its unfilled-slot sentinel refuses at the fire
/// minute. `build-input.py`'s ground guard, typed: `sleepreap` refuses while
/// `OPERATING-GROUND.md` carries `RECORD-RUN-PENDING` (docs repo `3c7cbb6`,
/// `dispatch-rigs/sleepreap/build-input.py`, sha256
/// `032ee3d7598ac361a731a5f0917b47a6b1ea7c32c3cc3b9670b3c11c32db23b7`,
/// lines 28–33), and `liminal22` does the same over `TBD-LINUX-TESTS`
/// (`b51f60f780e542dffbaa6879db2294ccd3fa050e8138b33e87e1ef3cd0d06351`,
/// lines 20–25).
#[test]
fn an_artifact_still_carrying_its_sentinel_refuses_at_the_fire_minute() {
    let unfilled = "# OPERATING GROUND\nRECORD-RUN-PENDING\n";
    let parsed = checked(&staging(&gate_digest(), &digest_of(unfilled.as_bytes())));
    let tree_with_sentinel = StatedTree::new()
        .with(".fleet/gate-entry.sh", GATE_BYTES)
        .with("OPERATING-GROUND.md", unfilled);
    let fired = fire(
        &parsed.recipes[0],
        &parsed.commands,
        &parsed.artifacts,
        &composed_branch(&parsed),
        &met(),
        tree(),
        &Ports {
            source: &tree_with_sentinel,
            firer: &passing_firer(),
            runner: &StatedRunner::default(),
        },
    )
    .unwrap_or_else(|error| unreachable!("the receipt is buildable: {error}"));
    let refusal = fired
        .refusal
        .unwrap_or_else(|| unreachable!("an unfilled ground must refuse"));
    assert_eq!(refusal.cause(), CommandCause::SlotUnfilled);
    assert_eq!(refusal.observed_bytes(), "RECORD-RUN-PENDING");
}

/// A RESTORE that is not byte-exact refuses. Restoration is BYTE-COMPARED,
/// never assumed — L6's own rule.
#[test]
fn an_inexact_restoration_refuses() {
    let parsed = checked(&document(&format!(
        "artifact hazard_step \"scripts/hazard.sh\"\n  digest \"{}\"\n  \
         restores \".fleet/gate-entry.sh\"\n\n\
         recipe run_hazard\n  step fire_it\n    fires artifact hazard_step expecting \"restored\"",
        digest_of(b"#!/bin/sh\n")
    )));
    let tree_bytes = StatedTree::new()
        .with("scripts/hazard.sh", "#!/bin/sh\n")
        .with(".fleet/gate-entry.sh", GATE_BYTES);
    let fired = fire(
        &parsed.recipes[0],
        &parsed.commands,
        &parsed.artifacts,
        &compose(&parsed.recipes[0], &Composition::new())
            .unwrap_or_else(|error| unreachable!("no slots: {error:?}")),
        &Preconditions::new(),
        tree(),
        &Ports {
            source: &tree_bytes,
            firer: &SpoilingFirer(&tree_bytes),
            runner: &StatedRunner::default(),
        },
    )
    .unwrap_or_else(|error| unreachable!("the receipt is buildable: {error}"));
    let refusal = fired
        .refusal
        .unwrap_or_else(|| unreachable!("an inexact restoration must refuse"));
    assert_eq!(refusal.cause(), CommandCause::RestoreInexact);
    assert!(
        refusal
            .message()
            .contains(&digest_of(GATE_BYTES.as_bytes())),
        "the refusal must carry the digest the bytes had BEFORE the fire; got: {}",
        refusal.message()
    );
}

// ───────────────────────────────────────────────────────────────── GREEN

/// A specimen-fire step against the known-answer red PASSES, and its receipt
/// carries the TYPED comparison — plus the census it measured, so a reader
/// can see what tree the green was over.
///
/// The exhibit is the checklist's own known-answer red at the pinned bytes:
/// `speech1.STAGING.md` (`1150e466…`, step 4) — "TEST THE GATE'S OWN
/// CONTROLS … it must go RED with `deliverable sdks/liminal-py/pyproject.toml
/// absent from the diff` … Record the output and compare against THAT
/// string."
#[test]
fn a_specimen_fire_against_the_known_answer_red_passes_and_its_receipt_carries_the_comparison() {
    let parsed = checked(&staging(&gate_digest(), &ground_digest()));
    let fired = fire(
        &parsed.recipes[0],
        &parsed.commands,
        &parsed.artifacts,
        &composed_branch(&parsed),
        &met(),
        tree(),
        &Ports {
            source: &stated_tree(),
            firer: &passing_firer(),
            runner: &StatedRunner::default(),
        },
    )
    .unwrap_or_else(|error| unreachable!("the receipt is buildable: {error}"));
    assert!(
        fired.passed(),
        "a clean census, a met precondition and a matching answer must pass: {:?}",
        fired.refusal.map(|refusal| refusal.rendered())
    );

    let rendered = fired.receipt.rendered();
    assert!(rendered.contains("7c3dc0b4c0aa508c9baf127a628fb814e152c15a"));
    assert!(rendered.contains("CLEAN"));
    assert!(rendered.contains("census: 2 artifact(s) measured"));
    assert!(rendered.contains(&gate_digest()));
    assert!(
        rendered.contains("compared: expected deliverable_absent at the gate layer"),
        "the receipt must carry the TYPED comparison; got:\n{rendered}"
    );
    assert!(rendered.contains("matched"));

    // The receipt lists the census it measured, row by row.
    assert_eq!(fired.receipt.census().len(), 2);
    assert!(
        fired
            .receipt
            .census()
            .iter()
            .all(aion_awl::command::recipe::CensusRow::agrees)
    );

    // And the steps ran in DEPENDENCY order, not source order.
    let steps: Vec<&str> = fired
        .receipt
        .steps()
        .iter()
        .map(|outcome| outcome.step.as_str())
        .collect();
    assert_eq!(steps, vec!["read_ground", "self_test", "push"]);
}

/// The census is exactly what the recipe REFERENCES, each artifact named
/// once however many steps reach for it.
#[test]
fn the_census_is_the_set_of_referenced_artifacts_named_once() {
    let parsed = checked(&document(&format!(
        "artifact gate_entry \".fleet/gate-entry.sh\"\n  digest \"{}\"\n\n\
         artifact unreferenced \"scripts/never-fired.sh\"\n  digest \"{}\"\n\n\
         recipe stage\n  step one\n    fires artifact gate_entry expecting \"x\"\n  \
         step two\n    after one\n    captures gate: String of artifact gate_entry",
        gate_digest(),
        ground_digest()
    )));
    let referenced = referenced_artifacts(&parsed.recipes[0], &parsed.artifacts);
    assert_eq!(referenced.len(), 1);
    assert_eq!(referenced[0].name, "gate_entry");
}

/// A fire with a CLEAN census proceeds and its receipt lists the census
/// measured — the other half of BRIEF-R6's acceptance.
#[test]
fn a_clean_census_proceeds_and_the_receipt_lists_what_was_measured() {
    let parsed = checked(&staging(&gate_digest(), &ground_digest()));
    let fired = fire(
        &parsed.recipes[0],
        &parsed.commands,
        &parsed.artifacts,
        &composed_branch(&parsed),
        &met(),
        tree(),
        &Ports {
            source: &stated_tree(),
            firer: &passing_firer(),
            runner: &StatedRunner::default(),
        },
    )
    .unwrap_or_else(|error| unreachable!("the receipt is buildable: {error}"));
    assert!(fired.passed());
    let addresses: Vec<&str> = fired
        .receipt
        .census()
        .iter()
        .map(|row| row.address.as_str())
        .collect();
    // The census is measured in the order the recipe's STEPS reach for the
    // artifacts, which is the order a reader follows the recipe in.
    assert_eq!(
        addresses,
        vec!["OPERATING-GROUND.md", ".fleet/gate-entry.sh"]
    );
}

// ───────────────── BRIEF-R6: the one-truth fence and the placeholder fence

/// THE ONE-TRUTH FENCE, mechanical. A recipe may CONSUME a value the
/// dispatch declares; it may not state it again.
#[test]
fn a_recipe_may_consume_a_dispatch_value_and_may_not_restate_it() {
    use aion_awl::command::recipe::DISPATCH_DECLARED;

    // Every declared name refuses when RESTATED …
    for name in DISPATCH_DECLARED {
        let restated = document(&format!(
            "command dispatch\n  program \"true\"\n  admits \"--\"\n\n\
             recipe stage\n  parameter {name}: String = \"a hand copy\" skeleton\n  \
             step go\n    runs command dispatch"
        ));
        let refused = aion_awl::check_in(
            &aion_awl::parse(&restated)
                .unwrap_or_else(|error| unreachable!("the document parses: {error}")),
            std::env::temp_dir().as_path(),
        )
        .into_iter()
        .any(|error| error.message.contains("named cause: venue_value_restated"));
        assert!(
            refused,
            "restating `{name}` must name `venue_value_restated`"
        );
    }

    // … and every one is CONSUMABLE with no default.
    let mut consumed = String::new();
    for name in DISPATCH_DECLARED {
        let _ = writeln!(consumed, "  parameter {name}: String per_item");
    }
    let source = document(&format!(
        "command dispatch\n  program \"true\"\n  admits \"--\"\n\n\
         recipe stage\n{consumed}  step go\n    runs command dispatch"
    ));
    let parsed = checked(&source);
    assert_eq!(parsed.recipes[0].parameters.len(), DISPATCH_DECLARED.len());
}

/// CUT-1's BRACE-PLACEHOLDER FENCE IS CONSUMED, NOT RE-IMPLEMENTED. This cut
/// adds NO venue-binding hole beyond the existing `{workspace_root}`: the
/// fence lives where cut 1 put it, the recipe surface routes through it
/// unchanged, and the conformance read is mechanical rather than a claim.
#[test]
fn this_cut_adds_no_venue_binding_hole_beyond_the_one_cut_one_admitted() {
    // Cut-1's fence still refuses a second placeholder, byte-identically.
    let second_hole = document(
        "command dispatch\n  program \"true\"\n  admits \"--\"\n  cwd \"{repo_root}/src\"",
    );
    let refused = aion_awl::check_in(
        &aion_awl::parse(&second_hole)
            .unwrap_or_else(|error| unreachable!("the document parses: {error}")),
        std::env::temp_dir().as_path(),
    )
    .into_iter()
    .any(|error| {
        error
            .message
            .contains("named cause: cwd_placeholder_unknown")
    });
    assert!(
        refused,
        "cut-1's `cwd_placeholder_unknown` must still refuse a second placeholder"
    );

    // And the one admitted placeholder is still admitted, unchanged.
    let admitted = document(
        "command dispatch\n  program \"true\"\n  admits \"--\"\n  cwd \"{workspace_root}\"",
    );
    assert!(
        aion_awl::check_in(
            &aion_awl::parse(&admitted)
                .unwrap_or_else(|error| unreachable!("the document parses: {error}")),
            std::env::temp_dir().as_path(),
        )
        .is_empty(),
        "`{{workspace_root}}` is the ONE admitted placeholder and stays admitted"
    );

    // THE FENCE IS CONSUMED, NOT RE-IMPLEMENTED: the recipe surface's own
    // source carries no brace-placeholder rule. A second implementation of
    // one rule is the disease the fence exists to close, written into the
    // fence itself.
    //
    // `workspace_root` DOES appear in the recipe surface — as a member of
    // `DISPATCH_DECLARED`, the one-truth fence's list of names a dispatch
    // already declares. That is a different rule about a different thing (a
    // parameter NAME, not a brace placeholder in a path), so what is
    // asserted below is the absence of the placeholder MACHINERY rather than
    // the absence of the word.
    let recipe_sources = [
        include_str!("../src/command/recipe/check.rs"),
        include_str!("../src/command/recipe/parse.rs"),
        include_str!("../src/command/recipe/project.rs"),
        include_str!("../src/command/recipe/census.rs"),
        include_str!("../src/command/recipe/fire.rs"),
        include_str!("../src/command/recipe/compose.rs"),
        include_str!("../src/command/recipe/derivation.rs"),
    ];
    for source in recipe_sources {
        let code: String = source
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<&str>>()
            .join("\n");
        assert!(
            !code.contains("WORKSPACE_ROOT_PLACEHOLDER"),
            "the recipe surface must CONSUME cut-1's placeholder fence, never re-implement one"
        );
        assert!(
            !code.contains(".find('{')") && !code.contains(".find('}')"),
            "the recipe surface scans for a brace placeholder of its own, which is a SECOND \
             implementation of the one rule the fence exists to be"
        );
    }
}

// ────────── BRIEF-R1's `runs command` step, actually run and honestly said

/// A step that runs a declared command hands the EMITTED ARGV to the runner,
/// and the receipt says what actually ran — never that a command ran when
/// nothing did.
#[test]
fn a_runs_command_step_hands_the_emitted_argv_to_the_runner() {
    let parsed = checked(&staging(&gate_digest(), &ground_digest()));
    let runner = StatedRunner::answering(0);
    let fired = fire(
        &parsed.recipes[0],
        &parsed.commands,
        &parsed.artifacts,
        &composed_branch(&parsed),
        &met(),
        tree(),
        &Ports {
            source: &stated_tree(),
            firer: &passing_firer(),
            runner: &runner,
        },
    )
    .unwrap_or_else(|error| unreachable!("the receipt is buildable: {error}"));
    assert!(fired.passed());

    // THE ARGV IS THE DECLARATION'S OWN, emitted through cut-1's emitter —
    // the composed value reaches the operand, and the end-of-options marker
    // the declaration admits is placed unconditionally.
    assert_eq!(
        runner.ran(),
        vec![vec![
            "git".to_owned(),
            "push".to_owned(),
            "--".to_owned(),
            "fleet/awl-cut2-result".to_owned(),
        ]],
        "the step must run exactly the argv the checked declaration emits"
    );
    let rendered = fired.receipt.rendered();
    assert!(
        rendered.contains(
            "ran declared command `push_branch` as \"git push -- \
                           fleet/awl-cut2-result\", exit 0"
        ),
        "the receipt must say what actually ran; got:\n{rendered}"
    );
}

/// A declared command that runs and FAILS stops the stage. An assembly whose
/// act failed has not assembled anything.
#[test]
fn a_declared_command_that_fails_stops_the_stage() {
    let parsed = checked(&staging(&gate_digest(), &ground_digest()));
    let runner = StatedRunner::answering(128);
    let fired = fire(
        &parsed.recipes[0],
        &parsed.commands,
        &parsed.artifacts,
        &composed_branch(&parsed),
        &met(),
        tree(),
        &Ports {
            source: &stated_tree(),
            firer: &passing_firer(),
            runner: &runner,
        },
    )
    .unwrap_or_else(|error| unreachable!("the receipt is buildable: {error}"));
    let rendered = fired.receipt.rendered();
    let refusal = fired
        .refusal
        .unwrap_or_else(|| unreachable!("a failed command must stop the stage"));
    assert_eq!(refusal.cause(), CommandCause::StepCommandFailed);
    assert_eq!(refusal.layer(), CommandLayer::Fire);
    assert!(
        refusal.message().contains("exited 128"),
        "the refusal must carry the status; got: {}",
        refusal.message()
    );
    assert!(
        rendered.contains("REFUSED:"),
        "the receipt records the refusal rather than claiming a run; got:\n{rendered}"
    );
    // And the receipt does NOT claim the step succeeded.
    assert!(!rendered.contains("exit 0"));
}

/// A step naming a command the document does not declare cannot be run, and
/// says so rather than skipping the step silently.
#[test]
fn a_step_naming_an_undeclared_command_refuses_at_the_fire_layer() {
    // The document is checked WITHOUT the command, so this is the arm the
    // check layer already refuses — fired anyway, to prove the fire layer
    // does not run past a reference it cannot resolve.
    let parsed = checked(&staging(&gate_digest(), &ground_digest()));
    let fired = fire(
        &parsed.recipes[0],
        &[],
        &parsed.artifacts,
        &composed_branch(&parsed),
        &met(),
        tree(),
        &Ports {
            source: &stated_tree(),
            firer: &passing_firer(),
            runner: &StatedRunner::default(),
        },
    )
    .unwrap_or_else(|error| unreachable!("the receipt is buildable: {error}"));
    let refusal = fired.refusal.unwrap_or_else(|| {
        unreachable!("an undeclared command must refuse rather than be skipped")
    });
    assert_eq!(refusal.cause(), CommandCause::ReferenceUndeclared);
}

// ─────────── FOLD B, measured in the tree rather than asserted in a report

/// THE FRONT REACHES FOR NOTHING. The recipe family opens no file, spawns no
/// process and speaks no transport: the two ports are the whole seam, and a
/// direct reach past them would move execution machinery into the language
/// front the ratified placement keeps it out of.
///
/// Read from the sources themselves, so this is a fact about the bytes
/// rather than a claim in a report — the fold-B duty made mechanical.
#[test]
fn the_recipe_family_opens_no_file_and_spawns_no_process() {
    let sources: [(&str, &str); 15] = [
        ("census.rs", include_str!("../src/command/recipe/census.rs")),
        ("check.rs", include_str!("../src/command/recipe/check.rs")),
        (
            "compare.rs",
            include_str!("../src/command/recipe/compare.rs"),
        ),
        (
            "compose.rs",
            include_str!("../src/command/recipe/compose.rs"),
        ),
        ("decl.rs", include_str!("../src/command/recipe/decl.rs")),
        (
            "derivation.rs",
            include_str!("../src/command/recipe/derivation.rs"),
        ),
        ("fire.rs", include_str!("../src/command/recipe/fire.rs")),
        ("graph.rs", include_str!("../src/command/recipe/graph.rs")),
        ("mod.rs", include_str!("../src/command/recipe/mod.rs")),
        ("parse.rs", include_str!("../src/command/recipe/parse.rs")),
        ("print.rs", include_str!("../src/command/recipe/print.rs")),
        (
            "project.rs",
            include_str!("../src/command/recipe/project.rs"),
        ),
        (
            "receipt.rs",
            include_str!("../src/command/recipe/receipt.rs"),
        ),
        ("record.rs", include_str!("../src/command/recipe/record.rs")),
        ("sweep.rs", include_str!("../src/command/recipe/sweep.rs")),
    ];
    // Every reach a front must not make, by the name it is made under.
    let forbidden = [
        "std::fs",
        "std::process",
        "std::net",
        "Command::new",
        "tokio",
        "reqwest",
        "tonic",
        "std::thread",
        "std::env",
    ];
    let mut reaches = Vec::new();
    for (name, source) in sources {
        let code: String = source
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<&str>>()
            .join("\n");
        for reach in forbidden {
            if code.contains(reach) {
                reaches.push(format!("{name} reaches for `{reach}`"));
            }
        }
    }
    assert!(
        reaches.is_empty(),
        "the language front must reach the world only through its declared ports: {reaches:?}"
    );

    // THE CONTROL: the scan must be able to FIND a reach, or a green above
    // says only that it ran.
    let control = "let bytes = std::fs::read(path)?;";
    assert!(
        forbidden.iter().any(|reach| control.contains(reach)),
        "the control failed: the scan cannot recognise a reach it must refuse"
    );
    // And it must not find one in prose ABOUT a reach.
    let prose = "//! The executing verb uses std::process; this front does not.";
    let stripped: String = prose
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<&str>>()
        .join("\n");
    assert!(
        !forbidden.iter().any(|reach| stripped.contains(reach)),
        "the control failed: the scan reads comments as code"
    );
}
