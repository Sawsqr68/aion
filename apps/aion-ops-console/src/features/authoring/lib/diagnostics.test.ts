import { describe, expect, test } from 'bun:test';

import {
  mapDiagnostics,
  minimalTextDelta,
  nextDiagnosticsOpen,
  rangeToOffsets,
  statusForCheck,
} from './diagnostics';
import type { AwlDiagnostic } from './facade-check';

function diagnostic(
  message: string,
  line: number,
  column: number,
  range: AwlDiagnostic['range']
): AwlDiagnostic {
  return { class: 'error', message, line, column, range };
}

describe('authoring diagnostic mapping', () => {
  test('underlines the full wire range, not one code point', () => {
    const source = 'workflow demo\n\nstep go\n  after missing_step\n';
    const mapped = mapDiagnostics(source, [
      diagnostic('unknown after target `missing_step`', 4, 9, {
        start: { line: 3, character: 8 },
        end: { line: 3, character: 20 },
      }),
    ]);
    const lineStart = source.indexOf('  after ');
    expect(mapped.underlines).toEqual([
      {
        from: lineStart + 8,
        to: lineStart + 20,
        className: 'awl-underline-error',
        message: 'unknown after target `missing_step`',
      },
    ]);
    expect(mapped.lineBackgrounds).toEqual([{ line: 4, className: 'awl-line-error' }]);
    expect(mapped.gutterTexts).toEqual([{ line: 4, text: '!', className: 'awl-gutter-error' }]);
  });

  /**
   * 🔴 The documented drift class (2026-07-12): a diagnostic AFTER a
   * multibyte character must underline the right range. `🧭` is 2 UTF-16
   * units but 4 bytes; a byte-treating regression would place `café` at
   * offset 9 instead of 7.
   */
  test('a diagnostic after a multibyte character lands on the right code units', () => {
    const source = '//! 🧭 café\nworkflow probe\n';
    const mapped = mapDiagnostics(source, [
      diagnostic('probe', 1, 8, {
        start: { line: 0, character: 7 },
        end: { line: 0, character: 11 },
      }),
    ]);
    expect(mapped.underlines?.[0]?.from).toBe(7);
    expect(mapped.underlines?.[0]?.to).toBe(11);
    expect(source.slice(7, 11)).toBe('café');
  });

  test('a point range still gets a visible one-unit squiggle', () => {
    const mapped = mapDiagnostics('abc\ndef\n', [
      diagnostic('point', 1, 3, {
        start: { line: 0, character: 2 },
        end: { line: 0, character: 2 },
      }),
    ]);
    expect(mapped.underlines?.[0]).toMatchObject({ from: 2, to: 3 });
  });

  test('a stale range clamps to its line instead of bleeding across the newline', () => {
    const mapped = mapDiagnostics('abc\ndef\n', [
      diagnostic('stale', 1, 1, {
        start: { line: 0, character: 50 },
        end: { line: 0, character: 60 },
      }),
    ]);
    // Both ends clamp to the end of line 0 (offset 3); a point at the line
    // end stays flat — the gutter marker and line background carry the mark.
    expect(mapped.underlines?.[0]).toMatchObject({ from: 3, to: 3 });
  });

  test('rangeToOffsets answers the offsets navigation jumps to', () => {
    const source = 'workflow demo\nstep go\n';
    expect(
      rangeToOffsets(source, { start: { line: 1, character: 5 }, end: { line: 1, character: 7 } })
    ).toEqual({ from: 19, to: 21 });
  });

  test('rolls up both honest states', () => {
    expect(
      statusForCheck({ ok: true, deploysGreen: true, steps: 1, diagnostics: [], semantic: null })
    ).toEqual({
      tone: 'green',
      label: 'deploys green',
    });
    expect(
      statusForCheck({
        ok: false,
        deploysGreen: false,
        steps: 0,
        diagnostics: [
          diagnostic('x', 1, 1, {
            start: { line: 0, character: 0 },
            end: { line: 0, character: 1 },
          }),
        ],
        semantic: null,
      }).label
    ).toBe('1 errors');
  });

  // Fix-then-rebreak: the clean verdict spends the operator's open state,
  // and the next red must NOT resurrect the panel unasked.
  test('an emptied diagnostics list closes the panel and a later red does not reopen it', () => {
    expect(nextDiagnosticsOpen(true, 2)).toBe(true);
    expect(nextDiagnosticsOpen(true, 0)).toBe(false);
    expect(nextDiagnosticsOpen(false, 2)).toBe(false);
    expect(nextDiagnosticsOpen(false, 0)).toBe(false);
  });

  test('format computes one minimal replacement', () => {
    expect(minimalTextDelta('hello wrld', 'hello world')).toEqual([
      { from: 7, to: 7, insert: 'o' },
    ]);
  });
});
