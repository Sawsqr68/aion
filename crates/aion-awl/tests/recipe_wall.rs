//! BRIEF-R4 (DESIGN-R6, assigned to this cut by ruling): one wall, one home
//! — projected into a prompt rendering and a mechanical enforcement
//! rendering from the SAME declaration, with its negative vocabulary and its
//! structured diff.
//!
//! RED FIRST: the excluded-term family stands before the passing path.

mod recipe;

use aion_awl::command::recipe::{diff, project};
use aion_awl::command::{CommandCause, CommandLayer};

use recipe::support::{checked, document};

fn wall_document(body: &str) -> String {
    document(&format!("wall changed_paths\n{body}"))
}

// ─────────────────────────────────────────────────────────────────── RED

/// A projection that would emit a term the wall declares excluded refuses,
/// with the TERM as its observed bytes.
///
/// L1's negative vocabulary: a term a round MEASURED out is declared beside
/// the wall, so the way it comes back — somebody writing it again in good
/// faith — is a refusal rather than a review's luck.
#[test]
fn a_projection_that_would_emit_an_excluded_term_refuses() {
    let parsed = checked(&wall_document(
        "  allows \"crates/aion-awl\"\n  allows \"scripts/mutation-harness/\"\n  \
         excludes \"mutation\"",
    ));
    let Err(refusal) = project(&parsed.walls[0]) else {
        unreachable!("a projection carrying an excluded term must refuse");
    };
    assert_eq!(refusal.cause(), CommandCause::ExcludedTerm);
    assert_eq!(refusal.layer(), CommandLayer::Compose);
    assert_eq!(refusal.observed_bytes(), "mutation");
    assert!(
        refusal.message().contains("scripts/mutation-harness/"),
        "the refusal must name WHERE the term stands; got: {}",
        refusal.message()
    );
    assert!(
        refusal.message().contains("MEASURED out"),
        "the refusal must carry the reason an exclusion exists; got: {}",
        refusal.message()
    );
}

/// The exclusion reads the wall's own NAME too, so a wall cannot smuggle a
/// measured-out term into the one string every projection prints.
#[test]
fn an_excluded_term_in_the_walls_own_name_refuses() {
    let parsed = checked(&document(
        "wall mutation_wall\n  allows \".fleet/\"\n  excludes \"mutation\"",
    ));
    let Err(refusal) = project(&parsed.walls[0]) else {
        unreachable!("a wall whose own name carries an excluded term must refuse");
    };
    assert_eq!(refusal.cause(), CommandCause::ExcludedTerm);
    assert!(refusal.message().contains("mutation_wall"));
}

/// The exclusion must be able to say NOTHING IS WRONG, or the refusals above
/// say only that the check ran.
#[test]
fn a_wall_whose_exclusions_are_absent_from_it_projects() {
    let parsed = checked(&wall_document(
        "  allows \"crates/aion-awl\"\n  excludes \"mutation vocabulary\"",
    ));
    assert!(project(&parsed.walls[0]).is_ok());
}

/// A wall that admits nothing enforces nothing, and reads exactly like one
/// that was never declared. The two are different claims.
#[test]
fn a_wall_that_allows_nothing_is_a_check_error() {
    let source = wall_document("  excludes \"mutation vocabulary\"");
    let named = aion_awl::check_in(
        &aion_awl::parse(&source)
            .unwrap_or_else(|error| unreachable!("a wall with no `allows` still parses: {error}")),
        std::env::temp_dir().as_path(),
    )
    .into_iter()
    .any(|error| error.message.contains("named cause: wall_allows_nothing"));
    assert!(
        named,
        "a wall admitting nothing must name `wall_allows_nothing`"
    );
}

// ───────────────────────────────────────────────────────────────── GREEN

/// ONE DECLARATION, TWO RENDERINGS, fixture-compared. The L5 family — one
/// intent in two languages, only one executing — closes because both
/// renderings share one source.
///
/// The declaration below is the haem-wall allowlist at the pinned bytes:
/// docs repo `3c7cbb6`, `dispatch-rigs/haem-wall/INPUT-FINAL.json`, sha256
/// `77f0333fcea03ada0c017af9fdea725231c3e2aecce8ade71781571daff9115d`.
#[test]
fn one_wall_declaration_projects_into_both_renderings() {
    let parsed = checked(&document(
        "wall haem_wall\n  allows \"gates.json\"\n  allows \"known-carries*\"\n  \
         allows \"scripts/release/\"\n  allows \"docs/evidence/\"\n  \
         allows \"gate-logs/*.md\"\n  allows \"CONTRIBUTING.md\"\n  allows \"README.md\"\n  \
         allows \"docs/DOCUMENTATION.md\"\n  allows \".fleet/\"",
    ));
    let projection = project(&parsed.walls[0])
        .unwrap_or_else(|error| unreachable!("the wall projects: {error}"));

    assert_eq!(
        projection.prompt,
        "⛔ Wall `haem_wall`: every changed path stays inside the allowlist (gates.json, \
         known-carries*, scripts/release/, docs/evidence/, gate-logs/*.md, CONTRIBUTING.md, \
         README.md, docs/DOCUMENTATION.md, .fleet/). A path outside it is moved or removed, never \
         admitted by widening the wall."
    );
    assert_eq!(
        projection.enforcement,
        "^(gates\\.json|known-carries*|scripts/release/|docs/evidence/|gate-logs/*\\.md|\
         CONTRIBUTING\\.md|README\\.md|docs/DOCUMENTATION\\.md|\\.fleet/)"
    );

    // Both renderings name every declared pattern, which is the property
    // that makes them ONE intent rather than two.
    for pattern in &parsed.walls[0].allows {
        assert!(
            projection.prompt.contains(&pattern.text),
            "the prompt rendering omits `{}`",
            pattern.text
        );
    }
}

/// The enforcement rendering escapes what a path carries by ACCIDENT and
/// leaves `*` alone, because a rig writes `known-carries*` meaning "and what
/// follows". A declaration whose patterns were escaped into literals would
/// enforce a wall narrower than the one its prompt describes.
#[test]
fn the_enforcement_rendering_escapes_accidents_and_keeps_intent() {
    let parsed = checked(&wall_document(
        "  allows \"gates.json\"\n  allows \"known-carries*\"\n  allows \"a+b(c)\"",
    ));
    let projection = project(&parsed.walls[0])
        .unwrap_or_else(|error| unreachable!("the wall projects: {error}"));
    assert_eq!(
        projection.enforcement,
        "^(gates\\.json|known-carries*|a\\+b\\(c\\))"
    );
}

/// THE STRUCTURED DIFF of two declaration states, produced by test and
/// naming every changed projection — the hand-built refire artifact,
/// mechanized.
#[test]
fn the_structured_diff_names_every_changed_projection() {
    let before = checked(&wall_document(
        "  allows \"crates/aion-awl\"\n  allows \"Cargo.toml\"\n  excludes \"mutation vocabulary\"",
    ));
    let after = checked(&wall_document(
        "  allows \"crates/aion-awl\"\n  allows \"crates/aion-cli\"\n  \
         excludes \"mutation vocabulary\"\n  excludes \"phrasing was the wrong axis\"",
    ));
    let delta = diff(&before.walls[0], &after.walls[0])
        .unwrap_or_else(|error| unreachable!("both states project: {error}"));

    assert_eq!(delta.allowed_added, vec!["crates/aion-cli".to_owned()]);
    assert_eq!(delta.allowed_removed, vec!["Cargo.toml".to_owned()]);
    assert_eq!(
        delta.excluded_added,
        vec!["phrasing was the wrong axis".to_owned()]
    );
    assert!(delta.excluded_removed.is_empty());
    assert_eq!(delta.changed_projections(), vec!["prompt", "enforcement"]);
    assert!(!delta.is_empty());
}

/// The diff must be able to say NOTHING CHANGED, or naming a change above
/// proves only that it always names one.
#[test]
fn two_identical_declaration_states_diff_to_nothing() {
    let parsed = checked(&wall_document(
        "  allows \"crates/aion-awl\"\n  excludes \"mutation vocabulary\"",
    ));
    let delta = diff(&parsed.walls[0], &parsed.walls[0])
        .unwrap_or_else(|error| unreachable!("the state projects: {error}"));
    assert!(delta.is_empty());
    assert!(delta.changed_projections().is_empty());
}

/// A diff between a state and one that cannot be projected is not a diff: it
/// is the projection refusal, surfaced rather than swallowed.
#[test]
fn a_diff_against_an_unprojectable_state_is_the_projection_refusal() {
    let clean = checked(&wall_document("  allows \"crates/aion-awl\""));
    let dirty = checked(&wall_document(
        "  allows \"scripts/mutation-harness/\"\n  excludes \"mutation\"",
    ));
    let Err(refusal) = diff(&clean.walls[0], &dirty.walls[0]) else {
        unreachable!("a diff against an unprojectable state must refuse");
    };
    assert_eq!(refusal.cause(), CommandCause::ExcludedTerm);
}
