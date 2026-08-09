import guide from '../assets/awl-guide.json';

/**
 * The AWL glossary, as the console reads it.
 *
 * Nothing here writes a sentence about the language. The asset is generated
 * from `aion-awl`'s own glossary — the same table the CLI prints and the
 * language server hovers — by `aion awl guide --json`, and
 * `crates/aion-cli/tests/console_guide_asset.rs` fails if this copy and that
 * renderer disagree. The console keeps a copy rather than fetching one
 * because an author who cannot reach a server is exactly the author who needs
 * to look a word up.
 */
export interface AwlWord {
  /** The word itself, as an author types it. */
  readonly word: string;
  /** `keyword` for a reserved word, `word` for a positionally recognised one. */
  readonly kind: string;
  /** One sentence of meaning. */
  readonly summary: string;
  /**
   * Complete `.awl` documents demonstrating the word, verbatim from the
   * language's own glossary — one per honest form the word has. Each passes
   * `aion awl check` unchanged; the `aion-awl` guide tests run every example
   * through the real checker.
   */
  readonly examples: readonly string[];
  /** The reference section that explains the word in full. */
  readonly section: { readonly number: number; readonly title: string };
  /** The pointer to print under the summary, byte-identical to the CLI's. */
  readonly citation: string;
}

/** The whole glossary, with the documents an author is pointed at. */
export interface AwlGuide {
  readonly guide: string;
  readonly reference: string;
  readonly authoring: string;
  readonly workers: string;
  readonly commands: string;
  readonly declared_commands: string;
  readonly entries: readonly AwlWord[];
}

/** One reference section, with the words that belong to it. */
export interface AwlSection {
  readonly number: number;
  readonly title: string;
  readonly words: readonly AwlWord[];
}

export const awlGuide: AwlGuide = guide;

/**
 * The words grouped by the reference section they cite, sections in reference
 * order and words in the language's own inventory order.
 *
 * The order is derived from the data rather than listed here, so a word added
 * to the language appears in the right place without a second edit.
 */
export function sectionsOf(words: readonly AwlWord[]): AwlSection[] {
  const grouped = new Map<number, { title: string; words: AwlWord[] }>();
  for (const entry of words) {
    const existing = grouped.get(entry.section.number);
    if (existing === undefined) {
      grouped.set(entry.section.number, { title: entry.section.title, words: [entry] });
    } else {
      existing.words.push(entry);
    }
  }
  return [...grouped.entries()]
    .sort(([left], [right]) => left - right)
    .map(([number, { title, words: grouping }]) => ({ number, title, words: grouping }));
}

/**
 * The words a search matches, by word or by meaning.
 *
 * Searching the summary as well as the word is the difference between a
 * glossary and an index: an author who does not know that the word is
 * `distribute` can still find it by typing "parallel".
 */
export function matching(words: readonly AwlWord[], query: string): AwlWord[] {
  const needle = query.trim().toLowerCase();
  if (needle === '') return [...words];
  return words.filter(
    (entry) =>
      entry.word.toLowerCase().includes(needle) || entry.summary.toLowerCase().includes(needle)
  );
}
