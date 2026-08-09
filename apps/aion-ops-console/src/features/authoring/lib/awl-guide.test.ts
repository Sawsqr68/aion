import { expect, test } from 'bun:test';

import { awlGuide, matching, sectionsOf } from './awl-guide';

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
