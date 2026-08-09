import { describe, expect, test } from 'bun:test';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { renderToStaticMarkup } from 'react-dom/server';

import { ChangelogBody, WhatsNewPanel } from './WhatsNewPanel';

const CHANGELOG = [
  '# Changelog',
  '',
  '## 0.13.1 — 2026-08-10',
  '',
  "- **The What's-new panel.** The changelog ships in the binary and the",
  '  console shows it, with `aion` rendered as code.',
].join('\n');

describe('WhatsNewPanel', () => {
  test('closed by default: renders only the launcher and fetches nothing', () => {
    let fetches = 0;
    const markup = render(
      <WhatsNewPanel
        fetchChangelog={() => {
          fetches += 1;
          return Promise.resolve({ markdown: CHANGELOG });
        }}
      />
    );

    expect(markup).toContain('data-slot="whats-new-launcher"');
    expect(markup).not.toContain('data-slot="whats-new-panel"');
    expect(fetches).toBe(0);
  });

  test('open with the changelog cached: renders the markdown as elements', () => {
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    queryClient.setQueryData(['changelog'], { markdown: CHANGELOG });

    const markup = renderToStaticMarkup(
      <QueryClientProvider client={queryClient}>
        <WhatsNewPanel
          defaultOpen={true}
          fetchChangelog={() => Promise.resolve({ markdown: CHANGELOG })}
        />
      </QueryClientProvider>
    );

    expect(markup).toContain('data-slot="whats-new-panel"');
    expect(markup).toContain('data-slot="whats-new-changelog"');
    // Markdown became structure, not raw text: a release heading element and
    // an inline-code element.
    expect(markup).toContain('0.13.1 — 2026-08-10');
    expect(markup).toContain('<code');
    expect(markup).not.toContain('## 0.13.1');
  });

  test('open before the fetch resolves: says it is fetching, not broken', () => {
    const markup = render(
      <WhatsNewPanel defaultOpen={true} fetchChangelog={() => new Promise(() => undefined)} />
    );

    expect(markup).toContain('data-slot="whats-new-panel"');
    expect(markup).toContain('Fetching release notes…');
  });

  test('a failed fetch is shown, never swallowed', () => {
    const markup = renderToStaticMarkup(
      <ChangelogBody error={new Error('server unreachable')} markdown={undefined} pending={false} />
    );

    expect(markup).toContain('data-slot="whats-new-error"');
    expect(markup).toContain('server unreachable');
  });
});

function render(element: React.ReactElement) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });

  return renderToStaticMarkup(
    <QueryClientProvider client={queryClient}>{element}</QueryClientProvider>
  );
}
