//! R1 — the §5.2 refusal specimen set.
//!
//! **Each of the seventeen clauses has at least one refusing input, keyed
//! by cause code**, plus the derived charset clause's full nine-vector set,
//! plus the §9.2 exit-by-exit specimens.
//!
//! Three clauses are UNREACHABLE at whole-plan level because an earlier
//! clause subsumes them (clause 2's "nonempty" half, clause 12, clause 15).
//! Each of those is exercised by handing a real refusing input DIRECTLY to
//! the real clause function — see the `subsumed_clauses` module. The design
//! itself blesses the pattern for acyclicity: "the redundant check makes
//! the refusal named".

use aion_fleet_plan::cause::{CauseCode, ExitClass};
use aion_fleet_plan::validate::{CheckOutcome, PlanInputs, check_plan};

// ---------------------------------------------------------------- fixtures
//
// **Fixture-size law: every input in this file is a BOUNDARY-LEVEL
// SYNTHETIC input — at most a three-line `fleet-stages-v1` block. No
// multi-GB fixtures, no generated corpora, nothing that grows with time.**

/// Wrap stage lines in a `fleet-stages-v1` fence inside plan prose, the way
/// a planning counsel actually commits it.
fn plan_with(lines: &[&str]) -> String {
    format!(
        "# PLAN\n\nSome prose the validator must not read as a condition.\n\n\
         ```fleet-stages-v1\n{}\n```\n\nMore prose.\n",
        lines.join("\n")
    )
}

/// Inputs for a plan whose `.fleet` state is committed and clean.
fn inputs(plan_text: String, legs: &[&str]) -> PlanInputs {
    PlanInputs {
        plan_text,
        dispatch_leg_ids: legs.iter().map(|s| (*s).to_owned()).collect(),
        fleet_state_loose: false,
        loose_detail: String::new(),
        max_stages: None,
    }
}

/// The design §9.1 three-stage block, verbatim — the #140 exemplar.
const EXEMPLAR: [&str; 3] = [
    "stage id=l1 legs=l1-design after=- admit=always conclusions=design-ready,measured-red conclusion_leg=l1-design advance=auto",
    "stage id=l2 legs=l2-classify after=l1 admit=always conclusions=classification-ready conclusion_leg=l2-classify advance=auto",
    "stage id=l3 legs=l3-route after=l2 admit=l1.conclusion!=measured-red conclusions=route-ready conclusion_leg=l3-route advance=auto",
];

/// The #140 exemplar's dispatch-fixed leg ids.
const EXEMPLAR_LEGS: [&str; 3] = ["l1-design", "l2-classify", "l3-route"];

/// The design §2.1 mixed-graph example, verbatim.
const MIXED: [&str; 2] = [
    "stage id=build legs=a,b after=- admit=always conclusions=ready conclusion_leg=a advance=auto",
    "stage id=finish legs=c after=build admit=always conclusions=ready conclusion_leg=c advance=auto",
];

/// The mixed graph's dispatch-fixed leg ids.
const MIXED_LEGS: [&str; 3] = ["a", "b", "c"];

/// Assert a plan refuses with exactly this cause code.
#[track_caller]
fn refuses(lines: &[&str], legs: &[&str], expect: CauseCode) {
    let got = check_plan(&inputs(plan_with(lines), legs));
    match got {
        Err(refusal) => assert_eq!(
            refusal.cause_code, expect,
            "expected cause {expect}, got {} — {}",
            refusal.cause_code, refusal.detail
        ),
        Ok(other) => {
            unreachable!("expected refusal {expect}, but the plan was ACCEPTED as {other:?}")
        }
    }
}

#[track_caller]
fn accepts(lines: &[&str], legs: &[&str]) {
    match check_plan(&inputs(plan_with(lines), legs)) {
        Ok(CheckOutcome::Valid(_)) => {}
        other => unreachable!("expected a valid chain, got {other:?}"),
    }
}

// ---------------------------------------------------------------- positives

#[test]
fn the_140_exemplar_block_is_valid() {
    accepts(&EXEMPLAR, &EXEMPLAR_LEGS);
}

#[test]
fn the_design_mixed_graph_example_is_valid() {
    accepts(&MIXED, &MIXED_LEGS);
}

#[test]
fn a_plan_without_a_block_is_the_legacy_path_not_a_refusal() {
    // Design §7 clause 5: a plan WITHOUT a `fleet-stages-v1` block
    // "continues to run only through the existing `fleet_dev` entry point".
    let plan = "# PLAN\n\nOrdinary parallel-only plan prose. No block here.\n";
    assert_eq!(
        check_plan(&inputs(plan.to_owned(), &["a"])),
        Ok(CheckOutcome::NoChainBlock)
    );
}

#[test]
fn the_normalized_graph_is_the_ordering_authority() {
    // §5.3 final ¶. The graph carries the chain order, not the authored
    // line order: this block declares the stages out of chain order and the
    // graph still comes back root-first.
    let out_of_order = [EXEMPLAR[2], EXEMPLAR[0], EXEMPLAR[1]];
    let Ok(CheckOutcome::Valid(graph)) =
        check_plan(&inputs(plan_with(&out_of_order), &EXEMPLAR_LEGS))
    else {
        unreachable!("the exemplar block did not validate when authored out of order");
    };
    let order: Vec<&str> = graph.order.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(order, ["l1", "l2", "l3"]);
    assert_eq!(graph.order[0].conclusion_path(), ".fleet/conclusions/l1");
}

// ------------------------------------------------- clause 1: one block, one version

#[test]
fn clause_01_two_blocks_refuse() {
    let plan = format!("{}\n{}", plan_with(&EXEMPLAR), plan_with(&MIXED));
    let Err(refusal) = check_plan(&inputs(plan, &EXEMPLAR_LEGS)) else {
        unreachable!("two blocks were accepted");
    };
    assert_eq!(refusal.cause_code, CauseCode::MultipleBlocks);
}

#[test]
fn clause_01_unsupported_version_refuses_by_name() {
    let plan = "```fleet-stages-v2\nstage id=l1 legs=a after=- admit=always conclusions=r conclusion_leg=a advance=auto\n```\n";
    let Err(refusal) = check_plan(&inputs(plan.to_owned(), &["a"])) else {
        unreachable!("an unsupported block version was accepted");
    };
    assert_eq!(refusal.cause_code, CauseCode::UnsupportedBlockVersion);
}

#[test]
fn clause_01_an_empty_block_refuses() {
    let plan = "```fleet-stages-v1\n```\n";
    let Err(refusal) = check_plan(&inputs(plan.to_owned(), &["a"])) else {
        unreachable!("an empty block was accepted");
    };
    assert_eq!(refusal.cause_code, CauseCode::EmptyBlock);
}

// ------------------------------------------------------- clause 2: stage ids

#[test]
fn clause_02_duplicate_stage_id_refuses() {
    refuses(
        &[
            "stage id=l1 legs=a after=- admit=always conclusions=r conclusion_leg=a advance=auto",
            "stage id=l1 legs=b after=l1 admit=always conclusions=r conclusion_leg=b advance=auto",
        ],
        &["a", "b"],
        CauseCode::DuplicateStageId,
    );
}

// -------------------------------------------------------- clause 3: leg ids

#[test]
fn clause_03_a_leg_in_two_stages_refuses() {
    refuses(
        &[
            "stage id=l1 legs=a after=- admit=always conclusions=r conclusion_leg=a advance=auto",
            "stage id=l2 legs=a after=l1 admit=always conclusions=r conclusion_leg=a advance=auto",
        ],
        &["a"],
        CauseCode::DuplicateLegId,
    );
}

// ------------------------------------------------ clauses 4 and 5: dispatch cover

#[test]
fn clause_04_a_leg_dispatch_never_declared_refuses() {
    refuses(
        &[
            "stage id=l1 legs=ghost after=- admit=always conclusions=r conclusion_leg=ghost advance=auto",
        ],
        &["a"],
        CauseCode::UnknownLegId,
    );
}

#[test]
fn clause_05_a_dispatch_declared_leg_in_no_stage_refuses() {
    refuses(
        &["stage id=l1 legs=a after=- admit=always conclusions=r conclusion_leg=a advance=auto"],
        &["a", "orphan"],
        CauseCode::LegNotPlaced,
    );
}

// --------------------------------------------------- clause 6: stage has legs

#[test]
fn clause_06_a_stage_with_no_leg_refuses() {
    // `legs=` empty is refused at the canonical-form boundary, which is the
    // only way a legless stage can be authored: the grammar has no spelling
    // for "no legs" that survives §2.1.
    refuses(
        &["stage id=l1 legs= after=- admit=always conclusions=r conclusion_leg=a advance=auto"],
        &["a"],
        CauseCode::EmptyValue,
    );
}

// ------------------------------------------------------ clause 7: predecessors

#[test]
fn clause_07_unknown_predecessor_refuses() {
    // Design §9.2's first named exit-4 specimen: `after=l9`.
    let mut lines = EXEMPLAR;
    lines[1] = "stage id=l2 legs=l2-classify after=l9 admit=always conclusions=classification-ready conclusion_leg=l2-classify advance=auto";
    refuses(&lines, &EXEMPLAR_LEGS, CauseCode::UnknownPredecessor);
}

#[test]
fn clause_07_a_stage_following_itself_refuses() {
    refuses(
        &[
            "stage id=l1 legs=a after=- admit=always conclusions=r conclusion_leg=a advance=auto",
            "stage id=l2 legs=b after=l2 admit=always conclusions=r conclusion_leg=b advance=auto",
        ],
        &["a", "b"],
        CauseCode::SelfPredecessor,
    );
}

// ------------------------------------------------------- clause 9: acyclicity

#[test]
fn clause_09_a_cycle_refuses_by_name_not_as_a_missing_root() {
    // Design §9.2's second named exit-4 specimen: `l1 after=l3`. With the
    // cardinality check first this would report `no_root`; §5.2 wants the
    // refusal NAMED, so acyclicity is evaluated first.
    let mut lines = EXEMPLAR;
    lines[0] = "stage id=l1 legs=l1-design after=l3 admit=always conclusions=design-ready,measured-red conclusion_leg=l1-design advance=auto";
    refuses(&lines, &EXEMPLAR_LEGS, CauseCode::Cycle);
}

// -------------------------------------------------- clause 8: chain cardinality

#[test]
fn clause_08_no_root_refuses() {
    // Every stage names a predecessor, but there is no cycle: `l2 after=l1`
    // where l1 does not exist is clause 7. So: a two-stage chain where the
    // only root candidate was removed by making it follow a real stage.
    refuses(
        &[
            "stage id=l1 legs=a after=l2 admit=always conclusions=r conclusion_leg=a advance=auto",
            "stage id=l2 legs=b after=l1 admit=always conclusions=r conclusion_leg=b advance=auto",
        ],
        &["a", "b"],
        // l1 -> l2 -> l1 is a cycle, and the cycle check names it first.
        CauseCode::Cycle,
    );
    // A genuine no-root without a cycle is inexpressible when every stage
    // has at most one predecessor, so the named refusal above is the one
    // that fires. The `no_root` arm is reached through the direct clause
    // test in `subsumed_clauses`.
}

#[test]
fn clause_08_multiple_roots_refuse() {
    refuses(
        &[
            "stage id=l1 legs=a after=- admit=always conclusions=r conclusion_leg=a advance=auto",
            "stage id=l2 legs=b after=- admit=always conclusions=r conclusion_leg=b advance=auto",
        ],
        &["a", "b"],
        CauseCode::MultipleRoots,
    );
}

#[test]
fn clause_08_an_arbitrary_multi_parent_graph_refuses() {
    // Design §9.2's last named exit-4 specimen. In a grammar whose `after`
    // holds exactly one id, multi-parent can only be written as two stages
    // sharing a predecessor — so the refusal lands on the successor side.
    refuses(
        &[
            "stage id=l1 legs=a after=- admit=always conclusions=r conclusion_leg=a advance=auto",
            "stage id=l2 legs=b after=l1 admit=always conclusions=r conclusion_leg=b advance=auto",
            "stage id=l3 legs=c after=l1 admit=always conclusions=r conclusion_leg=c advance=auto",
        ],
        &["a", "b", "c"],
        CauseCode::MultipleSuccessors,
    );
}

// ------------------------------------------------- clause 10: conclusion owner

#[test]
fn clause_10_a_conclusion_owner_outside_its_stage_refuses() {
    // Design §9.2's named specimen: "a conclusion owner outside its stage".
    let mut lines = EXEMPLAR;
    lines[1] = "stage id=l2 legs=l2-classify after=l1 admit=always conclusions=classification-ready conclusion_leg=l1-design advance=auto";
    refuses(&lines, &EXEMPLAR_LEGS, CauseCode::ConclusionLegOutsideStage);
}

// ----------------------------------------- clause 11: multi-leg vocabulary shape

#[test]
fn clause_11_a_multi_leg_stage_with_multiple_conclusions_refuses() {
    // Design §9.2's named specimen: "a multi-leg stage with multiple
    // conclusion values".
    refuses(
        &[
            "stage id=build legs=a,b after=- admit=always conclusions=ready,not-ready conclusion_leg=a advance=auto",
            "stage id=finish legs=c after=build admit=always conclusions=ready conclusion_leg=c advance=auto",
        ],
        &MIXED_LEGS,
        CauseCode::MultiLegConclusionCount,
    );
}

// ------------------------------------------------------- clause 13: admit refs

#[test]
fn clause_13_an_admit_naming_no_stage_refuses() {
    let mut lines = EXEMPLAR;
    lines[2] = "stage id=l3 legs=l3-route after=l2 admit=l9.conclusion!=measured-red conclusions=route-ready conclusion_leg=l3-route advance=auto";
    refuses(&lines, &EXEMPLAR_LEGS, CauseCode::AdmitUnknownStage);
}

#[test]
fn clause_13_an_admit_on_a_later_stage_refuses() {
    // An admission derives only from an EARLIER stage's already-recorded
    // conclusion (§6 clause 6).
    let mut lines = EXEMPLAR;
    lines[1] = "stage id=l2 legs=l2-classify after=l1 admit=l3.conclusion==route-ready conclusions=classification-ready conclusion_leg=l2-classify advance=auto";
    refuses(&lines, &EXEMPLAR_LEGS, CauseCode::AdmitUnreachableStage);
}

// ------------------------------------------- clause 14: closed vocabulary

#[test]
fn clause_14_an_undeclared_compared_conclusion_refuses() {
    // Design §9.2's named specimen: "an undeclared conclusion".
    let mut lines = EXEMPLAR;
    lines[2] = "stage id=l3 legs=l3-route after=l2 admit=l1.conclusion!=never-declared conclusions=route-ready conclusion_leg=l3-route advance=auto";
    refuses(&lines, &EXEMPLAR_LEGS, CauseCode::AdmitUndeclaredConclusion);
}

// ------------------------------------------------------- clause 16: advance

#[test]
fn clause_16_an_advance_outside_auto_or_hold_refuses() {
    let mut lines = EXEMPLAR;
    lines[0] = "stage id=l1 legs=l1-design after=- admit=always conclusions=design-ready,measured-red conclusion_leg=l1-design advance=maybe";
    refuses(&lines, &EXEMPLAR_LEGS, CauseCode::InvalidAdvance);
}

#[test]
fn advance_hold_is_valid() {
    let mut lines = EXEMPLAR;
    lines[0] = "stage id=l1 legs=l1-design after=- admit=always conclusions=design-ready,measured-red conclusion_leg=l1-design advance=hold";
    accepts(&lines, &EXEMPLAR_LEGS);
}

// -------------------------------------------- clause 17: committed plan state

#[test]
fn clause_17_loose_fleet_state_refuses_at_exit_one() {
    let mut i = inputs(plan_with(&EXEMPLAR), &EXEMPLAR_LEGS);
    i.fleet_state_loose = true;
    i.loose_detail = " M .fleet/plan.md".to_owned();
    let Err(refusal) = check_plan(&i) else {
        unreachable!("loose .fleet state was accepted");
    };
    assert_eq!(refusal.cause_code, CauseCode::PlanStateUncommitted);
    // §5.3: absence keeps exit 1. It must NOT be widened into exit 4.
    assert_eq!(refusal.exit_class(), ExitClass::AbsentOrIncomplete);
    assert_eq!(refusal.exit_class().code(), 1);
}

// ------------------------------------------- §2.1 canonical authored form

#[test]
fn canonical_form_refusals_are_each_named() {
    // Each case carries its OWN dispatch leg list, so an unrelated clause
    // cannot fire first and mask the canonical-form refusal under test.
    let cases: &[(&str, &[&str], CauseCode)] = &[
        (
            "id=l1 legs=a after=- admit=always conclusions=r conclusion_leg=a advance=auto",
            &["a"],
            CauseCode::MalformedStageLine,
        ),
        (
            "stage id=l1 legs=a after=- admit=always conclusions=r conclusion_leg=a advance=auto extra=x",
            &["a"],
            CauseCode::UnknownKey,
        ),
        (
            "stage legs=a id=l1 after=- admit=always conclusions=r conclusion_leg=a advance=auto",
            &["a"],
            CauseCode::KeyOutOfOrder,
        ),
        (
            "stage id=l1 id=l2 legs=a after=- admit=always conclusions=r conclusion_leg=a advance=auto",
            &["a"],
            CauseCode::DuplicateKey,
        ),
        (
            "stage id=l1 legs=a after=- admit=always conclusions=r conclusion_leg=a",
            &["a"],
            CauseCode::MissingKey,
        ),
        (
            "stage id=\"l1\" legs=a after=- admit=always conclusions=r conclusion_leg=a advance=auto",
            &["a"],
            CauseCode::QuotedValue,
        ),
        (
            "stage id=l1 legs=a,,b after=- admit=always conclusions=r conclusion_leg=a advance=auto",
            &["a", "b"],
            CauseCode::EmptyValue,
        ),
        (
            "stage id=l1  legs=a after=- admit=always conclusions=r conclusion_leg=a advance=auto",
            &["a"],
            CauseCode::MalformedStageLine,
        ),
        (
            "stage id=l1 legs=a after=- admit=if-l1-said-so conclusions=r conclusion_leg=a advance=auto",
            &["a"],
            CauseCode::MalformedAdmit,
        ),
    ];
    for (line, legs, expect) in cases {
        refuses(&[line], legs, *expect);
    }
}

// -------------------------------- the derived charset clause: NINE vectors

/// The brief's full nine-vector specimen set, each naming its vector class.
/// **One specimen does not cover a vector class, and a synthetic arm only
/// holds the shapes you imagined — so every one is run against the REAL
/// validator on the REAL byte-specimen.**
const CHARSET_VECTORS: &[(&str, &str)] = &[
    ("../x", "parent-directory traversal"),
    ("a/b", "path separator / nesting"),
    (".", "current-directory selector"),
    ("..", "bare parent selector"),
    ("-lead", "leading `-` (option injection)"),
    ("_lead", "leading `_` (leading separator)"),
    ("Upper", "uppercase outside the charset"),
    ("", "empty id"),
    ("x.", "trailing dot"),
];

#[test]
fn charset_all_nine_stage_id_vectors_refuse() {
    for (specimen, class) in CHARSET_VECTORS {
        let line = format!(
            "stage id={specimen} legs=a after=- admit=always conclusions=r conclusion_leg=a advance=auto"
        );
        let got = check_plan(&inputs(plan_with(&[&line]), &["a"]));
        let Err(refusal) = got else {
            unreachable!(
                "charset vector {class:?} specimen {specimen:?} was ACCEPTED as a stage id"
            );
        };
        // The empty id cannot reach the charset clause: `id=` is refused at
        // the canonical-form boundary first, which is also a refusal and is
        // also named. Every other vector must name the charset clause.
        let acceptable = if specimen.is_empty() {
            [CauseCode::InvalidStageIdCharset, CauseCode::EmptyValue].contains(&refusal.cause_code)
        } else {
            refusal.cause_code == CauseCode::InvalidStageIdCharset
        };
        assert!(
            acceptable,
            "charset vector {class:?} specimen {specimen:?} refused as {} — expected \
             invalid_stage_id_charset",
            refusal.cause_code
        );
    }
}

#[test]
fn charset_all_nine_leg_id_vectors_refuse() {
    for (specimen, class) in CHARSET_VECTORS {
        let line = format!(
            "stage id=l1 legs={specimen} after=- admit=always conclusions=r conclusion_leg={specimen} advance=auto"
        );
        let got = check_plan(&inputs(plan_with(&[&line]), &[specimen, "a"]));
        let Err(refusal) = got else {
            unreachable!("charset vector {class:?} specimen {specimen:?} was ACCEPTED as a leg id");
        };
        let acceptable = if specimen.is_empty() {
            [CauseCode::InvalidLegIdCharset, CauseCode::EmptyValue].contains(&refusal.cause_code)
        } else {
            refusal.cause_code == CauseCode::InvalidLegIdCharset
        };
        assert!(
            acceptable,
            "charset vector {class:?} specimen {specimen:?} refused as {} — expected \
             invalid_leg_id_charset",
            refusal.cause_code
        );
    }
}

#[test]
fn charset_no_accepted_id_can_escape_the_conclusion_prefix() {
    // The property the charset clause exists for. If any vector were
    // accepted, this is the damage it would do.
    for (specimen, class) in CHARSET_VECTORS {
        let line = format!(
            "stage id={specimen} legs=a after=- admit=always conclusions=r conclusion_leg=a advance=auto"
        );
        if let Ok(CheckOutcome::Valid(graph)) = check_plan(&inputs(plan_with(&[&line]), &["a"])) {
            for stage in &graph.order {
                let path = stage.conclusion_path();
                assert!(
                    path.starts_with(".fleet/conclusions/") && !path.contains(".."),
                    "vector {class:?} produced an escaping path {path:?}"
                );
            }
        }
    }
}

// --------------------------------------------- the max_stages ceiling

#[test]
fn a_plan_over_the_dispatch_authored_ceiling_refuses_before_any_leg_is_dispatched() {
    let mut i = inputs(plan_with(&EXEMPLAR), &EXEMPLAR_LEGS);
    i.max_stages = Some(2);
    let Err(refusal) = check_plan(&i) else {
        unreachable!("a three-stage plan was accepted under a ceiling of 2");
    };
    assert_eq!(refusal.cause_code, CauseCode::StageCountExceedsCeiling);
}

#[test]
fn a_plan_at_the_ceiling_is_accepted() {
    let mut i = inputs(plan_with(&EXEMPLAR), &EXEMPLAR_LEGS);
    i.max_stages = Some(3);
    assert!(matches!(check_plan(&i), Ok(CheckOutcome::Valid(_))));
}

// ------------------------------------------------- exit-class discipline

#[test]
fn every_graph_refusal_is_exit_four_and_never_widens_plan_mismatch() {
    // §5.3: "an unknown predecessor, cycle, duplicate leg, invalid
    // conclusion owner/vocabulary shape, invalid condition, or unsupported
    // graph shape is neither absence nor an id naming mismatch."
    let graph_refusals: &[(&[&str], &[&str])] = &[
        (
            &[
                "stage id=l1 legs=a after=zz admit=always conclusions=r conclusion_leg=a advance=auto",
            ],
            &["a"],
        ),
        (
            &[
                "stage id=l1 legs=a after=- admit=always conclusions=r conclusion_leg=a advance=auto",
                "stage id=l2 legs=a after=l1 admit=always conclusions=r conclusion_leg=a advance=auto",
            ],
            &["a"],
        ),
        (
            &[
                "stage id=l1 legs=a after=- admit=always conclusions=r conclusion_leg=b advance=auto",
            ],
            &["a", "b"],
        ),
    ];
    for (lines, legs) in graph_refusals {
        let Err(refusal) = check_plan(&inputs(plan_with(lines), legs)) else {
            unreachable!("a graph-invalid plan was accepted: {lines:?}");
        };
        assert_eq!(
            refusal.exit_class(),
            ExitClass::PlanGraphInvalid,
            "{} took the wrong exit class",
            refusal.cause_code
        );
        assert_eq!(refusal.exit_class().code(), 4);
    }
}

#[test]
fn cause_code_wire_renderings_are_pinned() {
    // A cause code is a wire contract. Renaming a rendering breaks a
    // consumer, so the renderings are pinned here rather than trusted.
    let pinned: &[(CauseCode, &str)] = &[
        (CauseCode::MultipleBlocks, "multiple_blocks"),
        (
            CauseCode::UnsupportedBlockVersion,
            "unsupported_block_version",
        ),
        (CauseCode::DuplicateStageId, "duplicate_stage_id"),
        (CauseCode::DuplicateLegId, "duplicate_leg_id"),
        (CauseCode::UnknownLegId, "unknown_leg_id"),
        (CauseCode::LegNotPlaced, "leg_not_placed"),
        (CauseCode::StageWithoutLegs, "stage_without_legs"),
        (CauseCode::UnknownPredecessor, "unknown_predecessor"),
        (CauseCode::SelfPredecessor, "self_predecessor"),
        (CauseCode::Cycle, "cycle"),
        (CauseCode::NoRoot, "no_root"),
        (CauseCode::MultipleRoots, "multiple_roots"),
        (CauseCode::MultipleSuccessors, "multiple_successors"),
        (CauseCode::MultipleTerminals, "multiple_terminals"),
        (
            CauseCode::ConclusionLegOutsideStage,
            "conclusion_leg_outside_stage",
        ),
        (
            CauseCode::MultiLegConclusionCount,
            "multi_leg_conclusion_count",
        ),
        (CauseCode::MultiValueLegCount, "multi_value_leg_count"),
        (CauseCode::AdmitUnknownStage, "admit_unknown_stage"),
        (CauseCode::AdmitUnreachableStage, "admit_unreachable_stage"),
        (
            CauseCode::AdmitUndeclaredConclusion,
            "admit_undeclared_conclusion",
        ),
        (
            CauseCode::DuplicateConclusionPath,
            "duplicate_conclusion_path",
        ),
        (CauseCode::InvalidAdvance, "invalid_advance"),
        (CauseCode::PlanStateUncommitted, "plan_state_uncommitted"),
        (CauseCode::InvalidStageIdCharset, "invalid_stage_id_charset"),
        (CauseCode::InvalidLegIdCharset, "invalid_leg_id_charset"),
        (
            CauseCode::StageCountExceedsCeiling,
            "stage_count_exceeds_ceiling",
        ),
        (CauseCode::EmptyStageId, "empty_stage_id"),
    ];
    for (code, wire) in pinned {
        assert_eq!(code.as_str(), *wire);
    }
}

// ---------------------------------- clauses subsumed at whole-plan level

/// The three clauses an earlier clause subsumes, each handed a real
/// refusing input DIRECTLY. The design blesses exactly this pattern for
/// acyclicity: "the redundant check makes the refusal named and prevents a
/// future relaxation from silently accepting cycles" (§5.2).
mod subsumed_clauses {
    use aion_fleet_plan::cause::CauseCode;
    use aion_fleet_plan::grammar::{RawStage, parse_stage_line};
    use aion_fleet_plan::validate::{
        PathClaim, check_chain_cardinality, check_conclusion_path_distinctness,
        check_multi_value_leg_count, check_stage_id_nonempty,
    };

    fn raw(line: &str) -> RawStage {
        match parse_stage_line(line) {
            Ok(stage) => stage,
            Err(refusal) => unreachable!("fixture line did not parse: {refusal}"),
        }
    }

    #[test]
    fn clause_02_nonempty_refuses_an_empty_stage_id() {
        let got = check_stage_id_nonempty(&[String::new()]);
        let Err(refusal) = got else {
            unreachable!("an empty stage id was accepted");
        };
        assert_eq!(refusal.cause_code, CauseCode::EmptyStageId);
    }

    #[test]
    fn clause_08_no_root_refuses_a_rootless_set() {
        let stages = vec![raw(
            "stage id=l1 legs=a after=l2 admit=always conclusions=r conclusion_leg=a advance=auto",
        )];
        let Err(refusal) = check_chain_cardinality(&stages) else {
            unreachable!("a rootless stage set was accepted");
        };
        assert_eq!(refusal.cause_code, CauseCode::NoRoot);
    }

    #[test]
    fn clause_08_multiple_terminals_refuse() {
        // Two stages, one root, and the root's successor missing — so both
        // stages are terminal.
        let stages = vec![
            raw(
                "stage id=l1 legs=a after=- admit=always conclusions=r conclusion_leg=a advance=auto",
            ),
            raw(
                "stage id=l2 legs=b after=zz admit=always conclusions=r conclusion_leg=b advance=auto",
            ),
        ];
        let Err(refusal) = check_chain_cardinality(&stages) else {
            unreachable!("a two-terminal graph was accepted");
        };
        assert_eq!(refusal.cause_code, CauseCode::MultipleTerminals);
    }

    #[test]
    fn clause_12_multi_value_multi_leg_refuses() {
        let stages = vec![raw(
            "stage id=build legs=a,b after=- admit=always conclusions=x,y conclusion_leg=a advance=auto",
        )];
        let Err(refusal) = check_multi_value_leg_count(&stages) else {
            unreachable!("a multi-value multi-leg stage was accepted by clause 12");
        };
        assert_eq!(refusal.cause_code, CauseCode::MultiValueLegCount);
    }

    #[test]
    fn clause_15_colliding_conclusion_paths_refuse() {
        let claims = vec![
            PathClaim {
                stage_id: "l1".to_owned(),
                path: ".fleet/conclusions/l1".to_owned(),
            },
            PathClaim {
                stage_id: "l2".to_owned(),
                path: ".fleet/conclusions/L1".to_owned(),
            },
        ];
        let Err(refusal) = check_conclusion_path_distinctness(&claims) else {
            unreachable!("two stages claiming one conclusion path were accepted");
        };
        assert_eq!(refusal.cause_code, CauseCode::DuplicateConclusionPath);
    }

    #[test]
    fn clause_06_a_legless_stage_refuses_at_the_clause() {
        use aion_fleet_plan::validate::check_stage_has_legs;
        let mut stage = raw(
            "stage id=l1 legs=a after=- admit=always conclusions=r conclusion_leg=a advance=auto",
        );
        stage.legs.clear();
        let Err(refusal) = check_stage_has_legs(&[stage]) else {
            unreachable!("a legless stage was accepted");
        };
        assert_eq!(refusal.cause_code, CauseCode::StageWithoutLegs);
    }
}
