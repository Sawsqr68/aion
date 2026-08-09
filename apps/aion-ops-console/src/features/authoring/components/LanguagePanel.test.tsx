import { beforeAll, expect, test } from 'bun:test';
import { renderToStaticMarkup } from 'react-dom/server';

import { awlGuide } from '../lib/awl-guide';
import { loadAwlHighlighter } from '../lib/awl-static-highlight';
import { LanguagePanel } from './LanguagePanel';

// The highlighter is a process-wide singleton the cards read synchronously
// once loaded. Loading it up front makes every render in this file the
// steady-state one — highlighted — instead of depending on which test file
// happened to load it first.
beforeAll(async () => {
  await loadAwlHighlighter();
});

test('the panel shows every word of the language with its meaning', () => {
  const html = renderToStaticMarkup(<LanguagePanel />);
  for (const entry of awlGuide.entries) {
    expect(html).toContain(`>${entry.word}<`);
  }
  expect(html).toContain('Declares a typed worker contract whose name is the task queue');
});

/**
 * The panel exists so an author does not have to leave the console to find
 * out where the full treatment lives. A word without its citation is a
 * definition with no way to go deeper.
 */
test('every word carries the reference section that explains it', () => {
  const html = renderToStaticMarkup(<LanguagePanel />);
  expect(html).toContain('AWL reference §4 — Workers, actions, and calls');
  expect(html).toContain('§8 — Parallel and sequential work');
});

/**
 * The citation names a SECTION, never a repository path: the same glossary
 * ships inside the installed binary, whose user has no checkout to resolve
 * `examples/…` against. The console's own "Reference" affordance above the
 * list is the in-repo pointer, and that is where paths belong.
 */
test('the per-word citation carries no repository path', () => {
  const html = renderToStaticMarkup(<LanguagePanel />);
  for (const entry of awlGuide.entries) {
    expect(entry.citation).not.toContain('.md');
    expect(entry.citation).not.toContain('/');
  }
  expect(html).not.toContain('AWL-REFERENCE.md §');
});

/**
 * Every word teaches by example: complete documents, rendered monospace so
 * their two-space AWL indentation survives, that an author can paste into a
 * file and check unchanged. A word with several forms shows every one.
 * The documents render highlighted, so the assertions match a token class
 * around a known word rather than a flat run of text.
 */
test('every word renders its complete example documents', () => {
  const html = renderToStaticMarkup(<LanguagePanel />);
  for (const entry of awlGuide.entries) {
    expect(entry.examples.length).toBeGreaterThan(0);
    for (const example of entry.examples) {
      expect(example.startsWith('//!')).toBe(true);
    }
  }
  const sample = awlGuide.entries.find((entry) => entry.word === 'sleep');
  expect(sample).toBeDefined();
  if (sample !== undefined) {
    expect(sample.examples.length).toBeGreaterThan(1);
    expect(html).toContain('awl-syntax-keyword">sleep<');
    expect(html).toContain('awl-syntax-number">30s<');
    expect(html).toContain('awl-syntax-function">cool_down<');
    expect(html).toContain('awl-syntax-function">await_export<');
  }
  expect(html).toContain('Example 1');
  expect(html).toContain('Example 2');
});

/**
 * Every example on the panel is a per-document disclosure, and every one of
 * them starts collapsed: a page of definitions, not a wall of monospace. The
 * count is derived from the asset, so a new example arrives collapsed without
 * a second edit here.
 */
test('every example document starts collapsed', () => {
  const html = renderToStaticMarkup(<LanguagePanel />);
  const totalExamples = awlGuide.entries.reduce((total, entry) => total + entry.examples.length, 0);
  expect(totalExamples).toBeGreaterThan(50); // non-vacuity: the corpus is loaded
  expect(html.split('<details').length - 1).toBe(totalExamples);
  expect(html.split('<summary').length - 1).toBe(totalExamples);
  expect(html).not.toContain('<details open');
});

test('the panel names the documents an author needs, including the one about sandboxing', () => {
  const html = renderToStaticMarkup(<LanguagePanel />);
  expect(html).toContain('docs/authoring/AWL.md');
  expect(html).toContain('docs/workers/DECLARED-COMMANDS.md');
  expect(html).toContain('examples/assistant/resources/WORKERS.md');
});

/**
 * The console's copy of the glossary is a copy, and this is the console-side
 * half of keeping it honest: the panel must render what the asset says, not a
 * sentence written here. The Rust gate
 * (`crates/aion-cli/tests/console_guide_asset.rs`) owns the other half — that
 * the asset is what `aion awl guide --json` renders today.
 */
test('the rendered text is the asset text, not a paraphrase of it', () => {
  const html = renderToStaticMarkup(<LanguagePanel />);
  const sample = awlGuide.entries.find((entry) => entry.word === 'run');
  expect(sample).toBeDefined();
  if (sample !== undefined) {
    expect(html).toContain(sample.summary);
    expect(html).toContain(sample.citation);
  }
});

test('the panel says how many words it is showing', () => {
  const html = renderToStaticMarkup(<LanguagePanel />);
  expect(html).toContain(`${awlGuide.entries.length}`);
});
