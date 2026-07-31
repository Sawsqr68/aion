import { beforeAll, describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { Language, Parser, Query } from 'web-tree-sitter';

// The vendored highlighting artefacts are what the operator's authoring surface
// actually loads: highlight.worker.ts imports these two files and nothing else.
//
// crates/aion-awl/src/lexer/grammar_vocabulary_tests.rs gates the SOURCE
// grammar against the compiler's own keyword list, but it reads grammar.js —
// so editing the grammar and forgetting `tree-sitter build --wasm` +
// `cargo xtask build-ops-console` leaves that gate green while the console
// still colours the old vocabulary. This file closes that gap by asking the
// compiled artefact itself, not the source it was built from.

const ASSETS = join(import.meta.dir, '..', 'assets');
// lib -> authoring -> features -> src -> aion-ops-console -> apps -> repo root.
const REPO_ROOT = join(import.meta.dir, '..', '..', '..', '..', '..', '..');
const GRAMMAR_JS = join(REPO_ROOT, 'tools', 'tree-sitter-awl', 'grammar.js');

let parser: Parser;
let query: Query;

beforeAll(async () => {
  await Parser.init();
  const language = await Language.load(new Uint8Array(readFileSync(join(ASSETS, 'awl.wasm'))));
  parser = new Parser();
  parser.setLanguage(language);
  query = new Query(language, readFileSync(join(ASSETS, 'highlights.scm'), 'utf8'));
});

/** Capture names the vendored grammar and query give to `word` in `source`. */
function capturesOf(source: string, word: string): string[] {
  const tree = parser.parse(source);
  if (tree === null || tree === undefined) throw new Error('the vendored grammar failed to parse');
  const names = query
    .captures(tree.rootNode)
    .filter((capture) => capture.node.text === word)
    .map((capture) => capture.name);
  tree.delete();
  return names;
}

/**
 * Every quoted word inside the grammar's `const keywords = [ ... ];`.
 *
 * Extraction starts AFTER the opening bracket so the explanatory comment above
 * it — which names words in backticks — cannot leak in. This mirrors
 * `grammar_keywords()` in the Rust gate deliberately: both read the same source
 * of truth, and this one then checks the compiled artefact against it.
 */
function grammarKeywords(): string[] {
  const source = readFileSync(GRAMMAR_JS, 'utf8');
  const opened = source.split('const keywords = [')[1] ?? '';
  const body = opened.split('];')[0] ?? '';
  return body.split("'").filter((_, index) => index % 2 === 1);
}

describe('the vendored highlighting artefacts', () => {
  test('colour every word the grammar declares', () => {
    const words = grammarKeywords();
    expect(words.length).toBeGreaterThan(50); // non-vacuity: the array extracted

    // A keyword alone on a line lexes as `keyword` in this grammar, which is
    // what lets the assertion be driven by the grammar rather than a list
    // copied by hand — a copied list would drift the same way the wasm did.
    const uncoloured = words.filter((word) => !capturesOf(word, word).includes('keyword'));
    expect(uncoloured).toEqual([]);
  });

  test('colour a known keyword and leave an ordinary name alone', () => {
    expect(capturesOf('worker', 'worker')).toContain('keyword');
    expect(capturesOf('incident_id', 'incident_id')).not.toContain('keyword');
  });

  test('colour the action markers that were missing until 973b3d58', () => {
    const document = [
      'worker jobs',
      '  action ask(prompt: String) -> String',
      '    agent, timeout 10m',
      '  action announce(dir: String) -> Nil',
      '    advisory, node shell',
      '',
    ].join('\n');
    expect(capturesOf(document, 'agent')).toContain('keyword');
    expect(capturesOf(document, 'advisory')).toContain('keyword');
  });

  test("leave an author's own binding plain", () => {
    // `answer` is a positional word deliberately absent from `const keywords`:
    // this grammar cannot tell a statement opener from a binding, so colouring
    // it would tint the result bound here. See the Rust gate's
    // `the_bindable_positional_words_are_deliberately_left_plain`.
    expect(capturesOf('wait approval timeout 30s -> answer\n', 'answer')).toEqual([]);
  });
});
