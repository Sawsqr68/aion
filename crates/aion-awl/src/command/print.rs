//! Canonical printing of a command declaration.
//!
//! `parse ∘ print = id` and `print ∘ parse ∘ print = print` hold for command
//! declarations exactly as they do for the rest of the language, which is
//! what lets `aion awl fmt` rewrite a document without changing what it says
//! — and what lets every guide example be pinned canonical.
//!
//! One shape, no choices: clauses print in the language's own order
//! (`program`, the end-of-options property, its specimen, options, operands,
//! environment, working directory, hardening, timeout), never in the order
//! they happened to be written. A canonical printer that preserved clause
//! order would have two renderings of one declaration.

use std::fmt::Write as _;

use crate::ast::TypeRef;

use super::decl::{CommandDecl, END_OF_OPTIONS_MARKER};
use super::fill::{Fill, FillSegment};
use super::words::{
    ADMITS_WORD, COMMAND_WORD, CWD_WORD, ENV_WORD, HARDENING_WORD, OPEN_FILES_WORD, OPERAND_WORD,
    OPTION_WORD, OWNER_WORD, PATH_WORD, PROGRAM_WORD, REFUSES_WORD, SPECIMEN_WORD,
};

/// The header line, `command name(a: T, b: T = "d")`, without indentation.
#[must_use]
pub fn command_header(decl: &CommandDecl) -> String {
    if decl.parameters.is_empty() {
        return format!("{COMMAND_WORD} {}", decl.name);
    }
    let rendered: Vec<String> = decl
        .parameters
        .iter()
        .map(|parameter| {
            let mut text = format!("{}: {}", parameter.name, type_text(&parameter.ty));
            if let Some(default) = &parameter.default {
                let _ = write!(text, " = {}", fill_text(default));
            }
            text
        })
        .collect();
    format!("{COMMAND_WORD} {}({})", decl.name, rendered.join(", "))
}

/// Every clause line of the body, in the language's own order, without
/// indentation. A `hardening` member is returned with its own two-space
/// prefix, since it stands one level deeper than its block.
#[must_use]
pub fn command_body_lines(decl: &CommandDecl) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(program) = &decl.program {
        let words: Vec<String> = program
            .words
            .iter()
            .map(|word| string_text(&word.text))
            .collect();
        lines.push(format!("{PROGRAM_WORD} {}", words.join(" ")));
    }
    for clause in &decl.end_of_options {
        let word = if clause.admits {
            ADMITS_WORD
        } else {
            REFUSES_WORD
        };
        lines.push(format!("{word} {}", string_text(END_OF_OPTIONS_MARKER)));
    }
    if let Some(specimen) = &decl.specimen {
        lines.push(format!("{SPECIMEN_WORD} {}", string_text(&specimen.bytes)));
    }
    for option in &decl.options {
        let mut line = format!("{OPTION_WORD} {}", string_text(&option.flag));
        if let Some(value) = &option.value {
            let _ = write!(
                line,
                ": {} = {}",
                type_text(&value.ty),
                fill_text(&value.fill)
            );
        }
        lines.push(line);
    }
    for operand in &decl.operands {
        lines.push(format!(
            "{OPERAND_WORD} {}: {} = {}",
            operand.name,
            type_text(&operand.value.ty),
            fill_text(&operand.value.fill)
        ));
    }
    for binding in &decl.env {
        lines.push(format!(
            "{ENV_WORD} {} = {}",
            binding.name,
            fill_text(&binding.fill)
        ));
    }
    if let Some(cwd) = &decl.cwd {
        lines.push(format!("{CWD_WORD} {}", string_text(&cwd.value)));
    }
    for block in &decl.hardening {
        lines.push(HARDENING_WORD.to_owned());
        if let Some(path) = &block.path {
            lines.push(format!("  {PATH_WORD} {}", fill_text(&path.fill)));
        }
        if let Some(limit) = &block.open_files {
            lines.push(format!("  {OPEN_FILES_WORD} {}", limit.value));
        }
    }
    if let Some(timeout) = &decl.timeout {
        lines.push(format!(
            "timeout {} {OWNER_WORD} {}",
            crate::printer::duration_text(&timeout.duration),
            string_text(&timeout.owner)
        ));
    }
    lines
}

/// One declaration as canonical text, at the given indentation, with a
/// trailing newline on every line.
#[must_use]
pub fn print_command_decl(decl: &CommandDecl) -> String {
    let mut out = command_header(decl);
    out.push('\n');
    for line in command_body_lines(decl) {
        out.push_str("  ");
        out.push_str(&line);
        out.push('\n');
    }
    out
}

/// A fill as the string literal it was read from.
fn fill_text(fill: &Fill) -> String {
    let mut text = String::new();
    for segment in &fill.segments {
        match segment {
            FillSegment::Literal(literal) => text.push_str(literal),
            FillSegment::Hole { name, .. } => {
                let _ = write!(text, "{{{{{name}}}}}");
            }
        }
    }
    string_text(&text)
}

/// A string literal, escaped exactly as the language's own printer does.
fn string_text(text: &str) -> String {
    crate::printer::string_literal(text)
}

/// A type reference as an author writes it.
fn type_text(ty: &TypeRef) -> String {
    crate::printer::type_ref_text(ty)
}
