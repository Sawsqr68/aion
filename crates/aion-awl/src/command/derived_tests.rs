//! What the derived-shape rule must catch, and — just as load-bearing — what
//! it must leave alone.

use super::{DerivedShape, derived_shape_in};

/// RED: the three named shapes are caught, wherever they sit in a literal.
#[test]
fn the_three_derived_shapes_are_caught() {
    let pin = "5e2fb5a3ed32b53d48dbc585aa914acd056458c6";
    assert_eq!(
        derived_shape_in(pin),
        Some((DerivedShape::GitObjectName, pin))
    );
    assert_eq!(
        derived_shape_in(&format!("{pin}..HEAD")),
        Some((DerivedShape::GitObjectName, pin)),
        "a pin embedded in a revision range is still a pin"
    );

    let digest = "1e1e3ad2d189b5b2c8fb07b6e302e5cc3505ac7fa7f84ac5561020e30ff10a1e";
    assert_eq!(
        derived_shape_in(digest),
        Some((DerivedShape::ContentDigest, digest))
    );

    let token = "awlcut1-ctl-4d057098-waffles-20260823";
    assert_eq!(
        derived_shape_in(token),
        Some((DerivedShape::ControlToken, token))
    );
    assert_eq!(
        derived_shape_in(&format!("--token={token}")),
        Some((DerivedShape::ControlToken, token))
    );
}

/// The rule must not guess. A rule that refused ordinary text would be worse
/// than no rule, because it would teach authors to route around it.
#[test]
fn ordinary_text_is_left_alone() {
    for literal in [
        "",
        "HEAD",
        "main",
        "--name-only",
        "{{base_pin}}..HEAD",
        "/home/rocketfish/fleet/runs",
        "abc123",
        "deadbeef",
        "5e2fb5a3",
        "5e2fb5a3ed32b53d48dbc585aa914acd056458c",
        "5E2FB5A3ED32B53D48DBC585AA914ACD056458C6",
        "5e2fb5a3ed32b53d48dbc585aa914acd056458c6a",
        "awlcut1-ctl-4d057098-waffles",
        "awlcut1-ctl-4d05709-waffles-20260823",
        "awlcut1-ctl-4d057098-waffles-2026082x",
        "awlcut1-plan-4d057098-waffles-20260823",
    ] {
        assert_eq!(
            derived_shape_in(literal),
            None,
            "`{literal}` is ordinary text and must not be refused"
        );
    }
}

/// Every shape says what derives it, so the refusal teaches the cure.
#[test]
fn every_shape_names_its_derivation() {
    for shape in [
        DerivedShape::GitObjectName,
        DerivedShape::ContentDigest,
        DerivedShape::ControlToken,
    ] {
        assert!(!shape.derivation().is_empty());
        assert!(shape.derivation().contains("never transcribed"));
    }
}
