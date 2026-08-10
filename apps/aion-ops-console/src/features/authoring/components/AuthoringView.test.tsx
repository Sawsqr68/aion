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

/**
 * The guide overlay rides the editing surfaces: its launcher must be mounted
 * on the workspace and on the deployed view. These assertions match the
 * overlay's `data-slot`, never its visible text — the Language panel's own
 * search label ('Search the language') is a prefix of the overlay's
 * ('Search the language guide'), so a text match could pass for the wrong
 * component entirely.
 */
test('the guide overlay launcher is mounted on the workspace and deployed surfaces', () => {
  expect(wrap(<AuthoringView />, '/authoring')).toContain('data-slot="guide-overlay-launcher"');
  expect(wrap(<AuthoringView />, '/authoring?source=deployed')).toContain(
    'data-slot="guide-overlay-launcher"'
  );
});

/** The Language tab IS the guide; floating a second copy over it would be noise. */
test('the language tab carries no guide overlay', () => {
  const html = wrap(<AuthoringView />, '/authoring?source=language');
  expect(html).not.toContain('data-slot="guide-overlay-launcher"');
  expect(html).not.toContain('data-slot="guide-overlay"');
});

/**
 * On the composed view the glossary reads as definitions, not a wall of
 * monospace: every example document is a disclosure and none arrives
 * pre-opened. This holds whether or not the highlighter has loaded — the
 * collapse is markup, not colouring.
 */
test('the language tab’s example documents arrive collapsed', () => {
  const html = wrap(<AuthoringView />, '/authoring?source=language');
  expect(html).toContain('data-slot="word-card-example"');
  expect(html).toContain('<details');
  expect(html).not.toContain('<details open');
  // And each disclosure carries its "Try it" footer on the composed view too:
  // the affordance rides the shared WordCard, not one surface's wiring.
  expect(html).toContain('data-slot="word-card-try-it"');
});
