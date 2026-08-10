import { useEffect, useMemo, useState } from 'react';

import { useTryExample } from '../hooks/useTryExample';
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
          <ExampleTryIt example={example} />
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

/**
 * The footer of an expanded example: one "Try it" button that turns the
 * example the author just read into a workspace document — verbatim source,
 * atomic server-side create, `_2`/`_3` collision retry — and lands them on
 * it in the editor, Deploy/Run live.
 *
 * It sits INSIDE the disclosure, below the code, so it is only offered once
 * the example is on screen.
 */
function ExampleTryIt({ example }: { example: string }) {
  const { pending, refusal, tryExample } = useTryExample(example);
  return <TryItFooter onTry={() => void tryExample()} pending={pending} refusal={refusal} />;
}

/**
 * The footer's markup alone, with the behaviour injected: a test seam
 * (exported, mirroring the documented `highlighter?` prop's convention) so
 * the pending and refusal states can be pinned directly — a static render of
 * the full card can never be mid-flight.
 *
 * The button never takes focus uninvited (the guide overlay's non-modal
 * contract) and disables itself while a create is in flight. A refusal is
 * announced, not just shown: `role="alert"` in the console's one failure
 * vocabulary (`text-destructive`), exactly as the workspace panel's
 * create-document form renders its own — a screen reader hears the failure
 * instead of it passing as success.
 */
export function TryItFooter({
  onTry,
  pending,
  refusal,
}: {
  onTry: () => void;
  pending: boolean;
  refusal: string | null;
}) {
  return (
    <div className="mt-1 flex flex-wrap items-baseline gap-2" data-slot="word-card-try-it">
      <button
        className="rounded-md border border-border px-2 py-0.5 text-muted-foreground text-xs outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-accent-primary disabled:pointer-events-none disabled:opacity-60"
        disabled={pending}
        onClick={onTry}
        type="button"
      >
        {pending ? 'Creating…' : 'Try it'}
      </button>
      {refusal !== null && (
        <p className="text-destructive text-xs" data-slot="word-card-try-it-refusal" role="alert">
          {refusal}
        </p>
      )}
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
