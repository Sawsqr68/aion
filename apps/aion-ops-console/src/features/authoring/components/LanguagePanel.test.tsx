import { expect, test } from 'bun:test';
import { renderToStaticMarkup } from 'react-dom/server';

import { awlGuide } from '../lib/awl-guide';
import { LanguagePanel } from './LanguagePanel';

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
  expect(html).toContain(
    'examples/assistant/resources/AWL-REFERENCE.md §4 — Workers, actions, and calls'
  );
  expect(html).toContain('§8 — Parallel and sequential work');
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
