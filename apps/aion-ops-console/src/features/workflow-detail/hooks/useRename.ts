import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useMemo } from 'react';

import { requireSelectedNamespace } from '@/features/namespace';
import type { ApiClient, RenameWorkflowResult, RequestOptions } from '@/lib/api';
import { createConfiguredApiClient } from '@/lib/config';
import { isBlankDisplayName } from '@/lib/workflowIdentity';
import type { Namespace, RunId, WorkflowId } from '@/types';

/**
 * Rename submit path (#211): record a run's operator-facing display name via
 * `POST /workflows/rename`.
 *
 * The name is a LABEL over the run's UUID identity. The target is always the
 * id — this hook takes a `workflowId` (and optionally the concrete `runId`) and
 * NEVER a name, so no console path addresses a workflow by name.
 */

export type RenameClient = Pick<ApiClient, 'rename'>;

export type RenameWorkflowVariables = {
  workflowId: WorkflowId;
  /** The new label. Trimmed before it is sent; blank is refused. */
  displayName: string;
  /** Absent targets the workflow's latest run (server-resolved). */
  runId?: RunId | undefined;
};

/**
 * Query-key prefixes a recorded rename makes stale: the workflow list (whose
 * summaries carry `display_name`), the detail history (whose recorded
 * `SearchAttributesUpdated` is the name's source of truth), and the detail
 * summary (the server's own projection of the current name). Invalidating the
 * latter two matters for a run whose live socket is closed — a terminal run's
 * header would otherwise keep showing the superseded name until a reload.
 */
export const RENAME_INVALIDATED_QUERY_PREFIXES: readonly (readonly string[])[] = [
  ['workflows'],
  ['workflow-history'],
  ['workflow-summary'],
];

export type UseRenameOptions = {
  /** The namespace scope for the command (the auth scope). */
  namespace: Namespace | null;
  /** Override the REST client (tests); defaults to the namespace-scoped client. */
  apiClient?: RenameClient | undefined;
};

/**
 * Validate and submit one rename.
 *
 * A blank/whitespace-only name is REFUSED here rather than sent: it has no
 * meaning (this endpoint cannot un-name a run) and the engine rejects it
 * anyway, so the refusal is raised as a visible error instead of spending a
 * round trip on a request that cannot succeed. The trimmed name is what
 * travels, matching what the server records and echoes back.
 *
 * Exported so the submit path is exercised directly by tests; {@link useRename}
 * is the thin React binding over it.
 */
export async function submitRename(
  client: RenameClient,
  variables: RenameWorkflowVariables,
  options: RequestOptions
): Promise<RenameWorkflowResult> {
  // The blank predicate is shared with the start form and the REST client
  // (`@/lib/workflowIdentity`), so the three surfaces cannot drift into three
  // slightly different rules; only the sentence below differs, because this is
  // the one that can tell the operator the run keeps the name it has.
  if (isBlankDisplayName(variables.displayName)) {
    throw new Error('A display name cannot be blank — the run keeps the name it has.');
  }
  const displayName = variables.displayName.trim();

  return client.rename(
    {
      workflowId: variables.workflowId,
      displayName,
      ...(variables.runId === undefined ? {} : { runId: variables.runId }),
    },
    options
  );
}

/**
 * Mutation that records a run's display name.
 *
 * Resolves with the server's confirmed {@link RenameWorkflowResult} — the run
 * the name was recorded against and the name as recorded — so the view reports
 * success only on a confirmed append, never optimistically. A refusal (blank
 * name, absent workflow, denied namespace grant) surfaces as the mutation
 * `error`; nothing is swallowed. On success the workflow-list and
 * workflow-history queries are invalidated so every surface re-reads the name
 * from the server's projection rather than from a client-side guess.
 */
export function useRename({ namespace, apiClient }: UseRenameOptions) {
  const queryClient = useQueryClient();
  const client = useMemo<RenameClient>(
    () => apiClient ?? createConfiguredApiClient({ namespace }),
    [apiClient, namespace]
  );

  return useMutation<RenameWorkflowResult, Error, RenameWorkflowVariables>({
    mutationFn: (variables) =>
      submitRename(client, variables, {
        namespace: requireSelectedNamespace(namespace, 'renaming a workflow'),
      }),
    onSuccess: () => {
      for (const queryKey of RENAME_INVALIDATED_QUERY_PREFIXES) {
        void queryClient.invalidateQueries({ queryKey });
      }
    },
  });
}
