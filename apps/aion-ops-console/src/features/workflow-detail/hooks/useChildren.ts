import { useQuery } from '@tanstack/react-query';
import { useMemo } from 'react';

import type { ChildLeg, ChildrenReadClient } from '@/lib/api';
import { createConfiguredChildrenReader } from '@/lib/config';
import type { Namespace, RunId, WorkflowId } from '@/types';

export type UseChildrenOptions = {
  namespace: Namespace | null;
  workflowId: WorkflowId;
  runId: RunId | null;
  /** Latest parent ChildWorkflowStarted seq; socket changes trigger one derived reread. */
  discoveryRevision?: number;
  reader?: Pick<ChildrenReadClient, 'listChildren'>;
};

/** Pending child runs normally start immediately; 1s keeps the card live without a hot loop. */
export function pendingChildRefetchInterval(
  children: readonly ChildLeg[] | undefined
): number | false {
  return children?.some((child) => child.runId === null) === true ? 1_000 : false;
}

/** One discrete derived-history read; live child changes arrive on child sockets. */
export function useChildren({
  namespace,
  workflowId,
  runId,
  discoveryRevision = 0,
  reader,
}: UseChildrenOptions) {
  const client = useMemo(
    () => reader ?? createConfiguredChildrenReader({ namespace }),
    [namespace, reader]
  );
  return useQuery<ChildLeg[]>({
    enabled: namespace !== null,
    queryKey: ['workflow-children', namespace, workflowId, runId, discoveryRevision],
    refetchInterval: (query) => pendingChildRefetchInterval(query.state.data),
    queryFn: ({ signal }) => {
      if (namespace === null) return Promise.resolve([]);
      return client.listChildren(namespace, workflowId, runId, signal);
    },
  });
}
