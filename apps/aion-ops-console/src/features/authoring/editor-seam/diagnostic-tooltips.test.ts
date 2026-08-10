import { describe, expect, test } from 'bun:test';

import { cursorDiagnosticForTest, diagnosticMessagesForTest } from './codemirror';

const SOURCE = 'workflow demo\n\nstep go\n  after missing_step\n';
const SPAN = {
  from: SOURCE.indexOf('missing_step'),
  to: SOURCE.indexOf('missing_step') + 'missing_step'.length,
  message: 'unknown after target `missing_step` — declare the step first',
};

describe('diagnostic tooltip targeting (headless seam)', () => {
  test('a position inside the range answers the message, outside answers nothing', () => {
    expect(diagnosticMessagesForTest(SOURCE, [SPAN], SPAN.from + 3)).toEqual([SPAN.message]);
    expect(diagnosticMessagesForTest(SOURCE, [SPAN], SPAN.from)).toEqual([SPAN.message]);
    expect(diagnosticMessagesForTest(SOURCE, [SPAN], SPAN.to)).toEqual([SPAN.message]);
    expect(diagnosticMessagesForTest(SOURCE, [SPAN], SPAN.from - 1)).toEqual([]);
    expect(diagnosticMessagesForTest(SOURCE, [SPAN], SPAN.to + 1)).toEqual([]);
  });

  test('overlapping diagnostics all answer, in document order', () => {
    const other = { from: SPAN.from, to: SPAN.from + 4, message: 'second finding' };
    expect(diagnosticMessagesForTest(SOURCE, [SPAN, other], SPAN.from + 2)).toEqual([
      SPAN.message,
      other.message,
    ]);
  });

  /**
   * The WIRING pin: both tooltip halves are read out of the production
   * extension list itself (`editorExtensions` — the same single source
   * `createCodeMirrorEditor` installs). `state.field` throws on an absent
   * field, so removing `cursorDiagnosticField` or `diagnosticHover` from
   * the production array turns this red — the drift the hand-copied
   * subset could never see. `hoverActive` is asserted, not merely read:
   * a headless state has hovered nothing, so the hover extension's active
   * set is exactly empty — the assertion is what makes the field read
   * load-bearing rather than deletable dead code.
   */
  test('both tooltip halves ride the production extension list', () => {
    expect(cursorDiagnosticForTest(SOURCE, [SPAN], SPAN.from + 2)).toEqual({
      cursor: { pos: SPAN.from + 2 },
      hoverActive: 0,
    });
    expect(cursorDiagnosticForTest(SOURCE, [SPAN], 0)).toEqual({
      cursor: null,
      hoverActive: 0,
    });
  });

  /**
   * Interplay rule 1: a POINTER selection into a diagnostic range does not
   * raise the cursor tooltip — pointer users get the hover tooltip, not a
   * second one at the caret. The keyboard-selection twin is the presence
   * control proving the suppression (and not some other absence) is what
   * kept the tooltip away.
   */
  test('a pointer selection into the range is suppressed, a keyboard one is not', () => {
    const inside = SPAN.from + 2;
    expect(
      cursorDiagnosticForTest(SOURCE, [SPAN], 0, { kind: 'pointer-select', to: inside }).cursor
    ).toBeNull();
    expect(
      cursorDiagnosticForTest(SOURCE, [SPAN], 0, { kind: 'keyboard-select', to: inside }).cursor
    ).toEqual({ pos: inside });
  });

  /**
   * Interplay rule 2: a document edit HIDES the cursor tooltip until fresh
   * diagnostics arrive — stale remedy prose must not sit pinned beside live
   * typing. The no-edit call above is the presence control; the edit is
   * before the range, so the mapped span still covers the cursor and only
   * the hide rule can explain the absence.
   */
  test('a document edit hides the cursor tooltip until the next check', () => {
    const probe = cursorDiagnosticForTest(SOURCE, [SPAN], SPAN.from + 2, {
      kind: 'edit',
      from: 0,
      to: 0,
      insert: '//! new line\n',
    });
    expect(probe.cursor).toBeNull();
  });

  /**
   * The spans are live state, not paint: an edit BEFORE the range moves the
   * tooltip target with the text, so hover keeps answering at the token's
   * new position until the next check replaces the set.
   */
  test('an edit before the range maps the tooltip target through the change', () => {
    const inserted = '//! new line\n';
    const moved = SPAN.from + inserted.length;
    expect(
      diagnosticMessagesForTest(SOURCE, [SPAN], moved + 3, { from: 0, to: 0, insert: inserted })
    ).toEqual([SPAN.message]);
    expect(
      diagnosticMessagesForTest(SOURCE, [SPAN], SPAN.from + 3, { from: 0, to: 0, insert: inserted })
    ).toEqual([]);
  });
});
