import { beforeAll, expect, test } from 'bun:test';
import { renderToStaticMarkup } from 'react-dom/server';

import { type AwlWord, awlGuide } from '../lib/awl-guide';
import { type AwlHighlighter, loadAwlHighlighter } from '../lib/awl-static-highlight';
import { WordCard } from './WordCard';

let highlighter: AwlHighlighter;

beforeAll(async () => {
  highlighter = await loadAwlHighlighter();
});

function entryOf(word: string): AwlWord {
  const entry = awlGuide.entries.find((candidate) => candidate.word === word);
  if (entry === undefined) throw new Error(`the glossary asset has no entry for ${word}`);
  return entry;
}

function occurrences(html: string, needle: string): number {
  return html.split(needle).length - 1;
}

/**
 * Every example is its own native disclosure, and every one of them STARTS
 * COLLAPSED — static markup renders `<details>` with no `open` attribute,
 * which is the exact fact the browser's collapsed-by-default behavior rests
 * on. Expanding is per-example: one `<details>` per document, never a shared
 * wrapper.
 */
test('each example is a separate disclosure that starts collapsed', () => {
  const entry = entryOf('sleep');
  expect(entry.examples.length).toBeGreaterThan(1);
  const html = renderToStaticMarkup(<WordCard entry={entry} highlighter={highlighter} />);
  expect(occurrences(html, '<details')).toBe(entry.examples.length);
  expect(occurrences(html, '<summary')).toBe(entry.examples.length);
  expect(html).not.toContain('<details open');
  expect(html).toContain('Example 1');
  expect(html).toContain('Example 2');
});

/** A word with one example labels it plainly — numbering one of one is noise. */
test('a single example is labelled without a number', () => {
  const entry = entryOf('otherwise');
  expect(entry.examples.length).toBe(1);
  const html = renderToStaticMarkup(<WordCard entry={entry} highlighter={highlighter} />);
  expect(html).toContain('>Example</summary>');
  expect(html).not.toContain('Example 1');
});

/**
 * Only the code collapses: the one-sentence meaning stays above the
 * disclosures and the citation stays below them, both always visible.
 */
test('the summary and citation stay outside the collapsed examples', () => {
  const entry = entryOf('sleep');
  const html = renderToStaticMarkup(<WordCard entry={entry} highlighter={highlighter} />);
  expect(html).toContain(entry.summary);
  expect(html).toContain(entry.citation);
  expect(html.indexOf(entry.summary)).toBeLessThan(html.indexOf('<details'));
  expect(html.indexOf(entry.citation)).toBeGreaterThan(html.lastIndexOf('</details>'));
});

/**
 * The document inside a disclosure is coloured through the vendored
 * tree-sitter grammar: keywords, durations, and declared names carry the
 * editor's own `awl-syntax-*` classes, and a plain binding carries none.
 * Assertions match a token class around a known word, never the full markup.
 */
test('an expanded example is syntax-highlighted with the editor’s classes', () => {
  const html = renderToStaticMarkup(
    <WordCard entry={entryOf('sleep')} highlighter={highlighter} />
  );
  expect(html).toContain('awl-syntax-keyword">sleep<');
  expect(html).toContain('awl-syntax-keyword">workflow<');
  expect(html).toContain('awl-syntax-number">30s<');
  expect(html).toContain('awl-syntax-function">cool_down<');
  expect(html).toContain('awl-syntax-comment-documentation-workflow">');
  // The control: at the route site `done(build_id)` both words are an
  // author's own names, so the run must sit UNWRAPPED right after the
  // highlighted keyword — proving the tokenizer colours the vocabulary
  // without tinting the author's bindings.
  expect(html).toContain('route</span> done(build_id)');
});

/**
 * Losing the highlighter never loses the example: the document renders
 * verbatim, uncoloured, and the card says so instead of degrading silently.
 */
test('an unavailable highlighter shows plain examples and says why', () => {
  const entry = entryOf('sleep');
  const html = renderToStaticMarkup(
    <WordCard entry={entry} highlighter={{ unavailable: 'the wasm fetch failed' }} />
  );
  expect(html).not.toContain('awl-syntax-');
  expect(html).toContain('sleep 30s');
  expect(html).toContain('data-slot="word-card-highlighting-notice"');
  expect(html).toContain('title="the wasm fetch failed"');
  expect(html).toContain('Syntax colouring is unavailable');
});

/** While the highlighter is still loading, examples render plain with no notice. */
test('a still-loading highlighter renders plain examples without a warning', () => {
  const html = renderToStaticMarkup(<WordCard entry={entryOf('sleep')} highlighter="loading" />);
  expect(html).not.toContain('awl-syntax-');
  expect(html).toContain('sleep 30s');
  expect(html).not.toContain('data-slot="word-card-highlighting-notice"');
});
