//! `aion awl recipe compose` and `aion awl recipe fire`.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use aion_awl::command::recipe::{Composition, Ports, RecipeDecl, compose, fire};

use super::args::RecipeCommand;
use super::firer::{ProcessFirer, ProcessRunner};
use super::tree::WorkingTree;

/// Runs an `aion awl recipe` subcommand.
pub(crate) fn run(command: &RecipeCommand) -> ExitCode {
    match command {
        RecipeCommand::Compose { file, name, fills } => compose_command(file, name, fills),
        RecipeCommand::Fire {
            file,
            name,
            fills,
            met,
            root,
            receipt,
        } => fire_command(file, name, fills, met, root.as_deref(), receipt.as_deref()),
    }
}

fn compose_command(file: &Path, name: &str, fills: &[String]) -> ExitCode {
    let Some(document) = checked_document(file) else {
        return ExitCode::FAILURE;
    };
    let Some(recipe) = named_recipe(&document, file, name) else {
        return ExitCode::FAILURE;
    };
    let Some(composition) = bindings(fills) else {
        return ExitCode::FAILURE;
    };
    match compose(recipe, &composition) {
        Ok(composed) => {
            println!("composed: recipe {name} of {}", file.display());
            for (slot, value) in &composed.resolved {
                println!("  {slot} = {value:?}");
            }
            for slot in &composed.deferred {
                println!("  {slot} — DEFERRED to the fire minute (re_derive)");
            }
            ExitCode::SUCCESS
        }
        Err(refusals) => {
            for refusal in &refusals {
                eprintln!(
                    "{}:{}:{}: error: {}",
                    file.display(),
                    refusal.span().line,
                    refusal.span().column,
                    refusal.rendered()
                );
            }
            ExitCode::FAILURE
        }
    }
}

fn fire_command(
    file: &Path,
    name: &str,
    fills: &[String],
    met: &[String],
    root: Option<&Path>,
    receipt_path: Option<&Path>,
) -> ExitCode {
    let Some(document) = checked_document(file) else {
        return ExitCode::FAILURE;
    };
    let Some(recipe) = named_recipe(&document, file, name) else {
        return ExitCode::FAILURE;
    };
    let Some(composition) = bindings(fills) else {
        return ExitCode::FAILURE;
    };
    let composed = match compose(recipe, &composition) {
        Ok(composed) => composed,
        Err(refusals) => {
            for refusal in &refusals {
                eprintln!(
                    "{}:{}:{}: error: {}",
                    file.display(),
                    refusal.span().line,
                    refusal.span().column,
                    refusal.rendered()
                );
            }
            return ExitCode::FAILURE;
        }
    };

    let root = root.map_or_else(|| document_root(file), Path::to_path_buf);
    let tree = WorkingTree::rooted(&root);
    let measured = match tree.measure() {
        Ok(measured) => measured,
        Err(error) => {
            eprintln!("error: the tree could not say what it is: {error}");
            return ExitCode::FAILURE;
        }
    };

    let mut preconditions = aion_awl::command::recipe::Preconditions::new();
    for answered in met {
        preconditions = preconditions.answered(answered, true);
    }

    let fired = match fire(
        recipe,
        &document.commands,
        &document.artifacts,
        &composed,
        &preconditions,
        measured,
        &Ports {
            source: &tree,
            firer: &ProcessFirer::rooted(&root),
            runner: &ProcessRunner::rooted(&root),
        },
    ) {
        Ok(fired) => fired,
        Err(error) => {
            eprintln!("error: the receipt could not be built: {error}");
            return ExitCode::FAILURE;
        }
    };

    // RECEIPT FIRST, all the way out: the receipt prints whether the fire
    // passed or refused, and is written before the exit code is decided.
    let rendered = fired.receipt.rendered();
    print!("{rendered}");
    if let Some(path) = receipt_path {
        if let Err(error) = std::fs::write(path, &rendered) {
            eprintln!("error: failed to write {}: {error}", path.display());
            return ExitCode::FAILURE;
        }
        println!("receipt written: {}", path.display());
    }

    match fired.refusal {
        None => ExitCode::SUCCESS,
        Some(refusal) => {
            eprintln!(
                "{}:{}:{}: error: {}",
                file.display(),
                refusal.span().line,
                refusal.span().column,
                refusal.rendered()
            );
            ExitCode::FAILURE
        }
    }
}

/// Parse, check, and hand back the document — or report every diagnostic.
fn checked_document(file: &Path) -> Option<aion_awl::Document> {
    let source = match std::fs::read_to_string(file) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("error: failed to read {}: {error}", file.display());
            return None;
        }
    };
    let document = match aion_awl::parse(&source) {
        Ok(document) => document,
        Err(error) => {
            eprintln!(
                "{}:{}:{}: error: {}",
                file.display(),
                error.span.line,
                error.span.column,
                error.message
            );
            return None;
        }
    };
    let errors = aion_awl::check_in(&document, &document_root(file));
    if errors.is_empty() {
        return Some(document);
    }
    for error in &errors {
        eprintln!(
            "{}:{}:{}: error: {}",
            file.display(),
            error.span.line,
            error.span.column,
            error.message
        );
    }
    None
}

/// The recipe the document declares by that name.
fn named_recipe<'a>(
    document: &'a aion_awl::Document,
    file: &Path,
    name: &str,
) -> Option<&'a RecipeDecl> {
    let found = document.recipes.iter().find(|recipe| recipe.name == name);
    if found.is_none() {
        let declared: Vec<&str> = document
            .recipes
            .iter()
            .map(|recipe| recipe.name.as_str())
            .collect();
        eprintln!(
            "error: {} declares no recipe `{name}`; it declares {declared:?}",
            file.display()
        );
    }
    found
}

/// `slot=value` bindings, or the one that could not be read.
fn bindings(fills: &[String]) -> Option<Composition> {
    let mut composition = Composition::new();
    for fill in fills {
        let Some((slot, value)) = fill.split_once('=') else {
            eprintln!("error: `--fill {fill}` is not a `slot=value` binding");
            return None;
        };
        if slot.trim().is_empty() {
            eprintln!("error: `--fill {fill}` names no slot");
            return None;
        }
        composition = composition.with(slot.trim(), value);
    }
    Some(composition)
}

/// The directory a document's addresses resolve against.
fn document_root(file: &Path) -> PathBuf {
    file.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}
