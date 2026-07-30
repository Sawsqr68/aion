//! Literal rendering shared by the scaffold emitters.

use std::fmt::Write as _;

/// Renders `value` as a double-quoted Rust (and TOML basic) string literal.
///
/// Both grammars escape `"` and `\` identically and neither tolerates a raw
/// control character, so one helper serves the emitted `.rs` and the emitted
/// `Cargo.toml`. Every emitted literal goes through it: an action name or a
/// path is data the generator did not write, and a stray quote in one must
/// never become a syntax error in the generated crate.
pub(super) fn string_literal(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            control if control.is_control() => {
                let _ = write!(out, "\\u{{{:x}}}", control as u32);
            }
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

/// Renders `text` as a block of `///` doc-comment lines at `indent` spaces,
/// with no trailing whitespace on a blank line.
pub(super) fn doc_lines(out: &mut String, indent: usize, text: &str) {
    let pad = " ".repeat(indent);
    for line in text.lines() {
        if line.is_empty() {
            let _ = writeln!(out, "{pad}///");
        } else {
            let _ = writeln!(out, "{pad}/// {line}");
        }
    }
}

/// Renders a comma-separated list of quoted names, for a diagnostic or a
/// documentation line.
pub(super) fn quoted_list(names: impl IntoIterator<Item = impl AsRef<str>>) -> String {
    names
        .into_iter()
        .map(|name| format!("`{}`", name.as_ref()))
        .collect::<Vec<_>>()
        .join(", ")
}
