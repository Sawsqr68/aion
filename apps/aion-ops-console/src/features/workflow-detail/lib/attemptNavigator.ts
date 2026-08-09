import type { Event, RunId } from '@/types';

import type { ActivityTimelineEntry, TimelineEntry } from '../types';

/**
 * The DURABLE attempt navigator (NOI post-mortem fix).
 *
 * The old attempts panel enumerated only the LIVE intervenable attempts
 * (`POST /workflows/attempts`), which returns empty for a terminal run — so a
 * failed/finished workflow's attempts went blank and their (durable) transcripts
 * were unreachable. This module instead derives the selectable attempt list from
 * the projected timeline (durable history + live), so every attempt a run ever ran
 * stays selectable after the run ends.
 *
 * Each row is one `(activityId, attempt)` — the STABLE identity across the whole
 * lifecycle (`ActivityStarted/Completed/Failed/Cancelled` all carry the same
 * one-based `attempt`). Rows and the current selection are keyed by that durable
 * identity, NEVER by array index (indexes shift as live events extend the
 * timeline; the durable key survives refresh and post-mortem). `attempt: 0` is the
 * legacy/unknown sentinel (never a real attempt) — it is labelled distinctly and
 * remains selectable.
 *
 * # The generation each row belongs to
 *
 * One workflow id spans a whole continue-as-new chain, and ordinals + attempts
 * restart in every generation — so `(activityId, attempt)` names a coordinate,
 * not an attempt, and a transcript cannot be addressed without the run. The run
 * is recovered from the history itself: a `WorkflowStarted` opens a generation
 * and every later event belongs to it until the next one. The loaded history is
 * a WINDOW (the tail), so a row can genuinely precede any loaded
 * `WorkflowStarted`; that row's `runId` is `null` — the honest "this window
 * cannot say which generation" — and the caller must not address a transcript
 * with it. `null` is never papered over with a nearby run.
 */

/** Terminal state of one `(activityId, attempt)`, derived from its terminal event. */
export type AttemptStatus = 'started' | 'completed' | 'failed' | 'cancelled';

export type AttemptNavigatorRow = {
  /**
   * Durable identity `${runId}:${activityId}:${attempt}` (the run is `-` when the
   * loaded window cannot name it). Survives refresh + post-mortem and is the sole
   * selection key (never an array index).
   */
  key: string;
  /**
   * The generation this attempt ran in, or `null` when no `WorkflowStarted` for
   * it is inside the loaded history window. A `null` row is NOT addressable as a
   * transcript stream.
   */
  runId: RunId | null;
  activityId: string;
  /** Human activity type when the schedule is known; otherwise `null`. */
  activityType: string | null;
  /** One-based attempt number; `0` is the legacy/unknown sentinel. */
  attempt: number;
  /** True for the `attempt: 0` legacy sentinel (still selectable, labelled apart). */
  isLegacy: boolean;
  status: AttemptStatus;
};

/**
 * The one true selection key for one attempt. The run leads the key because it
 * leads the identity: two generations of a chain otherwise produce the same
 * string for two different attempts. An unresolvable run is written `-`, which
 * cannot collide with a UUID.
 */
export function attemptRowKey(
  runId: RunId | null,
  activityId: string | number,
  attempt: number
): string {
  return `${runId ?? '-'}:${activityId}:${attempt}`;
}

/** The attempt-bearing terminal/start events for one activity (all carry `attempt`). */
type AttemptEventType =
  | 'ActivityStarted'
  | 'ActivityCompleted'
  | 'ActivityFailed'
  | 'ActivityCancelled';

const ATTEMPT_EVENT_TYPES = new Set<Event['type']>([
  'ActivityStarted',
  'ActivityCompleted',
  'ActivityFailed',
  'ActivityCancelled',
]);

function isAttemptEvent(event: Event): event is Extract<Event, { type: AttemptEventType }> {
  return ATTEMPT_EVENT_TYPES.has(event.type);
}

/** Accumulated presence of each terminal for one attempt (order-independent). */
type AttemptSlot = {
  started: boolean;
  completed: boolean;
  failed: boolean;
  cancelled: boolean;
  /** The generation this attempt ran in; `null` when the window cannot name it. */
  runId: RunId | null;
  /** The one-based attempt number (`0` is the legacy sentinel). */
  attempt: number;
};

function emptySlot(runId: RunId | null, attempt: number): AttemptSlot {
  return {
    started: false,
    completed: false,
    failed: false,
    cancelled: false,
    runId,
    attempt,
  };
}

/**
 * A completed/cancelled attempt takes precedence over a failure, which takes
 * precedence over a bare start. (An attempt has exactly one real terminal, but we
 * resolve defensively so a malformed group never yields an ambiguous status.)
 */
function slotStatus(slot: AttemptSlot): AttemptStatus {
  if (slot.completed) {
    return 'completed';
  }
  if (slot.cancelled) {
    return 'cancelled';
  }
  if (slot.failed) {
    return 'failed';
  }
  return 'started';
}

/**
 * Group every activity's attempt-bearing events by their one-based `attempt` and
 * emit one navigator row per `(activityId, attempt)`, ordered by activity then
 * attempt. Scheduled-only activities (no attempt has run yet) yield no rows —
 * there is no transcript to observe until an attempt starts.
 */
export function deriveAttemptNavigator(timeline: readonly TimelineEntry[]): AttemptNavigatorRow[] {
  const generations = deriveGenerations(timeline);
  const rows: AttemptNavigatorRow[] = [];

  for (const entry of timeline) {
    if (entry.kind === 'activity') {
      rows.push(...activityRows(entry, generations));
    }
  }

  // Generation order, not lexicographic run order: rows of one coordinate from
  // two generations must list oldest-first, the order they ran in.
  const runOrder = new Map(generations.map((generation, index) => [generation.runId, index]));
  return rows.sort((left, right) => compareRows(left, right, runOrder));
}

/**
 * One generation boundary: the sequence at which a `WorkflowStarted` opened
 * `runId`. Ascending by `seq`.
 */
type Generation = { seq: number; runId: RunId };

/**
 * Collect the generation boundaries present in the loaded window, ascending.
 *
 * ONLY `WorkflowStarted` opens a generation. `WorkflowReopened` also carries a
 * `run_id`, but it names the run it REOPENS — it does not begin a new one — so
 * treating it as a boundary would be a second, redundant statement of the same
 * fact and would misread a reopen that arrives out of order. The one authority
 * is the start event.
 */
export function deriveGenerations(timeline: readonly TimelineEntry[]): Generation[] {
  const generations: Generation[] = [];
  for (const entry of timeline) {
    for (const event of entry.events) {
      if (event.type === 'WorkflowStarted') {
        generations.push({ seq: event.data.envelope.seq, runId: event.data.run_id });
      }
    }
  }
  return generations.sort((left, right) => left.seq - right.seq);
}

/**
 * The generation an event at `seq` belongs to: the LAST `WorkflowStarted` at or
 * before it. `null` when the window holds no start at or before `seq` — the
 * honest answer for a tail window that begins mid-generation.
 */
function generationAt(generations: readonly Generation[], seq: number): RunId | null {
  let resolved: RunId | null = null;
  for (const generation of generations) {
    if (generation.seq > seq) {
      break;
    }
    resolved = generation.runId;
  }
  return resolved;
}

/**
 * Group one activity's attempt-bearing events into navigator rows, keyed by
 * `(run, attempt)`.
 *
 * The grouping key MUST carry the run. The timeline projection groups an
 * activity's events by activity id across the WHOLE history, and a
 * continue-as-new chain restarts both ordinals and attempts — so grouping by
 * `attempt` alone fuses generation two's activity-0 attempt-1 onto generation
 * one's, producing a single row (and a single transcript target) for two
 * different attempts. That is the same fusion the durable keyspace grew a run
 * axis to stop, one layer up.
 */
function activityRows(
  entry: ActivityTimelineEntry,
  generations: readonly Generation[]
): AttemptNavigatorRow[] {
  const slots = new Map<string, AttemptSlot>();

  for (const event of entry.events) {
    if (isAttemptEvent(event)) {
      applyAttemptEvent(slots, event, generations);
    }
  }

  return [...slots.values()].map((slot) => ({
    key: attemptRowKey(slot.runId, entry.activityId, slot.attempt),
    runId: slot.runId,
    activityId: entry.activityId,
    activityType: entry.activityType,
    attempt: slot.attempt,
    isLegacy: slot.attempt === 0,
    status: slotStatus(slot),
  }));
}

/**
 * Record one attempt-bearing event's terminal presence in its `(run, attempt)`
 * slot. The run is resolved from THIS event's own sequence, so every event lands
 * in the generation it was actually recorded in.
 */
function applyAttemptEvent(
  slots: Map<string, AttemptSlot>,
  event: Extract<Event, { type: AttemptEventType }>,
  generations: readonly Generation[]
): void {
  const attempt = event.data.attempt;
  const runId = generationAt(generations, event.data.envelope.seq);
  const slotKey = `${runId ?? '-'}\u0000${attempt}`;
  const slot = slots.get(slotKey) ?? emptySlot(runId, attempt);
  if (event.type === 'ActivityStarted') {
    slot.started = true;
  } else if (event.type === 'ActivityCompleted') {
    slot.completed = true;
  } else if (event.type === 'ActivityFailed') {
    slot.failed = true;
  } else {
    slot.cancelled = true;
  }
  slots.set(slotKey, slot);
}

/**
 * Ascending by activity id (numeric when possible), then attempt, then
 * GENERATION order — the list order. The generation tiebreak only ever fires for
 * rows of a continue-as-new chain that share a coordinate; a run the window
 * cannot name sorts last, since it cannot be placed among the known generations.
 */
function compareRows(
  left: AttemptNavigatorRow,
  right: AttemptNavigatorRow,
  runOrder: ReadonlyMap<RunId, number>
): number {
  const leftId = Number(left.activityId);
  const rightId = Number(right.activityId);
  const bothNumeric = !Number.isNaN(leftId) && !Number.isNaN(rightId);
  const byActivity = bothNumeric
    ? leftId - rightId
    : left.activityId.localeCompare(right.activityId);
  const generationRank = (row: AttemptNavigatorRow): number =>
    row.runId === null
      ? Number.POSITIVE_INFINITY
      : (runOrder.get(row.runId) ?? Number.POSITIVE_INFINITY);
  const byGeneration = generationRank(left) - generationRank(right);
  return (
    byActivity || left.attempt - right.attempt || (Number.isNaN(byGeneration) ? 0 : byGeneration)
  );
}
