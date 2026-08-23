//! What a fill must refuse, and only then what it must accept.
//!
//! RED FIRST: every refusal below is written before the acceptance tests that
//! follow it. A template language that accepts everything is easy; the value
//! of this one is entirely in what it will not do.

use crate::Span;

use super::super::cause::{CommandCause, CommandLayer};
use super::{Fill, FillSegment};

fn span_for(source: &str) -> Span {
    Span {
        start: 0,
        end: source.len() + 2,
        line: 1,
        column: 1,
    }
}

fn refusal(source: &str) -> (CommandCause, CommandLayer, String) {
    match Fill::parse(source, span_for(source)) {
        Ok(fill) => unreachable!("`{source}` must be refused; it parsed to {fill:?}"),
        Err(refusal) => (
            refusal.cause(),
            refusal.layer(),
            refusal.observed_bytes().to_owned(),
        ),
    }
}

/// RED: a backtick is command substitution. It runs a shell at evaluation
/// time, which the determinism boundary does not admit, so it is refused
/// where it is written rather than checked for later.
#[test]
fn a_backtick_is_refused_at_the_parse_layer_with_its_own_cause() {
    for source in [
        "`git rev-parse HEAD`",
        "prefix-`whoami`",
        "{{base}}..`git rev-parse HEAD`",
        "`",
    ] {
        let (cause, layer, observed) = refusal(source);
        assert_eq!(
            cause,
            CommandCause::BacktickSubstitution,
            "`{source}` must refuse as backtick substitution"
        );
        assert_eq!(layer, CommandLayer::Parse);
        assert_eq!(observed, source, "the refusal must carry the bytes it saw");
    }
}

/// RED: an interpolation that never closes.
#[test]
fn an_unterminated_interpolation_is_refused() {
    for source in ["{{base", "a{{base}}b{{tail", "{{"] {
        let (cause, layer, observed) = refusal(source);
        assert_eq!(cause, CommandCause::InterpolationUnterminated, "{source}");
        assert_eq!(layer, CommandLayer::Parse);
        assert_eq!(observed, source);
    }
}

/// RED: an interpolation that names nothing.
#[test]
fn an_empty_interpolation_is_refused() {
    for source in ["{{}}", "a{{}}b", "{{   }}"] {
        let (cause, _, observed) = refusal(source);
        assert_eq!(cause, CommandCause::InterpolationEmpty, "{source}");
        assert_eq!(observed, source);
    }
}

/// RED: `{{…}}` is not an expression slot. Anything that is not a plain
/// `snake_case` parameter name is refused, which is what keeps this from
/// drifting into a templating language one convenience at a time.
#[test]
fn an_interpolation_that_is_not_a_bare_parameter_name_is_refused() {
    for source in [
        "{{Base}}",
        "{{BASE}}",
        "{{base.sha}}",
        "{{base-pin}}",
        "{{base pin}}",
        "{{env('HOME')}}",
        "{{1base}}",
        "{{base | upper}}",
        "{{ if x }}",
        "{{_base}}",
    ] {
        let (cause, layer, observed) = refusal(source);
        assert_eq!(
            cause,
            CommandCause::InterpolationMalformed,
            "`{source}` must be refused: a fill has no expressions"
        );
        assert_eq!(layer, CommandLayer::Parse);
        assert_eq!(observed, source);
    }
}

/// A fill with no interpolation is one literal segment.
#[test]
fn a_plain_literal_is_one_segment() {
    let source = "--name-only";
    let fill = match Fill::parse(source, span_for(source)) {
        Ok(fill) => fill,
        Err(refusal) => unreachable!("`{source}` must parse: {}", refusal.rendered()),
    };
    assert_eq!(
        fill.segments,
        vec![FillSegment::Literal("--name-only".to_owned())]
    );
    assert_eq!(fill.sole_hole(), None);
    assert_eq!(fill.literals(), vec!["--name-only"]);
    assert!(fill.holes().is_empty());
}

/// A fill that is exactly one hole carries its parameter's own type through;
/// that shape is the only one a list may stand in.
#[test]
fn a_sole_hole_is_recognised_as_such() {
    let source = "{{base_pin}}";
    let fill = match Fill::parse(source, span_for(source)) {
        Ok(fill) => fill,
        Err(refusal) => unreachable!("must parse: {}", refusal.rendered()),
    };
    assert_eq!(fill.sole_hole(), Some("base_pin"));
    assert_eq!(fill.holes().len(), 1);
    assert!(fill.literals().is_empty());
}

/// Mixed text and holes: literal, hole, literal, in source order.
#[test]
fn a_mixed_fill_keeps_its_segments_in_source_order() {
    let source = "{{base_pin}}..HEAD";
    let fill = match Fill::parse(source, span_for(source)) {
        Ok(fill) => fill,
        Err(refusal) => unreachable!("must parse: {}", refusal.rendered()),
    };
    assert_eq!(fill.segments.len(), 2);
    assert_eq!(
        fill.sole_hole(),
        None,
        "text beside a hole makes it a String"
    );
    assert_eq!(fill.literals(), vec!["..HEAD"]);
    assert_eq!(
        fill.holes()
            .into_iter()
            .map(|(name, _)| name)
            .collect::<Vec<_>>(),
        vec!["base_pin"]
    );

    let source = "a{{one}}b{{two}}c";
    let fill = match Fill::parse(source, span_for(source)) {
        Ok(fill) => fill,
        Err(refusal) => unreachable!("must parse: {}", refusal.rendered()),
    };
    assert_eq!(fill.literals(), vec!["a", "b", "c"]);
    assert_eq!(
        fill.holes()
            .into_iter()
            .map(|(name, _)| name)
            .collect::<Vec<_>>(),
        vec!["one", "two"]
    );
}

/// A single brace is literal text: `{workspace_root}` is a cwd placeholder
/// read by an entirely different rule and must not be mistaken for a hole.
#[test]
fn a_single_brace_is_literal_text() {
    let source = "{workspace_root}/repo";
    let fill = match Fill::parse(source, span_for(source)) {
        Ok(fill) => fill,
        Err(refusal) => unreachable!("must parse: {}", refusal.rendered()),
    };
    assert_eq!(
        fill.segments,
        vec![FillSegment::Literal("{workspace_root}/repo".to_owned())]
    );
}

/// A hole's span narrows into the literal when the mapping is unambiguous, so
/// an editor underlines the interpolation rather than the whole string.
#[test]
fn a_hole_span_narrows_into_the_literal_when_it_can() {
    let source = "ab{{one}}";
    let outer = Span {
        start: 100,
        end: 100 + source.len() + 2,
        line: 7,
        column: 3,
    };
    let fill = match Fill::parse(source, outer) {
        Ok(fill) => fill,
        Err(refusal) => unreachable!("must parse: {}", refusal.rendered()),
    };
    let holes = fill.holes();
    let Some((name, span)) = holes.first() else {
        unreachable!("the fill carries one hole");
    };
    assert_eq!(*name, "one");
    assert_eq!(span.start, 100 + 1 + 2, "`{{{{` sits two bytes into `ab…`");
    assert_eq!(span.end, span.start + "{{one}}".len());
    assert_eq!(span.line, 7);

    // An escaped literal makes the mapping unknown; the whole literal's span
    // is then used, which is imprecise but never wrong.
    let escaped = Span {
        start: 100,
        end: 100 + source.len() + 6,
        line: 7,
        column: 3,
    };
    let fill = match Fill::parse(source, escaped) {
        Ok(fill) => fill,
        Err(refusal) => unreachable!("must parse: {}", refusal.rendered()),
    };
    let holes = fill.holes();
    let Some((_, span)) = holes.first() else {
        unreachable!("the fill carries one hole");
    };
    assert_eq!(*span, escaped);
}

/// DETERMINISM: the same fill text parses to the same segments every time,
/// and a fill has no input but its parameters — asserted, not assumed.
#[test]
fn the_same_fill_parses_identically_every_time() {
    let source = "{{base_pin}}..HEAD --{{tail}}";
    let first = Fill::parse(source, span_for(source));
    let second = Fill::parse(source, span_for(source));
    assert_eq!(first, second);
    let Ok(fill) = first else {
        unreachable!("must parse");
    };
    assert!(fill.is_deterministic());
}
