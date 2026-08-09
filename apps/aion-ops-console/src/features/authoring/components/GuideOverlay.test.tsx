import { beforeAll, expect, test } from 'bun:test';
import { renderToStaticMarkup } from 'react-dom/server';

import { awlGuide, matching } from '../lib/awl-guide';
import { loadAwlHighlighter } from '../lib/awl-static-highlight';
import { GuideOverlay } from './GuideOverlay';

// The highlighter is a process-wide singleton the cards read synchronously
// once loaded. Loading it up front makes every render in this file the
// steady-state one — highlighted — instead of depending on which test file
// happened to load it first.
beforeAll(async () => {
  await loadAwlHighlighter();
});

/**
 * Closed, the overlay is nothing but its launcher; open, the panel is there.
 * The launcher is a toggle, so it must say so to assistive tech.
 */
test('the overlay opens from a launcher toggle', () => {
  const closed = renderToStaticMarkup(<GuideOverlay />);
  expect(closed).toContain('data-slot="guide-overlay-launcher"');
  expect(closed).toContain('aria-expanded="false"');
  expect(closed).not.toContain('data-slot="guide-overlay"');

  const open = renderToStaticMarkup(<GuideOverlay defaultOpen />);
  expect(open).toContain('data-slot="guide-overlay"');
  expect(open).toContain('Search the language guide');
  expect(open).not.toContain('data-slot="guide-overlay-launcher"');
});

/**
 * The overlay must not behave like a dialog, because a dialog steals the
 * caret from the editor. Static markup cannot execute focus behavior, so
 * this test pins exactly the markup facts that behavior rests on: the
 * rendered HTML carries no `role="dialog"`, no `aria-modal`, no autofocus
 * attribute, and no full-viewport `inset-0` scrim element — it is an
 * `aside` (a complementary landmark) whose only fixed-position box is the
 * corner panel itself. Whether a browser honors those semantics is the
 * browser's contract, not this test's claim.
 */
test('the overlay markup carries no dialog, modal, autofocus, or scrim semantics', () => {
  const open = renderToStaticMarkup(<GuideOverlay defaultOpen />);
  expect(open).not.toContain('aria-modal');
  expect(open).not.toContain('role="dialog"');
  expect(open).not.toContain('autofocus');
  expect(open).not.toContain('autoFocus');
  expect(open).toContain('<aside');
  expect(open).not.toContain('inset-0');
});

/** The search filters the word list through the shared `matching` logic. */
test('search narrows the word list', () => {
  const open = renderToStaticMarkup(<GuideOverlay defaultOpen defaultQuery="distribute" />);
  expect(open).toContain('>distribute</button>');
  expect(open).not.toContain('>workflow</button>');

  const expected = matching(awlGuide.entries, 'distribute');
  expect(expected.map((entry) => entry.word)).toContain('distribute');
  for (const entry of expected) {
    expect(open).toContain(`>${entry.word}</button>`);
  }
});

/**
 * A search nothing matches says so instead of rendering an empty void.
 * (`elseif`, not `select`: the meaning-search legitimately finds `json`
 * for `select`, whose summary names the `run json` capture selector.)
 */
test('an unmatched search says no word matches', () => {
  const open = renderToStaticMarkup(<GuideOverlay defaultOpen defaultQuery="elseif" />);
  expect(open).toContain('No word of AWL matches');
});

/**
 * Selecting a word shows the full card: summary, every checked example, and
 * the citation — the same `WordCard` the Language panel renders. Each example
 * is now a disclosure, so the two documents are told apart by fragments only
 * one of them contains — `health` folds into `results`, `report?` into
 * `reports` — around the highlighted `collect` keyword.
 */
test('selecting a word shows its summary, examples, and citation', () => {
  const open = renderToStaticMarkup(<GuideOverlay defaultOpen defaultWord="collect" />);
  expect(open).toContain('data-slot="guide-overlay-detail"');
  expect(open).toContain('data-slot="word-card"');
  expect(open).toContain('Closes the nearest region');
  const entry = awlGuide.entries.find((candidate) => candidate.word === 'collect');
  expect(entry).toBeDefined();
  expect(entry?.examples.length).toBeGreaterThan(1);
  expect(open).toContain('awl-syntax-keyword">collect<');
  expect(open).toContain('collect</span> health ');
  expect(open).toContain('&gt;</span> results');
  expect(open).toContain('collect</span> report');
  expect(open).toContain('&gt;</span> reports');
  expect(open).toContain('AWL reference §8 — Parallel and sequential work');
  expect(open).toContain('aria-pressed="true"');
});

/**
 * The examples in the detail card start collapsed — one `<details>` per
 * document, none pre-opened — and the code behind each disclosure carries the
 * editor's own syntax classes, while an author's plain binding carries none.
 */
test('the selected word’s examples start collapsed and are highlighted', () => {
  const open = renderToStaticMarkup(<GuideOverlay defaultOpen defaultWord="collect" />);
  const entry = awlGuide.entries.find((candidate) => candidate.word === 'collect');
  expect(entry).toBeDefined();
  expect(open.split('<details').length - 1).toBe(entry?.examples.length ?? Number.NaN);
  expect(open).not.toContain('<details open');
  expect(open).toContain('awl-syntax-keyword">workflow<');
  expect(open).toContain('awl-syntax-operator">|&gt;<');
  expect(open).not.toContain('>health</span>');
});

/**
 * Open with nothing selected, the overlay offers the whole vocabulary and an
 * invitation. The markup also carries no `tabindex="-1"` wrapper — the
 * attribute a focus-managing dialog would stamp on its container — which is
 * as far as static markup can speak about the keyboard; runtime focus flow
 * is not claimed here.
 */
test('open with no selection, every word is offered', () => {
  const open = renderToStaticMarkup(<GuideOverlay defaultOpen />);
  for (const entry of awlGuide.entries) {
    expect(open).toContain(`>${entry.word}</button>`);
  }
  expect(open).toContain('Pick a word to see what it means');
  expect(open).not.toContain('tabindex="-1"');
});
