import { expect, test } from 'bun:test';

import type { Event, Payload } from '@/types';

import { deriveAttemptNavigator } from '../lib/attemptNavigator';
import { projectTimeline } from '../lib/timeline';

const jsonPayload: Payload = { content_type: 'Json', bytes: [123, 125] };

function envelope(seq: number) {
  return {
    seq,
    recorded_at: `2026-07-04T00:00:${String(seq).padStart(2, '0')}Z`,
    workflow_id: 'wf-1',
  };
}

const RUN_A = '00000000-0000-0000-0000-0000000000a1';
const RUN_B = '00000000-0000-0000-0000-0000000000a2';

/**
 * The generation boundary. Every timeline below opens with one, because a real
 * history does: a workflow cannot have an attempt before it has started. The
 * windowed case (a tail with no start inside it) is exercised on its own.
 */
function workflowStarted(seq: number, runId: string): Event {
  return {
    type: 'WorkflowStarted',
    data: {
      envelope: envelope(seq),
      workflow_type: 'charge',
      input: jsonPayload,
      run_id: runId,
      parent_run_id: null,
      package_version: 'charge$abc',
    },
  };
}

function scheduled(seq: number, id: number, type = 'charge-card'): Event {
  return {
    type: 'ActivityScheduled',
    data: {
      envelope: envelope(seq),
      activity_id: id,
      activity_type: type,
      input: jsonPayload,
      task_queue: 'default',
      node: null,
    },
  };
}

function started(seq: number, id: number, attempt: number): Event {
  return { type: 'ActivityStarted', data: { envelope: envelope(seq), activity_id: id, attempt } };
}

function failed(seq: number, id: number, attempt: number): Event {
  return {
    type: 'ActivityFailed',
    data: {
      envelope: envelope(seq),
      activity_id: id,
      attempt,
      error: { kind: 'Retryable', message: `attempt ${attempt} failed`, details: null },
    },
  };
}

function completed(seq: number, id: number, attempt: number): Event {
  return {
    type: 'ActivityCompleted',
    data: { envelope: envelope(seq), activity_id: id, attempt, result: jsonPayload },
  };
}

function cancelled(seq: number, id: number, attempt: number): Event {
  return { type: 'ActivityCancelled', data: { envelope: envelope(seq), activity_id: id, attempt } };
}

test('a terminal failed run with N failed attempts and no success keeps every attempt selectable', () => {
  const timeline = projectTimeline([
    workflowStarted(0, RUN_A),
    scheduled(1, 7),
    started(2, 7, 1),
    failed(3, 7, 1),
    started(4, 7, 2),
    failed(5, 7, 2),
    started(6, 7, 3),
    failed(7, 7, 3),
    {
      type: 'WorkflowFailed',
      data: { envelope: envelope(8), error: { message: 'gave up', details: null } },
    },
  ]);

  const rows = deriveAttemptNavigator(timeline);

  expect(rows.map((row) => row.key)).toEqual([`${RUN_A}:7:1`, `${RUN_A}:7:2`, `${RUN_A}:7:3`]);
  expect(rows.every((row) => row.status === 'failed')).toBe(true);
  expect(rows.every((row) => row.isLegacy === false)).toBe(true);
});

test('a completed run after retries surfaces the failed attempts plus the successful one', () => {
  const timeline = projectTimeline([
    workflowStarted(0, RUN_A),
    scheduled(1, 7),
    started(2, 7, 1),
    failed(3, 7, 1),
    started(4, 7, 2),
    completed(5, 7, 2),
  ]);

  const rows = deriveAttemptNavigator(timeline);

  expect(rows).toHaveLength(2);
  expect(rows.map((row) => [row.key, row.status])).toEqual([
    [`${RUN_A}:7:1`, 'failed'],
    [`${RUN_A}:7:2`, 'completed'],
  ]);
});

test('a running run shows the in-flight attempt as started (no terminal yet)', () => {
  const timeline = projectTimeline([workflowStarted(0, RUN_A), scheduled(1, 7), started(2, 7, 1)]);

  const rows = deriveAttemptNavigator(timeline);

  expect(rows).toEqual([
    {
      key: `${RUN_A}:7:1`,
      runId: RUN_A,
      activityId: '7',
      activityType: 'charge-card',
      attempt: 1,
      isLegacy: false,
      status: 'started',
    },
  ]);
});

test('two generations of one chain are two rows under the SAME coordinate', () => {
  // The defect the run axis exists to catch: a continue-as-new chain reuses the
  // workflow id and restarts ordinals AND attempts, so both generations emit
  // activity 0 attempt 1. Without the run these fuse into one row and one
  // transcript target; with it they are two rows with two keys.
  const timeline = projectTimeline([
    workflowStarted(0, RUN_A),
    scheduled(1, 0),
    started(2, 0, 1),
    completed(3, 0, 1),
    workflowStarted(4, RUN_B),
    scheduled(5, 0),
    started(6, 0, 1),
    completed(7, 0, 1),
  ]);

  const rows = deriveAttemptNavigator(timeline);

  expect(rows).toHaveLength(2);
  expect(new Set(rows.map((row) => row.key)).size).toBe(2);
  expect(rows.map((row) => row.runId).sort()).toEqual([RUN_A, RUN_B]);
  // Every row still names the SAME coordinate — the run is the only difference.
  expect(rows.every((row) => row.activityId === '0' && row.attempt === 1)).toBe(true);
});

test('a tail window with no WorkflowStarted names no generation, and never guesses one', () => {
  // The loaded history is a window; a long run's tail can begin mid-generation.
  // The honest answer is `null` — NOT the nearest run, which would label one
  // generation's attempt with another's identity.
  const timeline = projectTimeline([scheduled(1, 7), started(2, 7, 1), completed(3, 7, 1)]);

  const rows = deriveAttemptNavigator(timeline);

  expect(rows).toHaveLength(1);
  expect(rows[0]?.runId).toBeNull();
  expect(rows[0]?.key).toBe('-:7:1');
});

test('the legacy attempt:0 sentinel is labelled distinctly and still selectable', () => {
  const timeline = projectTimeline([
    workflowStarted(0, RUN_A),
    scheduled(1, 7),
    started(2, 7, 0),
    failed(3, 7, 0),
  ]);

  const rows = deriveAttemptNavigator(timeline);

  expect(rows).toHaveLength(1);
  expect(rows[0]?.key).toBe(`${RUN_A}:7:0`);
  expect(rows[0]?.isLegacy).toBe(true);
  expect(rows[0]?.status).toBe('failed');
});

test('a cancelled attempt reads as cancelled and rows sort by activity then attempt', () => {
  const timeline = projectTimeline([
    workflowStarted(0, RUN_A),
    scheduled(1, 9),
    started(2, 9, 1),
    cancelled(3, 9, 1),
    scheduled(4, 2),
    started(5, 2, 1),
    completed(6, 2, 1),
  ]);

  const rows = deriveAttemptNavigator(timeline);

  // Numeric activity ordering: activity 2 before activity 9 despite event order.
  expect(rows.map((row) => row.key)).toEqual([`${RUN_A}:2:1`, `${RUN_A}:9:1`]);
  expect(rows.find((row) => row.key === `${RUN_A}:9:1`)?.status).toBe('cancelled');
});

test('a scheduled-only activity with no started attempt yields no navigator rows', () => {
  const rows = deriveAttemptNavigator(
    projectTimeline([workflowStarted(0, RUN_A), scheduled(1, 7)])
  );
  expect(rows).toEqual([]);
});
