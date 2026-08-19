import { useQuery } from '@tanstack/react-query';
import { useMemo } from 'react';

import { useNamespace } from '@/features/namespace';
import type { ApiClient, WorkflowHead } from '@/lib/api';
import { createConfiguredApiClient } from '@/lib/config';
import type { Namespace, WorkflowId } from '@/types';

import { workflowHistoryRequestOptions } from './useWorkflowHistory';

/**
 * The detail view's read of ONE workflow's HEAD — the whole-history facts that a
 * loaded history WINDOW cannot establish.
 *
 * Both members are here for the same reason. `summary` carries the run's
 * server-projected status and its current `display_name` — a fold over the
 * window can only see the events in hand, so a run named at start with more
 * events than the window holds would otherwise read as unnamed. `generations`
 * carries the sequence at which each `WorkflowStarted` opened a run, which is
 * what lets a reader holding any window name the run an event belongs to; the
 * console used to scan the window for those boundaries and reported "run
 * unknown" on every row of any workflow longer than one window.
 *
 * History is not requested here; the windowed history endpoint already loads it.
 */

export type WorkflowHeadClient = Pick<ApiClient, 'getWorkflowHead'>;

export type WorkflowHeadOptions = {
  workflowId: WorkflowId;
  /** Injected client (tests); defaults to the namespace-scoped client. */
  apiClient?: WorkflowHeadClient | undefined;
  enabled?: boolean;
};

export function workflowHeadQueryKey(namespace: Namespace | null, workflowId: WorkflowId) {
  return ['workflow-head', namespace, workflowId] as const;
}

export function useWorkflowHead({ workflowId, apiClient, enabled = true }: WorkflowHeadOptions) {
  const { selectedNamespace } = useNamespace();
  const client = useMemo<WorkflowHeadClient>(
    () => apiClient ?? createConfiguredApiClient({ namespace: selectedNamespace }),
    [apiClient, selectedNamespace]
  );

  return useQuery<WorkflowHead>({
    enabled: enabled && selectedNamespace !== null && selectedNamespace.trim().length > 0,
    queryKey: workflowHeadQueryKey(selectedNamespace, workflowId),
    queryFn: ({ signal }) =>
      client.getWorkflowHead(workflowId, workflowHistoryRequestOptions(selectedNamespace, signal)),
  });
}
