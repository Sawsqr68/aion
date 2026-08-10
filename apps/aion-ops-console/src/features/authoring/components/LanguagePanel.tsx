import { memo, useMemo, useState } from 'react';
import Markdown from 'react-markdown';

import { cn } from '@/lib/utils';

import { type AwlWord, awlGuide, formsOf, matching, sectionsOf } from '../lib/awl-guide';
import { type ReferenceBlock, referenceBlocksOf } from '../lib/reference-markdown';
import { WordCard } from './WordCard';

/** The panel's grouping axes: the grammar first, the reference's sections. */
export type LanguageGrouping = 'form' | 'section';

/**
 * The studio's help affordance: every word of AWL, what form it is, what it
 * means, and the full reference it is explained in.
 *
 * This panel writes no prose about the language. Every sentence on screen
 * comes from the generated glossary asset, which is the same table
 * `aion awl guide` prints and the language server hovers — so an operator who
 * reads a tooltip, a teammate who reads the CLI, and this panel cannot end up
 * being told three different things. The grammar grouping is derived (in
 * `aion-awl`) from the same table the editor's completion offers words from,
 * and the full reference text ships inside the asset, so the panel teaches
 * with no server reachable.
 *
 * Grouping by form is the primary axis; the reference's own sections stay
 * one toggle away, which is also how the CLI still prints the glossary.
 */
export function LanguagePanel({
  defaultGrouping = 'form',
}: {
  /** Initial grouping axis: a test seam, mirroring `GuideOverlay`'s pattern. */
  defaultGrouping?: LanguageGrouping;
}) {
  const [query, setQuery] = useState('');
  const [grouping, setGrouping] = useState<LanguageGrouping>(defaultGrouping);
  const found = useMemo(() => matching(awlGuide.entries, query), [query]);
  // The active grouping, computed once per (query, axis) change — never on
  // an unrelated re-render.
  const grouped = useMemo(
    () =>
      grouping === 'form'
        ? ({ axis: 'form', groups: formsOf(found) } as const)
        : ({ axis: 'section', groups: sectionsOf(found) } as const),
    [grouping, found]
  );

  return (
    <div className="space-y-4 p-4">
      <div className="rounded-lg border border-border bg-surface-base p-3">
        <h2 className="font-semibold text-foreground text-sm">The language</h2>
        <p className="mt-1 text-muted-foreground text-sm">
          {grouping === 'form'
            ? 'Every word AWL has, grouped by the form it takes in a document, with the section of the reference that explains it.'
            : 'Every word AWL has, grouped by the section of the reference that explains it.'}{' '}
          The same text is printed by <code className="text-foreground">aion awl guide</code> and
          shown on hover in an editor running <code className="text-foreground">aion awl lsp</code>.
        </p>
        <dl className="mt-3 grid gap-x-4 gap-y-1 text-xs sm:grid-cols-[auto_1fr]">
          <DocumentPath label="Guide" path={awlGuide.guide} />
          <DocumentPath label="Reference" path={awlGuide.reference} />
          <DocumentPath label="Authoring path" path={awlGuide.authoring} />
          <DocumentPath label="Workers" path={awlGuide.workers} />
          <DocumentPath label="Commands" path={awlGuide.commands} />
          <DocumentPath label="Declared bodies" path={awlGuide.declared_commands} />
        </dl>
      </div>

      <ReferenceDisclosure />

      <div className="flex items-center gap-3">
        <input
          aria-label="Search the language"
          className="w-full rounded-md border border-border bg-surface-base px-2 py-1.5 text-foreground text-sm outline-none focus:ring-2 focus:ring-accent-primary"
          onChange={(event) => setQuery(event.target.value)}
          placeholder="Search a word or what it does"
          type="search"
          value={query}
        />
        <fieldset className="flex rounded-md border border-border bg-surface-base p-0.5">
          <legend className="sr-only">Group the language</legend>
          <GroupingButton current={grouping} label="By form" onSelect={setGrouping} value="form" />
          <GroupingButton
            current={grouping}
            label="By section"
            onSelect={setGrouping}
            value="section"
          />
        </fieldset>
        {/* The regrouping is announced, not just repainted: a screen-reader
            operator who flips the axis hears that the list re-organized. */}
        <span aria-live="polite" className="sr-only" data-slot="language-grouping-announcement">
          {grouping === 'form' ? 'Grouped by grammatical form' : 'Grouped by reference section'}
        </span>
        <span className="whitespace-nowrap text-muted-foreground text-xs">
          {found.length} of {awlGuide.entries.length}
        </span>
      </div>

      {found.length === 0 ? (
        <p className="text-muted-foreground text-sm">
          No word of AWL matches “{query}”. The language has no{' '}
          <code className="text-foreground">select</code>, no user functions, and no general
          arithmetic; the reference says what stands in their place.
        </p>
      ) : grouped.axis === 'form' ? (
        grouped.groups.map((group) => (
          <section data-slot="language-group-form" key={group.form.id}>
            <h3 className="font-medium text-muted-foreground text-xs uppercase tracking-wide">
              {group.form.label}
            </h3>
            <p className="mt-0.5 text-muted-foreground text-xs">{group.form.description}</p>
            <WordList words={group.words} />
          </section>
        ))
      ) : (
        grouped.groups.map((section) => (
          <section data-slot="language-group-section" key={section.number}>
            <h3 className="font-medium text-muted-foreground text-xs uppercase tracking-wide">
              §{section.number} — {section.title}
            </h3>
            <WordList words={section.words} />
          </section>
        ))
      )}
    </div>
  );
}

/**
 * The whole language reference, rendered from the asset's own verbatim
 * markdown — the same document `aion awl guide --reference` prints, shipped
 * inside the asset so it opens with no server reachable. Collapsed by
 * default: the panel is a glossary first, and the reference is the place the
 * citations under each word point into.
 *
 * The document is split into markdown and pipe-table segments first
 * (`referenceBlocksOf`), because the markdown renderer speaks CommonMark
 * only — see `lib/reference-markdown.ts`.
 *
 * `memo` with no props means this renders exactly once per mount: the ~34 KB
 * parse and the few-thousand-node markdown tree are never re-paid on the
 * panel's keystroke path (the search box and the grouping toggle re-render
 * the panel on every input), and the segmentation itself is memoized too.
 */
const ReferenceDisclosure = memo(function ReferenceDisclosure() {
  const blocks = useMemo(() => referenceBlocksOf(awlGuide.reference_text), []);
  return (
    <details
      className="rounded-lg border border-border bg-surface-base p-3"
      data-slot="language-reference"
    >
      <summary className="cursor-pointer select-none rounded font-semibold text-foreground text-sm outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-accent-primary">
        The full language reference
      </summary>
      <div className="mt-2">
        {blocks.map((block, index) => (
          // Blocks are a static segmentation of one fixed document; their
          // position in it is their only identity.
          // biome-ignore lint/suspicious/noArrayIndexKey: static document segments
          <ReferenceBlockView block={block} key={index} />
        ))}
      </div>
    </details>
  );
});

function ReferenceBlockView({ block }: { block: ReferenceBlock }) {
  if (block.kind === 'markdown') {
    return <Markdown components={MARKDOWN_COMPONENTS}>{block.text}</Markdown>;
  }
  return (
    <div className="mb-2 overflow-x-auto" data-slot="language-reference-table">
      <table className="w-full border-collapse text-sm">
        <thead>
          <tr>
            {block.header.map((cell, cellIndex) => (
              <th
                // Header cells of a fixed document are positional — keying
                // by label would collide on two identically-titled columns.
                // biome-ignore lint/suspicious/noArrayIndexKey: static document segments
                key={cellIndex}
                className="border border-border px-2 py-1 text-left font-medium text-foreground"
              >
                <ReferenceCell text={cell} />
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {block.rows.map((row, rowIndex) => (
            // Rows of a fixed document; position is identity, as above.
            // biome-ignore lint/suspicious/noArrayIndexKey: static document segments
            <tr key={rowIndex}>
              {row.map((cell, cellIndex) => (
                <td
                  // biome-ignore lint/suspicious/noArrayIndexKey: static document segments
                  key={cellIndex}
                  className="border border-border px-2 py-1 align-top text-muted-foreground"
                >
                  <ReferenceCell text={cell} />
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

/**
 * One table cell's text, with its inline markdown (code spans, emphasis)
 * rendered and the renderer's paragraph wrapper unwrapped — a cell is not a
 * paragraph.
 */
function ReferenceCell({ text }: { text: string }) {
  return <Markdown components={CELL_COMPONENTS}>{text}</Markdown>;
}

function GroupingButton({
  current,
  label,
  onSelect,
  value,
}: {
  current: LanguageGrouping;
  label: string;
  onSelect: (next: LanguageGrouping) => void;
  value: LanguageGrouping;
}) {
  return (
    <button
      aria-pressed={current === value}
      className={cn(
        'whitespace-nowrap rounded px-2 py-1 text-xs outline-none focus-visible:ring-2 focus-visible:ring-accent-primary',
        current === value
          ? 'bg-surface-elevated font-medium text-foreground'
          : 'text-muted-foreground hover:text-foreground'
      )}
      onClick={() => onSelect(value)}
      type="button"
    >
      {label}
    </button>
  );
}

function DocumentPath({ label, path }: { label: string; path: string }) {
  return (
    <>
      <dt className="text-muted-foreground">{label}</dt>
      <dd className="text-foreground">
        <code>{path}</code>
      </dd>
    </>
  );
}

function WordList({ words }: { words: readonly AwlWord[] }) {
  return (
    <ul className="mt-2 space-y-2">
      {words.map((entry) => (
        <li
          className="rounded-md border border-border bg-surface-elevated px-3 py-2"
          key={entry.word}
        >
          <WordCard entry={entry} />
        </li>
      ))}
    </ul>
  );
}

/**
 * Styled element mapping for the reference markdown — the console has no
 * global typography styles, so each element names its own classes. Code
 * blocks keep their AWL indentation in a scrollable monospace pane.
 */
const MARKDOWN_COMPONENTS = {
  h1: (props: React.ComponentProps<'h1'>) => (
    <h1 className="mb-2 font-semibold text-base text-foreground" {...props} />
  ),
  h2: (props: React.ComponentProps<'h2'>) => (
    <h2
      className="mt-4 mb-2 border-border border-b pb-1 font-semibold text-foreground text-sm"
      {...props}
    />
  ),
  h3: (props: React.ComponentProps<'h3'>) => (
    <h3
      className="mt-3 mb-1 font-semibold text-foreground text-xs uppercase tracking-wide"
      {...props}
    />
  ),
  p: (props: React.ComponentProps<'p'>) => (
    <p className="mb-2 text-muted-foreground text-sm leading-relaxed" {...props} />
  ),
  ul: (props: React.ComponentProps<'ul'>) => (
    <ul className="mb-2 list-disc space-y-1 pl-5 text-muted-foreground text-sm" {...props} />
  ),
  ol: (props: React.ComponentProps<'ol'>) => (
    <ol className="mb-2 list-decimal space-y-1 pl-5 text-muted-foreground text-sm" {...props} />
  ),
  li: (props: React.ComponentProps<'li'>) => <li className="leading-relaxed" {...props} />,
  // The fenced-block box lives on `pre`; the code-span styling below is
  // neutralized inside it, so a fenced AWL block is one box, not a rounded
  // padded box nested in another.
  pre: (props: React.ComponentProps<'pre'>) => (
    <pre
      className="mb-2 overflow-x-auto rounded border border-border bg-surface-elevated p-2 text-xs [&_code]:rounded-none [&_code]:bg-transparent [&_code]:p-0"
      {...props}
    />
  ),
  code: (props: React.ComponentProps<'code'>) => (
    <code
      className="rounded bg-surface-elevated px-1 py-0.5 font-mono text-foreground text-xs"
      {...props}
    />
  ),
  strong: (props: React.ComponentProps<'strong'>) => (
    <strong className="font-semibold text-foreground" {...props} />
  ),
  a: (props: React.ComponentProps<'a'>) => (
    <a className="text-accent-primary underline" rel="noreferrer" target="_blank" {...props} />
  ),
};

/**
 * The cell mapping: the block mapping's inline styles with the paragraph
 * wrapper unwrapped, because a table cell is not a paragraph.
 */
const CELL_COMPONENTS = {
  ...MARKDOWN_COMPONENTS,
  p: ({ children }: React.ComponentProps<'p'>) => <>{children}</>,
};
