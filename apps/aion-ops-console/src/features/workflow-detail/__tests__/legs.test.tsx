import { expect, test } from 'bun:test';
import { renderToStaticMarkup } from 'react-dom/server';

import type { ChildLeg } from '@/lib/api';
import type { ActivityId, Namespace, RunId, WorkflowId } from '@/types';

import { type LiveLegReaders, LiveLegsSection } from '../components/LegsSection';

const namespace = '244-test' as Namespace;
const startedLeg: ChildLeg = {
  workflowId: '24410000-0000-0000-0000-000000000001' as WorkflowId,
  runId: '24420000-0000-0000-0000-000000000002' as RunId,
  workflowType: 'builder_leg',
  displayName: 'Builder L1',
  status: 'Running',
  currentActivityId: 7 as ActivityId,
  currentAttempt: 2,
};

const emptyLive = {
  events: [],
  isTerminal: false,
  lastSeenSequence: undefined,
  terminalOutcome: null,
  timeline: [],
};

test('shipped leg container renders an explicit history error instead of a false empty transcript', () => {
  const readers: LiveLegReaders = {
    useHistory: () => ({ data: [], error: new Error('history unavailable'), state: 'error' }),
    useLiveEvents: () => emptyLive,
  };

  const markup = renderToStaticMarkup(
    <LiveLegsSection legs={[startedLeg]} namespace={namespace} readers={readers} />
  );

  expect(markup).toContain('role="alert"');
  expect(markup).toContain('Could not load activity history');
  expect(markup).not.toContain('No activity transcript stream recorded.');
});

test('shipped leg container keeps loading distinct from a recorded empty transcript', () => {
  const readers: LiveLegReaders = {
    useHistory: () => ({ data: [], error: null, state: 'loading' }),
    useLiveEvents: () => emptyLive,
  };

  const markup = renderToStaticMarkup(
    <LiveLegsSection legs={[startedLeg]} namespace={namespace} readers={readers} />
  );

  expect(markup).toContain('Loading activity history');
  expect(markup).not.toContain('No activity transcript stream recorded.');
});
