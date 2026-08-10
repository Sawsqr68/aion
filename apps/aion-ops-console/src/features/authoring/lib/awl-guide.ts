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
  /**
   * The id of the word's grammatical form — the primary grouping axis.
   * Derived in `aion-awl` from the same grammar table the language server's
   * completion offers words from, so this panel and an editor's completion
   * list are structurally unable to disagree about what form a word is.
   */
  readonly form: string;
  /** The ids of every grammar position the word may stand in, table order. */
  readonly positions: readonly string[];
}

/** One grammatical form of the language, as the panel groups words by it. */
export interface AwlForm {
  readonly id: string;
  readonly label: string;
  readonly description: string;
}

/** One grammar position a word may stand in, with its rendered label. */
export interface AwlPosition {
  readonly id: string;
  readonly label: string;
}

/** The whole glossary, with the documents an author is pointed at. */
export interface AwlGuide {
  readonly guide: string;
  readonly reference: string;
  readonly authoring: string;
  readonly workers: string;
  readonly commands: string;
  readonly declared_commands: string;
  /** The form taxonomy, in the display order the panel groups by. */
  readonly forms: readonly AwlForm[];
  /** Every grammar position, in the table's own order. */
  readonly positions: readonly AwlPosition[];
  readonly entries: readonly AwlWord[];
  /** The complete AWL language reference, verbatim markdown. */
  readonly reference_text: string;
}

/** One reference section, with the words that belong to it. */
export interface AwlSection {
  readonly number: number;
  readonly title: string;
  readonly words: readonly AwlWord[];
}

/** One grammatical form, with the words that are that form. */
export interface AwlFormGroup {
  readonly form: AwlForm;
  readonly words: readonly AwlWord[];
}

export const awlGuide: AwlGuide = guide;

/**
 * The words grouped by grammatical form — the panel's primary axis — in the
 * taxonomy's own display order, words in the language's inventory order.
 *
 * Forms no word matches (mid-search) are omitted rather than rendered as
 * empty headings. A word naming a form the taxonomy does not carry is a
 * generation defect the Rust gates refuse, so it fails loudly here rather
 * than being dropped from the panel in silence.
 *
 * CONSEQUENCE OF THE THROW, named deliberately (here and for `formOf` /
 * `positionLabelsOf`, which share it): these run during render, so a
 * malformed asset unmounts the whole Language tab into the console's
 * `RootErrorBoundary` rather than degrading word-by-word. That is the
 * chosen trade — the asset is generated and byte-gated against the CLI, so
 * a self-disagreeing copy means the build is wrong, and a loud dead tab
 * beats a panel quietly teaching a partial language.
 */
export function formsOf(words: readonly AwlWord[]): AwlFormGroup[] {
  for (const entry of words) {
    if (!awlGuide.forms.some((form) => form.id === entry.form)) {
      throw new Error(
        `the glossary asset files \`${entry.word}\` under an unknown form: ${entry.form}`
      );
    }
  }
  return awlGuide.forms
    .map((form) => ({ form, words: words.filter((entry) => entry.form === form.id) }))
    .filter((group) => group.words.length > 0);
}

/**
 * The rendered labels of every position `entry` may stand in, in the
 * table's own order — the card's "stands" line. A position the inventory
 * does not carry is a generation defect, refused loudly for the same reason
 * `formsOf` refuses an unknown form.
 */
export function positionLabelsOf(entry: AwlWord): string[] {
  return entry.positions.map((id) => {
    const position = awlGuide.positions.find((candidate) => candidate.id === id);
    if (position === undefined) {
      throw new Error(`the glossary asset stands \`${entry.word}\` in an unknown position: ${id}`);
    }
    return position.label;
  });
}

/**
 * The form `entry` is, in full. The asset is generated and gated, so a miss
 * is a defect worth failing loudly over, exactly as `formsOf` does.
 */
export function formOf(entry: AwlWord): AwlForm {
  const form = awlGuide.forms.find((candidate) => candidate.id === entry.form);
  if (form === undefined) {
    throw new Error(
      `the glossary asset files \`${entry.word}\` under an unknown form: ${entry.form}`
    );
  }
  return form;
}

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
