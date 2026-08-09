import { BookOpen, X } from 'lucide-react';
import { useMemo, useState } from 'react';

import { cn } from '@/lib/utils';

import { awlGuide, matching } from '../lib/awl-guide';
import { WordCard } from './WordCard';

/**
 * The definitions pop-over for the authoring surface: the whole language
 * guide, floating beside the editor instead of behind a tab swap.
 *
 * Deliberately NON-MODAL. An author reads this while typing, so it must
 * never behave like a dialog: no backdrop, no focus trap, no `aria-modal`,
 * no click-outside dismiss, and nothing here ever takes focus uninvited —
 * the editor keeps the caret unless the author clicks into the search box
 * themselves. It closes only from its own close button or the launcher
 * toggle. It renders as a `complementary` landmark pinned to the corner of
 * the viewport, clear of the editor column.
 *
 * Every sentence and example shown comes from the generated glossary asset
 * through the same [`WordCard`] the Language panel renders — one per-word
 * rendering, two surfaces, zero drift.
 */
export function GuideOverlay({
  defaultOpen = false,
  defaultQuery = '',
  defaultWord,
}: {
  /** Open on first render — the launcher toggle owns it afterwards. */
  defaultOpen?: boolean;
  /** Pre-filled search: a test seam, mirroring `ChatInputMorph`'s pattern. */
  defaultQuery?: string;
  /** Pre-selected word: a test seam, mirroring `ChatInputMorph`'s pattern. */
  defaultWord?: string;
}) {
  const [open, setOpen] = useState(defaultOpen);
  const [query, setQuery] = useState(defaultQuery);
  const [selected, setSelected] = useState<string | undefined>(defaultWord);

  const found = useMemo(() => matching(awlGuide.entries, query), [query]);
  const entry = awlGuide.entries.find((candidate) => candidate.word === selected);

  if (!open) {
    return (
      <button
        aria-expanded={false}
        className="fixed right-4 bottom-4 z-40 inline-flex items-center gap-2 rounded-full border border-border bg-surface-elevated px-3 py-2 text-foreground text-xs shadow-lg outline-none hover:bg-surface-base focus-visible:ring-2 focus-visible:ring-accent-primary"
        data-slot="guide-overlay-launcher"
        onClick={() => setOpen(true)}
        type="button"
      >
        <BookOpen aria-hidden="true" className="size-3.5" />
        Language guide
      </button>
    );
  }

  return (
    <aside
      aria-label="AWL language guide"
      className="fixed right-4 bottom-4 z-40 flex max-h-[70vh] w-96 max-w-[calc(100vw-2rem)] flex-col overflow-hidden rounded-xl border border-border bg-surface-elevated shadow-xl"
      data-slot="guide-overlay"
    >
      <header className="flex items-center justify-between gap-2 border-border border-b px-3 py-2">
        <span className="font-semibold text-foreground text-xs uppercase tracking-wide">
          Language guide
        </span>
        <button
          aria-label="Close the language guide"
          className="rounded-md p-1 text-muted-foreground outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-accent-primary"
          onClick={() => setOpen(false)}
          type="button"
        >
          <X aria-hidden="true" className="size-3.5" />
        </button>
      </header>

      <div className="border-border border-b px-3 py-2">
        <input
          aria-label="Search the language guide"
          className="w-full rounded-md border border-border bg-surface-base px-2 py-1 text-foreground text-sm outline-none focus:ring-2 focus:ring-accent-primary"
          onChange={(event) => setQuery(event.target.value)}
          placeholder="Search a word or what it does"
          type="search"
          value={query}
        />
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto p-3">
        {found.length === 0 ? (
          <p className="text-muted-foreground text-sm">
            No word of AWL matches “{query}”. The reference says what stands in its place.
          </p>
        ) : (
          <ul className="flex flex-wrap gap-1.5" data-slot="guide-overlay-words">
            {found.map((candidate) => (
              <li key={candidate.word}>
                <button
                  aria-pressed={candidate.word === selected}
                  className={cn(
                    'rounded-md border px-2 py-0.5 font-mono text-xs outline-none focus-visible:ring-2 focus-visible:ring-accent-primary',
                    candidate.word === selected
                      ? 'border-accent-primary bg-accent text-foreground'
                      : 'border-border text-muted-foreground hover:text-foreground'
                  )}
                  onClick={() =>
                    setSelected(candidate.word === selected ? undefined : candidate.word)
                  }
                  type="button"
                >
                  {candidate.word}
                </button>
              </li>
            ))}
          </ul>
        )}

        {entry ? (
          <div className="mt-3 border-border border-t pt-3" data-slot="guide-overlay-detail">
            <WordCard entry={entry} />
          </div>
        ) : (
          found.length > 0 && (
            <p className="mt-3 text-muted-foreground text-xs">
              Pick a word to see what it means, with checked examples.
            </p>
          )
        )}
      </div>
    </aside>
  );
}
