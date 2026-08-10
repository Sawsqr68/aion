use std::path::Path;

use aion_awl::{
    Span, Utf16Position, byte_offset_at_utf16, utf16_position_at, utf16_range_for_span,
};
use lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};

/// Converts a byte offset in `source` to a zero-based UTF-16 LSP position.
///
/// The conversion itself is `aion_awl`'s — one shared mapping for the LSP and
/// the server's wire diagnostics — this adapter only rehomes it into
/// `lsp_types`. Offsets beyond the document are clamped to its end.
#[must_use]
pub fn position_at(source: &str, byte_offset: usize) -> Position {
    let position = utf16_position_at(source, byte_offset);
    Position::new(position.line, position.character)
}

/// Converts a zero-based UTF-16 LSP position to a byte offset in `source`.
///
/// Positions past a line or the document are clamped to the nearest end.
#[must_use]
pub fn byte_offset_at(source: &str, position: Position) -> usize {
    byte_offset_at_utf16(
        source,
        Utf16Position {
            line: position.line,
            character: position.character,
        },
    )
}

/// Converts an AWL byte span to an LSP range against the live document text.
#[must_use]
pub fn range_for_span(source: &str, span: Span) -> Range {
    let range = utf16_range_for_span(source, span);
    Range::new(
        Position::new(range.start.line, range.start.character),
        Position::new(range.end.line, range.end.character),
    )
}

fn diagnostic(source: &str, span: Span, message: String) -> Diagnostic {
    let mut diagnostic = Diagnostic::new_simple(range_for_span(source, span), message);
    diagnostic.severity = Some(DiagnosticSeverity::ERROR);
    diagnostic.source = Some("awl".to_owned());
    diagnostic
}

/// Runs the one real AWL parser and checker and maps their errors to LSP.
///
/// When `root` is present, schema imports resolve relative to the document's
/// directory through `aion_awl::check_in`; otherwise the same checker runs via
/// `aion_awl::check` and reports unresolved imports normally.
#[must_use]
pub fn diagnostics(source: &str, root: Option<&Path>) -> Vec<Diagnostic> {
    let document = match aion_awl::parse(source) {
        Ok(document) => document,
        Err(error) => return vec![diagnostic(source, error.span, error.message)],
    };
    let errors = root.map_or_else(
        || aion_awl::check(&document),
        |directory| aion_awl::check_in(&document, directory),
    );
    errors
        .into_iter()
        .map(|error| diagnostic(source, error.span, error.message))
        .collect()
}

/// Returns the canonical AWL printer output, or `None` when parsing fails.
#[must_use]
pub fn format_document(source: &str) -> Option<String> {
    aion_awl::parse(source)
        .ok()
        .map(|document| aion_awl::print(&document))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positions_are_utf16_not_bytes_or_scalar_columns() {
        let source = "//! 🧭 café\nworkflow probe\n";
        let workflow = source.find("workflow").unwrap_or_default();
        assert_eq!(position_at(source, workflow), Position::new(1, 0));
        let after_emoji = source.find(" café").unwrap_or_default();
        assert_eq!(position_at(source, after_emoji), Position::new(0, 6));
        assert_eq!(byte_offset_at(source, Position::new(0, 6)), after_emoji);
    }

    #[test]
    fn point_spans_stay_points() {
        let span = Span {
            start: 4,
            end: 4,
            line: 1,
            column: 5,
        };
        let range = range_for_span("café", span);
        assert_eq!(range.start, range.end);
        assert_eq!(range.start.character, 3);
    }
}
