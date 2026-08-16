import { expect, test } from 'bun:test';

import type { Namespace, RunId, WorkflowId } from '@/types';

import { ApiError } from './api-error';
import { DESCRIBE_LIVE_ENDPOINT, DescribeLiveReadClient } from './describe-live-read';

const namespace = 'default' as Namespace;
const workflowId = '00000000-0000-0000-0000-000000000001' as WorkflowId;
const runId = '00000000-0000-0000-0000-000000000002' as RunId;

function response(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    summary: {
      workflow_id: workflowId,
      workflow_type: 'builder_leg',
      status: 'Running',
      started_at: '2026-08-16T12:00:00Z',
      ended_at: null,
      parent: null,
      failed_step: null,
      failure_reason: null,
      display_name: 'Live builder',
    },
    current_step: {
      step: openStep(),
      attempt: liveAttempt(),
    },
    open_steps: [openStep()],
    transcript_streams: [
      {
        activity_id: 7,
        attempt: 2,
        activity_type: 'build',
        head_seq: 4,
        current: true,
        retention_truncated: false,
      },
    ],
    unserved: [],
    ...overrides,
  };
}

function openStep(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    activity_id: 7,
    activity_type: 'build',
    task_queue: 'builders',
    node: null,
    scheduled_at: '2026-08-16T12:01:00Z',
    state: {
      state: 'Dispatched',
      attempt: 2,
      dispatched_at: '2026-08-16T12:02:00Z',
    },
    ...overrides,
  };
}

function liveAttempt(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    attempt: 2,
    dispatched_at: '2026-08-16T12:02:00Z',
    liveness: {
      liveness: 'Live',
      capabilities: {
        supported: [{ primitive: 'InjectMessage' }, { primitive: 'Cancel' }],
      },
    },
    note: {
      note: 'Reported',
      payload: { content_type: 'Json', bytes: [123, 125] },
      reported_at: '2026-08-16T12:03:00Z',
    },
    transcript: {
      transcript: 'Window',
      events: [],
      head_seq: 4,
      omitted_before: 1,
      retention_truncated: false,
    },
    ...overrides,
  };
}

function clientReturning(body: unknown, status = 200): DescribeLiveReadClient {
  return new DescribeLiveReadClient({
    fetchImpl: async () =>
      new Response(JSON.stringify(body), {
        status,
        headers: { 'content-type': 'application/json' },
      }),
  });
}

test('describe-live omits tail from the strict wire body when the caller omits it', async () => {
  let captured: { url: string; init: RequestInit | undefined } | undefined;
  const client = new DescribeLiveReadClient({
    baseUrl: 'http://console.test/',
    fetchImpl: async (input, init) => {
      captured = { url: String(input), init };
      return Response.json(response());
    },
  });

  await client.describeLive(namespace, workflowId, runId, undefined);

  expect(captured?.url).toBe(`http://console.test${DESCRIBE_LIVE_ENDPOINT}`);
  expect(JSON.parse(String(captured?.init?.body))).toEqual({
    namespace,
    workflow_id: workflowId,
    run_id: runId,
  });
  expect(JSON.parse(String(captured?.init?.body))).not.toHaveProperty('tail');
});

test('describe-live sends the caller-selected tail and an explicit null run_id', async () => {
  const bodies: unknown[] = [];
  const client = new DescribeLiveReadClient({
    fetchImpl: async (_input, init) => {
      bodies.push(JSON.parse(String(init?.body)));
      return Response.json(response());
    },
  });

  await client.describeLive(namespace, workflowId, runId, 80);
  await client.describeLive(namespace, workflowId, null, 0);

  expect(bodies[0]).toEqual({ namespace, workflow_id: workflowId, run_id: runId, tail: 80 });
  expect(bodies[1]).toEqual({ namespace, workflow_id: workflowId, run_id: null, tail: 0 });
});

test('describe-live parses the full live attempt vocabulary', async () => {
  const result = await clientReturning(response()).describeLive(namespace, workflowId, runId, 20);

  expect(result.current_step?.step.state).toEqual({
    state: 'Dispatched',
    attempt: 2,
    dispatched_at: '2026-08-16T12:02:00Z',
  });
  expect(result.current_step?.attempt?.liveness).toEqual({
    liveness: 'Live',
    capabilities: {
      supported: [{ primitive: 'InjectMessage' }, { primitive: 'Cancel' }],
    },
  });
  expect(result.current_step?.attempt?.note.note).toBe('Reported');
  expect(result.current_step?.attempt?.transcript).toEqual({
    transcript: 'Window',
    events: [],
    head_seq: 4,
    omitted_before: 1,
    retention_truncated: false,
  });
});

test('describe-live accepts NoLiveOwner as an honest liveness answer', async () => {
  const result = await clientReturning(
    response({
      current_step: {
        step: openStep(),
        attempt: liveAttempt({ liveness: { liveness: 'NoLiveOwner' } }),
      },
    })
  ).describeLive(namespace, workflowId, runId, undefined);

  expect(result.current_step?.attempt?.liveness).toEqual({ liveness: 'NoLiveOwner' });
});

test('describe-live accepts NotRequested as an honest transcript answer', async () => {
  const result = await clientReturning(
    response({
      current_step: {
        step: openStep(),
        attempt: liveAttempt({ transcript: { transcript: 'NotRequested' } }),
      },
    })
  ).describeLive(namespace, workflowId, runId, undefined);

  expect(result.current_step?.attempt?.transcript).toEqual({ transcript: 'NotRequested' });
});

test('describe-live preserves an unavailable note caused by server restart', async () => {
  const result = await clientReturning(
    response({
      current_step: {
        step: openStep(),
        attempt: liveAttempt({
          note: {
            note: 'Unavailable',
            notes_held_since: '2026-08-16T12:05:00Z',
            restarted_since_attempt_began: true,
          },
        }),
      },
    })
  ).describeLive(namespace, workflowId, runId, undefined);

  expect(result.current_step?.attempt?.note).toEqual({
    note: 'Unavailable',
    notes_held_since: '2026-08-16T12:05:00Z',
    restarted_since_attempt_began: true,
  });
});

test('describe-live accepts a null current_step when the run is outside an activity', async () => {
  const result = await clientReturning(
    response({ current_step: null, open_steps: [] })
  ).describeLive(namespace, workflowId, runId, undefined);

  expect(result.current_step).toBeNull();
  expect(result.open_steps).toEqual([]);
});

test('describe-live surfaces non-success status through ApiError', async () => {
  try {
    await clientReturning({ code: 'namespace_denied', message: 'denied' }, 403).describeLive(
      namespace,
      workflowId,
      runId,
      undefined
    );
    throw new Error('request unexpectedly succeeded');
  } catch (error) {
    expect(error).toBeInstanceOf(ApiError);
    expect((error as ApiError).status).toBe(403);
  }
});

test('describe-live rejects a non-object response', async () => {
  await expect(
    clientReturning([]).describeLive(namespace, workflowId, runId, undefined)
  ).rejects.toBeInstanceOf(ApiError);
});

test('describe-live rejects non-array open_steps', async () => {
  await expect(
    clientReturning(response({ open_steps: {} })).describeLive(
      namespace,
      workflowId,
      runId,
      undefined
    )
  ).rejects.toBeInstanceOf(ApiError);
});

test('describe-live rejects an unknown liveness tag and names it', async () => {
  try {
    await clientReturning(
      response({
        current_step: {
          step: openStep(),
          attempt: liveAttempt({ liveness: { liveness: 'MaybeLive' } }),
        },
      })
    ).describeLive(namespace, workflowId, runId, undefined);
    throw new Error('decoder accepted an unknown liveness tag');
  } catch (error) {
    expect(error).toBeInstanceOf(ApiError);
    expect((error as Error).message).toContain('MaybeLive');
  }
});

test('describe-live rejects every live tagged union with an unknown tag', async () => {
  const malformedResponses = [
    response({ current_step: { step: openStep({ state: { state: 'Waiting' } }), attempt: null } }),
    response({
      current_step: {
        step: openStep(),
        attempt: liveAttempt({ note: { note: 'Lost' } }),
      },
    }),
    response({
      current_step: {
        step: openStep(),
        attempt: liveAttempt({ transcript: { transcript: 'Everything' } }),
      },
    }),
  ];

  for (const body of malformedResponses) {
    await expect(
      clientReturning(body).describeLive(namespace, workflowId, runId, undefined)
    ).rejects.toBeInstanceOf(ApiError);
  }
});

test('describe-live forwards AbortSignal to fetch', async () => {
  const controller = new AbortController();
  let capturedSignal: AbortSignal | null | undefined;
  const client = new DescribeLiveReadClient({
    fetchImpl: async (_input, init) => {
      capturedSignal = init?.signal;
      return Response.json(response());
    },
  });

  await client.describeLive(namespace, workflowId, runId, undefined, controller.signal);

  expect(capturedSignal).toBe(controller.signal);
});
