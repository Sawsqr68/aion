import { useQuery } from '@tanstack/react-query';

import type { Namespace, RunId, WorkflowId } from '@/types';

import type { LiveStepReader, LiveStepTail } from '../lib/live-step-port';

export const LIVE_STEP_POLL_INTERVAL_MS = 2_000;
export const DEFAULT_LIVE_STEP_TAIL = 50;

export type UseLiveStepOptions = {
  namespace: Namespace | null;
  workflowId: WorkflowId;
  runId: RunId | null;
  isTerminal: boolean;
  tail: LiveStepTail;
  reader: LiveStepReader;
};

/**
 * The history socket remains the terminal-state authority, while this bounded poll
 * reads volatile ownership, heartbeat, and transcript-registry state absent from it.
 */
export function liveStepQueryOptions({
  namespace,
  workflowId,
  runId,
  isTerminal,
  tail,
  reader,
}: UseLiveStepOptions) {
  const enabled = namespace !== null && !isTerminal;
  const refetchInterval: number | false = enabled ? LIVE_STEP_POLL_INTERVAL_MS : false;
  return {
    enabled,
    queryKey: ['workflow-live-step', namespace, workflowId, runId, tail] as const,
    refetchInterval,
    queryFn: ({ signal }: { signal: AbortSignal }) => {
      if (namespace === null) {
        throw new Error('Live-step read attempted without a namespace');
      }
      return reader.describeLive(namespace, workflowId, runId, tail, signal);
    },
  };
}

export function useLiveStep(options: UseLiveStepOptions) {
  return useQuery(liveStepQueryOptions(options));
}
