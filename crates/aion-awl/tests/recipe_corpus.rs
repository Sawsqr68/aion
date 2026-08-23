//! BRIEF-R7: the live composer corpus is EXPRESSIBLE against this surface,
//! proven as fixtures over the PINNED exhibit bytes.
//!
//! # Provenance discipline, and why it is mechanical
//!
//! Every exhibit is cited by source REPO, PATH and commit DIGEST, and every
//! digest is RE-HASHED on the run. A fixture that cited remembered text would
//! go on passing after the specimen moved; this one fails instead. The
//! exhibits travel in the flight tree at `.fleet/fixtures/corpus/`, staged
//! digest-pinned, and the OUTER path is what every fixture here cites.
//!
//! # Expressibility, NOT deployment
//!
//! No composer script retires, no rig converts, no checklist is deleted, no
//! deployed document moves. What is proven is that the declared surface can
//! SAY what the live layer does. The strike list is the contract of the
//! cutover, a staged act after landing, owned by staging.
//!
//! # The completeness claim
//!
//! `the_fixture_set_carries_its_own_completeness_claim` prints and asserts
//! it: which exhibits, from which repo, at which digests, and which known
//! corpus members were EXCLUDED with the reason. An undeclared corpus is not
//! a census, and a fixture set that did not say what it left out would be
//! reporting a coverage it does not have.

mod recipe;

use std::path::{Path, PathBuf};

use aion_awl::command::CommandCause;
use aion_awl::command::recipe::{Composition, compose, digest_of, project};

use recipe::support::{checked, document, refusals};

/// The repository every exhibit below came from, and the commit it was taken
/// at. Recorded in the flight's rig at staging.
const CORPUS_REPO: &str = "https://github.com/ablative-io/docs.git";
const CORPUS_COMMIT: &str = "3c7cbb6";

/// One pinned exhibit: the flight-tree path, the source path in the corpus
/// repo, the sha256 the rig pinned, and the BRIEF-R7 clause it answers.
struct Exhibit {
    file: &'static str,
    source_path: &'static str,
    sha256: &'static str,
    clause: &'static str,
}

/// Every exhibit this fixture set expresses. The digests are the rig's, and
/// they are re-hashed on the run.
const EXHIBITS: [Exhibit; 7] = [
    Exhibit {
        file: "sleepreap.build-input.py",
        source_path: "dispatch-rigs/sleepreap/build-input.py",
        sha256: "032ee3d7598ac361a731a5f0917b47a6b1ea7c32c3cc3b9670b3c11c32db23b7",
        clause: "R7(a)",
    },
    Exhibit {
        file: "liminal22.build-input.py",
        source_path: "dispatch-rigs/liminal22/build-input.py",
        sha256: "b51f60f780e542dffbaa6879db2294ccd3fa050e8138b33e87e1ef3cd0d06351",
        clause: "R7(a)",
    },
    Exhibit {
        file: "skeleton.input.json",
        source_path: "dispatch-rigs/issue-investigations/skeleton.input.json",
        sha256: "4e90bc3bf2e3c2f74546c794cc4b87b183fcf60672065668720552edb2eb1218",
        clause: "R7(b)",
    },
    Exhibit {
        file: "issue-investigations.README.md",
        source_path: "dispatch-rigs/issue-investigations/README.md",
        sha256: "b1db6ae3dfa3d261a29923ed091d56657c3978fc2bf453d3a3f6ae12d973fc65",
        clause: "R7(b)",
    },
    Exhibit {
        file: "speech1.STAGING.md",
        source_path: "dispatch-rigs/speech-1-python-wire/STAGING.md",
        sha256: "1150e466b0c3264da1ac6ce93d9301287d89a492c60615b88b4b9b420f7e5f4a",
        clause: "R7(c)",
    },
    Exhibit {
        file: "haem-wall.INPUT-FINAL.json",
        source_path: "dispatch-rigs/haem-wall/INPUT-FINAL.json",
        sha256: "77f0333fcea03ada0c017af9fdea725231c3e2aecce8ade71781571daff9115d",
        clause: "R7(d)",
    },
    Exhibit {
        file: "speech1.KIT-input-template.json",
        source_path: "dispatch-rigs/speech-1-python-wire/KIT-input-template.json",
        sha256: "1a95315217b431ae7d1956183da51098e02ddc671d3e7ab737d0593df429f5ed",
        clause: "R7(e)",
    },
];

/// Known corpus members this fixture set does NOT express, each with the
/// reason. A coverage claim without an exclusion list is a coverage claim
/// nobody can check.
const EXCLUDED: [(&str, &str); 5] = [
    (
        "speech1.STAGING.md steps 7 and 8's author-reviewer handoff (lines 28-29)",
        "OPERATOR LAW, not mechanism: author ≠ reviewer is a property of who acts, and design §8 \
         keeps it written words. The builder owns projection, never content.",
    ),
    (
        "speech1.STAGING.md's fire, verdict and landing authority (lines 41-43)",
        "OPERATOR LAW: fire authority and venue choice are the operator's, per Cally's clearance \
         precedent. Absorbing them would make a declaration decide who may act.",
    ),
    (
        "speech1.STAGING.md step 5's token mint (lines 24-25)",
        "SECURITY-ADJACENT: minting and rotation requirements come FROM Cally Ray's lane and are \
         never proposed here. The pointer stands exactly as the design brief carries it.",
    ),
    (
        "speech1.STAGING.md's sequencing wall (lines 41-43)",
        "OPERATOR LAW: a sequencing wall names which lane may proceed, which is a scheduling \
         decision an owner makes, not a step a recipe fires.",
    ),
    (
        "The four cut-1 gate-script specimens at .fleet/fixtures/gate-scripts/",
        "OUT OF CLAUSE: they are cut-1's R6 corpus, already expressed by cut-1's gate-census \
         fixtures. BRIEF-R7 names five exhibit groups (a)-(e) and none of them is a gate script; \
         the rig carries the specimens because BRIEF-R5/R7 fixtures MAY cite them, and this set \
         cites their family only through the known-answer red in `recipe_fire.rs`.",
    ),
];

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(".fleet/fixtures/corpus")
}

/// The haem-wall allowlist, quoted from the pinned INPUT.
const ALLOWLIST: &str = "gates.json, known-carries*, scripts/release/, docs/evidence/, \
                         gate-logs/*.md, CONTRIBUTING.md, README.md, docs/DOCUMENTATION.md, \
                         .fleet/";

/// The exhibit's bytes, re-hashed and checked against the pin.
fn exhibit(file: &str) -> String {
    let expected = EXHIBITS
        .iter()
        .find(|item| item.file == file)
        .unwrap_or_else(|| unreachable!("`{file}` is not a pinned exhibit"));
    let path = corpus_dir().join(file);
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|error| unreachable!("{} must be readable: {error}", path.display()));
    let digest = digest_of(&bytes);
    assert_eq!(
        digest, expected.sha256,
        "{file} has moved since the rig pinned it; this fixture would be comparing against a \
         stale copy"
    );
    String::from_utf8_lossy(&bytes).into_owned()
}

fn named(source: &str, cause: CommandCause) -> Option<String> {
    let marker = format!("named cause: {}", cause.as_str());
    refusals(source)
        .into_iter()
        .find(|text| text.contains(&marker))
}

// ─────────────────────────────────────────── PROVENANCE, before anything

/// Every pinned exhibit is present and hashes to the digest the rig named.
/// Nothing below compares against remembered text.
#[test]
fn every_exhibit_matches_the_digest_the_rig_pinned() {
    for item in &EXHIBITS {
        let text = exhibit(item.file);
        assert!(
            !text.is_empty(),
            "{} is empty; the pin would be meaningless",
            item.file
        );
    }
}

// ────────────────────────────── R7(a): build-input.py's proven behaviours

/// The TWO `build-input.py` copies DIFFER, and both are expressed. The
/// completeness claim names both or it is false.
///
/// Measured at the pinned bytes: `sleepreap` guards its argument count
/// (lines 19-21, `exit 2`) and composes across TWO repos (lines 23-24 and
/// 37-38); `liminal22` has NO argv guard (line 15 indexes `sys.argv[1]`
/// directly, an `IndexError` where the other refuses) and carries a
/// `--round2` variant selection (lines 17 and 29-40). Two behaviours, not
/// one.
#[test]
fn the_two_build_input_copies_differ_and_both_are_expressed() {
    let sleepreap = exhibit("sleepreap.build-input.py");
    let liminal22 = exhibit("liminal22.build-input.py");
    assert_ne!(
        sleepreap, liminal22,
        "the rig records that the two copies differ; if they no longer do, the completeness claim \
         below is wrong"
    );

    // The measured difference, asserted at the bytes rather than recalled.
    assert!(
        sleepreap.contains("if len(sys.argv) < 3:") && sleepreap.contains("sys.exit(2)"),
        "sleepreap guards its argument count and exits 2"
    );
    assert!(
        !liminal22.contains("len(sys.argv) <"),
        "liminal22 has NO argv guard — that is the measured difference"
    );
    assert!(
        liminal22.contains("--round2"),
        "liminal22 carries a variant selection sleepreap does not"
    );
    assert!(
        sleepreap.contains("fulcrum") && sleepreap.contains("RECORD-RUN-PENDING"),
        "sleepreap composes across two repos and guards its ground sentinel"
    );
    assert!(
        liminal22.contains("TBD-LINUX-TESTS"),
        "liminal22 guards a different ground sentinel"
    );

    // BOTH argv behaviours become ONE typed refusal, and the normalization
    // is named rather than smoothed over: the surface makes the composer
    // that raises an IndexError refuse BY NAME, which is what expressing it
    // buys.
    let expressed = checked(&document(
        "command compose_input\n  program \"true\"\n  admits \"--\"\n\n\
         recipe build_input\n  parameter docs_root: String per_item\n  \
         parameter fulcrum_root: String per_item\n  \
         step compose\n    runs command compose_input",
    ));
    let Err(missing) = compose(
        &expressed.recipes[0],
        &Composition::new().with("docs_root", "/home/aion/docs"),
    ) else {
        unreachable!("a missing second root must refuse, as sleepreap's exit 2 does");
    };
    assert_eq!(missing[0].cause(), CommandCause::SlotUnfilled);
    assert_eq!(missing[0].observed_bytes(), "fulcrum_root");
}

/// The GROUND-SENTINEL behaviour both copies share, expressed: an artifact
/// still carrying its sentinel refuses. Both sentinels are the corpus's own.
#[test]
fn the_ground_sentinel_guard_is_expressed_as_a_declared_artifact() {
    let sleepreap = exhibit("sleepreap.build-input.py");
    let liminal22 = exhibit("liminal22.build-input.py");
    for (sentinel, carrier) in [
        ("RECORD-RUN-PENDING", &sleepreap),
        ("TBD-LINUX-TESTS", &liminal22),
    ] {
        assert!(
            carrier.contains(sentinel),
            "`{sentinel}` must be the sentinel the pinned composer actually guards"
        );
        let expressed = checked(&document(&format!(
            "artifact operating_ground \"OPERATING-GROUND.md\"\n  \
             digest \"032ee3d7598ac361a731a5f0917b47a6b1ea7c32c3cc3b9670b3c11c32db23b7\"\n  \
             excludes \"{sentinel}\"\n\n\
             recipe compose_input\n  step read_ground\n    \
             captures ground: String of artifact operating_ground"
        )));
        assert_eq!(
            expressed.artifacts[0].excludes[0].text, sentinel,
            "the sentinel is declared where the fire will meet it"
        );
    }
}

// ─────────────────────── R7(b): the field-kind README rows as declarations

/// The README's field-kind table, expressed row by row AT THE BYTES. The
/// taxonomy is read off the corpus, not invented.
#[test]
fn the_field_kind_rows_become_parameter_kind_declarations() {
    let readme = exhibit("issue-investigations.README.md");
    let skeleton = exhibit("skeleton.input.json");

    // The rows, quoted from the pinned README.
    for (field, kind_text) in [
        ("`node_label`, `repo_url`, `workspace_root`", "**skeleton**"),
        ("`control_prompt`", "**skeleton**"),
        ("`control_expect`", "⚠️ **neither**"),
        ("`issue`, `threshold`", "per-issue"),
        ("`investigate_prompt`", "**both**"),
    ] {
        let row = readme
            .lines()
            .find(|line| line.starts_with(&format!("| {field} ")))
            .unwrap_or_else(|| unreachable!("the pinned README must carry the row for {field}"));
        assert!(
            row.contains(kind_text),
            "the row for {field} must say {kind_text}; got: {row}"
        );
    }
    assert!(
        readme.contains("At the dispatch minute** (not the compose minute — they drift)"),
        "the fire-minute law is the README's own words and must be quotable from it"
    );
    assert!(
        readme.contains("workflows/investigate/investigate_worker.awl")
            && readme.contains("sed -n '22p'"),
        "the derivation's source address and its extraction are both in the README"
    );

    // Expressed. `node_label` and `workspace_root` are DISPATCH-declared, so
    // they are consumed with no default rather than restated — the one-truth
    // fence, which is the same rule the README states as "must match the
    // harness's static value — do not change it".
    let expressed = checked(&document(
        "command dispatch\n  program \"true\"\n  admits \"--\"\n\n\
         recipe compose_investigation\n  \
         parameter node_label: String skeleton\n  \
         parameter repo_url: String skeleton\n  \
         parameter workspace_root: String skeleton\n  \
         parameter control_prompt: String = \"Read the file and reply with exactly line 22.\" \
         skeleton\n  \
         parameter control_expect: String re_derive derives \
         \"origin/main:workflows/investigate/investigate_worker.awl\" extract \"line 22\"\n  \
         parameter issue: String per_item\n  \
         parameter threshold: String per_item\n  \
         parameter investigate_prompt: String both\n  \
         step dispatch\n    runs command dispatch",
    ));
    assert_eq!(expressed.recipes[0].parameters.len(), 8);

    // And the skeleton's own placeholders refuse as unfilled slots.
    for placeholder in ["<<ISSUE_NUMBER>>", "<<ISSUE_TITLE>>"] {
        assert!(
            skeleton.contains(placeholder),
            "the pinned skeleton must carry {placeholder}"
        );
        let Err(refused) = compose(
            &expressed.recipes[0],
            &Composition::new()
                .with("node_label", "aion-workflows-vm")
                .with("repo_url", "https://github.com/ablative-io/aion.git")
                .with("workspace_root", "/home/aion/fleet/runs")
                .with("issue", placeholder)
                .with("threshold", "tom-ask-20260823")
                .with(
                    "investigate_prompt",
                    "You are investigating ONE GitHub issue.",
                ),
        ) else {
            unreachable!("a skeleton placeholder must refuse as an unfilled slot");
        };
        assert!(
            refused
                .iter()
                .any(|refusal| refusal.cause() == CommandCause::SlotUnfilled
                    && refusal.observed_bytes() == placeholder)
        );
    }
}

// ───────────────────── R7(c): STAGING.md's mechanizable steps as step kinds

/// The checklist's mechanizable steps become step kinds, and the fixture
/// names WHICH STEPS WERE DELIBERATELY NOT ABSORBED and why.
#[test]
fn the_staging_checklist_becomes_step_kinds_with_its_exclusions_named() {
    let staging = exhibit("speech1.STAGING.md");

    // The mechanizable steps, quoted from the pinned checklist.
    assert!(
        staging.contains("RECEIPT FIRST") && staging.contains("No receipt → no staging"),
        "step 1 is the receipt-first law"
    );
    assert!(
        staging.contains("TEST THE GATE'S OWN CONTROLS")
            && staging.contains("sdks/liminal-py/pyproject.toml absent from the diff")
            && staging.contains("compare against THAT string"),
        "step 4 is the known-answer red this cut fires mechanically"
    );
    assert!(
        staging.contains("Three-arm re-check at fire time")
            && staging.contains("idle-enough, workers serving dev, disk ≥ 40G"),
        "step 9 is the at-fire precondition set"
    );

    // Expressed: the known-answer red as a `fires` step, the three arms as
    // declared preconditions.
    let expressed = checked(&document(
        "artifact kit_gate \".fleet/gate-entry.sh\"\n  \
         digest \"1e1e3ad2ae66bb0bef5b82a8e154491032ee3d7598ac361a731a5f0917b47a6b\"\n\n\
         recipe stage_speech1\n  \
         step self_test\n    \
         requires \"idle-enough\"\n    requires \"workers serving dev\"\n    \
         requires \"disk >= 40G\"\n    \
         fires artifact kit_gate expecting \"deliverable_absent\"",
    ));
    assert_eq!(expressed.recipes[0].steps[0].requires.len(), 3);

    // AND the exclusions, named. Each is operator law or another lane's.
    for (member, reason) in EXCLUDED {
        assert!(
            !reason.is_empty(),
            "the exclusion of `{member}` must carry its reason"
        );
    }
    for line in [
        "HAND TO HERMES FOR REVIEW (author ≠ reviewer)",
        "Fire is mine; verdict is mine; landing re-gate at my desk.",
        "Mint token: speech1-ctl-",
        "Sequencing wall: DO NOT stage until",
    ] {
        assert!(
            staging.contains(line),
            "the excluded step `{line}` must be a step the pinned checklist actually carries"
        );
    }
}

// ────────────────────────── R7(d): the haem-wall allowlist, ONE declaration

/// The haem-wall allowlist as ONE declaration and N projections — AND THE
/// COUNT IS MEASURED, NOT REMEMBERED.
///
/// ⚠️ FINDING F-R7D-1, reported rather than rounded to fit. BRIEF-R4 calls
/// this the "×4-copies exhibit". At the pinned bytes the allowlist appears
/// VERBATIM TWICE (`fix_prompt`, `legs[0].prompt`), is referred to WITHOUT
/// restatement twice more (`fix_prompt` again — "nothing committed outside
/// the allowlist"; `judge_prompt` — "every changed path inside the
/// allowlist"), and is enforced a fifth time by the gate script the rig
/// names. Two verbatim copies, two references, one mechanical rendering.
/// The fixture states what it measured.
#[test]
fn the_haem_wall_allowlist_is_one_declaration_and_its_projections_are_measured() {
    let input = exhibit("haem-wall.INPUT-FINAL.json");

    let verbatim = input.matches(ALLOWLIST).count();
    let referenced = input.matches("allowlist").count() - verbatim;
    assert_eq!(
        verbatim, 2,
        "MEASURED: the allowlist stands verbatim twice in the pinned INPUT (F-R7D-1)"
    );
    assert_eq!(
        referenced, 2,
        "MEASURED: it is referred to without restatement twice more (F-R7D-1)"
    );
    assert!(
        input.contains("\"gate_entry\": \".fleet/gate-entry.sh\""),
        "and the gate script is the mechanical rendering the rig names"
    );

    // TWO FURTHER one-declaration-many-projections pairs in the same file.
    let control_prompt_copies = input
        .matches("Read the file .fleet/control-token.txt")
        .count();
    assert_eq!(
        control_prompt_copies, 2,
        "`control_prompt` and `counsel_control_prompt` are byte-identical copies"
    );
    assert_eq!(
        input.matches("wall-ctl-b508cd84-waffles-20260823").count(),
        2,
        "`control_expect` and `counsel_control_expect` are byte-identical copies"
    );

    // EXPRESSED: one declaration, projected.
    let expressed = checked(&document(
        "wall haem_wall\n  allows \"gates.json\"\n  allows \"known-carries*\"\n  \
         allows \"scripts/release/\"\n  allows \"docs/evidence/\"\n  \
         allows \"gate-logs/*.md\"\n  allows \"CONTRIBUTING.md\"\n  allows \"README.md\"\n  \
         allows \"docs/DOCUMENTATION.md\"\n  allows \".fleet/\"",
    ));
    let projection = project(&expressed.walls[0])
        .unwrap_or_else(|error| unreachable!("the wall projects: {error}"));
    assert!(
        projection.prompt.contains(ALLOWLIST),
        "the prompt rendering reproduces the allowlist AT THE BYTES; got: {}",
        projection.prompt
    );
    assert!(projection.enforcement.starts_with("^(gates\\.json|"));

    // The two hand-copied control tokens refuse as `literal_where_derived`
    // in a re-derive slot — the same disease, one level down.
    let slot = document(
        "command dispatch\n  program \"true\"\n  admits \"--\"\n\n\
         recipe haem\n  parameter control_expect: String = \
         \"wall-ctl-b508cd84-waffles-20260823\" re_derive derives \
         \".fleet/control-token.txt\" extract \"line 1\"\n  \
         step dispatch\n    runs command dispatch",
    );
    assert!(named(&slot, CommandCause::LiteralWhereDerived).is_some());
}

// ───────────────────────── R7(e): the speech-1 kit's sed-fill slots

/// The kit's `__SENTINEL__` slots become declared parameters, and its
/// double-carry becomes a check error.
#[test]
fn the_kit_sed_fill_slots_become_declared_parameters() {
    let kit = exhibit("speech1.KIT-input-template.json");

    for (sentinel, count) in [
        ("__CTL_TOKEN__", 2),
        ("__RIG_COMMIT__", 2),
        ("__CUT_COMMIT__", 1),
    ] {
        assert_eq!(
            kit.matches(sentinel).count(),
            count,
            "MEASURED: the pinned kit carries {sentinel} {count} time(s)"
        );
    }
    // THE DOUBLE CARRY, measured: `leg_ids_csv` restates `legs[0].id`, and
    // `repo` restates `workspace_root` + `run_name`.
    assert!(kit.contains("\"leg_ids_csv\": \"build\"") && kit.contains("\"id\": \"build\""));
    assert!(
        kit.contains("\"repo\": \"/home/aion/fleet/runs/speech1-liminal-py-flight1/repo\"")
            && kit.contains("\"workspace_root\": \"/home/aion/fleet/runs\"")
            && kit.contains("\"run_name\": \"speech1-liminal-py-flight1\""),
        "`repo` restates workspace_root + run_name"
    );

    // Expressed: the slots refuse unfilled, and the double carry is refused
    // at check.
    let expressed = checked(&document(
        "command dispatch\n  program \"true\"\n  admits \"--\"\n\n\
         recipe stage_speech1\n  parameter ctl_token: String per_item\n  \
         parameter rig_commit: String per_item\n  parameter cut_commit: String per_item\n  \
         step dispatch\n    runs command dispatch",
    ));
    let Err(refused) = compose(
        &expressed.recipes[0],
        &Composition::new()
            .with("ctl_token", "__CTL_TOKEN__")
            .with("rig_commit", "__RIG_COMMIT__")
            .with("cut_commit", "__CUT_COMMIT__"),
    ) else {
        unreachable!("three unfilled sed slots must refuse");
    };
    assert_eq!(refused.len(), 3);
    assert!(
        refused
            .iter()
            .all(|r| r.cause() == CommandCause::SlotUnfilled)
    );

    let doubled = document(
        "command dispatch\n  program \"true\"\n  admits \"--\"\n\n\
         recipe stage_speech1\n  parameter leg_ids: String = \"build\" skeleton\n  \
         parameter first_leg_id: String = \"build\" skeleton\n  \
         step dispatch\n    runs command dispatch",
    );
    assert!(named(&doubled, CommandCause::DoubleCarry).is_some());
}

// ──────────────────────────────────────────── THE COMPLETENESS CLAIM

/// The fixture set says what it covers, from which repo, at which digests,
/// and what it deliberately leaves out. Printed so a reader of the run's
/// output has the claim without reading this file.
#[test]
fn the_fixture_set_carries_its_own_completeness_claim() {
    println!("BRIEF-R7 fixture set — completeness claim");
    println!("  source repo: {CORPUS_REPO} at commit {CORPUS_COMMIT}");
    println!("  exhibits expressed, re-hashed on this run:");
    let mut clauses: Vec<&str> = Vec::new();
    for item in &EXHIBITS {
        let bytes = std::fs::read(corpus_dir().join(item.file))
            .unwrap_or_else(|error| unreachable!("{} must be readable: {error}", item.file));
        let digest = digest_of(&bytes);
        assert_eq!(digest, item.sha256);
        println!(
            "    {} — {} — {} — sha256 {digest}",
            item.clause, item.file, item.source_path
        );
        if !clauses.contains(&item.clause) {
            clauses.push(item.clause);
        }
    }
    assert_eq!(
        clauses,
        vec!["R7(a)", "R7(b)", "R7(c)", "R7(d)", "R7(e)"],
        "every BRIEF-R7 clause (a)-(e) must be answered by at least one exhibit"
    );
    println!("  known corpus members EXCLUDED, with the reason:");
    for (member, reason) in EXCLUDED {
        println!("    {member}\n      {reason}");
    }
    assert_eq!(EXHIBITS.len(), 7, "all seven staged exhibits are expressed");
    assert_eq!(EXCLUDED.len(), 5);
}
