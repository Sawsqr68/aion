//! R3 — the sealed conclusion verifier, over REAL git objects.
//!
//! **Each of the three cause codes is produced by a real byte-specimen
//! committed into a real repository and read back from an EXACT commit
//! object id.** `conclusion_malformed` gets THREE distinct specimens, as
//! the brief requires: token + trailing garbage, token with no LF, and two
//! lines.
//!
//! **Fixture-size law: every specimen here is at most 20 bytes, written
//! into a scratch repository that holds three files. Boundary-level
//! synthetic inputs — no multi-GB fixtures, nothing generated.**

use std::path::Path;
use std::process::Command;

use aion_fleet_plan::error::{ConclusionCause, VerifierError};
use aion_fleet_plan::verify::{ExactCommit, Verdict, VerifyRequest, verify_conclusion};

/// Build a scratch repo, write `bytes` at `.fleet/conclusions/<stage>` when
/// `Some`, commit, and return the EXACT commit object id.
fn repo_with(dir: &Path, stage: &str, bytes: Option<&[u8]>) -> String {
    let git = |args: &[&str]| {
        let out = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .unwrap_or_else(|e| unreachable!("git {args:?} could not run: {e}"));
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_owned()
    };
    git(&["init", "-q", "-b", "main"]);
    git(&["config", "user.email", "fleet@example.invalid"]);
    git(&["config", "user.name", "fleet"]);
    // A file that is always present, so "the tree exists" is never the
    // thing under test.
    std::fs::write(dir.join("README"), b"scratch\n")
        .unwrap_or_else(|e| unreachable!("cannot write README: {e}"));
    if let Some(bytes) = bytes {
        let conclusions = dir.join(".fleet/conclusions");
        std::fs::create_dir_all(&conclusions)
            .unwrap_or_else(|e| unreachable!("cannot create dir: {e}"));
        std::fs::write(conclusions.join(stage), bytes)
            .unwrap_or_else(|e| unreachable!("cannot write specimen: {e}"));
    }
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "specimen"]);
    git(&["rev-parse", "HEAD"])
}

struct Case {
    dir: tempfile::TempDir,
    commit: String,
}

fn case(stage: &str, bytes: Option<&[u8]>) -> Case {
    let dir = tempfile::tempdir().unwrap_or_else(|e| unreachable!("no tempdir: {e}"));
    let commit = repo_with(dir.path(), stage, bytes);
    Case { dir, commit }
}

#[track_caller]
fn verdict(case: &Case, stage: &str, vocabulary: &[&str]) -> Verdict {
    let exact = ExactCommit::parse(&case.commit)
        .unwrap_or_else(|e| unreachable!("the test's own commit is not exact: {e}"));
    let vocab: Vec<String> = vocabulary.iter().map(|v| (*v).to_owned()).collect();
    let request = VerifyRequest {
        stage_id: stage,
        child_workflow_id: "child-1",
        result_commit: &exact,
        vocabulary: &vocab,
        kept_root: "/kept/root",
        last_accepted_commit: "",
    };
    verify_conclusion(case.dir.path(), &request)
        .unwrap_or_else(|e| unreachable!("the verifier errored rather than refusing: {e}"))
}

#[track_caller]
fn expect_refused(v: &Verdict, cause: ConclusionCause, observed: &str) {
    let Verdict::Refused(record) = v else {
        unreachable!("expected a refusal with cause {cause}, got {v:?}");
    };
    assert_eq!(record.cause_code, cause, "wrong cause code");
    assert_eq!(record.observed_bytes, observed, "observed_bytes drifted");
    assert_eq!(record.conclusion_path, ".fleet/conclusions/l1");
}

// ------------------------------------------------------------ the happy arm

#[test]
fn a_declared_token_verifies_from_the_exact_commit() {
    let c = case("l1", Some(b"design-ready\n"));
    let Verdict::Verified(record) = verdict(&c, "l1", &["design-ready", "measured-red"]) else {
        unreachable!("a well-formed declared token did not verify");
    };
    assert_eq!(record.conclusion, "design-ready");
    assert_eq!(record.result_commit, c.commit);
    assert_eq!(record.conclusion_path, ".fleet/conclusions/l1");
    assert_eq!(record.stage_id, "l1");
    assert_eq!(record.child_workflow_id, "child-1");
}

#[test]
fn measured_red_verifies_as_a_success_conclusion() {
    // Design §9.4 clause 1: `measured-red` "is not used as `judge_pass` and
    // therefore does not route to `judge_flagged`". A declared conclusion
    // such as `measured-red` is NOT a failure (§4).
    let c = case("l1", Some(b"measured-red\n"));
    let Verdict::Verified(record) = verdict(&c, "l1", &["design-ready", "measured-red"]) else {
        unreachable!("`measured-red` did not verify as a SUCCESS conclusion");
    };
    assert_eq!(record.conclusion, "measured-red");
}

// -------------------------------------------------- conclusion_missing

#[test]
fn conclusion_missing_when_the_path_is_absent_at_the_exact_commit() {
    let c = case("l1", None);
    expect_refused(
        &verdict(&c, "l1", &["design-ready"]),
        ConclusionCause::ConclusionMissing,
        "",
    );
}

#[test]
fn conclusion_missing_when_the_path_is_a_tree_not_a_blob() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| unreachable!("no tempdir: {e}"));
    // `.fleet/conclusions/l1/` as a DIRECTORY holding a file — the
    // canonical path resolves to a tree, which is not a token.
    let nested = dir.path().join(".fleet/conclusions/l1");
    std::fs::create_dir_all(&nested).unwrap_or_else(|e| unreachable!("{e}"));
    std::fs::write(nested.join("inner"), b"design-ready\n").unwrap_or_else(|e| unreachable!("{e}"));
    let run = |args: &[&str]| {
        let out = Command::new("git")
            .arg("-C")
            .arg(dir.path())
            .args(args)
            .output()
            .unwrap_or_else(|e| unreachable!("{e}"));
        String::from_utf8_lossy(&out.stdout).trim().to_owned()
    };
    run(&["init", "-q", "-b", "main"]);
    run(&["config", "user.email", "f@example.invalid"]);
    run(&["config", "user.name", "f"]);
    run(&["add", "-A"]);
    run(&["commit", "-q", "-m", "tree-at-path"]);
    let commit = run(&["rev-parse", "HEAD"]);
    let exact = ExactCommit::parse(&commit).unwrap_or_else(|e| unreachable!("{e}"));
    let vocab = vec!["design-ready".to_owned()];
    let request = VerifyRequest {
        stage_id: "l1",
        child_workflow_id: "child-1",
        result_commit: &exact,
        vocabulary: &vocab,
        kept_root: "",
        last_accepted_commit: "",
    };
    let v = verify_conclusion(dir.path(), &request).unwrap_or_else(|e| unreachable!("{e}"));
    let Verdict::Refused(record) = v else {
        unreachable!("a tree at the conclusion path was accepted as a token");
    };
    assert_eq!(record.cause_code, ConclusionCause::ConclusionMissing);
}

// ------------------------------------------------ conclusion_malformed
// The brief requires THREE distinct malformed specimens.

#[test]
fn conclusion_malformed_specimen_1_token_plus_trailing_garbage() {
    let c = case("l1", Some(b"design-ready\nGARBAGE"));
    expect_refused(
        &verdict(&c, "l1", &["design-ready"]),
        ConclusionCause::ConclusionMalformed,
        "design-ready\\nGARBAGE",
    );
}

#[test]
fn conclusion_malformed_specimen_2_no_lf() {
    let c = case("l1", Some(b"design-ready"));
    expect_refused(
        &verdict(&c, "l1", &["design-ready"]),
        ConclusionCause::ConclusionMalformed,
        "design-ready",
    );
}

#[test]
fn conclusion_malformed_specimen_3_two_lines() {
    let c = case("l1", Some(b"design-ready\nmeasured-red\n"));
    expect_refused(
        &verdict(&c, "l1", &["design-ready", "measured-red"]),
        ConclusionCause::ConclusionMalformed,
        "design-ready\\nmeasured-red\\n",
    );
}

#[test]
fn conclusion_malformed_further_byte_shapes() {
    // Not required by the brief, but the boundary is verified rather than
    // trusted to care: CRLF, a bare LF, an empty file, leading and trailing
    // space, and a NUL all refuse.
    let shapes: &[(&[u8], &str)] = &[
        (b"design-ready\r\n", "design-ready\\r\\n"),
        (b"\n", "\\n"),
        (b"", ""),
        (b" design-ready\n", " design-ready\\n"),
        (b"design-ready \n", "design-ready \\n"),
        (b"design\x00ready\n", "design\\x00ready\\n"),
    ];
    for (bytes, rendered) in shapes {
        let c = case("l1", Some(bytes));
        let v = verdict(&c, "l1", &["design-ready"]);
        let Verdict::Refused(record) = &v else {
            unreachable!(
                "byte shape {:?} was accepted as a well-formed token",
                String::from_utf8_lossy(bytes)
            );
        };
        // An empty file is a present-but-empty blob, so it is malformed,
        // not missing.
        assert_eq!(
            record.cause_code,
            ConclusionCause::ConclusionMalformed,
            "byte shape {:?} took the wrong cause",
            String::from_utf8_lossy(bytes)
        );
        assert_eq!(record.observed_bytes, *rendered);
    }
}

// ---------------------------------------------- conclusion_undeclared

#[test]
fn conclusion_undeclared_when_the_token_is_outside_the_closed_vocabulary() {
    let c = case("l1", Some(b"improvised\n"));
    expect_refused(
        &verdict(&c, "l1", &["design-ready", "measured-red"]),
        ConclusionCause::ConclusionUndeclared,
        "improvised\\n",
    );
}

#[test]
fn the_vocabulary_comparison_is_byte_exact_not_prose() {
    // A near miss is undeclared, not "close enough". Design §4: free-form
    // report text is never a branch condition.
    for near in [
        &b"design-Ready\n"[..],
        b"designready\n",
        b"design_ready\n",
        b"design-ready-ish\n",
    ] {
        let c = case("l1", Some(near));
        let v = verdict(&c, "l1", &["design-ready"]);
        let Verdict::Refused(record) = &v else {
            unreachable!(
                "near-miss token {:?} verified against a closed vocabulary",
                String::from_utf8_lossy(near)
            );
        };
        assert_eq!(record.cause_code, ConclusionCause::ConclusionUndeclared);
    }
}

// ------------------------------------------------------------- the seal

#[test]
fn the_verifier_refuses_a_ref_name_before_any_git_runs() {
    // Design §6, verbatim: resolving the object with `git rev-parse`,
    // reading a live branch ref, or reading a worktree VIOLATES the design.
    // The verifier cannot be handed one: `ExactCommit` has no other
    // constructor.
    for name in ["HEAD", "main", "refs/heads/main", "origin/main", "HEAD~1"] {
        let Err(VerifierError::NotAnObjectId { given }) = ExactCommit::parse(name) else {
            unreachable!("{name:?} was accepted as an exact object id");
        };
        assert_eq!(given, name);
    }
}

#[test]
fn the_verifier_refuses_an_unsafe_stage_id_before_deriving_a_path() {
    let c = case("l1", Some(b"design-ready\n"));
    let exact = ExactCommit::parse(&c.commit).unwrap_or_else(|e| unreachable!("{e}"));
    let vocab = vec!["design-ready".to_owned()];
    for unsafe_id in ["../x", "a/b", "..", "Upper", ""] {
        let request = VerifyRequest {
            stage_id: unsafe_id,
            child_workflow_id: "child-1",
            result_commit: &exact,
            vocabulary: &vocab,
            kept_root: "",
            last_accepted_commit: "",
        };
        let got = verify_conclusion(c.dir.path(), &request);
        assert!(
            matches!(got, Err(VerifierError::UnsafeStageId { .. })),
            "unsafe stage id {unsafe_id:?} reached path derivation"
        );
    }
}

#[test]
fn the_verifier_reads_the_commit_it_was_given_not_the_current_branch() {
    // The property that makes the seal load-bearing: after the branch moves
    // on, verifying the OLD exact commit still reads the OLD bytes.
    let dir = tempfile::tempdir().unwrap_or_else(|e| unreachable!("{e}"));
    let first = repo_with(dir.path(), "l1", Some(b"design-ready\n"));

    // Move the branch on, replacing the token with a different declared
    // value. A verifier that resolved `HEAD` would now read `measured-red`.
    let conclusions = dir.path().join(".fleet/conclusions");
    std::fs::write(conclusions.join("l1"), b"measured-red\n")
        .unwrap_or_else(|e| unreachable!("{e}"));
    for args in [vec!["add", "-A"], vec!["commit", "-q", "-m", "second"]] {
        let out = Command::new("git")
            .arg("-C")
            .arg(dir.path())
            .args(&args)
            .output()
            .unwrap_or_else(|e| unreachable!("{e}"));
        assert!(out.status.success());
    }

    let exact = ExactCommit::parse(&first).unwrap_or_else(|e| unreachable!("{e}"));
    let vocab = vec!["design-ready".to_owned(), "measured-red".to_owned()];
    let request = VerifyRequest {
        stage_id: "l1",
        child_workflow_id: "child-1",
        result_commit: &exact,
        vocabulary: &vocab,
        kept_root: "",
        last_accepted_commit: "",
    };
    let Verdict::Verified(record) =
        verify_conclusion(dir.path(), &request).unwrap_or_else(|e| unreachable!("{e}"))
    else {
        unreachable!("verifying an older exact commit did not verify");
    };
    assert_eq!(
        record.conclusion, "design-ready",
        "the verifier read the CURRENT branch instead of the exact recorded commit"
    );
    assert_eq!(record.result_commit, first);
}

#[test]
fn a_refusal_carries_every_field_the_design_names() {
    // Design §4's `stage_conclusion_refused` block, field for field.
    let c = case("l1", Some(b"improvised\n"));
    let exact = ExactCommit::parse(&c.commit).unwrap_or_else(|e| unreachable!("{e}"));
    let vocab = vec!["design-ready".to_owned()];
    let last_accepted = "b".repeat(40);
    let request = VerifyRequest {
        stage_id: "l1",
        child_workflow_id: "child-77",
        result_commit: &exact,
        vocabulary: &vocab,
        kept_root: "/fleet/runs/r1",
        last_accepted_commit: &last_accepted,
    };
    let Verdict::Refused(record) =
        verify_conclusion(c.dir.path(), &request).unwrap_or_else(|e| unreachable!("{e}"))
    else {
        unreachable!("an undeclared token was accepted");
    };
    assert_eq!(record.stage_id, "l1");
    assert_eq!(record.child_workflow_id, "child-77");
    assert_eq!(record.result_commit, c.commit);
    assert_eq!(record.conclusion_path, ".fleet/conclusions/l1");
    assert_eq!(record.cause_code, ConclusionCause::ConclusionUndeclared);
    assert_eq!(record.observed_bytes, "improvised\\n");
    assert_eq!(record.kept_root, "/fleet/runs/r1");
    assert_eq!(record.last_accepted_commit, last_accepted);
}

#[test]
fn the_three_cause_codes_are_exactly_the_designs_three() {
    assert_eq!(
        ConclusionCause::ConclusionMissing.as_str(),
        "conclusion_missing"
    );
    assert_eq!(
        ConclusionCause::ConclusionMalformed.as_str(),
        "conclusion_malformed"
    );
    assert_eq!(
        ConclusionCause::ConclusionUndeclared.as_str(),
        "conclusion_undeclared"
    );
}
