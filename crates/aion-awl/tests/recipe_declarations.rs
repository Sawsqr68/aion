//! BRIEF-R1: recipe declarations — grammar, checked DAG, and amendment 1's
//! structural fence in BOTH directions.
//!
//! RED FIRST is a property of this file's order, not of its comments: every
//! refusal below stands before the passing path it protects.

mod recipe;

use std::fmt::Write as _;

use aion_awl::command::CommandCause;
use aion_awl::command::recipe::{ParameterKind, topological_order};

use recipe::support::{checked, document, parse, refusals};

/// The cause a document names, if any.
fn named(source: &str, cause: CommandCause) -> Option<String> {
    let marker = format!("named cause: {}", cause.as_str());
    refusals(source)
        .into_iter()
        .find(|text| text.contains(&marker))
}

// ─────────────────────────────────────────────────────── RED: refused forms

/// A step body written as a shell string is UNPARSEABLE, not linted. The
/// whole defect family the command surface removes from argv, removed one
/// level up from assembly.
#[test]
fn a_shell_string_step_body_is_unparseable() {
    for body in [
        "runs \"cargo test\"",
        "runs \"sh -c 'git push'\"",
        "runs \"./scripts/stage.sh --round2\"",
    ] {
        let source = document(&format!("recipe stage\n  step build\n    {body}"));
        assert!(
            aion_awl::parse(&source).is_err(),
            "`{body}` must be refused at the PARSE layer, not merely checked against"
        );
        let refusal = named(&source, CommandCause::ShellStringBody)
            .unwrap_or_else(|| unreachable!("`{body}` must name `shell_string_body`"));
        assert!(
            refusal.contains("reader"),
            "the refusal must carry the review property as its reason; got: {refusal}"
        );
    }
}

/// AMENDMENT 1, first direction, byte-identical to cut 1: no recipe,
/// dependency, or assembly form is reachable from a gate-leg command
/// declaration. This cut extends the grammar around that fence and must not
/// move it.
#[test]
fn every_assembly_form_still_refuses_inside_a_command_declaration() {
    for word in ["recipe", "depends", "needs", "after", "steps", "then"] {
        let source = document(&format!(
            "command probe_command\n  program \"git\"\n  admits \"--\"\n  {word} \"x\""
        ));
        let Err(error) = aion_awl::parse(&source) else {
            unreachable!("`{word}` must still be refused inside a command body");
        };
        assert!(
            error.message.contains("named cause: recipe_form_refused"),
            "`{word}` must refuse BY NAME as an assembly form; got: {}",
            error.message
        );
        assert!(
            error.message.contains("a reader"),
            "the refusal must carry the review property as its reason; got: {}",
            error.message
        );
    }
}

/// AMENDMENT 1, the CONVERSE direction. A `command` declaration cannot be
/// nested inside a recipe body, so a recipe cannot smuggle a gate-leg
/// declaration into a place no reader of the top level meets — which would
/// make the recipe layer reachable from a gate leg by a second route.
#[test]
fn a_command_declaration_cannot_be_nested_inside_a_recipe() {
    let source = document(
        "recipe stage\n  command inner\n    program \"git\"\n    admits \"--\"\n  step push\n    \
         runs command inner",
    );
    assert!(
        aion_awl::parse(&source).is_err(),
        "a nested command declaration must be refused at the PARSE layer"
    );
    let refusal = named(&source, CommandCause::CommandDeclarationNested).unwrap_or_else(|| {
        unreachable!("a nested command declaration must name `command_declaration_nested`")
    });
    assert!(
        refusal.contains("TOP LEVEL"),
        "the refusal must say where a command is declared instead; got: {refusal}"
    );
    assert!(
        refusal.contains("reader"),
        "the refusal must carry the review property as its reason; got: {refusal}"
    );
}

/// A cyclic dependency is a check error WITH A SPAN, and the diagnostic
/// renders the cycle a reader has to follow.
#[test]
fn a_cyclic_dependency_is_a_check_error_with_a_span() {
    let source = document(
        "command push_branch\n  program \"git\" \"push\"\n  admits \"--\"\n\n\
         recipe stage\n  step first\n    after third\n    runs command push_branch\n  \
         step second\n    after first\n    runs command push_branch\n  \
         step third\n    after second\n    runs command push_branch",
    );
    let parsed = parse(&source).unwrap_or_else(|error| {
        unreachable!("a cycle is a CHECK error, so the document must parse: {error}")
    });
    let errors = aion_awl::check_in(&parsed, std::env::temp_dir().as_path());
    let cycle = errors
        .iter()
        .find(|error| error.message.contains("named cause: dependency_cycle"))
        .unwrap_or_else(|| unreachable!("a cyclic recipe must name `dependency_cycle`"));
    assert!(
        cycle.span.line > 0 && cycle.span.end > cycle.span.start,
        "the cycle refusal must carry a span a reader can open: {:?}",
        cycle.span
    );
    assert!(
        cycle.message.contains("->"),
        "the cycle refusal must render the cycle; got: {}",
        cycle.message
    );
}

/// The graph walk is an explicit-stack traversal, so a deeply chained recipe
/// produces a refusal or an order — never a stack overflow, which is not an
/// outcome a caller can handle.
#[test]
fn a_deep_chain_orders_rather_than_overflowing() {
    let depth = 2_000;
    let mut body = String::from("recipe deep\n");
    for index in 0..depth {
        let _ = writeln!(body, "  step s{index}");
        if index > 0 {
            let _ = writeln!(body, "    after s{}", index - 1);
        }
        body.push_str("    runs command push_branch\n");
    }
    let source = document(&format!(
        "command push_branch\n  program \"git\" \"push\"\n  admits \"--\"\n\n{}",
        body.trim_end()
    ));
    let parsed =
        parse(&source).unwrap_or_else(|error| unreachable!("a deep chain must parse: {error}"));
    let ordered = topological_order(&parsed.recipes[0])
        .unwrap_or_else(|error| unreachable!("a chain closes no cycle: {error:?}"));
    assert_eq!(ordered.len(), depth);
    assert_eq!(ordered[0].name, "s0", "the chain's root must run first");
}

// ────────────────────────────────────────────────── GREEN: the passing path

/// Recipe, wall and artifact declarations parse, check, and ROUND-TRIP
/// through the canonical printer — `parse ∘ print = id` and
/// `print ∘ parse ∘ print = print`, exactly as the rest of the language.
#[test]
fn the_assembly_surface_round_trips_through_the_canonical_printer() {
    let source = document(recipe::support::STAGING);
    let parsed = checked(&source);
    let printed = aion_awl::print(&parsed);
    assert_eq!(
        printed, source,
        "the canonical printer must reproduce the document byte for byte"
    );
    let reparsed = parse(&printed)
        .unwrap_or_else(|error| unreachable!("the printed document must parse: {error}"));
    assert_eq!(
        aion_awl::print(&reparsed),
        printed,
        "printing is idempotent: print ∘ parse ∘ print = print"
    );
}

/// The three step bodies parse into the three typed forms, and none of them
/// is a string.
#[test]
fn the_three_step_bodies_parse_into_their_typed_forms() {
    let parsed = checked(&document(recipe::support::STAGING));
    let recipe = &parsed.recipes[0];
    let bodies: Vec<&'static str> = recipe
        .steps
        .iter()
        .map(|step| match &step.body {
            Some(aion_awl::command::recipe::StepBody::RunsCommand { .. }) => "runs",
            Some(aion_awl::command::recipe::StepBody::FiresArtifact { .. }) => "fires",
            Some(aion_awl::command::recipe::StepBody::CapturesArtifact { .. }) => "captures",
            None => "none",
        })
        .collect();
    assert_eq!(bodies, vec!["captures", "fires", "runs"]);
}

/// The dependency edges — not the source order — are what the order is.
#[test]
fn the_edges_and_not_the_source_order_decide_what_runs_first() {
    let source = document(
        "command push_branch\n  program \"git\" \"push\"\n  admits \"--\"\n\n\
         recipe stage\n  step last\n    after middle\n    runs command push_branch\n  \
         step middle\n    after first\n    runs command push_branch\n  \
         step first\n    runs command push_branch",
    );
    let parsed = checked(&source);
    let ordered: Vec<&str> = topological_order(&parsed.recipes[0])
        .unwrap_or_else(|error| unreachable!("no cycle: {error:?}"))
        .into_iter()
        .map(|step| step.name.as_str())
        .collect();
    assert_eq!(ordered, vec!["first", "middle", "last"]);
}

/// The parameter-kind taxonomy is CLOSED, round-trips through its own
/// spellings, and answers `None` for anything outside it — never a nearest
/// match.
#[test]
fn the_parameter_kind_taxonomy_is_closed() {
    for kind in ParameterKind::ALL {
        assert_eq!(ParameterKind::from_word(kind.as_str()), Some(kind));
    }
    assert_eq!(ParameterKind::from_word("re-derive"), None);
    assert_eq!(ParameterKind::from_word(""), None);
    assert_eq!(ParameterKind::ALL.len(), 4);
}
