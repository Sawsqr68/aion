import { useState } from 'react';

import type { AttemptCapabilities } from '@/lib/api';
import type { Namespace, RunId, WorkflowId } from '@/types';
import type { DescribeLiveResponse, HeartbeatNote, TranscriptTail } from '@/types/generated';

import {
  InterventionActions,
  InterventionComposer,
  useInterventionController,
} from '../../transcript/components/InterventionControls';
import { DEFAULT_LIVE_STEP_TAIL, useLiveStep } from '../hooks/useLiveStep';
import type { LiveStepReader, LiveStepTail } from '../lib/live-step-port';

const TAIL_OPTIONS = [20, 50, 100] as const;

type LiveStepCardProps = {
  namespace: Namespace;
  workflowId: WorkflowId;
  runId: RunId | null;
  isTerminal: boolean;
  reader: LiveStepReader;
};

/** Polling container. The operator owns the transcript cost through the explicit tail selector. */
export function LiveStepCard(props: LiveStepCardProps) {
  const [tail, setTail] = useState<LiveStepTail>(DEFAULT_LIVE_STEP_TAIL);
  const liveStep = useLiveStep({ ...props, tail });

  return (
    <section aria-labelledby="live-step-heading" className="border-t border-border px-6 py-5">
      <div className="mb-4 flex flex-wrap items-start justify-between gap-3">
        <div>
          <h2 className="text-sm font-semibold" id="live-step-heading">
            Live step
          </h2>
          <p className="text-xs text-muted-foreground">
            Current activity ownership, worker note, and retained transcript tail.
          </p>
        </div>
        <label className="flex items-center gap-2 text-xs">
          Transcript tail
          <select
            aria-label="Transcript tail size"
            className="rounded border border-border bg-surface px-2 py-1"
            onChange={(event) => setTail(Number(event.target.value))}
            value={tail}
          >
            {TAIL_OPTIONS.map((option) => (
              <option key={option} value={option}>
                {option} events
              </option>
            ))}
          </select>
        </label>
      </div>

      {liveStep.isPending ? (
        <p className="text-xs text-muted-foreground">Reading live step…</p>
      ) : null}
      {liveStep.isError ? (
        <p className="text-xs text-danger" role="alert">
          Could not read live step: {errorMessage(liveStep.error)}
        </p>
      ) : null}
      {liveStep.data !== undefined ? (
        <LiveStepContent {...props} response={liveStep.data} />
      ) : props.isTerminal ? (
        <p className="text-xs text-muted-foreground">
          Run is terminal; live-step polling is stopped.
        </p>
      ) : null}
    </section>
  );
}

// biome-ignore lint/complexity/noExcessiveCognitiveComplexity: typed live-state branches stay visibly distinct for operators.
export function LiveStepContent({
  namespace,
  workflowId,
  runId,
  response,
}: Pick<LiveStepCardProps, 'namespace' | 'workflowId' | 'runId'> & {
  response: DescribeLiveResponse;
}) {
  const current = response.current_step;
  const attempt = current?.attempt ?? null;
  const controlTarget: AttemptCapabilities | null =
    runId !== null && current !== null && attempt?.liveness.liveness === 'Live'
      ? {
          runId,
          activityId: current.step.activity_id,
          attempt: attempt.attempt,
          capabilities: attempt.liveness.capabilities,
        }
      : null;
  const controller = useInterventionController({ namespace, workflowId, attempt: controlTarget });
  const supported =
    attempt?.liveness.liveness === 'Live' ? attempt.liveness.capabilities.supported : [];

  if (current === null) {
    return (
      <div className="rounded-md border border-border bg-surface p-4 text-sm">
        This run is not inside an activity; it may be completed, failed, or blocked on a timer or
        signal.
      </div>
    );
  }

  const { step } = current;
  const attemptNumber =
    attempt?.attempt ?? (step.state.state === 'Dispatched' ? step.state.attempt : null);

  return (
    <div className="space-y-4 rounded-md border border-border bg-surface p-4">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-xs">
          <dt className="text-muted-foreground">Activity</dt>
          <dd>
            <code>{step.activity_id}</code> · {step.activity_type}
          </dd>
          <dt className="text-muted-foreground">Task queue</dt>
          <dd>{step.task_queue}</dd>
          {step.node !== null ? (
            <>
              <dt className="text-muted-foreground">Node</dt>
              <dd>{step.node}</dd>
            </>
          ) : null}
          <dt className="text-muted-foreground">State</dt>
          <dd>{step.state.state}</dd>
          {attemptNumber !== null ? (
            <>
              <dt className="text-muted-foreground">Attempt</dt>
              <dd>{attemptNumber}</dd>
            </>
          ) : null}
        </dl>
        {controller !== null ? <InterventionActions controller={controller} /> : null}
      </div>

      {response.open_steps.length > 1 ? (
        <p className="text-xs text-warning">
          {response.open_steps.length} activities are open; this card shows the most recent one and{' '}
          {response.open_steps.length - 1} sibling{' '}
          {response.open_steps.length === 2 ? 'activity' : 'activities'}.
        </p>
      ) : null}

      {response.unserved.map((item) => (
        <p className="text-xs text-danger" key={`${item.activity_id}:${item.attempt}`} role="alert">
          Unserved activity {item.activity_id} ({item.activity_type}) on {item.task_queue}:{' '}
          {item.reason}. {item.detail}
        </p>
      ))}

      {attempt === null ? (
        <p className="text-xs text-muted-foreground">
          No attempt exists yet, so worker liveness, heartbeat, transcript, and controls are not
          available.
        </p>
      ) : (
        <div className="space-y-3">
          <LivenessText response={response} />
          <HeartbeatText note={attempt.note} />
          <TranscriptText transcript={attempt.transcript} />
          {controller !== null ? (
            <>
              <InterventionComposer controller={controller} />
              {supported.length > 0 ? (
                <p className="text-xs text-muted-foreground">
                  Other intervention controls are withheld because this harness does not advertise
                  those primitives.
                </p>
              ) : null}
            </>
          ) : (
            <p className="text-xs text-muted-foreground">
              Intervention controls unavailable: no live owner is connected.
            </p>
          )}
        </div>
      )}
    </div>
  );
}

function LivenessText({ response }: { response: DescribeLiveResponse }) {
  const liveness = response.current_step?.attempt?.liveness;
  if (liveness === undefined) return null;
  if (liveness.liveness === 'Live') {
    return liveness.capabilities.supported.length === 0 ? (
      <p className="text-xs">
        Live owner connected — observability-only; this harness advertises no intervention controls.
      </p>
    ) : (
      <p className="text-xs">
        Live owner connected. Advertised intervention controls are available below.
      </p>
    );
  }
  return response.unserved.length === 0 ? (
    <p className="text-xs text-warning">
      No live owner — this may be the ordinary hand-off between dispatch and lease.
    </p>
  ) : (
    <p className="text-xs text-danger">
      No live owner — the fleet reports this dispatched activity cannot currently be served.
    </p>
  );
}

function HeartbeatText({ note }: { note: HeartbeatNote }) {
  switch (note.note) {
    case 'Reported':
      return (
        <p className="text-xs">
          Worker heartbeat reported at {note.reported_at}:{' '}
          <code>{JSON.stringify(note.payload)}</code>
        </p>
      );
    case 'NoneSent':
      return (
        <p className="text-xs">
          This process holds the attempt, and the worker has reported no heartbeat note.
        </p>
      );
    case 'Unavailable':
      return note.restarted_since_attempt_began ? (
        <p className="text-xs text-warning">
          Heartbeat note unavailable: this server restarted after the attempt began, so the restart
          explains the missing measurement.
        </p>
      ) : (
        <p className="text-xs text-warning">
          Heartbeat note unavailable: this server has held notes only since {note.notes_held_since},
          so it cannot say whether the worker reported one earlier.
        </p>
      );
  }
}

function TranscriptText({ transcript }: { transcript: TranscriptTail }) {
  if (transcript.transcript === 'NotRequested') {
    return (
      <p className="text-xs text-muted-foreground">
        Transcript tail was not requested; retained stream contents are unknown.
      </p>
    );
  }
  return (
    <div className="text-xs">
      <p>
        {transcript.events.length} retained transcript events in this window;{' '}
        {transcript.omitted_before} earlier retained events omitted.
      </p>
      {transcript.retention_truncated ? (
        <p className="text-warning">
          Retention closed this stream; the retained end does not mean the agent went quiet.
        </p>
      ) : null}
    </div>
  );
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : 'Unknown live-step read failure';
}
