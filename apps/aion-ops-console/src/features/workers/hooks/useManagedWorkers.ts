import { type QueryClient, useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect, useMemo } from 'react';

import type { AionClusterStreamManager, ClusterStreamListener } from '@/lib/api/cluster-stream';
import { configuredClusterStream } from '@/lib/config';
import type { ClusterEvent } from '@/types';

import {
  type ManagedWorkerAction,
  type ManagedWorkerReport,
  type ManagedWorkerStatus,
  type ManagedWorkersClient,
  managedWorkersClient,
} from '../lib/managedWorkers';

export const managedWorkersQueryKey = ['workers', 'managed'] as const;

export type UseManagedWorkersOptions = {
  /** Injected client (tests); defaults to the shared fetch-backed client. */
  client?: Pick<ManagedWorkersClient, 'report'> | undefined;
  /** Gate the query (e.g. only fetch while the panel is mounted). */
  enabled?: boolean | undefined;
};

/**
 * Query the managed-worker report (`GET /workers/managed`): desired state
 * joined with actual state, plus the commissioned flag and its remedy.
 */
export function useManagedWorkersReport({ client, enabled = true }: UseManagedWorkersOptions = {}) {
  const resolved = useMemo(() => client ?? managedWorkersClient, [client]);
  return useQuery<ManagedWorkerReport>({
    enabled,
    queryKey: managedWorkersQueryKey,
    queryFn: () => resolved.report(),
  });
}

/**
 * Whether one cluster-stream delta warrants re-reading the report.
 *
 * What actually echoes on the wire, stated honestly:
 *
 * - `WorkerDeploymentPut` / `WorkerDeploymentDeleted` — emitted by the HTTP
 *   deployment handlers on record mutation;
 * - `WorkerDeploymentDesiredStateChanged` — emitted by those handlers AND by
 *   the supervisor's lifecycle transitions (the supervisor's ONLY cluster
 *   emission, `fleet.rs`);
 * - `WorkerConnected` / `WorkerDisconnected` — emitted by the WORKER REGISTRY
 *   when a spawned worker's protocol session opens or closes;
 * - `SupervisorStarted` / `SupervisorStopped` — NODE-supervisor lifecycle
 *   (the node's shard supervisor coming up or down, not per-worker state);
 *   refreshed on because a node arriving or leaving changes what is being
 *   supervised at all.
 *
 * What does NOT echo — and therefore needs the panel's manual refresh (or an
 * action's own response) to become visible: a spawn refusal (a worker whose
 * connection settings are unresolvable sits at `starting`/`backoff` with its
 * `last_error` set and never opens a session), `backoff`/`failed`
 * transitions, restart counts, and exit records. Closing that gap is the
 * supervision-state cluster-events follow-up (#182); until it lands, the
 * refresh affordance is the honest bridge.
 */
export function shouldRefreshOnClusterEvent(event: ClusterEvent): boolean {
  switch (event.type) {
    case 'WorkerConnected':
    case 'WorkerDisconnected':
    case 'WorkerDeploymentPut':
    case 'WorkerDeploymentDesiredStateChanged':
    case 'WorkerDeploymentDeleted':
    case 'SupervisorStarted':
    case 'SupervisorStopped':
      return true;
    default:
      return false;
  }
}

/**
 * Subscribe the managed-workers query to the cluster stream: every delta
 * {@link shouldRefreshOnClusterEvent} admits — and every re-priming snapshot
 * (anything can have happened while the socket was down) — invalidates the
 * report. Returns the unsubscribe. Extracted from the hook so a test can
 * drive it with a fake manager and a real `QueryClient`.
 */
export function wireManagedWorkersLiveRefresh(
  queryClient: Pick<QueryClient, 'invalidateQueries'>,
  manager: Pick<AionClusterStreamManager, 'subscribe'>
): () => void {
  const invalidate = () => {
    void queryClient.invalidateQueries({ queryKey: managedWorkersQueryKey });
  };
  const listener: ClusterStreamListener = {
    onSnapshot: invalidate,
    onEvent: (event) => {
      if (shouldRefreshOnClusterEvent(event)) invalidate();
    },
  };
  return manager.subscribe(listener);
}

/**
 * Socket-assisted refresh for the panel: re-read the report when the cluster
 * stream announces a change that echoes on the wire. This is deliberately NOT
 * a claim of completeness — see {@link shouldRefreshOnClusterEvent} for what
 * does not echo and why the panel also carries a manual refresh.
 */
export function useManagedWorkersLiveRefresh({
  manager = configuredClusterStream,
  enabled = true,
}: {
  manager?: Pick<AionClusterStreamManager, 'subscribe'>;
  enabled?: boolean;
} = {}) {
  const queryClient = useQueryClient();
  useEffect(() => {
    if (!enabled) return;
    return wireManagedWorkersLiveRefresh(queryClient, manager);
  }, [enabled, manager, queryClient]);
}

export type ManagedWorkerActionVariables = {
  name: string;
  action: ManagedWorkerAction;
};

/**
 * Mutation for one lifecycle command. The response's `worker` object is the
 * server's post-action truth, applied to the cached report immediately; a full
 * refetch follows so the join (and any concurrent change) is re-read whole.
 * A refusal — 404 unknown name, 409 uncommissioned-with-remedy, deploy
 * denied, or the structurally unmounted lifecycle surface — surfaces as the
 * mutation error carrying the server's message (or the structural condition)
 * verbatim.
 */
export function useManagedWorkerAction({
  client,
}: {
  client?: Pick<ManagedWorkersClient, 'act'> | undefined;
} = {}) {
  const queryClient = useQueryClient();
  const resolved = useMemo(() => client ?? managedWorkersClient, [client]);
  return useMutation<ManagedWorkerStatus, Error, ManagedWorkerActionVariables>({
    mutationFn: (variables) => resolved.act(variables.name, variables.action),
    onSuccess: (worker) => {
      queryClient.setQueryData<ManagedWorkerReport>(managedWorkersQueryKey, (report) =>
        report === undefined ? report : applyWorker(report, worker)
      );
      void queryClient.invalidateQueries({ queryKey: managedWorkersQueryKey });
    },
  });
}

/** Replace (or, for a name the report has not seen, append) one worker row. */
export function applyWorker(
  report: ManagedWorkerReport,
  worker: ManagedWorkerStatus
): ManagedWorkerReport {
  const known = report.workers.some((candidate) => candidate.name === worker.name);
  return {
    ...report,
    workers: known
      ? report.workers.map((candidate) => (candidate.name === worker.name ? worker : candidate))
      : [...report.workers, worker].sort((a, b) => a.name.localeCompare(b.name)),
  };
}
