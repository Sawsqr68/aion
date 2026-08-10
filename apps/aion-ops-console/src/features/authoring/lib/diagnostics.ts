import type { EditorDecorations } from '../editor-seam/types';
import type { AwlDiagnostic, CheckResult } from './facade';
import type { DiagnosticPosition, DiagnosticRange } from './facade-check';

export type AuthoringStatus =
  | { tone: 'green'; label: 'deploys green' }
  | { tone: 'red'; label: string };

/**
 * Maps wire diagnostics onto editor decorations. The wire carries each
 * diagnostic's full extent as a UTF-16 range (converted server-side), and
 * JavaScript strings index in UTF-16 code units, so `line start + character`
 * IS the CodeMirror document offset — nothing here reconstructs or guesses a
 * width.
 */
export function mapDiagnostics(
  source: string,
  diagnostics: readonly AwlDiagnostic[]
): EditorDecorations {
  const starts = lineStarts(source);
  return {
    lineBackgrounds: diagnostics.map((diagnostic) => ({
      line: diagnostic.line,
      className: 'awl-line-error',
    })),
    gutterTexts: diagnostics.map((diagnostic) => ({
      line: diagnostic.line,
      text: '!',
      className: 'awl-gutter-error',
    })),
    underlines: diagnostics.map((diagnostic) => ({
      ...offsetsForRange(starts, source, diagnostic.range),
      className: 'awl-underline-error',
      message: diagnostic.message,
    })),
  };
}

/**
 * A diagnostic range as document offsets against `source`, for navigation
 * (the diagnostics panel jumps the editor to `from`).
 */
export function rangeToOffsets(
  source: string,
  range: DiagnosticRange
): { from: number; to: number } {
  return offsetsForRange(lineStarts(source), source, range);
}

/**
 * The diagnostics-panel open policy for a fresh check verdict: an emptied
 * list CLOSES the panel. Fix-then-rebreak must not resurrect it unasked —
 * once the findings are gone the operator's open state is spent, and the
 * next red opens only by their hand.
 */
export function nextDiagnosticsOpen(open: boolean, diagnosticsCount: number): boolean {
  return diagnosticsCount === 0 ? false : open;
}

export function statusForCheck(result: CheckResult): AuthoringStatus {
  const errors = result.diagnostics.length;
  if (!result.ok || errors > 0) return { tone: 'red', label: `${errors} errors` };
  return { tone: 'green', label: 'deploys green' };
}

export function minimalTextDelta(before: string, after: string) {
  if (before === after) return [];
  let start = 0;
  while (start < before.length && start < after.length && before[start] === after[start])
    start += 1;
  let beforeEnd = before.length;
  let afterEnd = after.length;
  while (beforeEnd > start && afterEnd > start && before[beforeEnd - 1] === after[afterEnd - 1]) {
    beforeEnd -= 1;
    afterEnd -= 1;
  }
  return [{ from: start, to: beforeEnd, insert: after.slice(start, afterEnd) }];
}

function lineStarts(source: string) {
  const starts = [0];
  for (let index = 0; index < source.length; index += 1)
    if (source[index] === '\n') starts.push(index + 1);
  return starts;
}

function offsetsForRange(
  starts: readonly number[],
  source: string,
  range: DiagnosticRange
): { from: number; to: number } {
  const from = offsetOfPosition(starts, source, range.start);
  const to = Math.max(offsetOfPosition(starts, source, range.end), from);
  if (to > from) return { from, to };
  // A point diagnostic (or one clamped flat by a raced edit) still deserves a
  // visible squiggle: cover one code unit, without crossing the line end. If
  // the point IS the line end, from === to stands and the line background and
  // gutter marker carry the mark.
  return { from, to: Math.min(from + 1, lineEnd(starts, source, range.start.line)) };
}

function offsetOfPosition(
  starts: readonly number[],
  source: string,
  position: DiagnosticPosition
): number {
  const lineIndex = Math.min(Math.max(position.line, 0), starts.length - 1);
  const start = starts[lineIndex] ?? 0;
  // Clamp within the line: a stale response raced by an edit must not bleed
  // a decoration across a newline. The next debounced check reconciles.
  return Math.min(start + Math.max(position.character, 0), lineEnd(starts, source, lineIndex));
}

function lineEnd(starts: readonly number[], source: string, line: number): number {
  const lineIndex = Math.min(Math.max(line, 0), starts.length - 1);
  const nextStart = starts[lineIndex + 1];
  return nextStart === undefined ? source.length : nextStart - 1;
}
