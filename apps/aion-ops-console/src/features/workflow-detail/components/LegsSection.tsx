import type { ChildLeg } from '@/lib/api';
import { shortWorkflowId } from '@/lib/workflowIdentity';
import type { ActivityId, Namespace, RunId } from '@/types';

import { TranscriptList } from '../../transcript/components/TranscriptList';
import { useTranscript } from '../../transcript/hooks/useTranscript';
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

/** Recorded-fixture-friendly presentation of the child legs folded into a run page. */
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
          <article className="rounded-md border border-border bg-surface" key={leg.workflowId}>
            <header className="flex flex-wrap items-center gap-2 border-b border-border px-4 py-3">
              <strong>{leg.displayName ?? leg.workflowType}</strong>
              <code className="text-xs text-muted-foreground">
                {shortWorkflowId(leg.workflowId)}
              </code>
              <span className="ml-auto text-xs font-medium">{leg.status}</span>
              {leg.currentActivityId !== null && leg.currentAttempt !== null ? (
                <span className="text-xs text-muted-foreground">
                  Activity {leg.currentActivityId} · attempt {leg.currentAttempt}
                </span>
              ) : null}
            </header>
            <div className="space-y-3 p-3">
              {leg.runId === null ? (
                <p className="text-xs text-muted-foreground">
                  Child start recorded; run not started yet.
                </p>
              ) : null}
              {leg.attempts.map((attempt) => (
                <AttemptHeading
                  attempt={attempt}
                  key={`${attempt.activityId}:${attempt.attempt}`}
                />
              ))}
            </div>
          </article>
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
    <article className="rounded-md border border-border bg-surface">
      <header className="flex flex-wrap items-center gap-2 border-b border-border px-4 py-3">
        <strong>{leg.displayName ?? leg.workflowType}</strong>
        <code className="text-xs text-muted-foreground">{shortWorkflowId(leg.workflowId)}</code>
        <span className="ml-auto text-xs font-medium">
          {liveStatus(leg.status, live.terminalOutcome)}
        </span>
        {(current ?? fallback) !== null ? (
          <span className="text-xs text-muted-foreground">
            Activity {(current ?? fallback)?.activityId} · attempt {(current ?? fallback)?.attempt}
          </span>
        ) : null}
      </header>
      <div className="space-y-3 p-3">
        {leg.runId === null ? (
          <p className="text-xs text-muted-foreground">
            Child start recorded; run not started yet.
          </p>
        ) : null}
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
      </div>
    </article>
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
  const transcript = useTranscript({
    target:
      runId === null
        ? null
        : {
            namespace,
            workflowId,
            runId,
            activityId: attempt.activityId,
            attempt: attempt.attempt,
          },
  });
  return (
    <div className="rounded border border-border" data-attempt-state={attempt.state}>
      <div className="flex flex-wrap items-center gap-2 border-b border-border px-3 py-2 text-xs">
        <strong>{attempt.activityType ?? `Activity ${attempt.activityId}`}</strong>
        <span>attempt {attempt.attempt}</span>
        <span className="text-muted-foreground">
          {attempt.state === 'superseded'
            ? `Superseded by attempt ${attempt.supersededBy}`
            : attempt.state}
        </span>
        <span className="ml-auto text-muted-foreground">{transcript.status}</span>
      </div>
      {transcript.entries.length === 0 ? (
        <p className="px-3 py-4 text-xs text-muted-foreground">Waiting for transcript events.</p>
      ) : (
        <TranscriptList
          entries={transcript.entries}
          hasEarlier={transcript.hasEarlier}
          isLoadingEarlier={transcript.isLoadingEarlier}
          onLoadEarlier={transcript.loadEarlier}
        />
      )}
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

function liveStatus(status: string, outcome: string | null): string {
  if (outcome === null) return status;
  const labels: Record<string, string> = {
    cancelled: 'Cancelled',
    completed: 'Completed',
    continued_as_new: 'ContinuedAsNew',
    failed: 'Failed',
    timed_out: 'TimedOut',
  };
  return labels[outcome] ?? status;
}
