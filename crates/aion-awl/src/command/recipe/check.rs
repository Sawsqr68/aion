//! The check layer of the recipe surface: what a parsed assembly must SAY.
//!
//! Parsing established that the bytes are a recipe, a wall, or an artifact.
//! This establishes that what they say is something a machine could act on:
//! every reference names something the document declares, the dependency
//! edges close no cycle, a `re_derive` parameter carries the derivation its
//! kind promises, no declaration carries one truth twice, and nothing
//! restates a value the dispatch already declares.

use std::collections::BTreeMap;

use crate::Span;

use super::super::cause::CommandCause;
use super::super::decl::CommandDecl;
use super::super::error::CommandRefusal;
use super::decl::{
    ArtifactDecl, ParamLiteral, ParameterKind, RecipeDecl, RecipeParam, StepBody, WallDecl,
};
use super::derivation::parse_extraction;
use super::graph::topological_order;
use super::words::{
    ALLOWS_WORD, ARTIFACT_WORD, COMMAND_WORD, DERIVES_WORD, EXTRACT_WORD, PARAMETER_WORD,
};

/// The values a DISPATCH already declares, which a recipe therefore may not
/// declare a second time.
///
/// The one-truth fence, mechanical. Read off the live composer corpus rather
/// than invented: `.fleet/fixtures/corpus/speech1.KIT-input-template.json`
/// at `1a953152…` and `.fleet/fixtures/corpus/haem-wall.INPUT-FINAL.json` at
/// `77f0333f…` both carry exactly these keys at their top level, and both
/// carry `leg_ids_csv` restating `legs[].id` — the disease this closes.
///
/// A recipe may still CONSUME any of them: a parameter of one of these names
/// with NO default is the dispatch's value arriving. What is refused is the
/// second hand-copy — a default that states the value again, where nothing
/// re-checks it against the dispatch that already said it.
pub const DISPATCH_DECLARED: [&str; 11] = [
    "workspace_root",
    "run_name",
    "repo",
    "repo_url",
    "base_ref",
    "result_branch",
    "leg_ids_csv",
    "task_queue",
    "namespace",
    "workflow_type",
    "node_label",
];

/// Check every recipe, wall and artifact of a document, including the rules
/// that are about the SET of them rather than about any one.
#[must_use]
pub fn check_assembly(
    recipes: &[RecipeDecl],
    walls: &[WallDecl],
    artifacts: &[ArtifactDecl],
    commands: &[CommandDecl],
) -> Vec<CommandRefusal> {
    let mut refusals = Vec::new();
    names_are_stated_once(
        recipes
            .iter()
            .map(|item| (item.name.as_str(), item.name_span)),
        "recipe",
        &mut refusals,
    );
    names_are_stated_once(
        walls
            .iter()
            .map(|item| (item.name.as_str(), item.name_span)),
        "wall",
        &mut refusals,
    );
    names_are_stated_once(
        artifacts
            .iter()
            .map(|item| (item.name.as_str(), item.name_span)),
        "artifact",
        &mut refusals,
    );
    for wall in walls {
        check_wall(wall, &mut refusals);
    }
    for artifact in artifacts {
        check_artifact(artifact, &mut refusals);
    }
    for recipe in recipes {
        check_recipe(recipe, artifacts, commands, &mut refusals);
    }
    refusals
}

/// A wall that allows nothing enforces nothing.
fn check_wall(wall: &WallDecl, refusals: &mut Vec<CommandRefusal>) {
    if wall.allows.is_empty() {
        refusals.push(CommandRefusal::refuse(
            CommandCause::WallAllowsNothing,
            wall.span,
            &wall.name,
            format!(
                "wall `{}` allows nothing, so its enforcement rendering would admit no path and \
                 its prompt rendering would name no wall. Declare at least one `{ALLOWS_WORD} \
                 \"<pattern>\"`, or remove the wall — a wall that admits nothing and an absent \
                 wall are different claims and must not read alike",
                wall.name
            ),
        ));
    }
}

/// A declared digest is a CONTENT DIGEST or it is nothing a measurement
/// could ever match.
fn check_artifact(artifact: &ArtifactDecl, refusals: &mut Vec<CommandRefusal>) {
    let Some(digest) = &artifact.digest else {
        return;
    };
    let well_formed = digest.text.len() == 64
        && digest
            .text
            .chars()
            .all(|character| character.is_ascii_digit() || ('a'..='f').contains(&character));
    if !well_formed {
        refusals.push(CommandRefusal::refuse(
            CommandCause::ArtifactDigestMalformed,
            digest.span,
            &digest.text,
            format!(
                "artifact `{}` declares a digest that is not a content digest — 64 lowercase \
                 hexadecimal characters, which is what the bytes hash to. A digest no measurement \
                 could match is a census row that can only ever refuse",
                artifact.name
            ),
        ));
    }
}

/// One recipe, whole.
fn check_recipe(
    recipe: &RecipeDecl,
    artifacts: &[ArtifactDecl],
    commands: &[CommandDecl],
    refusals: &mut Vec<CommandRefusal>,
) {
    if recipe.steps.is_empty() {
        refusals.push(CommandRefusal::refuse(
            CommandCause::RecipeStatesNoStep,
            recipe.span,
            &recipe.name,
            format!(
                "recipe `{}` declares no step, so it assembles nothing. A recipe is an assembly \
                 act: it orders declared work, and a recipe with no work to order is a parameter \
                 list wearing a recipe's name",
                recipe.name
            ),
        ));
    }
    names_are_stated_once(
        recipe
            .parameters
            .iter()
            .map(|item| (item.name.as_str(), item.name_span)),
        "parameter",
        refusals,
    );
    for parameter in &recipe.parameters {
        check_parameter(recipe, parameter, refusals);
    }
    check_double_carry(recipe, refusals);
    check_steps(recipe, artifacts, commands, refusals);
}

/// A `re_derive` parameter carries a derivation; every other kind does not.
fn check_parameter(
    recipe: &RecipeDecl,
    parameter: &RecipeParam,
    refusals: &mut Vec<CommandRefusal>,
) {
    match (parameter.kind, &parameter.derivation) {
        (ParameterKind::ReDerive, None) => {
            refusals.push(CommandRefusal::refuse(
                CommandCause::DerivationMissing,
                parameter.kind_span,
                ParameterKind::ReDerive.as_str(),
                format!(
                    "recipe `{}`'s parameter `{}` is declared `{}` and carries no derivation, so \
                     nothing says what would be re-derived or from where. Write `{DERIVES_WORD} \
                     \"<source>\" {EXTRACT_WORD} \"<extraction>\"`: the value is read at the FIRE \
                     minute, not the compose minute — they drift, and a re-derive parameter with \
                     no source is a copied literal that has not admitted it yet",
                    recipe.name,
                    parameter.name,
                    ParameterKind::ReDerive
                ),
            ));
        }
        (kind, Some(derivation)) if kind != ParameterKind::ReDerive => {
            refusals.push(CommandRefusal::refuse(
                CommandCause::DerivationMissing,
                derivation.span,
                &derivation.source,
                format!(
                    "recipe `{}`'s parameter `{}` is declared `{kind}` and carries a derivation. \
                     Only a `{}` parameter is read at the fire minute; a derivation on any other \
                     kind is a source nothing would ever read",
                    recipe.name,
                    parameter.name,
                    ParameterKind::ReDerive
                ),
            ));
        }
        _ => {}
    }
    if let Some(derivation) = &parameter.derivation {
        if let Err(admitted) = parse_extraction(&derivation.extraction) {
            refusals.push(CommandRefusal::refuse(
                CommandCause::ExtractionUnknown,
                derivation.extraction_span,
                &derivation.extraction,
                format!(
                    "recipe `{}`'s parameter `{}` names the extraction `{}`, which is not one \
                     this surface performs — {admitted}. An extraction the fire cannot run is a \
                     derivation that would fail at the dispatch minute, when it is most expensive",
                    recipe.name, parameter.name, derivation.extraction
                ),
            ));
        }
    }
    check_venue_restatement(recipe, parameter, refusals);
    check_literal_where_derived(recipe, parameter, refusals);
}

/// The one-truth fence: a recipe may consume a dispatch-declared value, and
/// may not state it again.
fn check_venue_restatement(
    recipe: &RecipeDecl,
    parameter: &RecipeParam,
    refusals: &mut Vec<CommandRefusal>,
) {
    let Some(default) = &parameter.default else {
        return;
    };
    if !DISPATCH_DECLARED.contains(&parameter.name.as_str()) {
        return;
    }
    refusals.push(CommandRefusal::refuse(
        CommandCause::VenueValueRestated,
        default.span,
        &default.text,
        format!(
            "recipe `{}`'s parameter `{}` states a value the DISPATCH already declares. Consume \
             it — declare the parameter with no default and let the dispatch supply it — because \
             a second hand-copy of a truth the dispatch carries is a copy nothing re-checks, and \
             venue binding is the deploy surface's act rather than a declaration's",
            recipe.name, parameter.name
        ),
    ));
}

/// Cut-1's `literal_where_derived`, CONSUMED on its declaration side.
///
/// Cut-1 fires this cause on the literal's SHAPE (a 40-hex object name, a
/// 64-hex digest, a control token). This is the other trigger the same cause
/// needs and did not have: a slot whose parameter DECLARES a derivation
/// refuses a hand default whatever it looks like. `"main"` is invisible to a
/// shape rule and is exactly as wrong in a re-derive slot as a copied sha.
/// The cause code, its wire string, and cut-1's closed shape set are all
/// unchanged — this cut adds the declaration side and re-mints nothing.
fn check_literal_where_derived(
    recipe: &RecipeDecl,
    parameter: &RecipeParam,
    refusals: &mut Vec<CommandRefusal>,
) {
    if parameter.kind != ParameterKind::ReDerive {
        return;
    }
    let Some(default) = &parameter.default else {
        return;
    };
    refusals.push(CommandRefusal::refuse(
        CommandCause::LiteralWhereDerived,
        default.span,
        &default.text,
        format!(
            "recipe `{}`'s parameter `{}` is declared `{}` and carries the hand literal `{}`. A \
             re-derive slot is read from its declared source at the FIRE minute — not the compose \
             minute, because they drift — so a literal here records what was true once, on a day, \
             and nothing re-checks it",
            recipe.name,
            parameter.name,
            ParameterKind::ReDerive,
            default.text
        ),
    ));
}

/// One source, one declared projection: two parameters must not carry the
/// same truth.
fn check_double_carry(recipe: &RecipeDecl, refusals: &mut Vec<CommandRefusal>) {
    let mut seen: BTreeMap<&str, &RecipeParam> = BTreeMap::new();
    for parameter in &recipe.parameters {
        let Some(default) = &parameter.default else {
            continue;
        };
        if default.text.trim().is_empty() {
            continue;
        }
        if let Some(first) = seen.insert(default.text.as_str(), parameter) {
            refusals.push(CommandRefusal::refuse(
                CommandCause::DoubleCarry,
                default.span,
                &default.text,
                format!(
                    "recipe `{}`'s parameters `{}` and `{}` both declare the value `{}`. One \
                     source, one declared projection: a declaration carrying the same truth twice \
                     has two places to correct and no mechanism that says so — declare it once \
                     and let the second read the first",
                    recipe.name, first.name, parameter.name, default.text
                ),
            ));
        }
    }
}

/// Every step names things that exist, is named once, and closes no cycle.
fn check_steps(
    recipe: &RecipeDecl,
    artifacts: &[ArtifactDecl],
    commands: &[CommandDecl],
    refusals: &mut Vec<CommandRefusal>,
) {
    check_step_names(recipe, refusals);
    for step in &recipe.steps {
        check_step_body(recipe, step, artifacts, commands, refusals);
    }
    check_step_cycle(recipe, refusals);
}

/// One name, one step: a repeated step name makes an edge name two things.
fn check_step_names(recipe: &RecipeDecl, refusals: &mut Vec<CommandRefusal>) {
    let mut seen: BTreeMap<&str, Span> = BTreeMap::new();
    for step in &recipe.steps {
        if let Some(first) = seen.insert(step.name.as_str(), step.name_span) {
            refusals.push(CommandRefusal::refuse(
                CommandCause::RecipeStepRestated,
                step.name_span,
                &step.name,
                format!(
                    "recipe `{}` declares the step `{}` again, having already declared it at line \
                     {}; one name, one step, or a dependency edge naming it names two things",
                    recipe.name, step.name, first.line
                ),
            ));
        }
    }
}

/// One step: every edge it names, and what it says it does.
fn check_step_body(
    recipe: &RecipeDecl,
    step: &super::decl::RecipeStep,
    artifacts: &[ArtifactDecl],
    commands: &[CommandDecl],
    refusals: &mut Vec<CommandRefusal>,
) {
    {
        for reference in &step.after {
            if !recipe
                .steps
                .iter()
                .any(|other| other.name == reference.name)
            {
                refusals.push(undeclared(
                    reference.name_span,
                    &reference.name,
                    format!(
                        "recipe `{}`'s step `{}` runs after `{}`, which this recipe does not \
                         declare. Assembly is FROM declarations: an edge to nothing is an edge no \
                         sweep can follow",
                        recipe.name, step.name, reference.name
                    ),
                ));
            }
        }
        match &step.body {
            None => refusals.push(CommandRefusal::refuse(
                CommandCause::ReferenceUndeclared,
                step.span,
                &step.name,
                format!(
                    "recipe `{}`'s step `{}` says what it comes after and never says what it \
                     does. A step declares one act: `runs {COMMAND_WORD} <name>`, `fires \
                     {ARTIFACT_WORD} <name> expecting \"…\"`, or `captures <name>: <Type> of \
                     {ARTIFACT_WORD} <name>`",
                    recipe.name, step.name
                ),
            )),
            Some(StepBody::RunsCommand {
                command,
                command_span,
                ..
            }) => {
                if !commands.iter().any(|declared| declared.name == *command) {
                    refusals.push(undeclared(
                        *command_span,
                        command,
                        format!(
                            "recipe `{}`'s step `{}` runs the command `{command}`, which this \
                             document does not declare. A step names a command declared at the \
                             top level, so the command a reader sees is the command that ran",
                            recipe.name, step.name
                        ),
                    ));
                }
            }
            Some(
                StepBody::FiresArtifact {
                    artifact,
                    artifact_span,
                    ..
                }
                | StepBody::CapturesArtifact {
                    artifact,
                    artifact_span,
                    ..
                },
            ) => {
                if !artifacts.iter().any(|declared| declared.name == *artifact) {
                    refusals.push(undeclared(
                        *artifact_span,
                        artifact,
                        format!(
                            "recipe `{}`'s step `{}` names the artifact `{artifact}`, which this \
                             document does not declare. Every artifact a recipe reaches outside \
                             itself is declared with its address, so the reference census can \
                             measure it before anything fires",
                            recipe.name, step.name
                        ),
                    ));
                }
            }
        }
    }
}

/// The edges are a DAG, or they are not an order at all.
fn check_step_cycle(recipe: &RecipeDecl, refusals: &mut Vec<CommandRefusal>) {
    if let Err(cycle) = topological_order(recipe) {
        let at = recipe
            .steps
            .iter()
            .find(|step| step.name == cycle.closed_by)
            .map_or(recipe.span, |step| {
                step.after.first().map_or(step.span, |edge| edge.span)
            });
        refusals.push(CommandRefusal::refuse(
            CommandCause::DependencyCycle,
            at,
            cycle.rendered(),
            format!(
                "recipe `{}`'s dependency edges close a cycle: {}. No order satisfies them, so \
                 nothing could run first — an assembly's edges are a DAG or they are not an order \
                 at all",
                recipe.name,
                cycle.rendered()
            ),
        ));
    }
}

fn undeclared(span: Span, observed: &str, message: String) -> CommandRefusal {
    CommandRefusal::refuse(CommandCause::ReferenceUndeclared, span, observed, message)
}

/// Refuse a name bound twice, naming where it was bound first.
fn names_are_stated_once<'a>(
    items: impl Iterator<Item = (&'a str, Span)>,
    what: &str,
    refusals: &mut Vec<CommandRefusal>,
) {
    let mut seen: BTreeMap<&str, Span> = BTreeMap::new();
    for (name, span) in items {
        if let Some(first) = seen.insert(name, span) {
            let cause = if what == PARAMETER_WORD {
                CommandCause::ParameterRestated
            } else {
                CommandCause::AssemblyDeclarationRestated
            };
            refusals.push(CommandRefusal::refuse(
                cause,
                span,
                name,
                format!(
                    "the {what} `{name}` is declared again, having already been declared at line \
                     {}; one name, one declaration",
                    first.line
                ),
            ));
        }
    }
}

/// Whether a declared literal is one a composition must still fill.
///
/// The sentinel shapes are read off the live composer corpus rather than
/// invented: `.fleet/fixtures/corpus/skeleton.input.json` at `4e90bc3b…`
/// carries `<<ISSUE_NUMBER>>`, `<<ISSUE_TITLE>>` and
/// `<<RE-DERIVE AT THE DISPATCH MINUTE …>>`, and
/// `.fleet/fixtures/corpus/speech1.KIT-input-template.json` at `1a953152…`
/// carries `__CTL_TOKEN__`, `__RIG_COMMIT__` and `__CUT_COMMIT__`.
#[must_use]
pub fn is_unfilled_sentinel(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.starts_with("<<") && trimmed.ends_with(">>") && trimmed.len() > 4 {
        return true;
    }
    trimmed.starts_with("__")
        && trimmed.ends_with("__")
        && trimmed.len() > 4
        && trimmed[2..trimmed.len() - 2].chars().all(|character| {
            character.is_ascii_uppercase() || character.is_ascii_digit() || character == '_'
        })
}

/// The bytes a slot refusal observes when the slot was simply never bound.
#[must_use]
pub fn unbound_bytes(parameter: &RecipeParam) -> String {
    parameter.default.as_ref().map_or_else(
        || parameter.name.clone(),
        |ParamLiteral { text, .. }| text.clone(),
    )
}
