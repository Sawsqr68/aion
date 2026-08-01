import { useQuery } from '@tanstack/react-query';

import { deployedFacade } from '../lib/deployed-facade';

export function useDeployedVersions() {
  return useQuery({
    queryKey: ['authoring', 'deployed', 'versions'],
    queryFn: () => deployedFacade.listDeployedVersions(),
  });
}

/**
 * One deployed version's archived document.
 *
 * A deployed version is IMMUTABLE — it is named by the content hash of the
 * package it came from — so once fetched it can never go stale. The query is
 * pinned accordingly rather than being refetched on every focus, which also
 * keeps a console tab from re-extracting archives on the server for no reason.
 */
export function useDeployedDocument(workflowType: string | null, contentHash: string | null) {
  return useQuery({
    queryKey: ['authoring', 'deployed', 'document', workflowType, contentHash],
    queryFn: () => deployedFacade.loadDeployedDocument(workflowType ?? '', contentHash ?? ''),
    enabled: workflowType !== null && contentHash !== null,
    staleTime: Number.POSITIVE_INFINITY,
    refetchOnWindowFocus: false,
  });
}
