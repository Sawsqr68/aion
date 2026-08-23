//! Canonical printing of the assembly surface: walls, artifacts, recipes.
//!
//! Its own module rather than a third of `document.rs`, for the reason the
//! estate states as a law: a file that renders every declaration of a
//! growing language is a file nobody can read whole. What lives here is the
//! ORDER and the trivia; the line shapes themselves are the surface's own
//! (`crate::command::recipe`), so there is one rendering of a recipe and not
//! two.

use crate::ast::{Comment, DocLine, Document, Lead};

use super::document::Printer;

/// Print the assembly surface: walls, artifacts, then recipes.
///
/// Walls and artifacts print BEFORE the recipes that reference them, so a
/// reader meets a declaration before the step that names it — the same
/// ordering reason commands print before steps.
pub(super) fn print_assembly(printer: &mut Printer, document: &Document) {
    for wall in &document.walls {
        print_assembly_decl(
            printer,
            &wall.lead,
            &wall.docs,
            wall.trailing.as_ref(),
            &crate::command::recipe::wall_header(wall),
            &crate::command::recipe::wall_body_lines(wall),
        );
    }
    for artifact in &document.artifacts {
        print_assembly_decl(
            printer,
            &artifact.lead,
            &artifact.docs,
            artifact.trailing.as_ref(),
            &crate::command::recipe::artifact_header(artifact),
            &crate::command::recipe::artifact_body_lines(artifact),
        );
    }
    for recipe in &document.recipes {
        print_assembly_decl(
            printer,
            &recipe.lead,
            &recipe.docs,
            recipe.trailing.as_ref(),
            &crate::command::recipe::recipe_header(recipe),
            &crate::command::recipe::recipe_body_lines(recipe),
        );
    }
}

/// One assembly declaration: its trivia, its header, then its body lines at
/// one level of indentation. A body line that already carries its own
/// two-space prefix stands one level deeper still, which is how a recipe's
/// step clauses print under their step.
fn print_assembly_decl(
    printer: &mut Printer,
    lead: &[Lead],
    docs: &[DocLine],
    trailing: Option<&Comment>,
    header: &str,
    body: &[String],
) {
    printer.leads(0, lead);
    printer.docs(0, docs);
    printer.line(0, header, trailing);
    for line in body {
        printer.line(1, line, None);
    }
}
