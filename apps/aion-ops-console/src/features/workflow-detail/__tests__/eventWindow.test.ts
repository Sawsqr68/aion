import { expect, test } from 'bun:test';

import type { Event } from '@/types';

import { updateTimelineProjection } from '../lib/incrementalTimeline';
import { eventSequence, mergeEventWindowBySequence } from '../lib/timeline';

function event(sequence: number): Event {
  return {
    type: 'WorkflowReopened',
    data: {
      run_id: '00000000-0000-0000-0000-000000000002',
      reopened: [],
      envelope: {
        seq: sequence,
        recorded_at: '2026-07-01T00:00:00Z',
        workflow_id: '00000000-0000-0000-0000-000000000001',
      },
    },
  } as Event;
}

test('live event window never exceeds its cap and retains the newest sequences', () => {
  let window: Event[] = [];
  for (let sequence = 0; sequence < 2000; sequence += 1) {
    window = mergeEventWindowBySequence(window, [event(sequence)], 500);
  }

  expect(window).toHaveLength(500);
  expect(eventSequence(window[0] as Event)).toBe(1500);
  expect(eventSequence(window.at(-1) as Event)).toBe(1999);

  const unchanged = mergeEventWindowBySequence(window, [event(1999)], 500);
  expect(unchanged).toBe(window);
});

test('incremental projection preserves unchanged timeline node identity', () => {
  const firstEvent = event(1);
  const initial = updateTimelineProjection(null, [firstEvent]);
  const firstEntry = initial.timeline[0];
  const nextEvents = mergeEventWindowBySequence(initial.events, [event(2)], 500);

  const updated = updateTimelineProjection(initial, nextEvents);

  expect(updated.timeline).toHaveLength(2);
  expect(updated.timeline[0]).toBe(firstEntry);
});
