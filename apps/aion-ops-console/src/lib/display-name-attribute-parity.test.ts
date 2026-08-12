import { describe, expect, test } from 'bun:test';
import { readdirSync, readFileSync } from 'node:fs';
import { join, relative } from 'node:path';

import { DISPLAY_NAME_ATTRIBUTE } from './workflowIdentity';

/**
 * Single-site guard for the display-name search-attribute key (#211).
 *
 * The key is declared once, in Rust (`aion_core::DISPLAY_NAME_ATTRIBUTE`), and
 * reaches TypeScript by GENERATION: the ts-rs pipeline emits it into
 * `src/types/generated/index.ts`, and `crates/aion-core`'s
 * `export_ops_console_wire_types` fails when the committed bindings no longer
 * match — so a Rust-side rename goes red in the Rust battery, in the lane that
 * renamed it, and regenerating carries the new key across with no hand edit.
 * There is no mirror left to check.
 *
 * What generation cannot prevent is a SECOND spelling: a reader or a fixture
 * that types the key as a literal keeps matching the OLD key after a rename,
 * and the console silently renders every named run as unnamed — the worst shape
 * a defect can take. That is what this file gates, and it is deliberately the
 * only thing left in it.
 */

const SRC_ROOT = join(import.meta.dir, '..');

/**
 * The only file allowed to contain the key: the `@generated` bindings that
 * DECLARE it. It is never hand-edited, so it cannot drift on its own.
 *
 * `lib/workflowIdentity.ts` is NOT exempt — it re-exports the generated value
 * and never spells it — and neither is this guard, which derives the key from
 * the constant.
 */
const ALLOWED = new Set(['types/generated/index.ts']);

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

describe('display-name attribute key', () => {
  // Catches: a second TS site spelling the key — a reader or a test fixture the
  // generated declaration does not reach, which would go on matching the OLD key
  // after a Rust-side rename. Tests are scanned too: a fixture built from a
  // literal stays green against a stale key and hides the break it should expose.
  test('only the generated bindings spell the key', () => {
    const offenders: string[] = [];
    for (const file of sourceFiles(SRC_ROOT)) {
      const name = relative(SRC_ROOT, file);
      if (ALLOWED.has(name)) {
        continue;
      }
      readFileSync(file, 'utf8')
        .split('\n')
        .forEach((line, index) => {
          if (line.includes(DISPLAY_NAME_ATTRIBUTE)) {
            offenders.push(`${name}:${index + 1}: ${line.trim()}`);
          }
        });
    }
    expect(offenders).toEqual([]);
  });
});
