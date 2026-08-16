import type { Namespace, RunId, WorkflowId } from '@/types';
import type { DescribeLiveResponse } from '@/types/generated';

/** How much transcript tail to request. Omitted = the server reads none. */
export type LiveStepTail = number | undefined;

/** The one call the live-step surface needs. L1 implements it; L2 consumes it. */
export type LiveStepReader = {
  describeLive(
    namespace: Namespace,
    workflowId: WorkflowId,
    runId: RunId | null,
    tail: LiveStepTail,
    signal?: AbortSignal
  ): Promise<DescribeLiveResponse>;
};
