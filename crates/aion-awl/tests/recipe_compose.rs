//! BRIEF-R3 (DESIGN-R4a, L2): refuse-on-unfilled-slot, typed, and
//! sweep-by-claim as a builder function with its own record.
//!
//! RED FIRST: the refusals stand before the passing path.
//!
//! Every corpus citation below names its source REPO, PATH and DIGEST, and
//! the digests are re-hashed on the run by `recipe_corpus.rs` — a specimen
//! edited after this file was written fails there rather than passing here
//! against a stale copy.

mod recipe;

use aion_awl::command::recipe::{Composition, compose, is_unfilled_sentinel, sweep_by_claim};
use aion_awl::command::{CommandCause, CommandLayer};

use recipe::support::{checked, document};

fn stage_document(parameters: &str) -> String {
    document(&format!(
        "command push_branch\n  program \"git\" \"push\"\n  admits \"--\"\n\n\
         recipe stage\n{parameters}\n  step push\n    runs command push_branch"
    ))
}

// ─────────────────────────────────────────────────────────────────── RED

/// A slot nothing filled refuses BY NAME, carrying the slot's own name as
/// the bytes observed — because nothing was seen, and the thing that was
/// missing is what a reader must be handed.
///
/// This is `build-input.py`'s exit-2 behaviour, absorbed and typed. Measured
/// at the pinned bytes: `sleepreap.build-input.py` (docs repo `3c7cbb6`,
/// `dispatch-rigs/sleepreap/build-input.py`,
/// sha256 `032ee3d7598ac361a731a5f0917b47a6b1ea7c32c3cc3b9670b3c11c32db23b7`)
/// guards its argument count at lines 19–21 and exits 2, while
/// `liminal22.build-input.py` (`dispatch-rigs/liminal22/build-input.py`,
/// sha256 `b51f60f780e542dffbaa6879db2294ccd3fa050e8138b33e87e1ef3cd0d06351`)
/// has NO argv guard at line 15 and raises an `IndexError` for the same
/// mistake. Two behaviours become one typed refusal, and the normalization
/// is the point rather than a rounding.
#[test]
fn an_unfilled_slot_refuses_with_the_slot_name_in_observed_bytes() {
    let parsed = checked(&stage_document("  parameter docs_root: String per_item"));
    let Err(refusals) = compose(&parsed.recipes[0], &Composition::new()) else {
        unreachable!("a slot nothing filled must refuse");
    };
    let refusal = &refusals[0];
    assert_eq!(refusal.cause(), CommandCause::SlotUnfilled);
    assert_eq!(refusal.layer(), CommandLayer::Compose);
    assert_eq!(
        refusal.observed_bytes(),
        "docs_root",
        "the slot's own name is what was observed missing"
    );
    assert!(
        refusal.message().contains("`docs_root`"),
        "the refusal must name the slot; got: {}",
        refusal.message()
    );
}

/// A bound value still carrying a skeleton's sentinel is a template copied
/// and not filled. The sentinels are the corpus's own: `skeleton.input.json`
/// (docs repo `3c7cbb6`, `dispatch-rigs/issue-investigations/skeleton.input.json`,
/// sha256 `4e90bc3bf2e3c2f74546c794cc4b87b183fcf60672065668720552edb2eb1218`)
/// carries `<<ISSUE_NUMBER>>`, `<<ISSUE_TITLE>>`, `<<TRUTHFUL_TRIGGER …>>`
/// and `<<RE-DERIVE AT THE DISPATCH MINUTE …>>`; the speech-1 KIT
/// (`dispatch-rigs/speech-1-python-wire/KIT-input-template.json`,
/// sha256 `1a95315217b431ae7d1956183da51098e02ddc671d3e7ab737d0593df429f5ed`)
/// carries `__CTL_TOKEN__`, `__RIG_COMMIT__` and `__CUT_COMMIT__`.
#[test]
fn a_bound_value_still_carrying_its_sentinel_refuses() {
    let parsed = checked(&stage_document("  parameter issue: String per_item"));
    for sentinel in [
        "<<ISSUE_NUMBER>>",
        "<<ISSUE_TITLE>>",
        "<<TRUTHFUL_TRIGGER e.g. tom-ask-YYYYMMDD>>",
        "__CTL_TOKEN__",
        "__RIG_COMMIT__",
        "__CUT_COMMIT__",
    ] {
        let composition = Composition::new().with("issue", sentinel);
        let Err(refusals) = compose(&parsed.recipes[0], &composition) else {
            unreachable!("`{sentinel}` is an unfilled slot and must refuse");
        };
        assert_eq!(refusals[0].cause(), CommandCause::SlotUnfilled);
        assert_eq!(
            refusals[0].observed_bytes(),
            sentinel,
            "the sentinel is what was actually seen"
        );
    }
}

/// The sentinel rule must be able to say NO, or the refusal above says only
/// that the check ran.
#[test]
fn the_sentinel_rule_reads_a_real_value_as_a_real_value() {
    for real in [
        "build",
        "aion-workflows-vm",
        "https://github.com/ablative-io/aion.git",
        "__",
        "<<>>",
        "a <<PLACEHOLDER>> inside a sentence",
    ] {
        assert!(
            !is_unfilled_sentinel(real),
            "`{real}` is a value, not a sentinel"
        );
    }
    assert!(is_unfilled_sentinel("<<ISSUE_NUMBER>>"));
    assert!(is_unfilled_sentinel("  __CTL_TOKEN__  "));
}

/// Every unfilled slot is reported at once, so an author filling one at a
/// time is not an author running the composer five times.
#[test]
fn every_unfilled_slot_is_reported_at_once() {
    let parsed = checked(&stage_document(
        "  parameter one: String per_item\n  parameter two: String per_item\n  \
         parameter three: String per_item",
    ));
    let Err(refusals) = compose(&parsed.recipes[0], &Composition::new()) else {
        unreachable!("three unfilled slots must refuse");
    };
    assert_eq!(refusals.len(), 3);
}

// ───────────────────────────────────────────────────────────────── GREEN

/// A composition that fills every slot resolves, and leaves exactly the
/// re-derive slots for the fire minute.
#[test]
fn a_filled_composition_resolves_and_defers_only_the_re_derive_slots() {
    let parsed = checked(&document(recipe::support::STAGING));
    let composed = compose(
        &parsed.recipes[0],
        &Composition::new().with("branch", "fleet/awl-cut2-result"),
    )
    .unwrap_or_else(|error| unreachable!("every non-re-derive slot is filled: {error:?}"));
    assert_eq!(
        composed.resolved.get("branch").map(String::as_str),
        Some("fleet/awl-cut2-result")
    );
    assert_eq!(composed.deferred, vec!["control_expect".to_owned()]);
}

/// A declared default fills a slot the composition omits. ABSENCE IS A LEGAL
/// DECLARED STATE where the declaration made it one.
#[test]
fn a_declared_default_fills_a_slot_the_composition_omits() {
    let parsed = checked(&stage_document(
        "  parameter tracker: String = \"github\" skeleton",
    ));
    let composed = compose(&parsed.recipes[0], &Composition::new())
        .unwrap_or_else(|error| unreachable!("the default fills the slot: {error:?}"));
    assert_eq!(
        composed.resolved.get("tracker").map(String::as_str),
        Some("github")
    );
}

/// SWEEP BY CLAIM. Three consumers, one source, one correction, one record
/// listing all three — the found-in-one-lives-in-three class closed by
/// construction rather than by a reviewer's luck.
#[test]
fn a_correction_propagates_to_every_consumer_with_a_sweep_record_listing_them() {
    let source = document(
        "command push_branch\n  program \"git\" \"push\"\n  admits \"--\"\n\n\
         artifact gate_entry \".fleet/gate-entry.sh\"\n  \
         digest \"1e1e3ad2ae66bb0bef5b82a8e154491032ee3d7598ac361a731a5f0917b47a6b\"\n\n\
         recipe first_consumer\n  parameter branch: String per_item\n  \
         step self_test\n    fires artifact gate_entry expecting \"deliverable_absent\"\n\n\
         recipe second_consumer\n  parameter branch: String per_item\n  \
         step read_it\n    captures gate: String of artifact gate_entry\n\n\
         recipe third_consumer\n  \
         parameter gate_line: String re_derive derives \".fleet/gate-entry.sh\" extract \"line 1\"\n  \
         step push\n    runs command push_branch",
    );
    let parsed = checked(&source);
    let record = sweep_by_claim(
        &parsed.recipes,
        &parsed.walls,
        &parsed.artifacts,
        "gate_entry",
        "77f0333fcea03ada0c017af9fdea725231c3e2aecce8ade71781571daff9115d",
    );
    assert_eq!(
        record.reprojected.len(),
        2,
        "both artifact consumers must be reached: {:#?}",
        record.reprojected
    );
    let byname = sweep_by_claim(
        &parsed.recipes,
        &parsed.walls,
        &parsed.artifacts,
        ".fleet/gate-entry.sh",
        "a corrected gate script",
    );
    assert_eq!(
        byname.reprojected.len(),
        1,
        "the derivation consumer is reached by its declared SOURCE ADDRESS: {:#?}",
        byname.reprojected
    );

    let reached: Vec<&str> = record
        .reprojected
        .iter()
        .map(|input| input.recipe.as_str())
        .chain(byname.reprojected.iter().map(|input| input.recipe.as_str()))
        .collect();
    assert_eq!(
        reached,
        vec!["first_consumer", "second_consumer", "third_consumer"],
        "all three consumers of the one source must be listed"
    );

    // THE COMPLETENESS CLAIM NAMES ITS OWN DERIVATION. A claim that said
    // only "this list is complete" would be the assertion the mechanism
    // exists to replace.
    assert!(
        record
            .completeness
            .contains("assembly is FROM declarations")
            && record.completeness.contains("NAMING")
            && record.completeness.contains("edge set"),
        "the sweep record must name the derivation of its completeness; got: {}",
        record.completeness
    );
    assert!(
        record.rendered().contains("completeness:"),
        "the rendered record must carry the claim a reader checks"
    );
    assert_eq!(
        record.correction_digest.len(),
        64,
        "the record names what it swept against by digest"
    );
}

/// The sweep must be able to reach NOTHING, or a green above would mean only
/// that it always answers.
#[test]
fn a_source_no_recipe_names_is_swept_to_an_empty_list() {
    let parsed = checked(&document(recipe::support::STAGING));
    let record = sweep_by_claim(
        &parsed.recipes,
        &parsed.walls,
        &parsed.artifacts,
        "an_artifact_nothing_names",
        "corrected",
    );
    assert!(record.reprojected.is_empty());
}
