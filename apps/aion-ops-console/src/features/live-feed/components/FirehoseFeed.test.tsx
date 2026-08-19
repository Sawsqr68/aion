import { expect, test } from 'bun:test';
import { renderToStaticMarkup } from 'react-dom/server';

import type { Event, Namespace } from '@/types';
import { appendFirehoseEvents, FirehoseFeedContent } from './FirehoseFeed';

const namespace = 'default' as Namespace;

test('FirehoseFeedContent renders explicit empty and disconnected states', () => {
  const markup = renderToStaticMarkup(
    <FirehoseFeedContent events={[]} namespace={namespace} status="disconnected" />
  );

  expect(markup).toContain('No live events yet');
  expect(markup).toContain('Live socket is disconnected');
});

test('FirehoseFeedContent warns when a reconnected live tail may have a gap', () => {
  const markup = renderToStaticMarkup(
    <FirehoseFeedContent events={[]} namespace={namespace} status="resynced-with-possible-gap" />
  );

  expect(markup).toContain('events may have been missed after reconnect');
  expect(markup).toContain('Live with possible gap');
});

test('FirehoseFeedContent surfaces a socket error message as visible state', () => {
  const markup = renderToStaticMarkup(
    <FirehoseFeedContent
      error={{
        kind: 'reconnect-exhausted',
        subscriptionId: 'aion-events-1',
        message: 'Live connection lost after 5 reconnect attempts. Reload to retry.',
        cause: null,
      }}
      events={[]}
      namespace={namespace}
      status="disconnected"
    />
  );

  expect(markup).toContain('Live connection lost after 5 reconnect attempts');
  expect(markup).toContain('data-socket-error="reconnect-exhausted"');
  expect(markup).toContain('role="alert"');
});

test('FirehoseFeedContent renders streamed events in newest-first order', () => {
  const markup = renderToStaticMarkup(
    <FirehoseFeedContent
      events={[workflowStarted(1), workflowCompleted(2)]}
      namespace={namespace}
      status="connected"
    />
  );

  expect(markup).toContain('Live firehose');
  expect(markup.indexOf('seq 2')).toBeLessThan(markup.indexOf('seq 1'));
  expect(markup).toContain('WorkflowCompleted');
  expect(markup).toContain('workflow workflow-1');
});

test('appendFirehoseEvents inserts a frame by sequence, deduplicates, and caps the window', () => {
  const current = [workflowStarted(2), workflowStarted(4)];
  const next = appendFirehoseEvents(
    current,
    [workflowStarted(3), workflowStarted(4), workflowStarted(5)],
    3
  );

  expect(next.map((event) => event.data.envelope.seq)).toEqual([3, 4, 5]);
  expect(next[1]).toBe(current[1]);
});

test('appendFirehoseEvents preserves window identity when a delivered frame is all duplicates', () => {
  const current = [workflowStarted(2), workflowStarted(4)];

  expect(appendFirehoseEvents(current, [workflowCompleted(2), workflowCompleted(4)], 100)).toBe(
    current
  );
});

function workflowStarted(seq: number): Event {
  return {
    type: 'WorkflowStarted',
    data: {
      envelope: { seq, recorded_at: '2026-06-05T20:00:00Z', workflow_id: 'workflow-1' },
      input: { content_type: 'Json', bytes: [123, 125] },
      workflow_type: 'checkout',
      run_id: '00000000-0000-0000-0000-0000000000a1',
      parent_run_id: null,
      parent_workflow_id: null,
      package_version: '1.0.0',
    },
  };
}

function workflowCompleted(seq: number): Event {
  return {
    type: 'WorkflowCompleted',
    data: {
      envelope: { seq, recorded_at: '2026-06-05T20:01:00Z', workflow_id: 'workflow-1' },
      result: { content_type: 'Json', bytes: [123, 125] },
    },
  };
}
