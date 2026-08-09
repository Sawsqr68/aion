import { useQuery } from '@tanstack/react-query';
import { Sparkles, X } from 'lucide-react';
import { useState } from 'react';
import Markdown from 'react-markdown';

import type { ChangelogResponse } from '@/lib/api';

/**
 * The "What's new" panel: the changelog of the RUNNING server, fetched from
 * `GET /changelog` — bytes embedded in the binary at compile time, never a
 * file on the operator's disk, so the panel always describes the version
 * actually serving this console.
 *
 * Mirrors the guide overlay's shape deliberately: NON-MODAL — no backdrop, no
 * focus trap, no click-outside dismiss — a floating `complementary` landmark
 * the operator reads beside whatever they were doing. It closes only from its
 * own close button or the launcher toggle.
 *
 * The fetch waits until the panel is first opened (`enabled: open`): release
 * notes are read occasionally, and the console must not spend a request on
 * every page load for them.
 */
export function WhatsNewPanel({
  defaultOpen = false,
  fetchChangelog,
}: {
  /** Open on first render — the launcher toggle owns it afterwards. */
  defaultOpen?: boolean;
  /** Fetches `GET /changelog`; injected so tests need no live server. */
  fetchChangelog: () => Promise<ChangelogResponse>;
}) {
  const [open, setOpen] = useState(defaultOpen);

  const changelogQuery = useQuery({
    queryKey: ['changelog'],
    queryFn: fetchChangelog,
    enabled: open,
    // The changelog of a running binary cannot change while it runs.
    staleTime: Number.POSITIVE_INFINITY,
  });

  if (!open) {
    return (
      <button
        aria-expanded={false}
        className="inline-flex items-center gap-2 self-start rounded-md border border-border bg-surface-elevated px-3 py-1.5 text-foreground text-xs outline-none hover:bg-surface-base focus-visible:ring-2 focus-visible:ring-accent-primary sm:self-end"
        data-slot="whats-new-launcher"
        onClick={() => setOpen(true)}
        type="button"
      >
        <Sparkles aria-hidden="true" className="size-3.5" />
        What's new
      </button>
    );
  }

  return (
    <>
      <button
        aria-expanded={true}
        className="inline-flex items-center gap-2 self-start rounded-md border border-accent-primary bg-surface-elevated px-3 py-1.5 text-foreground text-xs outline-none hover:bg-surface-base focus-visible:ring-2 focus-visible:ring-accent-primary sm:self-end"
        data-slot="whats-new-launcher"
        onClick={() => setOpen(false)}
        type="button"
      >
        <Sparkles aria-hidden="true" className="size-3.5" />
        What's new
      </button>
      <aside
        aria-label="What's new in this release"
        className="fixed top-6 right-6 z-40 flex max-h-[80vh] w-[32rem] max-w-[calc(100vw-3rem)] flex-col overflow-hidden rounded-xl border border-border bg-surface-elevated shadow-xl"
        data-slot="whats-new-panel"
      >
        <header className="flex items-center justify-between gap-2 border-border border-b px-4 py-2.5">
          <span className="font-semibold text-foreground text-xs uppercase tracking-wide">
            What's new
          </span>
          <button
            aria-label="Close the what's-new panel"
            className="rounded-md p-1 text-muted-foreground outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-accent-primary"
            onClick={() => setOpen(false)}
            type="button"
          >
            <X aria-hidden="true" className="size-3.5" />
          </button>
        </header>

        <div className="min-h-0 flex-1 overflow-y-auto px-4 py-3">
          <ChangelogBody
            error={changelogQuery.error}
            markdown={changelogQuery.data?.markdown}
            pending={changelogQuery.isPending}
          />
        </div>
      </aside>
    </>
  );
}

/**
 * The panel body: exactly one of loading, error, or the rendered changelog.
 * Exported for its own tests — the error and pending branches are this
 * component's contract, independent of react-query's cache mechanics.
 */
export function ChangelogBody({
  error,
  markdown,
  pending,
}: {
  error: unknown;
  markdown: string | undefined;
  pending: boolean;
}) {
  if (markdown !== undefined) {
    return (
      <div data-slot="whats-new-changelog">
        <Markdown components={MARKDOWN_COMPONENTS}>{markdown}</Markdown>
      </div>
    );
  }

  if (error !== null && error !== undefined) {
    return (
      <p className="text-danger text-sm" data-slot="whats-new-error">
        The changelog could not be fetched from the server:{' '}
        {error instanceof Error ? error.message : String(error)}
      </p>
    );
  }

  if (pending) {
    return <p className="text-muted-foreground text-sm">Fetching release notes…</p>;
  }

  return null;
}

/**
 * Styled element mapping for the changelog markdown — the console has no
 * global typography styles, so each element names its own classes.
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
  code: (props: React.ComponentProps<'code'>) => (
    <code
      className="rounded bg-surface-base px-1 py-0.5 font-mono text-foreground text-xs"
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
