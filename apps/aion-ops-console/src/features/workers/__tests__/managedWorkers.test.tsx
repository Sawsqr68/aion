import { describe, expect, test } from 'bun:test';
import { QueryClient } from '@tanstack/react-query';
import { renderToStaticMarkup } from 'react-dom/server';

import type { ClusterStreamListener } from '@/lib/api/cluster-stream';
import type { ClusterEvent } from '@/types';

import { ManagedWorkersBody } from '../components/ManagedWorkersPanel';
import {
  applyWorker,
  managedWorkersQueryKey,
  shouldRefreshOnClusterEvent,
  wireManagedWorkersLiveRefresh,
} from '../hooks/useManagedWorkers';
import {
  createManagedWorkersClient,
  type ManagedWorkerReport,
  type ManagedWorkerStatus,
  ManagedWorkersRefusedError,
  ManagedWorkersSurfaceUnmountedError,
} from '../lib/managedWorkers';

const HASH = 'a'.repeat(64);

/** The exact serialization `crates/aion-server` writes for one worker. */
function wireWorker(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    name: 'briefs-worker',
    task_queue: 'briefs',
    desired: 'running',
    state: 'running',
    pid: 4242,
    process_group: -4242,
    restarts: 2,
    last_exit: { at: '2026-08-08T10:00:00Z', code: 1, signal: null, requested: false },
    last_error: 'exited with code 1',
    deployed_binary: { version: '0.2.0', commit: 'abc123', dirty: 'clean', content_hash: HASH },
    spawn_binary: {
      path: '/usr/local/bin/aion',
      content_hash: HASH,
      version: '0.2.0',
      commit: 'abc123',
      dirty: 'clean',
    },
    ...overrides,
  };
}

type Call = { url: string; init: RequestInit | undefined };

function stub(status: number, body: unknown, calls: Call[] = []) {
  const fetchImpl = (input: RequestInfo | URL, init?: RequestInit) => {
    calls.push({ url: String(input), init });
    return Promise.resolve(
      new Response(body === null ? null : JSON.stringify(body), {
        status,
        headers: { 'content-type': 'application/json' },
      })
    );
  };
  return { client: createManagedWorkersClient(fetchImpl), calls };
}

describe('the managed-workers client', () => {
  // Catches: a parser that loses/renames any field of the server's
  // ManagedWorkerStatus serialization, or misreads the report envelope.
  test('parses the full report shape the server serializes', async () => {
    const { client, calls } = stub(200, {
      commissioned: true,
      remedy: null,
      workers: [wireWorker()],
      undecodable: ['poisoned-row'],
    });
    const report = await client.report();
    expect(calls[0]?.url).toBe('/workers/managed');
    expect(report.commissioned).toBe(true);
    expect(report.remedy).toBeNull();
    expect(report.undecodable).toEqual(['poisoned-row']);
    expect(report.workers[0]).toEqual({
      name: 'briefs-worker',
      taskQueue: 'briefs',
      desired: 'running',
      state: 'running',
      pid: 4242,
      processGroup: -4242,
      restarts: 2,
      lastExit: { at: '2026-08-08T10:00:00Z', code: 1, signal: null, requested: false },
      lastError: 'exited with code 1',
      deployedBinary: { version: '0.2.0', commit: 'abc123', dirty: 'clean', contentHash: HASH },
      spawnBinary: {
        path: '/usr/local/bin/aion',
        contentHash: HASH,
        version: '0.2.0',
        commit: 'abc123',
        dirty: 'clean',
      },
    });
  });

  // Catches: the uncommissioned report — remedy present, empty fleet —
  // misread as an error or as "all is well".
  test('parses an uncommissioned report with its remedy', async () => {
    const { client } = stub(200, {
      commissioned: false,
      remedy: 'add a [worker_supervision] section to the server config',
      workers: [],
      undecodable: [],
    });
    const report = await client.report();
    expect(report.commissioned).toBe(false);
    expect(report.remedy).toBe('add a [worker_supervision] section to the server config');
  });

  // Catches: an unknown state token folded into some "close enough" state
  // instead of being reported.
  test('an unknown worker state is reported, never coerced', async () => {
    const { client } = stub(200, {
      commissioned: true,
      remedy: null,
      workers: [wireWorker({ state: 'quantum' })],
      undecodable: [],
    });
    await expect(client.report()).rejects.toThrow('unknown state');
  });

  // Catches: the action POST hitting the wrong path/method, or the `{worker}`
  // envelope being misread.
  test('an action POSTs to the verb route and returns the response worker', async () => {
    const calls: Call[] = [];
    const { client } = stub(200, { worker: wireWorker({ state: 'stopped', pid: null }) }, calls);
    const worker = await client.act('briefs-worker', 'stop');
    expect(calls[0]?.url).toBe('/workers/managed/briefs-worker/stop');
    expect(calls[0]?.init?.method).toBe('POST');
    expect(worker.state).toBe('stopped');
    expect(worker.pid).toBeNull();
  });

  test('the action route encodes the deployment name', async () => {
    const calls: Call[] = [];
    const { client } = stub(404, null, calls);
    await client.act('odd/name', 'start').catch(() => undefined);
    expect(calls[0]?.url).toBe('/workers/managed/odd%2Fname/start');
  });

  // Catches: the 409 uncommissioned refusal losing the server's remedy — the
  // error MESSAGE must be the server's message verbatim, and the code must
  // stay branchable.
  test('a 409 invalid_state refusal carries the remedy verbatim', async () => {
    const remedy =
      'worker supervision is not commissioned on this server; add a [worker_supervision] section and restart';
    const { client } = stub(409, { code: 'invalid_state', message: remedy });
    const refusal = await client
      .act('briefs-worker', 'start')
      .then(() => null)
      .catch((error: unknown) => error);
    expect(refusal).toBeInstanceOf(ManagedWorkersRefusedError);
    const typed = refusal as ManagedWorkersRefusedError;
    expect(typed.message).toBe(remedy);
    expect(typed.code).toBe('invalid_state');
    expect(typed.status).toBe(409);
  });

  // Catches: 404 and deploy-denied refusals flattened into a generic error.
  test('not-found and deploy-denied refusals keep the server message and code', async () => {
    const missing = stub(404, {
      code: 'not_found',
      message: 'managed worker `ghost` was not found',
    });
    const notFound = (await missing.client
      .act('ghost', 'restart')
      .catch((e: unknown) => e)) as ManagedWorkersRefusedError;
    expect(notFound.code).toBe('not_found');
    expect(notFound.message).toBe('managed worker `ghost` was not found');

    const denied = stub(403, {
      code: 'deploy_denied',
      message: 'managed-worker administration requires the deployment-wide deploy grant',
    });
    const refused = (await denied.client
      .report()
      .catch((e: unknown) => e)) as ManagedWorkersRefusedError;
    expect(refused.code).toBe('deploy_denied');
    expect(refused.message).toContain('deploy grant');
  });

  // Catches: a bodyless failure swallowed or surfaced without its status.
  test('a refusal with no wire-error body still reports the status', async () => {
    const { client } = stub(502, null);
    const refusal = (await client.report().catch((e: unknown) => e)) as ManagedWorkersRefusedError;
    expect(refusal.message).toContain('502');
    expect(refusal.code).toBe('unknown');
  });

  // M3. Catches: a BARE 404 from a lifecycle POST — the router with
  // `[deploy] enabled=false` mounts no lifecycle route at all — paraphrased
  // into "Managed-worker request failed (404)", indistinguishable from an
  // unknown worker. It is a structural condition and must be stated as one;
  // a 404 WITH a `not_found` wire body stays the server's verbatim answer
  // (pinned by the not-found test above).
  test('a bare bodyless 404 is the unmounted lifecycle surface, in those words', async () => {
    const { client } = stub(404, null);
    const refusal = await client
      .act('briefs-worker', 'start')
      .then(() => null)
      .catch((error: unknown) => error);
    expect(refusal).toBeInstanceOf(ManagedWorkersSurfaceUnmountedError);
    const typed = refusal as ManagedWorkersSurfaceUnmountedError;
    expect(typed.message).toContain('does not mount worker lifecycle commands');
    expect(typed.message).toContain('[deploy] enabled=false');
    expect(typed.message).not.toContain('(404)');
  });
});

function reportWith(workers: ManagedWorkerStatus[]): ManagedWorkerReport {
  return { commissioned: true, remedy: null, workers, undecodable: [] };
}

function parsedWorker(overrides: Partial<ManagedWorkerStatus> = {}): ManagedWorkerStatus {
  return {
    name: 'briefs-worker',
    taskQueue: 'briefs',
    desired: 'running',
    state: 'running',
    pid: 4242,
    processGroup: -4242,
    restarts: 0,
    lastExit: null,
    lastError: null,
    deployedBinary: { version: '0.2.0', commit: 'abc123', dirty: 'clean', contentHash: HASH },
    spawnBinary: null,
    ...overrides,
  };
}

describe('ManagedWorkersBody', () => {
  // Catches: the uncommissioned server rendered as a generic error (or as a
  // healthy empty list). The server's remedy must appear VERBATIM.
  test('renders the uncommissioned remedy verbatim', () => {
    const remedy =
      'worker supervision is not commissioned; add [worker_supervision] to the config and restart';
    const markup = renderToStaticMarkup(
      <ManagedWorkersBody
        actionError={null}
        onAction={() => undefined}
        onRefresh={() => undefined}
        pending={null}
        refreshing={false}
        report={{ commissioned: false, remedy, workers: [], undecodable: [] }}
      />
    );
    expect(markup).toContain(remedy);
    expect(markup).not.toContain('An unknown error occurred');
  });

  // Catches: rows losing their identity or their three lifecycle commands.
  test('renders each worker with state, queue, and Start/Stop/Restart', () => {
    const markup = renderToStaticMarkup(
      <ManagedWorkersBody
        actionError={null}
        onAction={() => undefined}
        onRefresh={() => undefined}
        pending={null}
        refreshing={false}
        report={reportWith([parsedWorker(), parsedWorker({ name: 'mailer', state: 'backoff' })])}
      />
    );
    expect(markup).toContain('briefs-worker');
    expect(markup).toContain('queue briefs');
    expect(markup).toContain('running');
    expect(markup).toContain('backoff');
    expect(markup).toContain('>Start<');
    expect(markup).toContain('>Stop<');
    expect(markup).toContain('>Restart<');
  });

  // Catches: a 409 action refusal rendered as a generic message instead of
  // the server's own words.
  test('an action refusal renders the server message verbatim', () => {
    const remedy = 'worker supervision is not commissioned on this server';
    const markup = renderToStaticMarkup(
      <ManagedWorkersBody
        actionError={new ManagedWorkersRefusedError(409, 'invalid_state', null, remedy)}
        onAction={() => undefined}
        onRefresh={() => undefined}
        pending={null}
        refreshing={false}
        report={reportWith([parsedWorker()])}
      />
    );
    expect(markup).toContain(remedy);
    expect(markup).toContain('Worker action refused');
  });

  // Catches: poisoned durable rows silently dropped from the panel.
  test('undecodable rows are reported by name', () => {
    const markup = renderToStaticMarkup(
      <ManagedWorkersBody
        actionError={null}
        onAction={() => undefined}
        onRefresh={() => undefined}
        pending={null}
        refreshing={false}
        report={{ commissioned: true, remedy: null, workers: [], undecodable: ['rot13-row'] }}
      />
    );
    expect(markup).toContain('rot13-row');
    expect(markup).toContain('Undecodable');
  });

  // Catches: a silent binary swap — spawn hash differing from deploy-time
  // hash must be stated.
  test('a spawned binary differing from the deployed identity is called out', () => {
    const markup = renderToStaticMarkup(
      <ManagedWorkersBody
        actionError={null}
        onAction={() => undefined}
        onRefresh={() => undefined}
        pending={null}
        refreshing={false}
        report={reportWith([
          parsedWorker({
            spawnBinary: {
              path: '/usr/local/bin/aion',
              contentHash: 'f'.repeat(64),
              version: null,
              commit: null,
              dirty: null,
            },
          }),
        ])}
      />
    );
    expect(markup).toContain('differs from the deploy-time binary');
  });
});

describe('applyWorker', () => {
  // Catches: the post-action cache update dropping the response's worker or
  // touching unrelated rows.
  test('replaces the matching row and keeps the rest', () => {
    const report = reportWith([parsedWorker(), parsedWorker({ name: 'mailer' })]);
    const next = applyWorker(report, parsedWorker({ state: 'stopped', pid: null }));
    expect(next.workers.map((worker) => [worker.name, worker.state])).toEqual([
      ['briefs-worker', 'stopped'],
      ['mailer', 'running'],
    ]);
  });

  test('appends (in name order) a worker the report has not seen', () => {
    const report = reportWith([parsedWorker({ name: 'mailer' })]);
    const next = applyWorker(report, parsedWorker({ name: 'archiver' }));
    expect(next.workers.map((worker) => worker.name)).toEqual(['archiver', 'mailer']);
  });
});

const META = { cluster_seq: 1, observed_at: '2026-08-10T00:00:00+00:00' };

const ECHOING_EVENTS: ClusterEvent[] = [
  {
    type: 'WorkerConnected',
    meta: META,
    worker_id: 'w-1',
    namespaces: [],
    task_queue: 'briefs',
    transport: { transport: 'Grpc' },
    node: null,
    deployment: null,
    deployment_association: null,
  },
  {
    type: 'WorkerDisconnected',
    meta: META,
    worker_id: 'w-1',
    namespaces: [],
    reason: { reason: 'Disconnect' },
  },
  {
    type: 'WorkerDeploymentPut',
    meta: META,
    name: 'briefs-worker',
    outcome: 'created',
    desired_state: 'running',
    binary_version: '0.2.0',
    binary_content_hash: HASH,
  },
  {
    type: 'WorkerDeploymentDesiredStateChanged',
    meta: META,
    name: 'briefs-worker',
    desired_state: 'stopped',
  },
  { type: 'WorkerDeploymentDeleted', meta: META, name: 'briefs-worker' },
  { type: 'SupervisorStarted', meta: META, node: 'node-a' },
  { type: 'SupervisorStopped', meta: META, node: 'node-a' },
];

const SILENT_EVENTS: ClusterEvent[] = [
  { type: 'PeerAdded', meta: META, peer_name: 'node-b', forward_addr: null },
  { type: 'PeerConnected', meta: META, peer_name: 'node-b', forward_addr: null },
];

describe('shouldRefreshOnClusterEvent', () => {
  // M6. Catches: a deleted switch case — every wire echo the panel relies on
  // must admit a refresh, enumerated one by one.
  for (const event of ECHOING_EVENTS) {
    test(`${event.type} warrants a refresh`, () => {
      expect(shouldRefreshOnClusterEvent(event)).toBe(true);
    });
  }

  // Catches: the filter widening into refetch-on-everything (which would turn
  // the shared cluster socket into a refetch firehose for unrelated topology
  // chatter).
  for (const event of SILENT_EVENTS) {
    test(`${event.type} does not`, () => {
      expect(shouldRefreshOnClusterEvent(event)).toBe(false);
    });
  }
});

describe('wireManagedWorkersLiveRefresh', () => {
  function harness() {
    const queryClient = new QueryClient();
    queryClient.setQueryData<ManagedWorkerReport>(managedWorkersQueryKey, reportWith([]));
    let captured: ClusterStreamListener | null = null;
    let unsubscribed = 0;
    const manager = {
      subscribe: (listener: ClusterStreamListener) => {
        captured = listener;
        return () => {
          unsubscribed += 1;
        };
      },
    };
    const unsubscribe = wireManagedWorkersLiveRefresh(queryClient, manager);
    const listener = captured as ClusterStreamListener | null;
    if (listener === null) throw new Error('the wiring must subscribe a listener');
    const invalidated = () =>
      queryClient.getQueryState(managedWorkersQueryKey)?.isInvalidated ?? false;
    return {
      queryClient,
      listener,
      unsubscribe,
      unsubscribedCount: () => unsubscribed,
      invalidated,
    };
  }

  // M6. Catches: `subscribe` deleted (nothing wired), or `onEvent` dropped —
  // an admitted delta must actually invalidate the cached report.
  test('an echoing delta invalidates the report query', () => {
    const { listener, invalidated } = harness();
    expect(invalidated()).toBe(false);
    listener.onEvent({ type: 'WorkerDeploymentDeleted', meta: META, name: 'briefs-worker' });
    expect(invalidated()).toBe(true);
  });

  test('a non-echoing delta leaves the report alone', () => {
    const { listener, invalidated } = harness();
    listener.onEvent({ type: 'PeerAdded', meta: META, peer_name: 'node-b', forward_addr: null });
    expect(invalidated()).toBe(false);
  });

  // Catches: `onSnapshot` deleted — a re-priming snapshot means the socket
  // was down and anything may have happened; the report must be re-read.
  test('a (re-)priming snapshot invalidates the report query', () => {
    const { listener, invalidated } = harness();
    listener.onSnapshot({} as never);
    expect(invalidated()).toBe(true);
  });

  test('the returned unsubscribe is the subscription teardown', () => {
    const { unsubscribe, unsubscribedCount } = harness();
    expect(unsubscribedCount()).toBe(0);
    unsubscribe();
    expect(unsubscribedCount()).toBe(1);
  });
});

describe('the refresh affordance (follow-up #182 bridge)', () => {
  // M2. Catches: the manual refresh demoted to an error-state affordance —
  // supervision transitions with no wire echo (spawn refusals, backoff,
  // failed) are only visible through it, so it must render in the ordinary
  // healthy state, alongside its honest caption.
  test('the body always offers Refresh with the honest live-coverage caption', () => {
    const markup = renderToStaticMarkup(
      <ManagedWorkersBody
        actionError={null}
        onAction={() => undefined}
        onRefresh={() => undefined}
        pending={null}
        refreshing={false}
        report={reportWith([parsedWorker()])}
      />
    );
    expect(markup).toContain('>Refresh<');
    expect(markup).toContain('other');
    expect(markup).toContain('supervision transitions appear on refresh');
  });

  // M3. Catches: the structural refusal rendered under the generic
  // "action refused" banner — the unmounted surface has its own name.
  test('the unmounted-surface refusal renders as the structural condition', () => {
    const markup = renderToStaticMarkup(
      <ManagedWorkersBody
        actionError={new ManagedWorkersSurfaceUnmountedError()}
        onAction={() => undefined}
        onRefresh={() => undefined}
        pending={null}
        refreshing={false}
        report={reportWith([parsedWorker()])}
      />
    );
    expect(markup).toContain('Lifecycle commands not mounted');
    expect(markup).toContain('does not mount worker lifecycle commands');
    expect(markup).not.toContain('Worker action refused');
  });
});
