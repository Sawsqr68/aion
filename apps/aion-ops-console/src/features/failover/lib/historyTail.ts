import type { ApiClient, RequestOptions } from '@/lib/api';
import type { Event, WorkflowId } from '@/types';

export const FAILOVER_HISTORY_WINDOW_SIZE = 500;
export const FAILOVER_PAYLOAD_LIMIT_BYTES = 1024;

export type FailoverHistoryClient = Pick<ApiClient, 'getHistoryWindow'>;

export async function fetchFailoverHistoryTail(
  client: FailoverHistoryClient,
  workflowId: WorkflowId,
  options: RequestOptions
): Promise<Event[]> {
  const discovery = await client.getHistoryWindow(
    workflowId,
    { fromSeq: 0, limit: 1, payloadLimitBytes: FAILOVER_PAYLOAD_LIMIT_BYTES },
    options
  );
  const fromSeq = Math.max(0, discovery.headSeq + 1 - FAILOVER_HISTORY_WINDOW_SIZE);
  const tail = await client.getHistoryWindow(
    workflowId,
    {
      fromSeq,
      limit: FAILOVER_HISTORY_WINDOW_SIZE,
      payloadLimitBytes: FAILOVER_PAYLOAD_LIMIT_BYTES,
    },
    options
  );
  return tail.events;
}
