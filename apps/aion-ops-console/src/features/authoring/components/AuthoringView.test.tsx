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

test('the studio offers every source and starts on the workspace', () => {
  const html = wrap(<AuthoringView />, '/authoring');
  expect(html).toContain('Authoring source');
  expect(html).toContain('Workspace');
  expect(html).toContain('Deployed');
  expect(html).toContain('Language');
  expect(html).toContain('AWL documents and deployment checks');
});

/**
 * The help is one click from the editor and one shareable link from anywhere
 * — the point of the whole weave. An author asking what `distribute` does
 * should never have to leave the console to find out.
 */
test('`?source=language` opens the glossary with the words in it', () => {
  const html = wrap(<AuthoringView />, '/authoring?source=language');
  expect(html).toContain('Every word of AWL, and where each one is explained in full');
  expect(html).toContain('Search the language');
  expect(html).toContain(
    'Opens a multi-step region that runs one item track per element in parallel.'
  );
  expect(html).not.toContain('AWL documents and deployment checks');
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
