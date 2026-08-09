import { RefreshCw } from 'lucide-react';

import { EmptyState } from '@/components/EmptyState';
import { ErrorState } from '@/components/ErrorState';
import { LoadingSkeleton } from '@/components/LoadingSkeleton';
import { Button } from '@/components/ui';

import {
  useManagedWorkerAction,
  useManagedWorkersLiveRefresh,
  useManagedWorkersReport,
} from '../hooks/useManagedWorkers';
import {
  type ManagedWorkerAction,
  type ManagedWorkerReport,
  type ManagedWorkerState,
  type ManagedWorkerStatus,
  type ManagedWorkersClient,
  ManagedWorkersSurfaceUnmountedError,
} from '../lib/managedWorkers';

export type ManagedWorkersPanelProps = {
  /** Injected client (tests). */
  client?: Pick<ManagedWorkersClient, 'report' | 'act'> | undefined;
  /** Set false to skip the cluster-stream subscription (tests). */
  liveRefresh?: boolean | undefined;
};

/**
 * The managed-worker fleet: every durable worker deployment joined with what
 * its supervised process is actually doing, with per-worker Start / Stop /
 * Restart. Refreshes arrive three ways, because no single one is complete:
 * cluster-stream deltas for what echoes on the wire, the response worker after
 * every action, and the always-available manual Refresh for the transitions
 * that have no wire echo yet (spawn refusals, backoff/failed — see
 * `shouldRefreshOnClusterEvent`; follow-up #182). Success state always comes
 * from the server's response, never optimistically.
 */
export function ManagedWorkersPanel({ client, liveRefresh = true }: ManagedWorkersPanelProps) {
  const report = useManagedWorkersReport({ client });
  const action = useManagedWorkerAction({ client });
  useManagedWorkersLiveRefresh({ enabled: liveRefresh });

  if (report.isPending) return <LoadingSkeleton />;
  if (report.isError) {
    return (
      <ErrorState
        error={report.error}
        onRetry={() => void report.refetch()}
        title="Could not load managed workers"
      />
    );
  }

  return (
    <ManagedWorkersBody
      actionError={action.isError ? action.error : null}
      onAction={(name, verb) => action.mutate({ name, action: verb })}
      onRefresh={() => void report.refetch()}
      pending={action.isPending ? (action.variables ?? null) : null}
      refreshing={report.isFetching}
      report={report.data}
    />
  );
}

export type ManagedWorkersBodyProps = {
  report: ManagedWorkerReport;
  /** The in-flight action, or null. Disables that worker's buttons. */
  pending: { name: string; action: ManagedWorkerAction } | null;
  /** The last action's refusal (server message verbatim), or null. */
  actionError: Error | null;
  onAction: (name: string, action: ManagedWorkerAction) => void;
  /** Manual re-read of the report. ALWAYS offered — see the panel docs. */
  onRefresh: () => void;
  /** True while a report read is in flight (disables the refresh button). */
  refreshing: boolean;
};

/** Presentational body — separated so tests can render every state directly. */
export function ManagedWorkersBody({
  report,
  pending,
  actionError,
  onAction,
  onRefresh,
  refreshing,
}: ManagedWorkersBodyProps) {
  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between gap-3">
        {/* Not every supervision transition echoes on the cluster stream yet
            (spawn refusals, backoff/failed — follow-up #182), so the manual
            re-read is a first-class affordance, not an error-state fallback. */}
        <span className="text-muted-foreground text-xs">
          Live updates cover deployment changes and worker connections; other supervision
          transitions appear on refresh.
        </span>
        <Button disabled={refreshing} onClick={onRefresh} size="sm" variant="outline">
          <RefreshCw className="size-3.5" />
          {refreshing ? 'Refreshing…' : 'Refresh'}
        </Button>
      </div>
      {report.commissioned || report.remedy === null ? null : (
        // The server's remedy, verbatim. It names the exact config section to
        // add; paraphrasing it would be a generic error with less truth.
        <p
          className="rounded-md border border-warning/40 bg-warning/10 p-3 text-sm text-warning"
          role="status"
        >
          {report.remedy}
        </p>
      )}
      {actionError === null ? null : (
        <ErrorState
          error={actionError}
          message={actionError.message}
          title={
            actionError instanceof ManagedWorkersSurfaceUnmountedError
              ? 'Lifecycle commands not mounted'
              : 'Worker action refused'
          }
        />
      )}
      {report.workers.length === 0 ? (
        <EmptyState
          description="No durable worker deployments exist on this server. Deploy a worker document from Authoring, or PUT /worker-deployments/{name}."
          title="No managed workers"
        />
      ) : (
        <ul className="space-y-2">
          {report.workers.map((worker) => (
            <WorkerRow
              key={worker.name}
              onAction={onAction}
              pending={pending?.name === worker.name ? pending.action : null}
              worker={worker}
            />
          ))}
        </ul>
      )}
      {report.undecodable.length === 0 ? null : (
        <p className="text-destructive text-xs" role="status">
          Undecodable deployment rows (present but unreadable, supervised by nothing):{' '}
          <span className="font-mono">{report.undecodable.join(', ')}</span>
        </p>
      )}
    </div>
  );
}

function WorkerRow({
  worker,
  pending,
  onAction,
}: {
  worker: ManagedWorkerStatus;
  pending: ManagedWorkerAction | null;
  onAction: (name: string, action: ManagedWorkerAction) => void;
}) {
  const busy = pending !== null;
  return (
    <li className="space-y-2 rounded-md border border-border px-4 py-3">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <span className="flex min-w-0 flex-col">
          <span className="font-medium text-foreground text-sm">{worker.name}</span>
          <span className="font-mono text-muted-foreground text-xs">
            queue {worker.taskQueue}
            {worker.pid === null ? '' : ` · pid ${worker.pid}`}
            {worker.restarts === 0 ? '' : ` · ${worker.restarts} restarts`}
          </span>
        </span>
        <span className="flex items-center gap-3">
          <StateBadge state={worker.state} />
          <span className="text-muted-foreground text-xs">desired {worker.desired}</span>
          <span className="flex gap-1.5">
            <Button
              disabled={busy}
              onClick={() => onAction(worker.name, 'start')}
              size="sm"
              variant="outline"
            >
              {pending === 'start' ? 'Starting…' : 'Start'}
            </Button>
            <Button
              disabled={busy}
              onClick={() => onAction(worker.name, 'stop')}
              size="sm"
              variant="outline"
            >
              {pending === 'stop' ? 'Stopping…' : 'Stop'}
            </Button>
            <Button
              disabled={busy}
              onClick={() => onAction(worker.name, 'restart')}
              size="sm"
              variant="outline"
            >
              {pending === 'restart' ? 'Restarting…' : 'Restart'}
            </Button>
          </span>
        </span>
      </div>
      {worker.lastError === null ? null : (
        <p className="text-destructive text-xs">{worker.lastError}</p>
      )}
      <BinaryDrift worker={worker} />
    </li>
  );
}

/**
 * A restart across an upgrade is a visible fact: when the spawned binary's
 * content hash differs from the deploy-time identity, say so.
 */
function BinaryDrift({ worker }: { worker: ManagedWorkerStatus }) {
  const spawn = worker.spawnBinary;
  if (spawn === null || spawn.contentHash === worker.deployedBinary.contentHash) return null;
  return (
    <p className="text-warning text-xs" role="status">
      Running binary differs from the deploy-time binary (spawned{' '}
      <span className="font-mono">{spawn.contentHash.slice(0, 12)}</span>, deployed{' '}
      <span className="font-mono">{worker.deployedBinary.contentHash.slice(0, 12)}</span>).
    </p>
  );
}

const STATE_STYLES: Record<ManagedWorkerState, string> = {
  starting: 'border-warning/40 bg-warning/10 text-warning',
  running: 'border-success/40 bg-success/10 text-success',
  backoff: 'border-warning/40 bg-warning/10 text-warning',
  stopped: 'border-border bg-surface-elevated text-muted-foreground',
  failed: 'border-destructive/40 bg-destructive/10 text-destructive',
  uncommissioned: 'border-warning/40 bg-warning/10 text-warning',
};

function StateBadge({ state }: { state: ManagedWorkerState }) {
  return (
    <span
      className={`inline-flex rounded-full border px-2.5 py-0.5 font-medium text-xs ${STATE_STYLES[state]}`}
    >
      {state}
    </span>
  );
}
