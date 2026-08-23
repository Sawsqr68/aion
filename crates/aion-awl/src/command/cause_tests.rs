//! What the closed cause vocabulary must satisfy.

use std::collections::BTreeSet;

use super::{CommandCause, CommandLayer};

/// `ALL` is how every consumer iterates the vocabulary, so it must carry
/// every variant. The layer function is an exhaustive match with no wildcard,
/// so a variant added without a layer fails to COMPILE; this then refuses an
/// `ALL` that does not carry it.
#[test]
fn the_inventory_carries_every_variant_exactly_once() {
    let mut seen = BTreeSet::new();
    for cause in CommandCause::ALL {
        assert!(
            seen.insert(cause),
            "{cause} appears twice in CommandCause::ALL"
        );
    }
    assert_eq!(seen.len(), CommandCause::ALL.len());
}

/// The wire strings are the contract: distinct, `snake_case`, and never empty.
#[test]
fn every_wire_string_is_distinct_and_shaped_like_a_cause_code() {
    let mut codes = BTreeSet::new();
    for cause in CommandCause::ALL {
        let code = cause.as_str();
        assert!(
            codes.insert(code),
            "cause code `{code}` is claimed by two variants"
        );
        assert!(!code.is_empty(), "{cause:?} renders an empty wire string");
        assert!(
            code.chars()
                .all(|character| character.is_ascii_lowercase() || character == '_'),
            "`{code}` is not snake_case"
        );
        assert!(
            !code.starts_with('_') && !code.ends_with('_'),
            "`{code}` is not a finished code"
        );
    }
}

/// Every code round-trips through its own wire string, and a string that
/// names no cause resolves to nothing rather than to a nearest match.
#[test]
fn a_cause_round_trips_through_its_wire_string() {
    for cause in CommandCause::ALL {
        assert_eq!(CommandCause::from_code(cause.as_str()), Some(cause));
    }
    assert_eq!(CommandCause::from_code("definitely_not_a_cause"), None);
    assert_eq!(CommandCause::from_code(""), None);
}

/// The layer partition is total and non-empty: every cause has a layer, and
/// every layer files at least one cause. A layer with no causes is a
/// discipline nothing enforces.
#[test]
fn every_layer_files_at_least_one_cause_and_every_cause_has_a_layer() {
    for layer in CommandLayer::ALL {
        let filed = CommandCause::ALL
            .into_iter()
            .filter(|cause| cause.layer() == layer)
            .count();
        assert!(filed > 0, "no cause is filed at the {layer} layer");
    }
    let counted: usize = CommandLayer::ALL
        .into_iter()
        .map(|layer| {
            CommandCause::ALL
                .into_iter()
                .filter(|cause| cause.layer() == layer)
                .count()
        })
        .sum();
    assert_eq!(
        counted,
        CommandCause::ALL.len(),
        "the layers partition the vocabulary: every cause exactly once"
    );
}

/// There is no catch-all. The whole point of a closed vocabulary is that a
/// refusal names something specific, so a code that reads like a shrug must
/// not exist — this pins the absence rather than trusting the review that
/// last read the list.
#[test]
fn the_vocabulary_carries_no_catch_all_code() {
    for cause in CommandCause::ALL {
        let code = cause.as_str();
        assert!(
            !matches!(
                code,
                "other" | "unknown" | "error" | "failed" | "invalid" | "bad" | "misc"
            ),
            "`{code}` is a catch-all; a refusal must name what it refused"
        );
    }
}

/// The layers themselves render distinct, non-empty wire strings.
#[test]
fn layer_wire_strings_are_distinct() {
    let mut seen = BTreeSet::new();
    for layer in CommandLayer::ALL {
        assert!(seen.insert(layer.as_str()));
        assert!(!layer.as_str().is_empty());
    }
    assert_eq!(seen.len(), CommandLayer::ALL.len());
}
