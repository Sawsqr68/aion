//! Byte-offset ↔ UTF-16 position conversion for AWL source text.
//!
//! AWL [`Span`]s carry BYTE offsets into the UTF-8 source. Editors — the LSP
//! protocol and the ops-console's `CodeMirror` seam alike — address text in
//! UTF-16 code units. This module is the one conversion between the two
//! worlds; the LSP adapter and the server's wire diagnostics both delegate
//! here so the mapping cannot drift between transports.

use crate::lexer::Span;

/// A zero-based position in a document, measured the way editors measure:
/// `line` counts `\n`-separated lines, `character` counts UTF-16 code units
/// from the start of that line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Utf16Position {
    /// Zero-based line index.
    pub line: u32,
    /// Zero-based UTF-16 code-unit offset within the line.
    pub character: u32,
}

/// A half-open range between two [`Utf16Position`]s (`start` inclusive,
/// `end` exclusive), the shape LSP ranges and editor decorations share.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Utf16Range {
    /// Where the range begins.
    pub start: Utf16Position,
    /// Where the range ends (exclusive).
    pub end: Utf16Position,
}

const fn utf16_width(ch: char) -> u32 {
    if ch.len_utf16() == 2 { 2 } else { 1 }
}

/// Converts a byte offset in `source` to a zero-based UTF-16 position.
///
/// Offsets beyond the document are clamped to its end. AWL spans are produced
/// on UTF-8 boundaries; if a caller supplies an interior byte, the position is
/// conservatively placed before that scalar value.
#[must_use]
pub fn utf16_position_at(source: &str, byte_offset: usize) -> Utf16Position {
    let target = byte_offset.min(source.len());
    let mut line = 0_u32;
    let mut character = 0_u32;
    for (offset, ch) in source.char_indices() {
        if offset >= target || offset.saturating_add(ch.len_utf8()) > target {
            break;
        }
        if ch == '\n' {
            line = line.saturating_add(1);
            character = 0;
        } else {
            character = character.saturating_add(utf16_width(ch));
        }
    }
    Utf16Position { line, character }
}

/// Converts a zero-based UTF-16 position to a byte offset in `source`.
///
/// Positions past a line or the document are clamped to the nearest end.
#[must_use]
pub fn byte_offset_at_utf16(source: &str, position: Utf16Position) -> usize {
    let mut line = 0_u32;
    let mut line_start = 0_usize;
    for (offset, ch) in source.char_indices() {
        if line == position.line {
            line_start = offset;
            break;
        }
        if ch == '\n' {
            line = line.saturating_add(1);
            line_start = offset + ch.len_utf8();
        }
    }
    if line < position.line {
        return source.len();
    }

    let mut utf16 = 0_u32;
    for (relative, ch) in source[line_start..].char_indices() {
        if ch == '\n' || utf16 >= position.character {
            return line_start + relative;
        }
        let next = utf16.saturating_add(utf16_width(ch));
        if next > position.character {
            return line_start + relative;
        }
        utf16 = next;
    }
    source.len()
}

/// Converts an AWL byte span to a UTF-16 range against the live document text.
///
/// A span whose `end` does not advance past its `start` collapses to a point
/// at `start` rather than an inverted range.
#[must_use]
pub fn utf16_range_for_span(source: &str, span: Span) -> Utf16Range {
    let start = utf16_position_at(source, span.start);
    let end = if span.end <= span.start {
        start
    } else {
        utf16_position_at(source, span.end)
    };
    Utf16Range { start, end }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The load-bearing pin: positions are UTF-16 code units, not bytes and
    /// not scalar values. `🧭` is 4 bytes / 2 UTF-16 units / 1 scalar; `é` is
    /// 2 bytes / 1 unit — a byte-counting or scalar-counting regression moves
    /// the answer.
    #[test]
    fn positions_are_utf16_not_bytes_or_scalar_columns() {
        let source = "//! 🧭 café\nworkflow probe\n";
        let workflow = source.find("workflow").unwrap_or_default();
        assert_eq!(
            utf16_position_at(source, workflow),
            Utf16Position {
                line: 1,
                character: 0
            }
        );
        let after_emoji = source.find(" café").unwrap_or_default();
        assert_eq!(
            utf16_position_at(source, after_emoji),
            Utf16Position {
                line: 0,
                character: 6
            }
        );
        assert_eq!(
            byte_offset_at_utf16(
                source,
                Utf16Position {
                    line: 0,
                    character: 6
                }
            ),
            after_emoji
        );
    }

    #[test]
    fn point_spans_stay_points() {
        let span = Span {
            start: 4,
            end: 4,
            line: 1,
            column: 5,
        };
        let range = utf16_range_for_span("café", span);
        assert_eq!(range.start, range.end);
        assert_eq!(range.start.character, 3);
    }

    #[test]
    fn offsets_past_the_document_clamp_to_its_end() {
        let source = "workflow x";
        assert_eq!(
            utf16_position_at(source, source.len() + 40),
            Utf16Position {
                line: 0,
                character: 10
            }
        );
        assert_eq!(
            byte_offset_at_utf16(
                source,
                Utf16Position {
                    line: 7,
                    character: 3
                }
            ),
            source.len()
        );
    }

    #[test]
    fn an_inverted_span_collapses_to_its_start() {
        let span = Span {
            start: 6,
            end: 2,
            line: 1,
            column: 7,
        };
        let range = utf16_range_for_span("abcdefgh", span);
        assert_eq!(range.start, range.end);
        assert_eq!(range.start.character, 6);
    }

    #[test]
    fn a_multibyte_span_covers_its_full_extent_in_utf16_units() {
        let source = "step go\n  make(\"🧭 café\") -> café_result\n";
        let start = source.find("café_result").unwrap_or_default();
        let span = Span {
            start,
            end: start + "café_result".len(),
            line: 2,
            column: 0,
        };
        let range = utf16_range_for_span(source, span);
        // Line prefix "  make(\"🧭 café\") -> " = 2+5+1+2+1+4+2+4 = 21 units.
        assert_eq!(
            range.start,
            Utf16Position {
                line: 1,
                character: 21
            }
        );
        // `café_result` is 11 scalar values, all BMP → 11 UTF-16 units.
        assert_eq!(
            range.end,
            Utf16Position {
                line: 1,
                character: 32
            }
        );
    }
}
