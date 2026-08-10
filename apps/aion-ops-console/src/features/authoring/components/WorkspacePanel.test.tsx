import { expect, test } from 'bun:test';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { renderToStaticMarkup } from 'react-dom/server';
import { MemoryRouter } from 'react-router';

import { authoringDocumentsQueryKey } from '../hooks/useAuthoringDocuments';
import type { AwlDocument } from '../lib/facade';
import { WorkspacePanel } from './WorkspacePanel';

/**
 * Renders the panel the way the try-it flow leaves the app: the document
 * list already fresh in the query cache (the flow awaits the refetch BEFORE
 * navigating) and the created document's path in `?doc=`. Static rendering
 * runs no effects and no fetches, so what these tests see is exactly what
 * the cache and the URL alone determine.
 */
function renderWithDocuments(url: string, documents: AwlDocument[]): string {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  queryClient.setQueryData(authoringDocumentsQueryKey, documents);
  return renderToStaticMarkup(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={[url]}>
        <WorkspacePanel />
      </MemoryRouter>
    </QueryClientProvider>
  );
}

const DOCUMENTS: AwlDocument[] = [
  { path: 'cool_down.awl', name: 'cool_down' },
  { path: 'cool_down_2.awl', name: 'cool_down_2' },
];

/**
 * The landing half of the guide's "Try it" flow: after the create and the
 * list refresh, the navigation sets `?doc=<created path>` (with no `source`,
 * which IS the workspace tab) — and the panel must read that URL as the
 * selection. Here the collision-suffixed document is the one selected,
 * pinning that the flow's `_2` retry lands on the retried name, not the base.
 */
test('the ?doc= path from the URL is the selected document', () => {
  const html = renderWithDocuments('/authoring?doc=cool_down_2.awl', DOCUMENTS);
  expect(html.split('aria-current="true"').length - 1).toBe(1);
  const selectedRow = html.slice(html.indexOf('aria-current="true"'));
  const rowEnd = selectedRow.indexOf('</button>');
  expect(rowEnd).toBeGreaterThan(-1);
  expect(selectedRow.slice(0, rowEnd)).toContain('cool_down_2.awl');
});

/** Both documents are listed and counted — the refreshed list is on screen. */
test('the refreshed document list shows every document with its count', () => {
  const html = renderWithDocuments('/authoring?doc=cool_down_2.awl', DOCUMENTS);
  expect(html).toContain('cool_down.awl');
  expect(html).toContain('cool_down_2.awl');
  expect(html).toContain(`>${DOCUMENTS.length}</span>`);
});
