//! BRIEF-R2 (DESIGN-R4b): parameter kinds with declared derivations, and the
//! fire-minute law that makes compose/fire drift impossible rather than
//! checked-for.
//!
//! RED FIRST: every refusal stands before the passing path it protects.

mod recipe;

use std::fmt::Write as _;

use aion_awl::command::CommandCause;
use aion_awl::command::recipe::{
    Composition, Extraction, ParameterKind, compose, parse_extraction, resolve,
};

use recipe::support::{StatedTree, checked, document, refusals};

/// The named refusal a document produces, if any.
fn named(source: &str, cause: CommandCause) -> Option<String> {
    let marker = format!("named cause: {}", cause.as_str());
    refusals(source)
        .into_iter()
        .find(|text| text.contains(&marker))
}

fn recipe_document(body: &str) -> String {
    document(&format!(
        "command push_branch\n  program \"git\" \"push\"\n  admits \"--\"\n\n\
         recipe stage\n{body}\n  step push\n    runs command push_branch"
    ))
}

// ─────────────────────────────────────────────────────────────────── RED

/// A `re_derive` parameter with no derivation says nothing about what would
/// be re-derived or from where.
#[test]
fn a_re_derive_parameter_without_a_derivation_is_a_check_error() {
    let source = recipe_document("  parameter control_expect: String re_derive");
    let refusal = named(&source, CommandCause::DerivationMissing).unwrap_or_else(|| {
        unreachable!("a re-derive parameter with no derivation must name `derivation_missing`")
    });
    assert!(
        refusal.contains("fire minute") || refusal.contains("FIRE minute"),
        "the refusal must carry the fire-minute law as its reason; got: {refusal}"
    );
}

/// A derivation on a kind that is never read at the fire minute is a source
/// nothing would ever read.
#[test]
fn a_derivation_on_a_non_re_derive_kind_is_a_check_error() {
    let source = recipe_document(
        "  parameter tracker: String = \"github\" skeleton derives \"origin/main:f\" extract \
         \"whole\"",
    );
    assert!(
        named(&source, CommandCause::DerivationMissing).is_some(),
        "a derivation on a `skeleton` parameter must be refused"
    );
}

/// The extraction set is CLOSED. An extraction the fire cannot run would
/// fail at the dispatch minute, when it is most expensive.
#[test]
fn an_extraction_outside_the_closed_set_is_a_check_error() {
    for extraction in ["the third paragraph", "line 0", "line -3", "lines 1..4", ""] {
        let source = recipe_document(&format!(
            "  parameter control_expect: String re_derive derives \"origin/main:f\" extract \
             \"{extraction}\""
        ));
        let refusal = named(&source, CommandCause::ExtractionUnknown).unwrap_or_else(|| {
            unreachable!("`{extraction}` names no extraction and must be refused")
        });
        assert!(
            refusal.contains("`line <n>`") && refusal.contains("`whole`"),
            "the refusal must print the admitted set; got: {refusal}"
        );
    }
}

/// ONE SOURCE, ONE DECLARED PROJECTION. The `leg_ids_csv` + `legs[].id`
/// disease, measured in this cut's own exhibits and refused: the speech-1
/// KIT (`.fleet/fixtures/corpus/speech1.KIT-input-template.json` at
/// `1a953152…`) carries `leg_ids_csv` = `"build"` at line 8 beside
/// `legs[0].id` = `"build"` at line 18, and the haem-wall INPUT
/// (`77f0333f…`) carries the identical pair.
#[test]
fn a_declaration_carrying_the_same_truth_twice_is_a_check_error() {
    let source = recipe_document(
        "  parameter leg_ids: String = \"build\" skeleton\n  \
         parameter first_leg: String = \"build\" skeleton",
    );
    let refusal = named(&source, CommandCause::DoubleCarry).unwrap_or_else(|| {
        unreachable!("two parameters declaring one value must name `double_carry`")
    });
    assert!(
        refusal.contains("observed: \"build\""),
        "the refusal must carry the doubled value as observed bytes; got: {refusal}"
    );
}

/// `literal_where_derived` is CONSUMED, not re-minted: the cause code and
/// its wire string are cut-1's, and this cut adds the DECLARATION-driven
/// trigger cut-1's shape rule could not have.
#[test]
fn literal_where_derived_fires_against_a_re_derive_slot_fed_a_literal() {
    // The control token the haem-wall exhibit carries TWICE by hand
    // (`control_expect` and `counsel_control_expect`, `77f0333f…`), which
    // cut-1's `DerivedShape::ControlToken` already knows by SHAPE.
    let by_shape = recipe_document(
        "  parameter control_expect: String = \"wall-ctl-b508cd84-waffles-20260823\" re_derive \
         derives \"origin/main:f\" extract \"line 22\"",
    );
    let refusal = named(&by_shape, CommandCause::LiteralWhereDerived).unwrap_or_else(|| {
        unreachable!("a control token in a re-derive slot must name `literal_where_derived`")
    });
    assert!(
        refusal.contains("wall-ctl-b508cd84-waffles-20260823"),
        "the literal must ride as observed bytes; got: {refusal}"
    );

    // C-2, THE DIVERGENCE THIS CUT CLOSES. Cut-1's trigger is the literal's
    // SHAPE, and `"main"` has none — it is invisible to `derived_shape_in`
    // and exactly as wrong in a re-derive slot as a copied sha. The
    // declaration-driven trigger is what makes it refuse; the cause code is
    // unchanged.
    let by_declaration = recipe_document(
        "  parameter control_expect: String = \"main\" re_derive derives \"origin/main:f\" extract \
         \"line 22\"",
    );
    let refusal = named(&by_declaration, CommandCause::LiteralWhereDerived).unwrap_or_else(|| unreachable!("an ORDINARY literal in a re-derive slot must refuse too — that is the declaration-driven \
         trigger this cut adds"));
    assert!(
        refusal.contains("observed: \"main\""),
        "the ordinary literal must ride as observed bytes; got: {refusal}"
    );

    // And the same bytes OUTSIDE a re-derive slot are ordinary text: the
    // declaration trigger does not widen cut-1's closed shape set.
    let ordinary = recipe_document("  parameter branch: String = \"main\" skeleton");
    assert!(
        named(&ordinary, CommandCause::LiteralWhereDerived).is_none(),
        "`main` in a skeleton slot is ordinary text; cut-1's shape set must not have widened"
    );
}

/// A hand value bound to a re-derive slot AT COMPOSE TIME refuses with the
/// same cause. Composing a re-derive slot is the compose-minute reading the
/// fire-minute law exists to remove.
#[test]
fn composing_a_re_derive_slot_refuses_with_the_consumed_cause() {
    let source = document(recipe::support::STAGING);
    let parsed = checked(&source);
    let composition = Composition::new()
        .with("branch", "fleet/awl-cut2-result")
        .with("control_expect", "0.1.0-1.89");
    let Err(refusals) = compose(&parsed.recipes[0], &composition) else {
        unreachable!("composing a re-derive slot must refuse");
    };
    let refusal = refusals
        .iter()
        .find(|refusal| refusal.cause() == CommandCause::LiteralWhereDerived)
        .unwrap_or_else(|| {
            unreachable!("the refusal must name `literal_where_derived`, cut-1's own code")
        });
    assert_eq!(refusal.observed_bytes(), "0.1.0-1.89");
    assert_eq!(
        CommandCause::LiteralWhereDerived.as_str(),
        "literal_where_derived",
        "the wire string is cut-1's contract and is not re-minted here"
    );
}

// ───────────────────────────────────────────────────────────────── GREEN

/// Each kind parses and checks. The taxonomy is read off the live composer
/// corpus at `.fleet/fixtures/corpus/issue-investigations.README.md`
/// (`b1db6ae3…`, the "Which fields are skeleton, which are per-issue"
/// table): `node_label`/`repo_url`/`workspace_root`/`control_prompt` are
/// skeleton, `control_expect` is ⚠️ neither — re-derive — `issue`/`threshold`
/// are per-issue, and `investigate_prompt` is both.
#[test]
fn each_declared_kind_parses_and_checks() {
    let source = recipe_document(
        "  parameter control_prompt: String = \"read the file and reply\" skeleton\n  \
         parameter issue: String per_item\n  \
         parameter investigate_prompt: String both\n  \
         parameter control_expect: String re_derive derives \
         \"origin/main:workflows/investigate/investigate_worker.awl\" extract \"line 22\"",
    );
    let parsed = checked(&source);
    let kinds: Vec<ParameterKind> = parsed.recipes[0]
        .parameters
        .iter()
        .map(|parameter| parameter.kind)
        .collect();
    assert_eq!(
        kinds,
        vec![
            ParameterKind::Skeleton,
            ParameterKind::PerItem,
            ParameterKind::Both,
            ParameterKind::ReDerive,
        ]
    );
}

/// The extraction grammar answers exactly two forms and nothing else.
#[test]
fn the_extraction_grammar_reads_exactly_two_forms() {
    assert_eq!(parse_extraction("whole"), Ok(Extraction::Whole));
    assert_eq!(parse_extraction("line 22"), Ok(Extraction::Line(22)));
    assert_eq!(parse_extraction("  line 1  "), Ok(Extraction::Line(1)));
    assert!(parse_extraction("line 0").is_err());
    assert!(parse_extraction("line").is_err());
    assert!(parse_extraction("everything").is_err());
}

/// THE FIRE-MINUTE LAW, proven. The source is MUTATED between compose and
/// fire, and the value the recipe fires with follows the SOURCE — not the
/// value that was true when the composer ran.
///
/// The law is the corpus's own, verbatim from
/// `.fleet/fixtures/corpus/issue-investigations.README.md` (`b1db6ae3…`):
/// read the literal "at the dispatch minute (not the compose minute — they
/// drift)".
#[test]
fn a_derivation_resolves_at_fire_time_against_the_named_source() {
    let source = document(recipe::support::STAGING);
    let parsed = checked(&source);
    let recipe = &parsed.recipes[0];
    let derivation = recipe.parameters[1]
        .derivation
        .as_ref()
        .unwrap_or_else(|| unreachable!("the re-derive parameter carries a derivation"));

    // 22 lines, because the declaration says `line 22` and a source with
    // fewer would be testing the out-of-range arm instead of the law.
    let mut compose_minute = String::new();
    for line in 1..=22 {
        let _ = writeln!(compose_minute, "line {line} at the compose minute");
    }
    let tree = StatedTree::new().with(
        "origin/main:workflows/investigate/investigate_worker.awl",
        &compose_minute,
    );
    // COMPOSE MINUTE: the composer binds nothing to a re-derive slot, and
    // the source says one thing.
    let composed =
        compose(recipe, &Composition::new().with("branch", "fleet/x")).unwrap_or_else(|error| {
            unreachable!("a composition that leaves the re-derive slot alone is clean: {error:?}")
        });
    assert_eq!(composed.deferred, vec!["control_expect".to_owned()]);

    // The source MOVES between the two minutes, which is the drift the law
    // is about.
    let mut fire_minute = String::new();
    for line in 1..=22 {
        let _ = writeln!(fire_minute, "line {line} at the FIRE minute");
    }
    tree.rewrite(
        "origin/main:workflows/investigate/investigate_worker.awl",
        &fire_minute,
    );

    // FIRE MINUTE: the value follows the source.
    let fired = resolve(derivation, "control_expect", &tree).unwrap_or_else(|error| {
        unreachable!("the source is readable and the extraction is in range: {error}")
    });
    assert_eq!(
        fired, "line 22 at the FIRE minute",
        "a re-derived value must follow the SOURCE at the fire minute, never the compose minute"
    );
}

/// The whole-source extraction reads every byte, and a line past the end of
/// its source refuses rather than supplying an empty value that reads like a
/// real one.
#[test]
fn an_extraction_past_the_end_of_its_source_refuses() {
    let parsed = checked(&document(recipe::support::STAGING));
    let derivation = parsed.recipes[0].parameters[1]
        .derivation
        .as_ref()
        .unwrap_or_else(|| unreachable!("a derivation"));
    let tree = StatedTree::new().with(
        "origin/main:workflows/investigate/investigate_worker.awl",
        "only one line\n",
    );
    let Err(refusal) = resolve(derivation, "control_expect", &tree) else {
        unreachable!("line 22 of a one-line source must refuse");
    };
    assert_eq!(refusal.cause(), CommandCause::ExtractionUnknown);
    assert!(
        refusal.rendered().contains("1 line"),
        "the refusal must say how many lines the source actually has; got: {}",
        refusal.rendered()
    );
}

/// An unreadable derivation source refuses at the FIRE layer rather than
/// falling back on the last value it had.
#[test]
fn an_unreadable_derivation_source_refuses_at_the_fire_layer() {
    let parsed = checked(&document(recipe::support::STAGING));
    let derivation = parsed.recipes[0].parameters[1]
        .derivation
        .as_ref()
        .unwrap_or_else(|| unreachable!("a derivation"));
    let Err(refusal) = resolve(derivation, "control_expect", &StatedTree::new()) else {
        unreachable!("an absent source must refuse");
    };
    assert_eq!(refusal.cause(), CommandCause::ArtifactUnreadable);
    assert_eq!(refusal.layer(), aion_awl::command::CommandLayer::Fire);
}
