import { expect, test } from 'bun:test';

import type { ChildLeg } from '@/lib/api';
import type { WorkflowId } from '@/types';

import { pendingChildRefetchInterval } from './useChildren';

const pendingChild = {
  workflowId: '24410000-0000-0000-0000-000000000001' as WorkflowId,
  runId: null,
  workflowType: 'builder_leg',
  displayName: null,
  status: null,
  currentActivityId: null,
  currentAttempt: null,
} satisfies ChildLeg;

test('pending child discovery keeps polling until the child run is visible', () => {
  expect(pendingChildRefetchInterval([pendingChild])).toBe(1_000);
  expect(pendingChildRefetchInterval([{ ...pendingChild, runId: pendingChild.workflowId }])).toBe(
    false
  );
});
