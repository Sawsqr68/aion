//! Canonical printing of the recipe surface.
//!
//! `parse ∘ print = id` and `print ∘ parse ∘ print = print` hold for
//! recipes, walls and artifacts exactly as they do for the rest of the
//! language, which is what lets `aion awl fmt` rewrite a document without
//! changing what it says.
//!
//! One shape, no choices: clauses print in the language's own order, never
//! in the order they happened to be written. A canonical printer that
//! preserved clause order would have two renderings of one declaration.

use std::fmt::Write as _;

use crate::ast::TypeRef;

use super::decl::{ArtifactDecl, RecipeDecl, RecipeStep, StepBody, WallDecl};
use super::words::{
    ALLOWS_WORD, ARTIFACT_WORD, CAPTURES_WORD, COMMAND_WORD, DERIVES_WORD, DIGEST_WORD,
    EXCLUDES_WORD, EXPECTING_WORD, EXTRACT_WORD, FIRES_WORD, PARAMETER_WORD, RECIPE_WORD,
    REQUIRES_WORD, RESTORES_WORD, RUNS_WORD, WALL_WORD,
};

/// The header line of a recipe, without indentation.
#[must_use]
pub fn recipe_header(decl: &RecipeDecl) -> String {
    format!("{RECIPE_WORD} {}", decl.name)
}

/// Every line of a recipe body, in the language's own order, without the
/// declaration's own indentation. A step's clauses come back with their own
/// two-space prefix, since they stand one level deeper than the step.
#[must_use]
pub fn recipe_body_lines(decl: &RecipeDecl) -> Vec<String> {
    let mut lines = Vec::new();
    for parameter in &decl.parameters {
        let mut line = format!(
            "{PARAMETER_WORD} {}: {}",
            parameter.name,
            type_text(&parameter.ty)
        );
        if let Some(default) = &parameter.default {
            let _ = write!(line, " = {}", string_text(&default.text));
        }
        let _ = write!(line, " {}", parameter.kind.as_str());
        if let Some(derivation) = &parameter.derivation {
            let _ = write!(
                line,
                " {DERIVES_WORD} {} {EXTRACT_WORD} {}",
                string_text(&derivation.source),
                string_text(&derivation.extraction)
            );
        }
        lines.push(line);
    }
    for step in &decl.steps {
        lines.push(format!("{} {}", crate::Keyword::Step.as_word(), step.name));
        lines.extend(step_lines(step));
    }
    lines
}

fn step_lines(step: &RecipeStep) -> Vec<String> {
    let mut lines = Vec::new();
    for reference in &step.after {
        lines.push(format!(
            "  {} {}",
            crate::Keyword::After.as_word(),
            reference.name
        ));
    }
    for precondition in &step.requires {
        lines.push(format!(
            "  {REQUIRES_WORD} {}",
            string_text(&precondition.text)
        ));
    }
    match &step.body {
        None => {}
        Some(StepBody::RunsCommand { command, .. }) => {
            lines.push(format!("  {RUNS_WORD} {COMMAND_WORD} {command}"));
        }
        Some(StepBody::FiresArtifact {
            artifact,
            expecting,
            ..
        }) => lines.push(format!(
            "  {FIRES_WORD} {ARTIFACT_WORD} {artifact} {EXPECTING_WORD} {}",
            string_text(expecting)
        )),
        Some(StepBody::CapturesArtifact {
            capture,
            ty,
            artifact,
            ..
        }) => lines.push(format!(
            "  {CAPTURES_WORD} {capture}: {} {} {ARTIFACT_WORD} {artifact}",
            type_text(ty),
            crate::Keyword::Of.as_word()
        )),
    }
    lines
}

/// The header line of a wall, without indentation.
#[must_use]
pub fn wall_header(decl: &WallDecl) -> String {
    format!("{WALL_WORD} {}", decl.name)
}

/// Every line of a wall body, in the language's own order.
#[must_use]
pub fn wall_body_lines(decl: &WallDecl) -> Vec<String> {
    let mut lines = Vec::new();
    for pattern in &decl.allows {
        lines.push(format!("{ALLOWS_WORD} {}", string_text(&pattern.text)));
    }
    for term in &decl.excludes {
        lines.push(format!("{EXCLUDES_WORD} {}", string_text(&term.text)));
    }
    lines
}

/// The header line of an artifact, address included, without indentation.
#[must_use]
pub fn artifact_header(decl: &ArtifactDecl) -> String {
    format!(
        "{ARTIFACT_WORD} {} {}",
        decl.name,
        string_text(&decl.address)
    )
}

/// Every line of an artifact body, in the language's own order.
#[must_use]
pub fn artifact_body_lines(decl: &ArtifactDecl) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(digest) = &decl.digest {
        lines.push(format!("{DIGEST_WORD} {}", string_text(&digest.text)));
    }
    if let Some(restores) = &decl.restores {
        lines.push(format!("{RESTORES_WORD} {}", string_text(&restores.text)));
    }
    for sentinel in &decl.excludes {
        lines.push(format!("{EXCLUDES_WORD} {}", string_text(&sentinel.text)));
    }
    lines
}

/// One recipe as canonical text, with a trailing newline on every line.
#[must_use]
pub fn print_recipe_decl(decl: &RecipeDecl) -> String {
    render(&recipe_header(decl), &recipe_body_lines(decl))
}

/// One wall as canonical text.
#[must_use]
pub fn print_wall_decl(decl: &WallDecl) -> String {
    render(&wall_header(decl), &wall_body_lines(decl))
}

/// One artifact as canonical text.
#[must_use]
pub fn print_artifact_decl(decl: &ArtifactDecl) -> String {
    render(&artifact_header(decl), &artifact_body_lines(decl))
}

fn render(header: &str, body: &[String]) -> String {
    let mut out = header.to_owned();
    out.push('\n');
    for line in body {
        out.push_str("  ");
        out.push_str(line);
        out.push('\n');
    }
    out
}

fn string_text(text: &str) -> String {
    crate::printer::string_literal(text)
}

fn type_text(ty: &TypeRef) -> String {
    crate::printer::type_ref_text(ty)
}
