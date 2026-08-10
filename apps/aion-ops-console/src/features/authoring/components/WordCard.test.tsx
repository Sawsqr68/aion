import { beforeAll, expect, test } from 'bun:test';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import type { ReactNode } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { MemoryRouter } from 'react-router';

import { type AwlWord, awlGuide } from '../lib/awl-guide';
import { type AwlHighlighter, loadAwlHighlighter } from '../lib/awl-static-highlight';
import { TryItFooter, WordCard } from './WordCard';

let highlighter: AwlHighlighter;

beforeAll(async () => {
  highlighter = await loadAwlHighlighter();
});

// The card's "Try it" affordance reads the URL and the query cache, so every
// render mounts inside the same providers the app gives it. Neither provider
// emits markup — the pins below are still about the card alone.
function render(node: ReactNode): string {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return renderToStaticMarkup(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={['/authoring?source=language']}>{node}</MemoryRouter>
    </QueryClientProvider>
  );
}

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
  const html = render(<WordCard entry={entry} highlighter={highlighter} />);
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
  const html = render(<WordCard entry={entry} highlighter={highlighter} />);
  expect(html).toContain('>Example</summary>');
  expect(html).not.toContain('Example 1');
});

/**
 * The card wears the word's grammar: the derived form beside the kind, and
 * the full list of positions the word may stand in — both read from the
 * asset, which carries them from the same table the editor's completion
 * offers words from. The strings are pinned exactly for two words whose
 * grammar differs, so a derivation change fails here naming the drift.
 */
test('the card names the word’s form and every position it stands in', () => {
  const sleep = render(<WordCard entry={entryOf('sleep')} highlighter={highlighter} />);
  expect(sleep).toContain('data-slot="word-card-form"');
  expect(sleep).toContain('>Step statement</span>');
  expect(sleep).toContain('data-slot="word-card-positions"');
  expect(sleep).toContain('Stands in a step or subflow body.');

  const run = render(<WordCard entry={entryOf('run')} highlighter={highlighter} />);
  expect(run).toContain('>Header clause</span>');
  expect(run).toContain('Stands in a worker block.');
});

/**
 * Only the code collapses: the one-sentence meaning stays above the
 * disclosures and the citation stays below them, both always visible.
 */
test('the summary and citation stay outside the collapsed examples', () => {
  const entry = entryOf('sleep');
  const html = render(<WordCard entry={entry} highlighter={highlighter} />);
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
  const html = render(<WordCard entry={entryOf('sleep')} highlighter={highlighter} />);
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
  const html = render(
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
  const html = render(<WordCard entry={entryOf('sleep')} highlighter="loading" />);
  expect(html).not.toContain('awl-syntax-');
  expect(html).toContain('sleep 30s');
  expect(html).not.toContain('data-slot="word-card-highlighting-notice"');
});

/**
 * Every example offers "Try it": one footer per disclosure, INSIDE the
 * `<details>` and below the `<pre>` — you read the example, then you try it.
 * The count is derived from the asset, so a new example arrives with its
 * button without a second edit here.
 */
test('every example carries a Try it footer inside its disclosure, below the code', () => {
  const entry = entryOf('sleep');
  const html = render(<WordCard entry={entry} highlighter={highlighter} />);
  expect(occurrences(html, 'data-slot="word-card-try-it"')).toBe(entry.examples.length);
  expect(occurrences(html, '>Try it</button>')).toBe(entry.examples.length);
  // Per disclosure: split at each `<details` — every chunk after the first is
  // one example, and each carries exactly one footer, after its code block
  // and before the disclosure closes.
  const disclosures = html.split('<details').slice(1);
  expect(disclosures.length).toBe(entry.examples.length);
  for (const disclosure of disclosures) {
    const body = disclosure.slice(0, disclosure.indexOf('</details>'));
    expect(occurrences(body, 'data-slot="word-card-try-it"')).toBe(1);
    expect(body.indexOf('data-slot="word-card-try-it"')).toBeGreaterThan(body.indexOf('</pre>'));
  }
});

/**
 * The button holds the overlay's non-modal line at rest: no autofocus, no
 * `tabindex="-1"`, and not disabled — the disabled state exists only while a
 * create is in flight, which a static render never is.
 */
test('the Try it button claims no focus and starts enabled', () => {
  const html = render(<WordCard entry={entryOf('sleep')} highlighter={highlighter} />);
  expect(html).not.toContain('autofocus');
  expect(html).not.toContain('autoFocus');
  expect(html).not.toContain('tabindex="-1"');
  // The attribute, not the `disabled:` styling variants in the class string:
  // React's static render writes a set boolean attribute as `disabled=""`.
  expect(html).not.toContain('disabled=""');
  expect(html).not.toContain('data-slot="word-card-try-it-refusal"');
});

/**
 * The footer's live states, pinned through the exported seam (a static
 * render of the full card can never be mid-flight or refused): a refusal is
 * ANNOUNCED, not just shown — `role="alert"` in the console's one failure
 * vocabulary, `text-destructive`, exactly as the workspace panel's
 * create-document form renders its own refusals — so a screen-reader
 * operator hears the failure instead of mistaking it for success.
 */
test('a try-it refusal is announced in the console’s failure vocabulary', () => {
  const html = renderToStaticMarkup(
    <TryItFooter
      onTry={() => undefined}
      pending={false}
      refusal="AWL workspace is not configured"
    />
  );
  expect(html).toContain('data-slot="word-card-try-it-refusal"');
  expect(html).toContain('role="alert"');
  expect(html).toContain('text-destructive');
  expect(html).toContain('AWL workspace is not configured');
});

/** While the create is in flight the button is disabled and says so. */
test('a pending try-it disables the button and names the state', () => {
  const html = renderToStaticMarkup(<TryItFooter onTry={() => undefined} pending refusal={null} />);
  expect(html).toContain('disabled=""');
  expect(html).toContain('Creating…');
  expect(html).not.toContain('>Try it</button>');
  expect(html).not.toContain('data-slot="word-card-try-it-refusal"');
});
