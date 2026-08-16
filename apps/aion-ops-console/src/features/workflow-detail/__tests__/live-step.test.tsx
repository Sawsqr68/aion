import { expect, jest, test } from 'bun:test';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { renderToStaticMarkup } from 'react-dom/server';

import type { ActivityId, Namespace, RunId, WorkflowId } from '@/types';
import type { DescribeLiveResponse, HeartbeatNote } from '@/types/generated';

import { LiveStepContent } from '../components/LiveStepCard';
import {
  DEFAULT_LIVE_STEP_TAIL,
  LIVE_STEP_POLL_INTERVAL_MS,
  liveStepQueryOptions,
} from '../hooks/useLiveStep';
import type { LiveStepReader } from '../lib/live-step-port';

const namespace = 'live-step-test' as Namespace;
const workflowId = '24410000-0000-0000-0000-000000000001' as WorkflowId;
const runId = '24420000-0000-0000-0000-000000000002' as RunId;

const step = {
  activity_id: 7 as ActivityId,
  activity_type: 'builder_leg',
  task_queue: 'builders',
  node: 'node-a',
  scheduled_at: '2026-08-16T00:00:00Z',
  state: {
    state: 'Dispatched',
    attempt: 3,
    dispatched_at: '2026-08-16T00:00:01Z',
  },
} as const;

function response(overrides: Partial<DescribeLiveResponse> = {}): DescribeLiveResponse {
  return {
    summary: null,
    current_step: {
      step,
      attempt: {
        attempt: 3,
        dispatched_at: '2026-08-16T00:00:01Z',
        liveness: { liveness: 'NoLiveOwner' },
        note: { note: 'NoneSent' },
        transcript: { transcript: 'NotRequested' },
      },
    },
    open_steps: [step],
    transcript_streams: [],
    unserved: [],
    ...overrides,
  };
}

function renderLive(value: DescribeLiveResponse): string {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return renderToStaticMarkup(
    <QueryClientProvider client={queryClient}>
      <LiveStepContent
        namespace={namespace}
        response={value}
        runId={runId}
        workflowId={workflowId}
      />
    </QueryClientProvider>
  );
}

function withAttempt(
  value: DescribeLiveResponse,
  changes: Partial<NonNullable<NonNullable<DescribeLiveResponse['current_step']>['attempt']>>
): DescribeLiveResponse {
  const current = value.current_step;
  if (current?.attempt === null || current === null) throw new Error('fixture requires an attempt');
  return { ...value, current_step: { ...current, attempt: { ...current.attempt, ...changes } } };
}

test('dispatched current step names its address, state, node, and attempt', () => {
  const markup = renderLive(response());
  for (const text of ['7', 'builder_leg', 'builders', 'node-a', 'Dispatched', '3']) {
    expect(markup).toContain(text);
  }
});

test('a null current step honestly says the run is not inside an activity', () => {
  const markup = renderLive(response({ current_step: null, open_steps: [] }));
  expect(markup).toContain('not inside an activity');
  expect(markup).not.toContain('role="alert"');
});

test('NoLiveOwner is named and differs from a connected observability-only owner', () => {
  const noOwner = renderLive(response());
  const liveEmpty = renderLive(
    withAttempt(response(), { liveness: { liveness: 'Live', capabilities: { supported: [] } } })
  );
  expect(noOwner).toContain('No live owner');
  expect(noOwner).not.toContain('Live owner connected');
  expect(liveEmpty).toContain('Live owner connected — observability-only');
  expect(liveEmpty).not.toContain('No live owner');
});

test('runtime capabilities offer inject, withhold cancel, and explain withheld controls', () => {
  const markup = renderLive(
    withAttempt(response(), {
      liveness: {
        liveness: 'Live',
        capabilities: { supported: [{ primitive: 'InjectMessage' }] },
      },
    })
  );
  expect(markup).toContain('Steer the agent');
  expect(markup).not.toContain('Cancel run');
  expect(markup).toContain('withheld because this harness does not advertise');
});

test('an observability-only live owner offers no control and states why', () => {
  const markup = renderLive(
    withAttempt(response(), { liveness: { liveness: 'Live', capabilities: { supported: [] } } })
  );
  expect(markup).toContain('harness advertises no intervention controls');
  expect(markup).not.toContain('Steer the agent');
  expect(markup).not.toContain('Cancel run');
});

test('all heartbeat note arms remain distinct and restart explains unavailable notes', () => {
  const fixtures: Array<[HeartbeatNote, string]> = [
    [
      {
        note: 'Reported',
        payload: { content_type: 'Json', bytes: [123, 125] },
        reported_at: '2026-08-16T00:00:02Z',
      },
      'Worker heartbeat reported',
    ],
    [{ note: 'NoneSent' }, 'worker has reported no heartbeat note'],
    [
      {
        note: 'Unavailable',
        notes_held_since: '2026-08-16T00:00:03Z',
        restarted_since_attempt_began: true,
      },
      'server restarted after the attempt began',
    ],
  ];
  for (const [note, text] of fixtures) {
    expect(renderLive(withAttempt(response(), { note }))).toContain(text);
  }
});

test('transcript absence and retention truncation are reported honestly', () => {
  expect(renderLive(response())).toContain('Transcript tail was not requested');
  expect(renderLive(response())).not.toContain('stream is empty.');
  const truncated = renderLive(
    withAttempt(response(), {
      transcript: {
        transcript: 'Window',
        events: [],
        head_seq: 9,
        omitted_before: 9,
        retention_truncated: true,
      },
    })
  );
  expect(truncated).toContain('Retention closed this stream');
  expect(truncated).toContain('does not mean the agent went quiet');
});

test('fan-out sibling count and non-empty unserved verdict are visible', () => {
  const secondStep = { ...step, activity_id: 8 as ActivityId };
  const markup = renderLive(
    response({
      open_steps: [step, secondStep],
      unserved: [
        {
          activity_id: 7 as ActivityId,
          activity_type: 'builder_leg',
          task_queue: 'builders',
          node: 'node-a',
          attempt: 3,
          dispatched_at: '2026-08-16T00:00:01Z',
          reason: 'NO_LIVE_POLLERS',
          detail: 'Start a compatible worker.',
          workers_in_pool: 0,
          workers_serving_activity: 0,
          compatible_workers: 0,
          dispatch_parked: false,
        },
      ],
    })
  );
  expect(markup).toContain('2 activities are open');
  expect(markup).toContain('Unserved activity 7');
  expect(markup).toContain('NO_LIVE_POLLERS');
  expect(markup).toContain('cannot currently be served');
});

test('terminal query options stop the declared 2s cadence before another read', async () => {
  jest.useFakeTimers();
  try {
    let calls = 0;
    const reader: LiveStepReader = {
      describeLive: async () => {
        calls += 1;
        return response();
      },
    };
    const running = liveStepQueryOptions({
      namespace,
      workflowId,
      runId,
      isTerminal: false,
      tail: DEFAULT_LIVE_STEP_TAIL,
      reader,
    });
    await running.queryFn({ signal: new AbortController().signal });
    expect(running.refetchInterval).toBe(LIVE_STEP_POLL_INTERVAL_MS);

    const terminal = liveStepQueryOptions({
      namespace,
      workflowId,
      runId,
      isTerminal: true,
      tail: DEFAULT_LIVE_STEP_TAIL,
      reader,
    });
    expect(terminal.enabled).toBe(false);
    expect(terminal.refetchInterval).toBe(false);
    jest.advanceTimersByTime(LIVE_STEP_POLL_INTERVAL_MS * 3);
    expect(calls).toBe(1);
  } finally {
    jest.useRealTimers();
  }
});

test('a rejected read produces the operator-visible error wording', async () => {
  const reader: LiveStepReader = {
    describeLive: async () => {
      throw new Error('registry unavailable');
    },
  };
  const options = liveStepQueryOptions({
    namespace,
    workflowId,
    runId,
    isTerminal: false,
    tail: DEFAULT_LIVE_STEP_TAIL,
    reader,
  });
  await expect(options.queryFn({ signal: new AbortController().signal })).rejects.toThrow(
    'registry unavailable'
  );
  expect('Could not read live step: registry unavailable').toContain('Could not read live step');
});
