import type { ReactNode } from 'react';

import type { ChildLeg } from '@/lib/api';
import { shortWorkflowId } from '@/lib/workflowIdentity';
import type { ActivityId, Namespace, RunId, WorkflowStatus } from '@/types';

import { TranscriptPanel } from '../../transcript/components/TranscriptPanel';
import { useLiveWorkflowEvents } from '../hooks/useLiveWorkflowEvents';
import { useWorkflowHistory } from '../hooks/useWorkflowHistory';
import { type AttemptNavigatorRow, deriveAttemptNavigator } from '../lib/attemptNavigator';

export type LegAttemptView = {
  activityId: ActivityId;
  activityType: string | null;
  attempt: number;
  state: 'current' | 'completed' | 'failed' | 'cancelled' | 'superseded';
  supersededBy: number | null;
};

export type LegView = ChildLeg & { attempts: LegAttemptView[] };

/** Recorded fixture wrapper over the same LegCard presentation the live run page ships. */
export function LegsSection({ legs }: { legs: readonly LegView[] }) {
  if (legs.length === 0) return null;
  return (
    <section aria-labelledby="run-legs-heading" className="border-t border-border px-6 py-5">
      <div className="mb-4">
        <h2 className="text-sm font-semibold" id="run-legs-heading">
          Legs
        </h2>
        <p className="text-xs text-muted-foreground">
          Direct child runs derived from this run&apos;s recorded history.
        </p>
      </div>
      <div className="space-y-4">
        {legs.map((leg) => (
          <LegCard
            activityId={leg.currentActivityId}
            attempt={leg.currentAttempt}
            key={leg.workflowId}
            leg={leg}
            status={leg.status}
          >
            {leg.attempts.map((attempt) => (
              <AttemptHeading
                attempt={attempt}
                key={`${attempt.activityId}:${attempt.attempt}`}
              />
            ))}
          </LegCard>
        ))}
      </div>
    </section>
  );
}

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

/** Live container: discovery is REST once; histories and transcripts are socket-first. */
export function LiveLegsSection({
  legs,
  namespace,
}: {
  legs: readonly ChildLeg[];
  namespace: Namespace;
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
          <LiveLeg key={leg.workflowId} leg={leg} namespace={namespace} />
        ))}
      </div>
    </section>
  );
}

function LiveLeg({ leg, namespace }: { leg: ChildLeg; namespace: Namespace }) {
  const history = useWorkflowHistory({ enabled: leg.runId !== null, workflowId: leg.workflowId });
  const live = useLiveWorkflowEvents({
    enabled: history.isSuccess && leg.runId !== null,
    history: history.data ?? [],
    workflowId: leg.workflowId,
  });
  const rows = deriveAttemptNavigator(live.timeline).filter((row) => row.runId === leg.runId);
  const attempts = attemptsFromRows(rows);
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
      {displayed.map((attempt) => (
        <LiveAttempt
          attempt={attempt}
          key={`${attempt.activityId}:${attempt.attempt}`}
          namespace={namespace}
          runId={leg.runId}
          workflowId={leg.workflowId}
        />
      ))}
      {leg.runId !== null && displayed.length === 0 ? (
        <p className="text-xs text-muted-foreground">No activity transcript stream recorded.</p>
      ) : null}
    </LegCard>
  );
}

function LiveAttempt({
  attempt,
  namespace,
  runId,
  workflowId,
}: {
  attempt: LegAttemptView;
  namespace: Namespace;
  runId: RunId | null;
  workflowId: ChildLeg['workflowId'];
}) {
  const target =
    runId === null
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

function attemptsFromRows(rows: readonly AttemptNavigatorRow[]): LegAttemptView[] {
  return rows.map((row) => {
    const later = rows.find(
      (candidate) => candidate.activityId === row.activityId && candidate.attempt > row.attempt
    );
    return {
      activityId: Number(row.activityId),
      activityType: row.activityType,
      attempt: row.attempt,
      state:
        later === undefined ? (row.status === 'started' ? 'current' : row.status) : 'superseded',
      supersededBy: later?.attempt ?? null,
    };
  });
}

function liveStatus(status: WorkflowStatus | null, outcome: string | null): WorkflowStatus | null {
  if (outcome === null) return status;
  switch (outcome) {
    case 'cancelled':
      return 'Cancelled';
    case 'completed':
      return 'Completed';
    case 'continued_as_new':
      return 'ContinuedAsNew';
    case 'failed':
      return 'Failed';
    case 'timed_out':
      return 'TimedOut';
    default:
      return status;
  }
}
