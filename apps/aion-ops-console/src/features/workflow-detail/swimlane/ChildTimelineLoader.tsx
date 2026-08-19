import { useEffect, useMemo } from 'react';

import type { WorkflowId } from '@/types';

import { useLiveWorkflowEvents } from '../hooks/useLiveWorkflowEvents';
import { useWorkflowHead } from '../hooks/useWorkflowHead';
import { useWorkflowHistory } from '../hooks/useWorkflowHistory';
import type { ChildTimelineState } from './laneTree';

export type ChildTimelineLoaderProps = {
  path: string;
  workflowId: WorkflowId;
  onState: (path: string, state: ChildTimelineState) => void;
};

/**
 * Mounted only for an expanded child path. The durable history backfill completes
 * before the live tail attaches; unmounting on collapse closes that subscription.
 */
export function ChildTimelineLoader({ path, workflowId, onState }: ChildTimelineLoaderProps) {
  const historyQuery = useWorkflowHistory({ workflowId });
  // The CHILD's own generation boundaries, read for the child's workflow id.
  // The parent's would name the parent's runs; see {@link ChildTimelineState}.
  const headQuery = useWorkflowHead({ workflowId });
  // Memoised on the query's own data identity: a fresh `[]` every render would
  // make the reporting effect below re-run (and re-set parent state) forever.
  const generations = useMemo(() => headQuery.data?.generations ?? [], [headQuery.data]);
  const live = useLiveWorkflowEvents({
    enabled: historyQuery.isSuccess,
    history: historyQuery.data ?? [],
    workflowId,
  });

  useEffect(() => {
    if (historyQuery.isError) {
      onState(path, {
        status: 'error',
        entries: [],
        generations: [],
        isRunning: false,
        message: errorMessage(historyQuery.error),
      });
      return;
    }
    if (!historyQuery.isSuccess) {
      onState(path, { status: 'loading', entries: [], generations: [], isRunning: true });
      return;
    }
    onState(path, {
      status: 'ready',
      entries: live.timeline,
      generations,
      isRunning: !live.isTerminal,
    });
  }, [
    generations,
    historyQuery.error,
    historyQuery.isError,
    historyQuery.isSuccess,
    live.isTerminal,
    live.timeline,
    onState,
    path,
  ]);

  return null;
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : 'Could not load child workflow history.';
}
