import { expect, test } from 'bun:test';

import { awlGuide } from './awl-guide';
import { cellsOf, referenceBlocksOf } from './reference-markdown';

/**
 * Nothing is dropped and nothing is invented: every line of the document is
 * accounted for by exactly one block — markdown lines verbatim, and for each
 * table its header line, its delimiter line, and one line per row.
 */
test('segmentation accounts for every line of the real reference', () => {
  const text = awlGuide.reference_text;
  const blocks = referenceBlocksOf(text);
  const accounted = blocks.reduce(
    (total, block) =>
      total + (block.kind === 'markdown' ? block.text.split('\n').length : block.rows.length + 2),
    0
  );
  expect(accounted).toBe(text.split('\n').length);
});

/** The real reference has tables, and their cells come through verbatim. */
test('the real reference’s tables are parsed with their cells intact', () => {
  const tables = referenceBlocksOf(awlGuide.reference_text).filter(
    (block) => block.kind === 'table'
  );
  expect(tables.length).toBeGreaterThan(0);
  const cells = tables.flatMap((table) => [...table.header, ...table.rows.flat()]);
  expect(cells).toContain('Pins dispatch to a worker node label.');
  expect(cells.some((cell) => cell.includes('`advisory`'))).toBe(true);
  for (const table of tables) {
    expect(table.header.length).toBeGreaterThan(0);
    for (const row of table.rows) {
      expect(row.length).toBeGreaterThan(0);
    }
  }
});

/** Cells split on pipes, but never on a pipe inside a code span. */
test('cells split on pipes outside code spans only', () => {
  expect(cellsOf('| a | b |')).toEqual(['a', 'b']);
  expect(cellsOf('| `a|b` | c |')).toEqual(['`a|b`', 'c']);
  expect(cellsOf('| `send name to target` | **only** when `Nil` |')).toEqual([
    '`send name to target`',
    '**only** when `Nil`',
  ]);
});

/**
 * GFM's `\|` escape is cell text — a literal pipe, backslash consumed —
 * while any other backslash pair stays as written; a trailing backslash is
 * kept rather than dropped.
 */
test('an escaped pipe is cell text, not a separator', () => {
  expect(cellsOf('| a \\| b | c |')).toEqual(['a | b', 'c']);
  expect(cellsOf('| path\\to | c |')).toEqual(['path\\to', 'c']);
  expect(cellsOf('| tail\\ | c |')).toEqual(['tail\\', 'c']);
});

/**
 * A row with an unterminated code span makes every later pipe ambiguous, so
 * it is refused loudly instead of being guessed into cells.
 */
test('an unterminated code span in a row is refused, never guessed', () => {
  expect(() => cellsOf('| `broken | c |')).toThrow('unterminated code span');
});

/**
 * An ESCAPED backtick is cell text, not a span delimiter — a GFM-legal
 * `` \` `` must neither open a span nor trip the unterminated-span refusal,
 * because the balance verdict comes from the same scan that consumes the
 * escape. Inside a span, a backslash is literal code and the next backtick
 * still closes it.
 */
test('an escaped backtick neither opens a span nor trips the refusal', () => {
  expect(cellsOf('| a \\` b | c |')).toEqual(['a \\` b', 'c']);
  expect(cellsOf('| `a\\` | c |')).toEqual(['`a\\`', 'c']);
});

/**
 * A pipe row inside a fenced code block is code, not a table — mis-reading
 * it would silently rewrite the document.
 */
test('pipe rows inside code fences stay markdown', () => {
  const text = 'before\n```\n| not | a table |\n|---|---|\n```\nafter';
  const blocks = referenceBlocksOf(text);
  expect(blocks.length).toBe(1);
  expect(blocks[0]?.kind).toBe('markdown');
  expect(blocks[0]?.kind === 'markdown' && blocks[0].text).toBe(text);
});

/** A pipe line with no delimiter row under it is not a table. */
test('a lone pipe line without a delimiter row stays markdown', () => {
  const text = '| just | a line |\nplain text';
  const blocks = referenceBlocksOf(text);
  expect(blocks.length).toBe(1);
  expect(blocks[0]?.kind).toBe('markdown');
});

/** A well-formed table parses into its header and rows. */
test('a well-formed table parses into header and rows', () => {
  const text = 'intro\n\n| Key | Meaning |\n|---|---|\n| `node` | Pins dispatch. |\ntail';
  const blocks = referenceBlocksOf(text);
  expect(blocks.map((block) => block.kind)).toEqual(['markdown', 'table', 'markdown']);
  const table = blocks[1];
  expect(table?.kind === 'table' && table.header).toEqual(['Key', 'Meaning']);
  expect(table?.kind === 'table' && table.rows).toEqual([['`node`', 'Pins dispatch.']]);
});
