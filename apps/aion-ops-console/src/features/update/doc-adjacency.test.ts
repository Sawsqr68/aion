/**
 * Structural guard for this feature's documentation (r5 review → r6, at the
 * coordinator's direction): every doc-block end must sit IMMEDIATELY on the
 * thing it documents. Five review rounds caught the same species by hand —
 * an edit inserts code between a doc block and its declaration (r1-M4 in
 * `state.rs`, r5-M1 in `followCheck.ts`), leaving the declaration
 * undocumented and the doc attached to the wrong code. This converts that
 * catch into a mechanism.
 *
 * # The guard's TRUE reach (r6 review, minor)
 *
 * It scans exactly the `.ts`/`.tsx` files under THIS feature directory
 * (`features/update/`) — nothing else. The lane's other surfaces — the rest
 * of the console it touched (client, config, AppShell) and the whole Rust
 * half, including the `state.rs` where r1-M4 actually happened — are
 * OUTSIDE its scan; the r6 review hand-checked those structurally (zero
 * hits), but no mechanism watches them. Known residual within its reach:
 * a declaration's doc orphaned at the very TOP of a file, with no code
 * above it, is textually indistinguishable from a module header and
 * passes.
 *
 * The rule, mechanically: after a doc block (`/` + `**` … `*` + `/`) ends,
 * the next line must be a code line — not the start of another comment
 * (stacking: two docs above one declaration means some declaration lost
 * its doc), not blank (a doc block floating above whitespace documents
 * nothing), and not end-of-file. The one exemption is a FILE-HEADER block:
 * a doc block with no code above it may be followed by a blank line, the
 * TS idiom for module-level documentation (`semver.ts` uses it — and this
 * file's own header sits above its imports for the same reason; the
 * guard's first red honestly flagged it sitting below them).
 *
 * String literals containing comment markers would confuse a line scanner,
 * so this file builds its own marker patterns by concatenation and keeps
 * its doc comments rule-compliant — it scans itself along with the rest.
 */

import { describe, expect, test } from 'bun:test';
import { readdirSync, readFileSync } from 'node:fs';
import { join, relative } from 'node:path';

const FEATURE_ROOT = import.meta.dir;

/** Doc-block start marker, built by concatenation so self-scanning is safe. */
const DOC_START = '/*' + '*';
/** Any block-comment start. */
const BLOCK_START = '/' + '*';
/** Block-comment end marker. */
const BLOCK_END = '*' + '/';
/** Line-comment start. */
const LINE_COMMENT = '/' + '/';

function sourceFiles(dir: string): string[] {
  const out: string[] = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) {
      out.push(...sourceFiles(path));
    } else if (/\.tsx?$/.test(entry.name)) {
      out.push(path);
    }
  }
  return out;
}

type Violation = {
  file: string;
  line: number;
  reason: string;
};

/**
 * Scan one file for doc blocks whose end is not immediately followed by the
 * code they document. Line-based and deliberately simple: it tracks whether
 * a code line has been seen (for the file-header exemption) and where each
 * doc block starts and ends.
 */
function scan(path: string): Violation[] {
  const lines = readFileSync(path, 'utf8').split('\n');
  const violations: Violation[] = [];
  const file = relative(FEATURE_ROOT, path);

  let inBlock = false;
  let blockIsDoc = false;
  let blockHasCodeAbove = false;
  let codeSeen = false;

  for (let index = 0; index < lines.length; index += 1) {
    const raw = lines[index] ?? '';
    const trimmed = raw.trim();

    if (!inBlock) {
      if (trimmed.startsWith(DOC_START)) {
        blockIsDoc = true;
        blockHasCodeAbove = codeSeen;
        if (!trimmed.endsWith(BLOCK_END)) {
          inBlock = true;
          continue;
        }
        // Single-line doc block: falls through to the end-check below.
      } else if (trimmed.startsWith(BLOCK_START)) {
        blockIsDoc = false;
        if (!trimmed.endsWith(BLOCK_END)) {
          inBlock = true;
          continue;
        }
      } else {
        if (trimmed.length > 0 && !trimmed.startsWith(LINE_COMMENT)) {
          codeSeen = true;
        }
        continue;
      }
    } else if (trimmed.endsWith(BLOCK_END)) {
      inBlock = false;
    } else {
      continue;
    }

    // A block just ended on this line. Non-doc blocks carry no adjacency
    // contract; doc blocks must sit on their declaration.
    if (!blockIsDoc) {
      continue;
    }
    const lineNumber = index + 1;
    const next = lines[index + 1];
    if (next === undefined) {
      violations.push({
        file,
        line: lineNumber,
        reason: 'doc block ends at end-of-file, documenting nothing',
      });
      continue;
    }
    const nextTrimmed = next.trim();
    if (nextTrimmed.startsWith(DOC_START) || nextTrimmed.startsWith(BLOCK_START)) {
      violations.push({
        file,
        line: lineNumber,
        reason:
          'doc block is immediately followed by another comment block — stacked docs mean a ' +
          'declaration lost its documentation (the r5-M1 shape)',
      });
      continue;
    }
    if (nextTrimmed.length === 0) {
      if (!blockHasCodeAbove) {
        // File-header block: module-level documentation, blank line allowed.
        continue;
      }
      violations.push({
        file,
        line: lineNumber,
        reason: 'doc block is followed by a blank line — displaced from the code it documents',
      });
      continue;
    }
    if (nextTrimmed.startsWith(LINE_COMMENT)) {
      violations.push({
        file,
        line: lineNumber,
        reason: 'doc block is followed by a line comment, not the declaration it documents',
      });
    }
  }
  return violations;
}

describe('doc-block adjacency (the r5-M1 species, mechanized)', () => {
  test('every doc block in this feature sits immediately on its declaration', () => {
    const files = sourceFiles(FEATURE_ROOT);
    expect(files.length).toBeGreaterThan(0);
    const violations = files.flatMap(scan);
    expect(
      violations.map((violation) => `${violation.file}:${violation.line} — ${violation.reason}`)
    ).toEqual([]);
  });
});
