//! Canonical printing of a worker block's `harness` section.
//!
//! The section prints in a FIXED field order — [`HARNESS_FIELDS`]' — rather than the
//! order it was written in, for the same reason an action's config line does: `aion awl
//! fmt` gives one shape per document, so two operators who declared the same launch in
//! different orders end up with the same bytes and their diff shows what actually
//! changed.
//!
//! The printer emits one `if let` per setting rather than WALKING that array: the twelve
//! values have twelve different types, and a table of extractors would trade a readable
//! sequence for a pile of closures. The correspondence is therefore not enforced by
//! construction — it is enforced by [`tests`], which prints a section carrying every
//! field in a scrambled order and asserts the emitted field words are exactly
//! [`HARNESS_FIELDS`], in its order, with nothing else. A field added to the grammar
//! with no print position fails there instead of vanishing on `aion awl fmt`.
//!
//! [`HARNESS_FIELDS`]: crate::ast::HARNESS_FIELDS
//!
//! The order itself reads outward from the harness: what it is (`kind`), how much of it
//! runs (`concurrency`), how it recovers (the reconnect trio), what crosses the process
//! boundary (`env_pass`), then the per-kind launch (`command`/`args`/`cwd`/`permission`/
//! `exit_grace` for ACP, `binary` for Norn).

use crate::ast::{
    HARNESS_ARGS_FIELD, HARNESS_BINARY_FIELD, HARNESS_COMMAND_FIELD, HARNESS_CONCURRENCY_FIELD,
    HARNESS_CWD_FIELD, HARNESS_ENV_PASS_FIELD, HARNESS_EXIT_GRACE_FIELD, HARNESS_KIND_FIELD,
    HARNESS_PERMISSION_FIELD, HARNESS_RECONNECT_INITIAL_BACKOFF_FIELD,
    HARNESS_RECONNECT_MAX_ATTEMPTS_FIELD, HARNESS_RECONNECT_MAX_BACKOFF_FIELD, HARNESS_WORD,
    HarnessDecl, HarnessSetting, HarnessText,
};

use super::document::Printer;
use super::exprs::{duration_text, string_literal};

/// Print the `harness` section of a worker block.
///
/// `indent` is the section header's level; its settings sit one level deeper.
pub(super) fn print_harness(printer: &mut Printer, indent: usize, harness: &HarnessDecl) {
    printer.leads(indent, &harness.lead);
    printer.docs(indent, &harness.docs);
    printer.line(indent, HARNESS_WORD, harness.trailing.as_ref());
    let settings = indent + 1;

    if let Some(kind) = &harness.kind {
        setting(
            printer,
            settings,
            HARNESS_KIND_FIELD,
            &kind.value.word,
            kind,
        );
    }
    if let Some(concurrency) = &harness.concurrency {
        let text = concurrency.value.value.to_string();
        setting(
            printer,
            settings,
            HARNESS_CONCURRENCY_FIELD,
            &text,
            concurrency,
        );
    }
    if let Some(backoff) = &harness.reconnect_initial_backoff {
        let text = duration_text(&backoff.value);
        setting(
            printer,
            settings,
            HARNESS_RECONNECT_INITIAL_BACKOFF_FIELD,
            &text,
            backoff,
        );
    }
    if let Some(backoff) = &harness.reconnect_max_backoff {
        let text = duration_text(&backoff.value);
        setting(
            printer,
            settings,
            HARNESS_RECONNECT_MAX_BACKOFF_FIELD,
            &text,
            backoff,
        );
    }
    if let Some(attempts) = &harness.reconnect_max_attempts {
        let text = attempts.value.value.to_string();
        setting(
            printer,
            settings,
            HARNESS_RECONNECT_MAX_ATTEMPTS_FIELD,
            &text,
            attempts,
        );
    }
    if let Some(env_pass) = &harness.env_pass {
        let text = text_list(&env_pass.value);
        setting(printer, settings, HARNESS_ENV_PASS_FIELD, &text, env_pass);
    }
    if let Some(command) = &harness.command {
        let text = string_literal(&command.value.value);
        setting(printer, settings, HARNESS_COMMAND_FIELD, &text, command);
    }
    if let Some(args) = &harness.args {
        let text = text_list(&args.value);
        setting(printer, settings, HARNESS_ARGS_FIELD, &text, args);
    }
    if let Some(cwd) = &harness.cwd {
        let text = string_literal(&cwd.value.value);
        setting(printer, settings, HARNESS_CWD_FIELD, &text, cwd);
    }
    if let Some(permission) = &harness.permission {
        let text = string_literal(&permission.value.value);
        setting(
            printer,
            settings,
            HARNESS_PERMISSION_FIELD,
            &text,
            permission,
        );
    }
    if let Some(grace) = &harness.exit_grace {
        let text = duration_text(&grace.value);
        setting(printer, settings, HARNESS_EXIT_GRACE_FIELD, &text, grace);
    }
    if let Some(binary) = &harness.binary {
        let text = string_literal(&binary.value.value);
        setting(printer, settings, HARNESS_BINARY_FIELD, &text, binary);
    }
}

/// Print one `<field> <value>` line with the trivia that belongs to it.
fn setting<V>(
    printer: &mut Printer,
    indent: usize,
    field: &str,
    value: &str,
    line: &HarnessSetting<V>,
) {
    printer.leads(indent, &line.lead);
    printer.docs(indent, &line.docs);
    printer.line(indent, &format!("{field} {value}"), line.trailing.as_ref());
}

/// A comma-separated list of quoted strings, in the order they were written.
///
/// Order is the operator's: agent arguments are positional, and an environment
/// pass-through list read back in a different order would make a review diff say
/// something changed when nothing did.
fn text_list(values: &[HarnessText]) -> String {
    values
        .iter()
        .map(|entry| string_literal(&entry.value))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests;
