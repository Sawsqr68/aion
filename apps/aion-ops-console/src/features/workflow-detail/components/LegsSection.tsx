import type { ReactNode } from 'react';

import type { ChildLeg } from '@/lib/api';
import { shortWorkflowId } from '@/lib/workflowIdentity';
import type {
  ActivityId,
  Event,
  Namespace,
  RunGeneration,
  RunId,
  WorkflowId,
  WorkflowStatus,
} from '@/types';

import { TranscriptPanel } from '../../transcript/components/TranscriptPanel';
import { type LiveWorkflowEvents, useLiveWorkflowEvents } from '../hooks/useLiveWorkflowEvents';
import { useWorkflowHead } from '../hooks/useWorkflowHead';
import { useWorkflowHistory } from '../hooks/useWorkflowHistory';
import { type AttemptNavigatorRow, deriveAttemptNavigator } from '../lib/attemptNavigator';
import type { LifecycleOutcome } from '../types';

type LegAttemptView = {
  activityId: ActivityId;
  activityType: string | null;
  attempt: number;
  state: 'current' | 'completed' | 'failed' | 'cancelled' | 'superseded';
  supersededBy: number | null;
};

function AttemptHeading({ attempt }: { attempt: LegAttemptView }) {
  return (
    <div className="rounded border border-border px-3 py-2" data-attempt-state={attempt.state}>
      <div className="flex flex-wrap items-center gap-2 text-xs">
        <strong>{attempt.activityType ?? `Activity ${attempt.activityId}`}</strong>
        <span>attempt {attempt.attempt}</span>
        <span className="text-muted-foreground">
          {attempt.state === 'superseded'
            ? `Superseded by attempt ${attempt.supersededBy}`
            : attempt.state}
        </span>
      </div>
    </div>
  );
}

function LegHeader({
  activityId,
  attempt,
  leg,
  status,
}: {
  activityId: ActivityId | null;
  attempt: number | null;
  leg: ChildLeg;
  status: WorkflowStatus | null;
}) {
  return (
    <header className="flex flex-wrap items-center gap-2 border-b border-border px-4 py-3">
      <strong>{leg.displayName ?? leg.workflowType}</strong>
      <code className="text-xs text-muted-foreground">{shortWorkflowId(leg.workflowId)}</code>
      <span className="ml-auto text-xs font-medium">{status ?? 'Not started'}</span>
      {activityId !== null && attempt !== null ? (
        <span className="text-xs text-muted-foreground">
          Activity {activityId} · attempt {attempt}
        </span>
      ) : null}
    </header>
  );
}

/** Shared shipped leg card: both the fixture pin and live run page render this markup. */
function LegCard({
  activityId,
  attempt,
  children,
  leg,
  status,
}: {
  activityId: ActivityId | null;
  attempt: number | null;
  children: ReactNode;
  leg: ChildLeg;
  status: WorkflowStatus | null;
}) {
  return (
    <article className="rounded-md border border-border bg-surface">
      <LegHeader activityId={activityId} attempt={attempt} leg={leg} status={status} />
      <div className="space-y-3 p-3">
        {leg.runId === null ? (
          <p className="text-xs text-muted-foreground">
            Child start recorded; run not started yet.
          </p>
        ) : null}
        {children}
      </div>
    </article>
  );
}

type LegHistoryRead = {
  data: readonly Event[];
  error: unknown;
  state: 'disabled' | 'error' | 'loading' | 'success';
};

export type LiveLegReaders = {
  useHistory(options: { enabled: boolean; workflowId: WorkflowId }): LegHistoryRead;
  useLiveEvents(options: {
    enabled: boolean;
    history: readonly Event[];
    workflowId: WorkflowId;
  }): LiveWorkflowEvents;
  /**
   * The LEG's own generation boundaries, server-stated over its whole history.
   *
   * A leg is a child workflow with a history of its own, so its boundaries must
   * be read for ITS workflow id — the parent's would name the parent's runs and
   * the leg's attempts would then be filtered against identities they can never
   * carry. Absent/loading answers an empty list, which yields no rows rather
   * than rows attributed to a guessed run.
   */
  useGenerations(options: { enabled: boolean; workflowId: WorkflowId }): readonly RunGeneration[];
};

const defaultReaders: LiveLegReaders = {
  useHistory: ({ enabled, workflowId }) => {
    const history = useWorkflowHistory({ enabled, workflowId });
    let state: LegHistoryRead['state'];
    if (!enabled) state = 'disabled';
    else if (history.isError) state = 'error';
    else if (history.isSuccess) state = 'success';
    else state = 'loading';
    return { data: history.data ?? [], error: history.error, state };
  },
  useLiveEvents: useLiveWorkflowEvents,
  useGenerations: ({ enabled, workflowId }) =>
    useWorkflowHead({ enabled, workflowId }).data?.generations ?? [],
};

/** Live container: discovery is REST once; histories and transcripts are socket-first. */
export function LiveLegsSection({
  legs,
  namespace,
  readers = defaultReaders,
}: {
  legs: readonly ChildLeg[];
  namespace: Namespace;
  readers?: LiveLegReaders;
}) {
  if (legs.length === 0) return null;
  return (
    <section aria-labelledby="run-legs-heading" className="border-t border-border px-6 py-5">
      <div className="mb-4">
        <h2 className="text-sm font-semibold" id="run-legs-heading">
          Legs
        </h2>
        <p className="text-xs text-muted-foreground">
          Live transcripts for direct child runs derived from this run&apos;s history.
        </p>
      </div>
      <div className="space-y-4">
        {legs.map((leg) => (
          <LiveLeg key={leg.workflowId} leg={leg} namespace={namespace} readers={readers} />
        ))}
      </div>
    </section>
  );
}

function LiveLeg({
  leg,
  namespace,
  readers,
}: {
  leg: ChildLeg;
  namespace: Namespace;
  readers: LiveLegReaders;
}) {
  const history = readers.useHistory({ enabled: leg.runId !== null, workflowId: leg.workflowId });
  const live = readers.useLiveEvents({
    enabled: history.state === 'success' && leg.runId !== null,
    history: history.data,
    workflowId: leg.workflowId,
  });
  const generations = readers.useGenerations({
    enabled: leg.runId !== null,
    workflowId: leg.workflowId,
  });
  const rows = deriveAttemptNavigator(live.timeline, generations).filter(
    (row) => row.runId === leg.runId
  );
  const { attempts, invalidActivityId } = attemptsFromRows(rows);
  const current = attempts.findLast((attempt) => attempt.state === 'current');
  const fallback =
    attempts.length === 0 && leg.currentActivityId !== null && leg.currentAttempt !== null
      ? ({
          activityId: leg.currentActivityId,
          activityType: null,
          attempt: leg.currentAttempt,
          state: 'current',
          supersededBy: null,
        } satisfies LegAttemptView)
      : null;
  const displayed = fallback === null ? attempts : [fallback];

  return (
    <LegCard
      activityId={(current ?? fallback)?.activityId ?? null}
      attempt={(current ?? fallback)?.attempt ?? null}
      leg={leg}
      status={liveStatus(leg.status, live.terminalOutcome)}
    >
      {history.state === 'loading' ? (
        <p className="text-xs text-muted-foreground">Loading activity history…</p>
      ) : null}
      {history.state === 'error' ? (
        <p className="text-xs text-danger" role="alert">
          Could not load activity history: {errorMessage(history.error)}
        </p>
      ) : null}
      {invalidActivityId !== null ? (
        <p className="text-xs text-danger" role="alert">
          Activity history contained an invalid activity id: {invalidActivityId}
        </p>
      ) : null}
      {displayed.map((attempt) => (
        <LiveAttempt
          attempt={attempt}
          key={`${attempt.activityId}:${attempt.attempt}`}
          namespace={namespace}
          runId={leg.runId}
          subscribe={attempt.state === 'current'}
          workflowId={leg.workflowId}
        />
      ))}
      {history.state === 'success' && leg.runId !== null && displayed.length === 0 ? (
        <p className="text-xs text-muted-foreground">No activity transcript stream recorded.</p>
      ) : null}
    </LegCard>
  );
}

function LiveAttempt({
  attempt,
  namespace,
  runId,
  subscribe,
  workflowId,
}: {
  attempt: LegAttemptView;
  namespace: Namespace;
  runId: RunId | null;
  subscribe: boolean;
  workflowId: ChildLeg['workflowId'];
}) {
  const target =
    runId === null || !subscribe
      ? null
      : {
          namespace,
          workflowId,
          runId,
          activityId: attempt.activityId,
          attempt: attempt.attempt,
        };
  return (
    <div className="space-y-2 rounded border border-border p-2" data-attempt-state={attempt.state}>
      <AttemptHeading attempt={attempt} />
      <TranscriptPanel target={target} />
    </div>
  );
}

function attemptsFromRows(rows: readonly AttemptNavigatorRow[]): {
  attempts: LegAttemptView[];
  invalidActivityId: string | null;
} {
  const attempts: LegAttemptView[] = [];
  let invalidActivityId: string | null = null;
  for (const row of rows) {
    const activityId = Number(row.activityId);
    if (!Number.isSafeInteger(activityId) || activityId < 0) {
      invalidActivityId ??= row.activityId;
      continue;
    }
    const later = rows.find(
      (candidate) => candidate.activityId === row.activityId && candidate.attempt > row.attempt
    );
    attempts.push({
      activityId: activityId as ActivityId,
      activityType: row.activityType,
      attempt: row.attempt,
      state:
        later === undefined ? (row.status === 'started' ? 'current' : row.status) : 'superseded',
      supersededBy: later?.attempt ?? null,
    });
  }
  return { attempts, invalidActivityId };
}

function liveStatus(
  status: WorkflowStatus | null,
  outcome: LifecycleOutcome | null
): WorkflowStatus | null {
  if (outcome === null) return status;
  switch (outcome) {
    case 'cancelled':
      return 'Cancelled';
    case 'completed':
      return 'Completed';
    case 'failed':
      return 'Failed';
    case 'started':
      return 'Running';
    case 'timed-out':
      return 'TimedOut';
  }
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : 'unknown error';
}
