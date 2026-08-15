import { expect, test } from 'bun:test';
import { renderToStaticMarkup } from 'react-dom/server';

import type { ActivityId, RunId, WorkflowId } from '@/types';

import { LegsSection, type LegView } from '../components/LegsSection';

const fixture: LegView[] = [
  {
    workflowId: '24410000-0000-0000-0000-000000000001' as WorkflowId,
    runId: '24420000-0000-0000-0000-000000000002' as RunId,
    workflowType: 'builder_leg',
    displayName: 'Builder L1',
    status: 'Running',
    currentActivityId: 7 as ActivityId,
    currentAttempt: 2,
    attempts: [
      {
        activityId: 7 as ActivityId,
        activityType: 'build',
        attempt: 1,
        state: 'superseded',
        supersededBy: 2,
      },
      {
        activityId: 7 as ActivityId,
        activityType: 'build',
        attempt: 2,
        state: 'current',
        supersededBy: null,
      },
    ],
  },
];

test('run page legs fixture shows name, short id, status, current attempt, and supersession', () => {
  const markup = renderToStaticMarkup(<LegsSection legs={fixture} />);

  expect(markup).toContain('>Legs<');
  expect(markup).toContain('Builder L1');
  expect(markup).toContain('24410000');
  expect(markup).toContain('Running');
  expect(markup).toContain('Activity 7 · attempt 2');
  expect(markup).toContain('attempt 1');
  expect(markup).toContain('Superseded by attempt 2');
});
