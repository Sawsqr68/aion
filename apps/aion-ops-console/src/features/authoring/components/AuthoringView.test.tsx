import { expect, test } from 'bun:test';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import type { ReactNode } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { MemoryRouter } from 'react-router';

import { AuthoringView } from './AuthoringView';

function wrap(node: ReactNode, url: string): string {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return renderToStaticMarkup(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={[url]}>{node}</MemoryRouter>
    </QueryClientProvider>
  );
}

test('the studio offers both sources and starts on the workspace', () => {
  const html = wrap(<AuthoringView />, '/authoring');
  expect(html).toContain('Authoring source');
  expect(html).toContain('Workspace');
  expect(html).toContain('Deployed');
  expect(html).toContain('AWL documents and deployment checks');
});

/**
 * The source lives in the URL, so a deployed view is one shareable link — the
 * same rule the open workspace document follows.
 */
test('`?source=deployed` selects the deployed side and says what it is showing', () => {
  const html = wrap(<AuthoringView />, '/authoring?source=deployed');
  expect(html).toContain('AWL source carried by the packages deployed on this server');
  expect(html).not.toContain('AWL documents and deployment checks');
});

/** An unrecognised source is the workspace, not a blank screen. */
test('an unknown source falls back to the workspace', () => {
  const html = wrap(<AuthoringView />, '/authoring?source=elsewhere');
  expect(html).toContain('AWL documents and deployment checks');
});
