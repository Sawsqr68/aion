import { useEffect, useMemo, useState } from 'react';

import type { AwlWord } from '../lib/awl-guide';
import {
  type AwlHighlighterState,
  type AwlToken,
  awlHighlighterIfReady,
  loadAwlHighlighter,
} from '../lib/awl-static-highlight';
import { syntaxClassFor } from '../lib/highlight-classes';

/**
 * One word of AWL, rendered in full: the word, its kind, its one-sentence
 * meaning, every checked example document, and the reference citation.
 *
 * This is THE per-word rendering — the Language panel's list and the guide
 * overlay both compose it, so the two surfaces cannot drift into showing an
 * author different accounts of the same word. Every string on screen comes
 * from the generated glossary asset; nothing here writes prose about the
 * language.
 *
 * The summary and citation are always visible; each example document is a
 * native `<details>` disclosure that STARTS COLLAPSED, so a word with three
 * long examples reads as one sentence and three headings, not a wall of
 * monospace. Expanded, an example is coloured through the same vendored
 * tree-sitter grammar and highlight query the editor uses — one grammar, one
 * class vocabulary, zero drift.
 */
export function WordCard({
  entry,
  highlighter,
}: {
  entry: AwlWord;
  /** Override the ambient highlighter state: a test seam, mirroring `GuideOverlay`'s. */
  highlighter?: AwlHighlighterState;
}) {
  const ambient = useAwlHighlighter();
  const state = highlighter ?? ambient;
  return (
    <div data-slot="word-card">
      <p className="flex items-baseline gap-2">
        <code className="font-medium text-foreground text-sm">{entry.word}</code>
        <span className="text-muted-foreground text-xs">{entry.kind}</span>
      </p>
      <p className="mt-1 text-foreground text-sm">{entry.summary}</p>
      {entry.examples.map((example, index) => (
        <details className="mt-2" data-slot="word-card-example" key={example}>
          <summary className="cursor-pointer select-none rounded text-muted-foreground text-xs outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-accent-primary">
            {entry.examples.length > 1 ? `Example ${index + 1}` : 'Example'}
          </summary>
          <pre className="mt-1 overflow-x-auto rounded border border-border bg-surface-base p-2 text-xs">
            <ExampleCode example={example} state={state} />
          </pre>
        </details>
      ))}
      {isUnavailable(state) && (
        <p
          className="mt-2 text-muted-foreground text-xs"
          data-slot="word-card-highlighting-notice"
          title={state.unavailable}
        >
          Syntax colouring is unavailable — examples are shown plain.
        </p>
      )}
      <p className="mt-1 text-muted-foreground text-xs">{entry.citation}</p>
    </div>
  );
}

function isUnavailable(state: AwlHighlighterState): state is { readonly unavailable: string } {
  return typeof state === 'object' && 'unavailable' in state;
}

/**
 * The shared highlighter, arriving asynchronously: `loading` until the
 * one-time wasm + query load settles, then the highlighter itself — or the
 * failure reason, which the card SHOWS instead of swallowing. Static renders
 * (and the test suite) see the loaded highlighter synchronously once
 * `loadAwlHighlighter` has resolved anywhere in the process.
 */
function useAwlHighlighter(): AwlHighlighterState {
  const [state, setState] = useState<AwlHighlighterState>(
    () => awlHighlighterIfReady() ?? 'loading'
  );
  useEffect(() => {
    if (state !== 'loading') return;
    let cancelled = false;
    loadAwlHighlighter().then(
      (loaded) => {
        if (!cancelled) setState(loaded);
      },
      (error: unknown) => {
        if (!cancelled)
          setState({ unavailable: error instanceof Error ? error.message : String(error) });
      }
    );
    return () => {
      cancelled = true;
    };
  }, [state]);
  return state;
}

/**
 * One example document, coloured when the highlighter is available and
 * verbatim plain text when it is not — the example itself is never withheld.
 */
function ExampleCode({ example, state }: { example: string; state: AwlHighlighterState }) {
  const tokens = useMemo(() => tokensOrNull(state, example), [state, example]);
  if (tokens === null) {
    return <code className="font-mono text-foreground">{example}</code>;
  }
  return (
    <code className="font-mono text-foreground">
      {tokens.map((token) =>
        token.capture === null ? (
          token.text
        ) : (
          <span className={syntaxClassFor(token.capture)} key={token.start}>
            {token.text}
          </span>
        )
      )}
    </code>
  );
}

// tokensOf can only throw on the parser's unreachable no-tree branch, and if
// that branch is ever reached the error propagates loudly (the same stance
// DeployedDocumentPane's refuseGesture takes) instead of being swallowed here.
function tokensOrNull(state: AwlHighlighterState, example: string): readonly AwlToken[] | null {
  if (state === 'loading' || isUnavailable(state)) return null;
  return state.tokensOf(example);
}
