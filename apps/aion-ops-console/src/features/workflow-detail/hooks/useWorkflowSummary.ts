import { useQuery } from '@tanstack/react-query';
import { useMemo } from 'react';

import { useNamespace } from '@/features/namespace';
import type { ApiClient } from '@/lib/api';
import { createConfiguredApiClient } from '@/lib/config';
import type { Namespace, WorkflowId, WorkflowSummary } from '@/types';

import { workflowHistoryRequestOptions } from './useWorkflowHistory';

/**
 * The detail view's read of ONE workflow's server-projected summary (#211).
 *
 * The summary is projected from the run's full recorded history server-side, so
 * it is the authority for the run's current `display_name` — a fold over the
 * loaded history WINDOW can only see the events in hand, and a run named at
 * start with more events than the window holds would otherwise read as unnamed.
 * History is not requested here; the windowed history endpoint already loads it.
 */

export type WorkflowSummaryClient = Pick<ApiClient, 'getWorkflowSummary'>;

export type WorkflowSummaryOptions = {
  workflowId: WorkflowId;
  /** Injected client (tests); defaults to the namespace-scoped client. */
  apiClient?: WorkflowSummaryClient | undefined;
  enabled?: boolean;
};

export function workflowSummaryQueryKey(namespace: Namespace | null, workflowId: WorkflowId) {
  return ['workflow-summary', namespace, workflowId] as const;
}

export function useWorkflowSummary({
  workflowId,
  apiClient,
  enabled = true,
}: WorkflowSummaryOptions) {
  const { selectedNamespace } = useNamespace();
  const client = useMemo<WorkflowSummaryClient>(
    () => apiClient ?? createConfiguredApiClient({ namespace: selectedNamespace }),
    [apiClient, selectedNamespace]
  );

  return useQuery<WorkflowSummary | null>({
    enabled: enabled && selectedNamespace !== null && selectedNamespace.trim().length > 0,
    queryKey: workflowSummaryQueryKey(selectedNamespace, workflowId),
    queryFn: ({ signal }) =>
      client.getWorkflowSummary(
        workflowId,
        workflowHistoryRequestOptions(selectedNamespace, signal)
      ),
  });
}
