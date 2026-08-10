import { type QueryClient, useQuery } from '@tanstack/react-query';

import { authoringFacade } from '../lib/facade';

/**
 * The one cache identity of the workspace document list. Everything that
 * reads or refreshes the list — the workspace panel's query and the guide's
 * "Try it" flow — goes through this key, so there is exactly one list to
 * agree about.
 */
export const authoringDocumentsQueryKey = ['authoring', 'documents'] as const;

export function useAuthoringDocuments() {
  return useQuery({
    queryKey: authoringDocumentsQueryKey,
    queryFn: () => authoringFacade.listDocuments(),
  });
}

/**
 * Fetches the document list fresh into the cache, resolving only once the
 * cache holds it. The "Try it" flow calls this BEFORE navigating: the
 * workspace panel treats a cached list as loaded and deselects any `?doc`
 * the list does not contain, so navigating on a stale cache would bounce the
 * operator straight off the document that was just created.
 */
export function fetchAuthoringDocuments(queryClient: QueryClient) {
  return queryClient.fetchQuery({
    queryKey: authoringDocumentsQueryKey,
    queryFn: () => authoringFacade.listDocuments(),
    staleTime: 0,
  });
}

export function useAuthoringDocument(path: string | null) {
  return useQuery({
    queryKey: ['authoring', 'document', path],
    queryFn: () => authoringFacade.loadDocument(path ?? ''),
    enabled: path !== null,
  });
}
