import type { AwlWord } from '../lib/awl-guide';

/**
 * One word of AWL, rendered in full: the word, its kind, its one-sentence
 * meaning, every checked example document, and the reference citation.
 *
 * This is THE per-word rendering — the Language panel's list and the guide
 * overlay both compose it, so the two surfaces cannot drift into showing an
 * author different accounts of the same word. Every string on screen comes
 * from the generated glossary asset; nothing here writes prose about the
 * language.
 */
export function WordCard({ entry }: { entry: AwlWord }) {
  return (
    <div data-slot="word-card">
      <p className="flex items-baseline gap-2">
        <code className="font-medium text-foreground text-sm">{entry.word}</code>
        <span className="text-muted-foreground text-xs">{entry.kind}</span>
      </p>
      <p className="mt-1 text-foreground text-sm">{entry.summary}</p>
      {entry.examples.map((example, index) => (
        <div key={example}>
          {entry.examples.length > 1 && (
            <p className="mt-2 text-muted-foreground text-xs">Example {index + 1}</p>
          )}
          <pre className="mt-1 overflow-x-auto rounded border border-border bg-surface-base p-2 text-xs">
            <code className="font-mono text-foreground">{example}</code>
          </pre>
        </div>
      ))}
      <p className="mt-1 text-muted-foreground text-xs">{entry.citation}</p>
    </div>
  );
}
