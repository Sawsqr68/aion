import { describe, expect, test } from 'bun:test';

import { isBlankDisplayName, normalizeDisplayName } from './workflowIdentity';

describe('normalizeDisplayName', () => {
  // Catches: a padded name from a foreign writer rendering padded. Every writer
  // in the system trims before recording — the server and all three caller SDKs
  // — so padding reaching a reader is foreign data, exactly like a blank. A
  // reader that returned it as-is would render the padding and would make two
  // readers holding `'X'` and `'  X  '` disagree about whether they are looking
  // at the same label.
  test.each([
    ['  Nightly settlement  ', 'Nightly settlement'],
    ['\tNightly settlement\n', 'Nightly settlement'],
    ['Nightly settlement', 'Nightly settlement'],
  ])('a surviving name is returned trimmed (%j)', (input, expected) => {
    expect(normalizeDisplayName(input)).toBe(expected);
  });

  // Catches: a blank taking the place the id would have occupied.
  test.each(['', '   ', '\t', '\n '])('a blank name is unnamed (%j)', (blank) => {
    expect(normalizeDisplayName(blank)).toBeNull();
  });

  test('absent and null are unnamed', () => {
    expect(normalizeDisplayName(undefined)).toBeNull();
    expect(normalizeDisplayName(null)).toBeNull();
  });
});

describe('isBlankDisplayName', () => {
  // Catches: the writer-side predicate drifting from the reader-side rule. The
  // start form, the rename submit path, and the REST client all refuse a name
  // this returns true for, so the two sides must agree on what "blank" is or a
  // name one surface accepts is one another renders as unnamed.
  test.each(['', ' ', '   ', '\t', '\n', '\r\n '])('%j is blank', (value) => {
    expect(isBlankDisplayName(value)).toBe(true);
    expect(normalizeDisplayName(value)).toBeNull();
  });

  test.each(['x', ' x ', 'Nightly settlement'])('%j is not blank', (value) => {
    expect(isBlankDisplayName(value)).toBe(false);
    expect(normalizeDisplayName(value)).not.toBeNull();
  });
});
