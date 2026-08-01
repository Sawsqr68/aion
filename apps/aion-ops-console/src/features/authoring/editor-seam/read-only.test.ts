import { expect, test } from 'bun:test';

import { applyInsertForTest } from './codemirror';

const SOURCE = 'workflow deployed_probe\n';

/**
 * 🔴 THE READ-ONLY SEAM PIN, both halves. A read-only editor state REFUSES a
 * dispatched document change (absence) — and the identical attempt against an
 * editable state applies (survival), so the refusal is the read-only flag
 * working and not the test dispatching nothing.
 *
 * This is dispatched from CODE, not typed: a surface showing a deployed
 * artifact must be unchangeable by any path, not merely lack a keyboard.
 */
test('a read-only editor state refuses a dispatched change, and an editable one takes it', () => {
  expect(applyInsertForTest(SOURCE, '//! injected\n', true)).toBe(SOURCE);
  expect(applyInsertForTest(SOURCE, '//! injected\n', false)).toBe(`//! injected\n${SOURCE}`);
});
