import { expect, test } from 'bun:test';

import {
  type AwlWord,
  awlGuide,
  formOf,
  formsOf,
  matching,
  positionLabelsOf,
  sectionsOf,
} from './awl-guide';

/**
 * The asset is generated from the language itself, so its size is a fact
 * about AWL rather than a number to pin. What must hold is that it is there
 * at all — an empty asset would render an empty panel that looks like a
 * language with no words.
 */
test('the glossary carries the whole language and the documents behind it', () => {
  expect(awlGuide.entries.length).toBeGreaterThan(50);
  expect(awlGuide.guide).toBe('docs/authoring/AWL.md');
  expect(awlGuide.reference).toBe('examples/assistant/resources/AWL-REFERENCE.md');
  expect(awlGuide.declared_commands).toBe('docs/workers/DECLARED-COMMANDS.md');
  for (const entry of awlGuide.entries) {
    expect(entry.word.length).toBeGreaterThan(0);
    expect(entry.summary.endsWith('.')).toBe(true);
    expect(entry.citation).toContain(`§${entry.section.number}`);
    // Complete documents, not fragments: each opens with the mandatory
    // narration and declares a workflow or worker family.
    expect(entry.examples.length).toBeGreaterThan(0);
    for (const example of entry.examples) {
      expect(example.startsWith('//!')).toBe(true);
      expect(/\n(workflow|worker) /.test(example)).toBe(true);
    }
  }
});

/** Sections come out in reference order, each appearing once. */
test('sections are ordered and appear once each', () => {
  const sections = sectionsOf(awlGuide.entries);
  const numbers = sections.map((section) => section.number);
  expect(numbers).toEqual([...numbers].sort((left, right) => left - right));
  expect(new Set(numbers).size).toBe(numbers.length);
  const total = sections.reduce((count, section) => count + section.words.length, 0);
  expect(total).toBe(awlGuide.entries.length);
});

/** Within a section, words keep the language's own inventory order. */
test('grouping preserves the order the words arrived in', () => {
  const words = awlGuide.entries.filter((entry) => entry.section.number === 8);
  const grouped = sectionsOf(awlGuide.entries).find((section) => section.number === 8);
  expect(grouped?.words.map((entry) => entry.word)).toEqual(words.map((entry) => entry.word));
});

/**
 * Searching the meaning as well as the word is the whole difference between a
 * glossary and an index — an author who does not know the word is
 * `distribute` finds it by typing what they want.
 */
test('search reads the meaning, not only the word', () => {
  const byWord = matching(awlGuide.entries, 'distribute');
  expect(byWord.map((entry) => entry.word)).toContain('distribute');

  const byMeaning = matching(awlGuide.entries, 'parallel');
  expect(byMeaning.map((entry) => entry.word)).toContain('distribute');
  expect(byMeaning.length).toBeLessThan(awlGuide.entries.length);
});

test('an empty search is every word, and an unmatched search is none', () => {
  expect(matching(awlGuide.entries, '   ').length).toBe(awlGuide.entries.length);
  // `elseif` is nothing AWL has and appears in no summary. `select` no longer
  // works as the unmatched probe: the language still has no `select`, but the
  // meaning-search legitimately finds `json`, whose summary names the
  // `run json` capture SELECTor.
  expect(matching(awlGuide.entries, 'elseif').length).toBe(0);
  expect(matching(awlGuide.entries, 'select').map((entry) => entry.word)).toEqual(['json']);
});

test('search ignores case', () => {
  expect(matching(awlGuide.entries, 'WORKER').map((entry) => entry.word)).toContain('worker');
});

/**
 * The grammar axis rides the asset whole: a form taxonomy and a position
 * inventory with distinct ids, and every entry filed under a form the
 * taxonomy carries, standing in positions the inventory carries. This is
 * the console-side half of the Rust wire gate.
 */
test('the grammar axis is closed over the taxonomy and the inventory', () => {
  expect(awlGuide.forms.length).toBeGreaterThan(0);
  expect(new Set(awlGuide.forms.map((form) => form.id)).size).toBe(awlGuide.forms.length);
  for (const form of awlGuide.forms) {
    expect(form.label.length).toBeGreaterThan(0);
    expect(form.description.endsWith('.')).toBe(true);
  }
  expect(awlGuide.positions.length).toBeGreaterThan(0);
  expect(new Set(awlGuide.positions.map((position) => position.id)).size).toBe(
    awlGuide.positions.length
  );
  for (const entry of awlGuide.entries) {
    expect(awlGuide.forms.some((form) => form.id === entry.form)).toBe(true);
    expect(entry.positions.length).toBeGreaterThan(0);
    for (const id of entry.positions) {
      expect(awlGuide.positions.some((position) => position.id === id)).toBe(true);
    }
  }
});

/**
 * Grouping by form partitions the vocabulary in the taxonomy's own display
 * order, words keeping the language's inventory order — the same contract
 * `sectionsOf` honours on the other axis.
 */
test('grouping by form partitions the words in taxonomy order', () => {
  const groups = formsOf(awlGuide.entries);
  expect(groups.map((group) => group.form.id)).toEqual(awlGuide.forms.map((form) => form.id));
  const total = groups.reduce((count, group) => count + group.words.length, 0);
  expect(total).toBe(awlGuide.entries.length);

  const statements = awlGuide.entries.filter((entry) => entry.form === 'statement');
  const grouped = groups.find((group) => group.form.id === 'statement');
  expect(grouped?.words.map((entry) => entry.word)).toEqual(statements.map((entry) => entry.word));
});

/** Mid-search, a form nothing matches is omitted, never an empty heading. */
test('grouping by form omits forms a search leaves empty', () => {
  const narrowed = formsOf(matching(awlGuide.entries, 'distribute'));
  expect(narrowed.length).toBeGreaterThan(0);
  for (const group of narrowed) {
    expect(group.words.length).toBeGreaterThan(0);
  }
  expect(narrowed.length).toBeLessThan(awlGuide.forms.length);
});

/**
 * The per-entry lookups answer from the asset, and an entry pointing outside
 * the taxonomy or the inventory fails loudly — a generated asset that
 * disagrees with itself is a defect, not a word to quietly drop.
 */
test('form and position lookups answer, and refuse an entry pointing nowhere', () => {
  const wait = awlGuide.entries.find((entry) => entry.word === 'wait');
  expect(wait).toBeDefined();
  if (wait !== undefined) {
    expect(formOf(wait).label).toBe('Step statement');
    expect(positionLabelsOf(wait)).toEqual(['in a step or subflow body']);
  }

  const stray: AwlWord = {
    word: 'stray',
    kind: 'keyword',
    summary: 'Not a word of the language.',
    examples: ['//! not real'],
    section: { number: 1, title: 'Document structure' },
    citation: 'AWL reference §1 — Document structure',
    form: 'no-such-form',
    positions: ['no-such-position'],
  };
  expect(() => formsOf([stray])).toThrow('unknown form');
  expect(() => formOf(stray)).toThrow('unknown form');
  expect(() => positionLabelsOf(stray)).toThrow('unknown position');
});

/**
 * The whole reference rides the asset, and every section a word cites is a
 * heading actually in it — the console-side echo of the Rust reference
 * gate, run against the copy this panel really renders.
 */
test('the reference text carries every section the words cite', () => {
  expect(awlGuide.reference_text).toContain('## 1. Document structure');
  expect(awlGuide.reference_text).toContain('## 16. Worker documents');
  for (const entry of awlGuide.entries) {
    expect(awlGuide.reference_text).toContain(`## ${entry.section.number}. ${entry.section.title}`);
  }
});
