import { beforeAll, expect, test } from 'bun:test';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { renderToStaticMarkup } from 'react-dom/server';
import { MemoryRouter } from 'react-router';

import { awlGuide, formsOf, sectionsOf } from '../lib/awl-guide';
import { loadAwlHighlighter } from '../lib/awl-static-highlight';
import { referenceBlocksOf } from '../lib/reference-markdown';
import { type LanguageGrouping, LanguagePanel } from './LanguagePanel';

// The highlighter is a process-wide singleton the cards read synchronously
// once loaded. Loading it up front makes every render in this file the
// steady-state one — highlighted — instead of depending on which test file
// happened to load it first.
beforeAll(async () => {
  await loadAwlHighlighter();
});

// The word cards' "Try it" affordance reads the URL and the query cache, so
// the panel renders inside the same providers the app mounts it under.
// Neither provider emits markup — every pin below is still about the panel.
function render(defaultGrouping?: LanguageGrouping): string {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return renderToStaticMarkup(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={['/authoring?source=language']}>
        <LanguagePanel {...(defaultGrouping === undefined ? {} : { defaultGrouping })} />
      </MemoryRouter>
    </QueryClientProvider>
  );
}

function occurrences(html: string, needle: string): number {
  return html.split(needle).length - 1;
}

/** Asset text as static markup carries it: React escapes `&<>"'`. */
function escaped(text: string): string {
  return text
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#x27;');
}

test('the panel shows every word of the language with its meaning', () => {
  const html = render();
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
  const html = render();
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
  const html = render();
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
  const html = render();
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
 * a second edit here. The one disclosure that is not an example is the full
 * reference, and it starts collapsed too.
 */
test('every example document starts collapsed', () => {
  const html = render();
  const totalExamples = awlGuide.entries.reduce((total, entry) => total + entry.examples.length, 0);
  expect(totalExamples).toBeGreaterThan(50); // non-vacuity: the corpus is loaded
  expect(html.split('<details').length - 1).toBe(totalExamples + 1);
  expect(html.split('<summary').length - 1).toBe(totalExamples + 1);
  expect(html).not.toContain('<details open');
});

/**
 * The grammar is the primary axis: with no seam supplied, the panel groups
 * by form — every form of the taxonomy as a heading with its description,
 * in the taxonomy's own order, and no reference-section headings at all.
 * The grouping is read whole from the asset's derived axis, so the panel
 * can never file a word differently than the completion table does.
 */
test('the panel groups by grammatical form by default', () => {
  const html = render();
  const groups = formsOf(awlGuide.entries);
  expect(groups.length).toBe(awlGuide.forms.length); // non-vacuity: no form is empty
  expect(occurrences(html, 'data-slot="language-group-form"')).toBe(groups.length);
  expect(occurrences(html, 'data-slot="language-group-section"')).toBe(0);
  for (const group of groups) {
    expect(html).toContain(`>${group.form.label}</h3>`);
    expect(html).toContain(escaped(group.form.description));
  }
  // The group headings appear in the taxonomy's own order. (The reference
  // disclosure renders its own h3 subheadings, so the order is read from the
  // labels themselves rather than from every h3 on the page.)
  const offsets = groups.map((group) => html.indexOf(`>${group.form.label}</h3>`));
  expect(offsets.every((offset) => offset >= 0)).toBe(true);
  expect(offsets).toEqual([...offsets].sort((left, right) => left - right));
});

/**
 * The reference's sections stay one toggle away (the axis the CLI still
 * prints). The seam mirrors `GuideOverlay`'s default-prop pattern; the
 * toggle is a fieldset with a screen-reader legend, both buttons declare
 * their state, a polite live region announces the regrouping, and the
 * panel's own description names the active axis instead of always claiming
 * the grammar one.
 */
test('the section axis stays available behind the grouping toggle', () => {
  const html = render();
  expect(html).toContain('>Group the language</legend>');
  expect(html).toContain('>By form</button>');
  expect(html).toContain('>By section</button>');
  expect(html).toContain('data-slot="language-grouping-announcement"');
  expect(html).toContain('aria-live="polite"');
  expect(html).toContain('Grouped by grammatical form');
  expect(html).toContain('grouped by the form it takes in a document');

  const bySection = render('section');
  const sections = sectionsOf(awlGuide.entries);
  expect(occurrences(bySection, 'data-slot="language-group-section"')).toBe(sections.length);
  expect(occurrences(bySection, 'data-slot="language-group-form"')).toBe(0);
  expect(bySection).toContain('>§1 — Document structure</h3>');
  expect(bySection).toContain('>§8 — Parallel and sequential work</h3>');
  expect(bySection).toContain('Grouped by reference section');
  expect(bySection).toContain('grouped by the section of the reference that explains it');
  expect(bySection).not.toContain('grouped by the form it takes in a document');
});

/**
 * Every word wears its grammar: the form chip beside the kind, and the
 * full list of positions the word may stand in — the same table the
 * editor's completion offers words from, so the two cannot disagree.
 */
test('every word carries its form and the positions it stands in', () => {
  const html = render();
  expect(occurrences(html, 'data-slot="word-card-form"')).toBe(awlGuide.entries.length);
  expect(occurrences(html, 'data-slot="word-card-positions"')).toBe(awlGuide.entries.length);
  expect(html).toContain('>Step statement</span>');
  expect(html).toContain('Stands in a step or subflow body.');
  expect(html).toContain(
    'Stands in a workflow header; in a worker block; in a step or subflow body; after wait or send, naming a signal.'
  );
});

/**
 * The full reference ships in the panel: the asset's verbatim markdown,
 * rendered — including the sections no glossary entry cites, which used to
 * be invisible to the console — with its pipe tables rendered as tables.
 */
test('the full reference is carried, first section to last, tables and all', () => {
  const html = render();
  expect(html).toContain('data-slot="language-reference"');
  expect(html).toContain('The full language reference');
  expect(html).toContain('>1. Document structure</h2>');
  expect(html).toContain('>16. Worker documents</h2>');
  const tables = referenceBlocksOf(awlGuide.reference_text).filter(
    (block) => block.kind === 'table'
  );
  expect(tables.length).toBeGreaterThan(0); // non-vacuity: the reference has tables
  expect(occurrences(html, 'data-slot="language-reference-table"')).toBe(tables.length);
  expect(html).toContain('Pins dispatch to a worker node label.');
});

/**
 * Every example on the full-page guide offers "Try it" — the same shared
 * `WordCard` footer the overlay's cards carry — so the whole corpus is one
 * click from the editor, and the count tracks the asset with no second edit.
 */
test('every example document offers Try it', () => {
  const html = render();
  const totalExamples = awlGuide.entries.reduce((total, entry) => total + entry.examples.length, 0);
  expect(html.split('data-slot="word-card-try-it"').length - 1).toBe(totalExamples);
});

test('the panel names the documents an author needs, including the one about sandboxing', () => {
  const html = render();
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
  const html = render();
  const sample = awlGuide.entries.find((entry) => entry.word === 'run');
  expect(sample).toBeDefined();
  if (sample !== undefined) {
    expect(html).toContain(sample.summary);
    expect(html).toContain(sample.citation);
  }
});

test('the panel says how many words it is showing', () => {
  const html = render();
  expect(html).toContain(`${awlGuide.entries.length}`);
});
