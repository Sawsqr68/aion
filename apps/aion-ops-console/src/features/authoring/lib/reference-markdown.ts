/**
 * Table-aware segmentation for the language reference's markdown.
 *
 * The reference uses GitHub-style pipe tables, which the console's markdown
 * renderer (react-markdown, CommonMark only) does not parse — it would show
 * the raw pipe rows as run-together paragraph text. Rather than adding a
 * plugin dependency for one document, the reference is split here into plain
 * markdown segments and table segments; the panel renders the former through
 * the markdown renderer and the latter as real tables, cell text verbatim.
 *
 * The segmentation is deliberately narrow: a table is a run of lines opening
 * with `|` whose second line is the `|---|` delimiter row, outside any fenced
 * code block. Anything that does not match that shape stays markdown text,
 * untouched — mis-reading a fence as a table would silently rewrite the
 * document, so fences are tracked and the tests pin both directions.
 */

/** One segment of the reference: markdown text, or a parsed pipe table. */
export type ReferenceBlock =
  | { readonly kind: 'markdown'; readonly text: string }
  | {
      readonly kind: 'table';
      readonly header: readonly string[];
      readonly rows: readonly (readonly string[])[];
    };

/** Whether `line` is a table delimiter row: only pipes, dashes, colons, spaces. */
function isDelimiterRow(line: string): boolean {
  const trimmed = line.trim();
  return trimmed.startsWith('|') && /^[|\s\-:]+$/.test(trimmed) && trimmed.includes('-');
}

/**
 * The cells of one pipe row, split on `|` outside backtick spans — a cell
 * whose code span contains a pipe (` `|>` `, say) must not be cut in half —
 * with GFM's `\|` escape honoured: an escaped pipe is cell TEXT (a literal
 * `|`, the backslash consumed, exactly as GFM renders it), never a
 * separator. The leading and trailing empty fields produced by the row's
 * outer pipes are dropped; interior cells keep their text verbatim, trimmed.
 *
 * A row whose backticks do not balance is REFUSED loudly: an unterminated
 * code span makes every later pipe ambiguous, and silently guessing would
 * rewrite the document's cells. The reference is authored prose, so the
 * throw reaches a human as a failing test or a dead reference pane, not as
 * a quietly mangled table.
 */
export function cellsOf(row: string): string[] {
  const trimmed = row.trim();
  const { cells, unterminated } = splitOnSeparatorPipes(trimmed);
  if (unterminated) {
    throw new Error(`a table row carries an unterminated code span: ${trimmed}`);
  }
  if (cells.length > 0 && cells[0]?.trim() === '') cells.shift();
  if (cells.length > 0 && cells[cells.length - 1]?.trim() === '') cells.pop();
  return cells.map((cell) => cell.trim());
}

/**
 * Split on `|` outside code spans, with `\|` kept as literal cell text.
 *
 * The unterminated-span verdict comes from THIS scan's own final state —
 * one state machine, so the escape handling and the balance check cannot
 * disagree about which backticks count (a `` \` `` outside a span is
 * escaped text and toggles nothing; a backtick inside a span always closes
 * it, backslashes there being literal code).
 */
function splitOnSeparatorPipes(row: string): { cells: string[]; unterminated: boolean } {
  const cells: string[] = [];
  let current = '';
  let inCode = false;
  let index = 0;
  while (index < row.length) {
    const character = row[index] ?? '';
    if (character === '\\' && !inCode) {
      current += escapedText(row[index + 1]);
      index += 2;
      continue;
    }
    if (character === '`') inCode = !inCode;
    if (character === '|' && !inCode) {
      cells.push(current);
      current = '';
    } else {
      current += character;
    }
    index += 1;
  }
  cells.push(current);
  return { cells, unterminated: inCode };
}

/**
 * The cell text a backslash pair contributes: GFM's `\|` is a literal pipe
 * with the backslash consumed; any other pair — or a trailing backslash —
 * stays as written.
 */
function escapedText(next: string | undefined): string {
  if (next === undefined) return '\\';
  return next === '|' ? '|' : `\\${next}`;
}

/**
 * The reference, split into markdown and table blocks in document order.
 * Concatenating the blocks' source lines reproduces the document: nothing is
 * dropped, nothing is rewritten — tables are re-shaped, never re-worded.
 */
export function referenceBlocksOf(text: string): ReferenceBlock[] {
  const lines = text.split('\n');
  const blocks: ReferenceBlock[] = [];
  let markdown: string[] = [];
  let inFence = false;

  const flushMarkdown = () => {
    if (markdown.length > 0) {
      blocks.push({ kind: 'markdown', text: markdown.join('\n') });
      markdown = [];
    }
  };

  let index = 0;
  while (index < lines.length) {
    const line = lines[index] ?? '';
    if (line.trimStart().startsWith('```')) {
      inFence = !inFence;
      markdown.push(line);
      index += 1;
      continue;
    }
    const startsTable =
      !inFence && line.trimStart().startsWith('|') && isDelimiterRow(lines[index + 1] ?? '');
    if (!startsTable) {
      markdown.push(line);
      index += 1;
      continue;
    }
    flushMarkdown();
    const header = cellsOf(line);
    index += 2; // Past the header and its delimiter row.
    const rows: string[][] = [];
    while (index < lines.length && (lines[index] ?? '').trimStart().startsWith('|')) {
      rows.push(cellsOf(lines[index] ?? ''));
      index += 1;
    }
    blocks.push({ kind: 'table', header, rows });
  }
  flushMarkdown();
  return blocks;
}
