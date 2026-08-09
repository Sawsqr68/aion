import { useMemo, useState } from 'react';

import { type AwlWord, awlGuide, matching, sectionsOf } from '../lib/awl-guide';

/**
 * The studio's help affordance: every word of AWL, what it means, and where
 * the full treatment lives.
 *
 * This panel writes no prose about the language. Every sentence on screen
 * comes from the generated glossary asset, which is the same table
 * `aion awl guide` prints and the language server hovers — so an operator who
 * reads a tooltip, a teammate who reads the CLI, and this panel cannot end up
 * being told three different things.
 */
export function LanguagePanel() {
  const [query, setQuery] = useState('');
  const sections = useMemo(() => sectionsOf(matching(awlGuide.entries, query)), [query]);
  const found = sections.reduce((total, section) => total + section.words.length, 0);

  return (
    <div className="space-y-4 p-4">
      <div className="rounded-lg border border-border bg-surface-base p-3">
        <h2 className="font-semibold text-foreground text-sm">The language</h2>
        <p className="mt-1 text-muted-foreground text-sm">
          Every word AWL has, with the section of the reference that explains it. The same text is
          printed by <code className="text-foreground">aion awl guide</code> and shown on hover in
          an editor running <code className="text-foreground">aion awl lsp</code>.
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

      <div className="flex items-center gap-3">
        <input
          aria-label="Search the language"
          className="w-full rounded-md border border-border bg-surface-base px-2 py-1.5 text-foreground text-sm outline-none focus:ring-2 focus:ring-accent-primary"
          onChange={(event) => setQuery(event.target.value)}
          placeholder="Search a word or what it does"
          type="search"
          value={query}
        />
        <span className="whitespace-nowrap text-muted-foreground text-xs">
          {found} of {awlGuide.entries.length}
        </span>
      </div>

      {found === 0 ? (
        <p className="text-muted-foreground text-sm">
          No word of AWL matches “{query}”. The language has no{' '}
          <code className="text-foreground">select</code>, no user functions, and no general
          arithmetic; the reference says what stands in their place.
        </p>
      ) : (
        sections.map((section) => (
          <section key={section.number}>
            <h3 className="font-medium text-muted-foreground text-xs uppercase tracking-wide">
              §{section.number} — {section.title}
            </h3>
            <ul className="mt-2 space-y-2">
              {section.words.map((entry) => (
                <WordEntry entry={entry} key={entry.word} />
              ))}
            </ul>
          </section>
        ))
      )}
    </div>
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

function WordEntry({ entry }: { entry: AwlWord }) {
  return (
    <li className="rounded-md border border-border bg-surface-elevated px-3 py-2">
      <p className="flex items-baseline gap-2">
        <code className="font-medium text-foreground text-sm">{entry.word}</code>
        <span className="text-muted-foreground text-xs">{entry.kind}</span>
      </p>
      <p className="mt-1 text-foreground text-sm">{entry.summary}</p>
      <p className="mt-1 text-muted-foreground text-xs">{entry.citation}</p>
    </li>
  );
}
