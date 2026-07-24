import { expect, test } from 'bun:test';

import type { HistoryWindowRequest, RequestOptions } from '@/lib/api';
import type { WorkflowId } from '@/types';

import {
  FAILOVER_HISTORY_WINDOW_SIZE,
  FAILOVER_PAYLOAD_LIMIT_BYTES,
  fetchFailoverHistoryTail,
} from './historyTail';

test('fallback history polling requests only a bounded payload-elided tail', async () => {
  const calls: HistoryWindowRequest[] = [];
  const client = {
    getHistoryWindow: async (_workflowId: WorkflowId, window: HistoryWindowRequest) => {
      calls.push(window);
      return { events: [], headSeq: 1499, nextFromSeq: null };
    },
  };

  await fetchFailoverHistoryTail(
    client,
    'workflow' as WorkflowId,
    {
      namespace: 'default',
    } as RequestOptions
  );

  expect(calls).toEqual([
    { fromSeq: 0, limit: 1, payloadLimitBytes: FAILOVER_PAYLOAD_LIMIT_BYTES },
    {
      fromSeq: 1000,
      limit: FAILOVER_HISTORY_WINDOW_SIZE,
      payloadLimitBytes: FAILOVER_PAYLOAD_LIMIT_BYTES,
    },
  ]);
});
