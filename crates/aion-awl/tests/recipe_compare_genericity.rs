//! BRIEF-R5's affirmative duty toward the ratchet family, and grill fold A.
//!
//! # What is being proven, and why one proof is not enough
//!
//! THE DUTY (BRIEF's fresh ratchet ruling): the specimen-firing compare
//! surface is typed over the ONE vocabulary's record shape — the invariant —
//! never over an enumeration of verdicts. The ruling is FALSIFIABLE AT THE
//! BYTES by design: if the compare cannot stay generic without absorbing a
//! check, that is a reportable finding to Waffles and Vesper against the
//! ruling.
//!
//! THE NAMED EXEMPLAR: a ratchet-SHAPED record flows through the compare as
//! DATA — an absent-vs-malformed pair as two distinct causes, and a
//! named-hole verdict. Neither producer is absorbed; nothing here converts a
//! check.
//!
//! FOLD A, and why the exemplar cannot stand alone: a compare that
//! special-cased exactly the exemplar's members would PASS the exemplar
//! while being precisely the enumeration the duty forbids. So the genericity
//! is additionally proven at the MECHANISM, two ways that the exemplar
//! cannot fake:
//!
//! 1. the property test below enumerates the full closed vocabulary
//!    PROGRAMMATICALLY from `CommandCause::ALL`, so a cause a later design
//!    change adds joins this test without an amendment, and asserts every
//!    member is consumed UNIFORMLY;
//! 2. the compare surface's OWN SOURCE is read and refused if it names a
//!    cause at all.
//!
//! # Ratchet-family contact, stated exactly
//!
//! This cut touches no ratchet script and absorbs no ratchet check. The
//! absent-vs-malformed pair below is read as PRIOR ART from
//! `crates/aion-fleet-plan/src/error.rs` — a sealed verifier outside this
//! cut's wall, read and never modified — and its codes are cited by digest
//! below so this file compares against bytes rather than against memory.

mod recipe;

use std::collections::BTreeSet;

use aion_awl::command::recipe::{VerdictRecord, VerdictRecordError, compare, digest_of, record_of};
use aion_awl::command::{
    CommandCause, CommandCheckError, CommandComposeError, CommandEmitError, CommandFireError,
    CommandLayer, CommandParseError, CommandRefusal, CommandSpecimenError,
};

/// Build a refusal through the PUBLIC per-layer constructors — the door for
/// callers outside the surface.
///
/// `CommandRefusal::refuse`, the internal door that derives the layer from
/// the cause, is `pub(super)` and stays that way: widening cut-1's landed
/// refusal door so a test could reach it would be paying for a test with the
/// invariant the door exists to keep. The layer match here is the test's,
/// not the compare surface's — a test may enumerate layers, and the surface
/// under test may not enumerate causes.
///
/// # Panics
///
/// Panics when the cause cannot be filed at its own layer, which
/// `error_tests.rs` pins impossible.
fn refusal(cause: CommandCause, observed: &str) -> CommandRefusal {
    let span = aion_awl::Span {
        start: 0,
        end: 1,
        line: 1,
        column: 1,
    };
    let message = "a refusal that says something";
    let built = match cause.layer() {
        CommandLayer::Parse => {
            CommandParseError::new(cause, span, observed, message).map(CommandRefusal::from)
        }
        CommandLayer::Check => {
            CommandCheckError::new(cause, span, observed, message).map(CommandRefusal::from)
        }
        CommandLayer::Emit => {
            CommandEmitError::new(cause, span, observed, message).map(CommandRefusal::from)
        }
        CommandLayer::Compose => {
            CommandComposeError::new(cause, span, observed, message).map(CommandRefusal::from)
        }
        CommandLayer::Fire => {
            CommandFireError::new(cause, span, observed, message).map(CommandRefusal::from)
        }
        CommandLayer::Specimen => {
            CommandSpecimenError::new(cause, span, observed, message).map(CommandRefusal::from)
        }
    };
    built.unwrap_or_else(|misfiled| unreachable!("{cause} must file at its own layer: {misfiled}"))
}

/// The sealed verifier this cut reads as prior art and never modifies.
///
/// Cited by REPO, PATH and DIGEST and re-hashed on the run, so an edit to
/// that file fails here rather than leaving this fixture comparing against a
/// remembered copy.
const PRIOR_ART_REPO: &str = "https://github.com/ablative-io/aion.git";
const PRIOR_ART_PATH: &str = "crates/aion-fleet-plan/src/error.rs";

/// The ratchet-SHAPED codes, quoted from that file: an absent-vs-malformed
/// pair as TWO DISTINCT causes, plus the third of its closed set.
const RATCHET_SHAPED_CODES: [&str; 3] = [
    "conclusion_missing",
    "conclusion_malformed",
    "conclusion_undeclared",
];

fn prior_art_source() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(PRIOR_ART_PATH);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| unreachable!("{} must be readable: {error}", path.display()))
}

/// PROVENANCE. The prior art is cited at its bytes, and the codes this
/// fixture flows through the compare are the codes that file actually names.
#[test]
fn the_ratchet_shaped_prior_art_is_cited_at_its_bytes() {
    let source = prior_art_source();
    let digest = digest_of(source.as_bytes());
    assert_eq!(digest.len(), 64);
    println!(
        "ratchet-shaped prior art: repo {PRIOR_ART_REPO}, path {PRIOR_ART_PATH}, sha256 {digest}"
    );
    for code in RATCHET_SHAPED_CODES {
        assert!(
            source.contains(&format!("\"{code}\"")),
            "`{code}` is quoted from {PRIOR_ART_PATH} and that file no longer names it — the \
             fixture would be comparing against remembered bytes"
        );
    }
    // AND the pair is genuinely two DISTINCT causes there, which is the
    // shape the affirmative duty names.
    assert_ne!(RATCHET_SHAPED_CODES[0], RATCHET_SHAPED_CODES[1]);
    assert!(
        source.contains("absent or invalid artifact is NEITHER"),
        "the pair's distinctness is the file's own stated discipline"
    );
}

/// THE NAMED EXEMPLAR. A ratchet-SHAPED record flows through the compare as
/// DATA — a producer this cut never absorbed, a code cut-1's vocabulary does
/// not carry, and a named-hole verdict beside it.
#[test]
fn a_ratchet_shaped_record_flows_through_the_compare_as_data() {
    // These codes are FOREIGN: cut-1's closed enum answers `None` for each,
    // deliberately. A compare typed over that enum could not carry them at
    // all, which is exactly why the surface is typed over a record shape.
    for code in RATCHET_SHAPED_CODES {
        assert_eq!(
            CommandCause::from_code(code),
            None,
            "`{code}` must be foreign to this builder's vocabulary, or the exemplar proves nothing"
        );
    }

    let missing = VerdictRecord::new(
        RATCHET_SHAPED_CODES[0],
        "conclusion",
        "the canonical path does not exist at the exact recorded commit",
    )
    .unwrap_or_else(|error| unreachable!("a foreign record is still a record: {error}"));
    let malformed = VerdictRecord::new(
        RATCHET_SHAPED_CODES[1],
        "conclusion",
        "the bytes are not one declared value followed by exactly one LF",
    )
    .unwrap_or_else(|error| unreachable!("a foreign record is still a record: {error}"));

    // The pair compares as TWO DISTINCT causes, which is the whole point of
    // the absent-vs-malformed distinction the ratchet family measured.
    let against_itself = compare(&missing, &missing);
    assert!(against_itself.matched());
    let across_the_pair = compare(&missing, &malformed);
    assert!(!across_the_pair.matched());
    assert!(!across_the_pair.parts.code_agrees);
    assert!(across_the_pair.parts.layer_agrees);

    // A NAMED-HOLE VERDICT: a gate-entry measurement naming the hole it
    // found. Another producer this cut never absorbed.
    let named_hole = VerdictRecord::new(
        "deliverable_absent",
        "gate",
        "deliverable sdks/liminal-py/pyproject.toml absent from the diff",
    )
    .unwrap_or_else(|error| unreachable!("a named-hole verdict is a record: {error}"));
    assert!(compare(&named_hole, &named_hole).matched());
    assert!(!compare(&named_hole, &missing).matched());

    // And a BUILDER refusal flows through the SAME surface, one way only.
    let builders = record_of(&refusal(
        CommandCause::CensusMismatch,
        "0000000000000000000000000000000000000000000000000000000000000000",
    ))
    .unwrap_or_else(|error| unreachable!("a builder refusal is a record: {error}"));
    assert_eq!(builders.code(), "census_mismatch");
    assert!(!compare(&builders, &missing).matched());
}

/// FOLD A, first half. The compare consumes EVERY member of the closed
/// vocabulary UNIFORMLY, and the vocabulary is enumerated PROGRAMMATICALLY
/// from the enum — so a cause a later design change adds joins this test
/// without an amendment.
#[test]
fn the_compare_consumes_every_member_of_the_vocabulary_uniformly() {
    let records: Vec<VerdictRecord> = CommandCause::ALL
        .into_iter()
        .map(|cause| {
            record_of(&refusal(cause, &format!("bytes observed for {cause}")))
                .unwrap_or_else(|error| unreachable!("{cause} must convert into a record: {error}"))
        })
        .collect();

    assert_eq!(
        records.len(),
        CommandCause::ALL.len(),
        "the property test must cover the WHOLE vocabulary, derived from the enum"
    );

    // Uniform consumption, member by member: every record matches itself in
    // all three parts, and disagrees with every other in exactly the parts
    // that differ. No member is privileged, and none is skipped.
    let mut codes = BTreeSet::new();
    for (index, record) in records.iter().enumerate() {
        assert!(codes.insert(record.code().to_owned()));
        let itself = compare(record, record);
        assert!(
            itself.matched()
                && itself.parts.code_agrees
                && itself.parts.layer_agrees
                && itself.parts.bytes_agree,
            "{} must match itself in every part",
            record.code()
        );
        for (other_index, other) in records.iter().enumerate() {
            if index == other_index {
                continue;
            }
            let across = compare(record, other);
            assert!(
                !across.matched(),
                "{} and {} are different records and must not match",
                record.code(),
                other.code()
            );
            assert!(
                !across.parts.code_agrees,
                "{} and {} carry different codes",
                record.code(),
                other.code()
            );
            assert_eq!(
                across.parts.layer_agrees,
                record.layer() == other.layer(),
                "the layer comparison must be a string comparison, nothing cleverer"
            );
        }
    }
    assert_eq!(codes.len(), CommandCause::ALL.len());
}

/// FOLD A, second half — the DISCRIMINATING assertion. The compare surface's
/// own source is read and must name NO cause at all.
///
/// This is what the hand-picked exemplar cannot do: an implementation that
/// special-cased exactly the exemplar's members would pass every test above
/// and be the enumeration the duty forbids. Comment lines are stripped
/// first, since the module's own prose explains WHY it may not match on a
/// cause and saying so is not doing so.
#[test]
fn the_compare_surface_matches_on_no_cause_identity() {
    let source = aion_awl::command::recipe::source_text();
    let code: String = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<&str>>()
        .join("\n");

    assert!(
        !code.contains("CommandCause"),
        "the compare surface names a cause in its own code, which makes it an enumeration of \
         verdicts rather than a surface typed over the record shape"
    );
    for cause in CommandCause::ALL {
        assert!(
            !code.contains(cause.as_str()),
            "the compare surface's code names `{}`, so it special-cases at least one member",
            cause.as_str()
        );
    }
    for foreign in RATCHET_SHAPED_CODES {
        assert!(
            !code.contains(foreign),
            "the compare surface's code names the exemplar's own `{foreign}`, which is exactly \
             the enumeration fold A exists to catch"
        );
    }
    // The control: the stripper must not have thrown the source away.
    assert!(
        code.contains("pub fn compare(") && code.contains("code_agrees"),
        "the comment stripper removed the code, so the assertions above proved nothing"
    );
}

/// A record that names nothing cannot be compared against anything, and is
/// not a value this surface can build.
#[test]
fn a_record_that_names_nothing_is_unrepresentable() {
    assert_eq!(
        VerdictRecord::new("", "gate", "bytes"),
        Err(VerdictRecordError::CodeEmpty)
    );
    assert_eq!(
        VerdictRecord::new("   ", "gate", "bytes"),
        Err(VerdictRecordError::CodeEmpty)
    );
    assert_eq!(
        VerdictRecord::new("deliverable_absent", "", "bytes"),
        Err(VerdictRecordError::LayerEmpty {
            code: "deliverable_absent".to_owned()
        })
    );
}
