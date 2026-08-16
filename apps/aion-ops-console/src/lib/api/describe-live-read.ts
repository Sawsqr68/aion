import type {
  ActivityEvent,
  InterventionCapabilities,
  InterventionPrimitive,
  Namespace,
  Payload,
  RunId,
  WorkflowId,
} from '@/types';
import type {
  AttemptLiveness,
  CurrentStep,
  DescribeLiveResponse,
  HeartbeatNote,
  LiveAttempt,
  OpenStep,
  StepState,
  TranscriptStreamHead,
  TranscriptTail,
  UnservedActivity,
} from '@/types/generated';

import { ApiError } from './api-error';
import { apiErrorFromResponse, normalizeDescribeSummary, readJson } from './client-normalize';
import {
  type ApiCredentials,
  buildScopedHeaders,
  buildUrl,
  stripTrailingSlash,
} from './client-transport';

export const DESCRIBE_LIVE_ENDPOINT = '/workflows/describe-live';

type FetchFn = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;

export type DescribeLiveReadOptions = {
  baseUrl?: string;
  fetchImpl?: FetchFn;
  credentials?: ApiCredentials;
};

/** Typed reader for the server's one-call projection of what a run is doing now. */
export class DescribeLiveReadClient {
  private readonly baseUrl: string;
  private readonly fetchImpl: FetchFn;
  private readonly credentials: ApiCredentials | undefined;

  constructor(options: DescribeLiveReadOptions = {}) {
    this.baseUrl = stripTrailingSlash(options.baseUrl ?? '');
    this.fetchImpl = options.fetchImpl ?? ((input, init) => globalThis.fetch(input, init));
    this.credentials = options.credentials;
  }

  async describeLive(
    namespace: Namespace,
    workflowId: WorkflowId,
    runId: RunId | null,
    tail: number | undefined,
    signal?: AbortSignal
  ): Promise<DescribeLiveResponse> {
    const response = await this.fetchImpl(buildUrl(this.baseUrl, DESCRIBE_LIVE_ENDPOINT), {
      method: 'POST',
      headers: buildScopedHeaders(this.credentials),
      body: JSON.stringify({
        namespace,
        workflow_id: workflowId,
        run_id: runId,
        // Omission is semantic: it asks the server not to read a transcript tail.
        ...(tail === undefined ? {} : { tail }),
      }),
      ...(signal === undefined ? {} : { signal }),
    });

    if (!response.ok) {
      const errorBody = await readJson(response).catch(() => null);
      throw apiErrorFromResponse(response.status, errorBody);
    }

    return readDescribeLiveResponse(await readJson(response));
  }
}

function readDescribeLiveResponse(value: unknown): DescribeLiveResponse {
  const body = requireRecord(value, 'response was not an object');
  if (!Object.hasOwn(body, 'summary')) {
    throw malformed('response missing summary');
  }
  if (!Object.hasOwn(body, 'current_step')) {
    throw malformed('response missing current_step');
  }

  const openSteps = requireArray(body.open_steps, 'open_steps').map(readOpenStep);
  const transcriptStreams = requireArray(body.transcript_streams, 'transcript_streams').map(
    readTranscriptStreamHead
  );
  const unserved = requireArray(body.unserved, 'unserved').map(readUnservedActivity);

  return {
    summary: normalizeDescribeSummary(body),
    current_step: body.current_step === null ? null : readCurrentStep(body.current_step),
    open_steps: openSteps,
    transcript_streams: transcriptStreams,
    unserved,
  };
}

function readCurrentStep(value: unknown): CurrentStep {
  const current = requireRecord(value, 'current_step was not an object');
  if (!Object.hasOwn(current, 'attempt')) {
    throw malformed('current_step missing attempt');
  }
  return {
    step: readOpenStep(current.step),
    attempt: current.attempt === null ? null : readLiveAttempt(current.attempt),
  };
}

function readOpenStep(value: unknown): OpenStep {
  const step = requireRecord(value, 'open step was not an object');
  return {
    activity_id: requireSequence(step.activity_id, 'open step activity_id'),
    activity_type: requireString(step.activity_type, 'open step activity_type'),
    task_queue: requireString(step.task_queue, 'open step task_queue'),
    node: requireNullableString(step.node, 'open step node'),
    scheduled_at: requireString(step.scheduled_at, 'open step scheduled_at'),
    state: readStepState(step.state),
  };
}

function readStepState(value: unknown): StepState {
  const state = requireRecord(value, 'step state was not an object');
  switch (state.state) {
    case 'Scheduled':
      return { state: 'Scheduled' };
    case 'Dispatched':
      return {
        state: 'Dispatched',
        attempt: requireSequence(state.attempt, 'Dispatched attempt'),
        dispatched_at: requireString(state.dispatched_at, 'Dispatched dispatched_at'),
      };
    case 'Reopened':
      return {
        state: 'Reopened',
        reopened_at: requireString(state.reopened_at, 'Reopened reopened_at'),
      };
    default:
      throw unknownTag('state', state.state);
  }
}

function readLiveAttempt(value: unknown): LiveAttempt {
  const attempt = requireRecord(value, 'live attempt was not an object');
  return {
    attempt: requireSequence(attempt.attempt, 'live attempt attempt'),
    dispatched_at: requireString(attempt.dispatched_at, 'live attempt dispatched_at'),
    liveness: readAttemptLiveness(attempt.liveness),
    note: readHeartbeatNote(attempt.note),
    transcript: readTranscriptTail(attempt.transcript),
  };
}

function readAttemptLiveness(value: unknown): AttemptLiveness {
  const liveness = requireRecord(value, 'liveness was not an object');
  switch (liveness.liveness) {
    case 'Live':
      return { liveness: 'Live', capabilities: readCapabilities(liveness.capabilities) };
    case 'NoLiveOwner':
      return { liveness: 'NoLiveOwner' };
    default:
      throw unknownTag('liveness', liveness.liveness);
  }
}

function readCapabilities(value: unknown): InterventionCapabilities {
  const capabilities = requireRecord(value, 'capabilities was not an object');
  return {
    supported: requireArray(capabilities.supported, 'capabilities.supported').map(
      readInterventionPrimitive
    ),
  };
}

function readInterventionPrimitive(value: unknown): InterventionPrimitive {
  const primitive = requireRecord(value, 'intervention primitive was not an object');
  switch (primitive.primitive) {
    case 'InjectMessage':
    case 'Cancel':
    case 'PauseResume':
    case 'UpdateBudget':
    case 'RespondToApproval':
      return { primitive: primitive.primitive };
    default:
      throw unknownTag('primitive', primitive.primitive);
  }
}

function readHeartbeatNote(value: unknown): HeartbeatNote {
  const note = requireRecord(value, 'heartbeat note was not an object');
  switch (note.note) {
    case 'Reported':
      return {
        note: 'Reported',
        payload: readPayload(note.payload),
        reported_at: requireString(note.reported_at, 'Reported reported_at'),
      };
    case 'NoneSent':
      return { note: 'NoneSent' };
    case 'Unavailable':
      return {
        note: 'Unavailable',
        notes_held_since: requireString(note.notes_held_since, 'Unavailable notes_held_since'),
        restarted_since_attempt_began: requireBoolean(
          note.restarted_since_attempt_began,
          'Unavailable restarted_since_attempt_began'
        ),
      };
    default:
      throw unknownTag('note', note.note);
  }
}

function readPayload(value: unknown): Payload {
  const payload = requireRecord(value, 'Reported payload was not an object');
  if (payload.content_type !== 'Json') {
    throw malformed('Reported payload has invalid content_type');
  }
  const bytes = payload.bytes;
  if (Array.isArray(bytes)) {
    return {
      content_type: 'Json',
      bytes: bytes.map((byte) => requireByte(byte, 'Reported payload byte')),
    };
  }
  const elision = requireRecord(bytes, 'Reported payload bytes were invalid');
  if (elision.__elided !== true) {
    throw malformed('Reported payload elision marker was invalid');
  }
  return {
    content_type: 'Json',
    bytes: {
      __elided: true,
      size_bytes: requireSequence(elision.size_bytes, 'Reported payload size_bytes'),
    },
  };
}

function readTranscriptTail(value: unknown): TranscriptTail {
  const transcript = requireRecord(value, 'transcript was not an object');
  switch (transcript.transcript) {
    case 'NotRequested':
      return { transcript: 'NotRequested' };
    case 'Window':
      return {
        transcript: 'Window',
        events: requireArray(transcript.events, 'transcript events').map(readActivityEvent),
        head_seq: requireSequence(transcript.head_seq, 'transcript head_seq'),
        omitted_before: requireSequence(transcript.omitted_before, 'transcript omitted_before'),
        retention_truncated: requireBoolean(
          transcript.retention_truncated,
          'transcript retention_truncated'
        ),
      };
    default:
      throw unknownTag('transcript', transcript.transcript);
  }
}

function readActivityEvent(value: unknown): ActivityEvent {
  return requireRecord(value, 'transcript event was not an object') as ActivityEvent;
}

function readTranscriptStreamHead(value: unknown): TranscriptStreamHead {
  const stream = requireRecord(value, 'transcript stream was not an object');
  return {
    activity_id: requireSequence(stream.activity_id, 'transcript stream activity_id'),
    attempt: requireSequence(stream.attempt, 'transcript stream attempt'),
    activity_type: requireNullableString(stream.activity_type, 'transcript stream activity_type'),
    head_seq: requireSequence(stream.head_seq, 'transcript stream head_seq'),
    current: requireBoolean(stream.current, 'transcript stream current'),
    retention_truncated: requireBoolean(
      stream.retention_truncated,
      'transcript stream retention_truncated'
    ),
  };
}

function readUnservedActivity(value: unknown): UnservedActivity {
  const activity = requireRecord(value, 'unserved activity was not an object');
  return {
    activity_id: requireSequence(activity.activity_id, 'unserved activity activity_id'),
    activity_type: requireString(activity.activity_type, 'unserved activity activity_type'),
    task_queue: requireString(activity.task_queue, 'unserved activity task_queue'),
    node: requireNullableString(activity.node, 'unserved activity node'),
    attempt: requireSequence(activity.attempt, 'unserved activity attempt'),
    dispatched_at: requireString(activity.dispatched_at, 'unserved activity dispatched_at'),
    reason: requireString(activity.reason, 'unserved activity reason'),
    detail: requireString(activity.detail, 'unserved activity detail'),
    workers_in_pool: requireSequence(activity.workers_in_pool, 'unserved workers_in_pool'),
    workers_serving_activity: requireSequence(
      activity.workers_serving_activity,
      'unserved workers_serving_activity'
    ),
    compatible_workers: requireSequence(activity.compatible_workers, 'unserved compatible_workers'),
    dispatch_parked: requireBoolean(activity.dispatch_parked, 'unserved dispatch_parked'),
  };
}

function requireRecord(value: unknown, detail: string): Record<string, unknown> {
  if (typeof value === 'object' && value !== null && !Array.isArray(value)) {
    return value as Record<string, unknown>;
  }
  throw malformed(detail);
}

function requireArray(value: unknown, field: string): unknown[] {
  if (Array.isArray(value)) return value;
  throw malformed(`${field} was not an array`);
}

function requireString(value: unknown, field: string): string {
  if (typeof value === 'string') return value;
  throw malformed(`${field} was not a string`);
}

function requireNullableString(value: unknown, field: string): string | null {
  if (value === null) return null;
  return requireString(value, field);
}

function requireBoolean(value: unknown, field: string): boolean {
  if (typeof value === 'boolean') return value;
  throw malformed(`${field} was not a boolean`);
}

function requireSequence(value: unknown, field: string): number {
  if (typeof value === 'number' && Number.isSafeInteger(value) && value >= 0) return value;
  throw malformed(`${field} was not a non-negative integer`);
}

function requireByte(value: unknown, field: string): number {
  const byte = requireSequence(value, field);
  if (byte <= 255) return byte;
  throw malformed(`${field} was outside the byte range`);
}

function unknownTag(field: string, value: unknown): ApiError {
  return malformed(`${field} has unknown tag ${String(value)}`);
}

function malformed(detail: string): ApiError {
  return new ApiError(200, `workflows/describe-live ${detail}`);
}
