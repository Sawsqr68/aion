import type { Event, RunGeneration, RunId } from '@/types';

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
 * not an attempt, and a transcript cannot be addressed without the run. A
 * `WorkflowStarted` opens a generation and every later event belongs to it until
 * the next one.
 *
 * 🔴 THE BOUNDARIES COME FROM THE SERVER (`DescribeWorkflowResponse.generations`)
 * AND ARE NEVER DERIVED HERE. This module used to scan the loaded history for
 * `WorkflowStarted` and set `runId` to `null` when it found none — honest about
 * the window, and catastrophic in practice: the loaded history is a TAIL window
 * of {@link HISTORY_WINDOW_SIZE} events, so on any workflow longer than one
 * window the start event is never in it and EVERY row rendered "run unknown". A
 * label that admits ignorance on every row teaches an operator to ignore the
 * column, which is worse than either a lie or a silence.
 *
 * The server holds the whole history and states the boundaries outright, so the
 * question "which run is this event in" is always answerable and the `null` case
 * no longer exists. It was deleted rather than hidden: the state that produced
 * the label cannot occur.
 */

/** Terminal state of one `(activityId, attempt)`, derived from its terminal event. */
export type AttemptStatus = 'started' | 'completed' | 'failed' | 'cancelled';

export type AttemptNavigatorRow = {
  /**
   * Durable identity `${runId}:${activityId}:${attempt}`. Survives refresh +
   * post-mortem and is the sole selection key (never an array index).
   */
  key: string;
  /** The generation this attempt ran in. Always known; see the module note. */
  runId: RunId;
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
 * string for two different attempts.
 */
export function attemptRowKey(
  runId: RunId,
  activityId: string | number,
  attempt: number
): string {
  return `${runId}:${activityId}:${attempt}`;
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
  /** The generation this attempt ran in. */
  runId: RunId;
  /** The one-based attempt number (`0` is the legacy sentinel). */
  attempt: number;
};

function emptySlot(runId: RunId, attempt: number): AttemptSlot {
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
export function deriveAttemptNavigator(
  timeline: readonly TimelineEntry[],
  generations: readonly RunGeneration[]
): AttemptNavigatorRow[] {
  const rows: AttemptNavigatorRow[] = [];

  for (const entry of timeline) {
    if (entry.kind === 'activity') {
      rows.push(...activityRows(entry, generations));
    }
  }

  // Generation order, not lexicographic run order: rows of one coordinate from
  // two generations must list oldest-first, the order they ran in.
  const runOrder = new Map(generations.map((generation, index) => [generation.run_id, index]));
  return rows.sort((left, right) => compareRows(left, right, runOrder));
}

/**
 * The generation an event at `seq` belongs to: the LAST boundary at or before
 * it.
 *
 * `generations` is the server's list over the WHOLE history, so a boundary at or
 * before any event this console can hold always exists — the first entry opens
 * the workflow's first event. `undefined` is therefore reachable only for an
 * EMPTY list, which means a workflow with no history and so no rows to place.
 * It is a totality answer, not a "cannot say": the server refuses to project a
 * run chain whose slices do not begin with `WorkflowStarted`, so a list that
 * failed to cover its own events never reaches us.
 */
function generationAt(
  generations: readonly RunGeneration[],
  seq: number
): RunId | undefined {
  let resolved = generations[0]?.run_id;
  for (const generation of generations) {
    if (generation.seq > seq) {
      break;
    }
    resolved = generation.run_id;
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
  generations: readonly RunGeneration[]
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
  generations: readonly RunGeneration[]
): void {
  const attempt = event.data.attempt;
  const runId = generationAt(generations, event.data.envelope.seq);
  if (runId === undefined) {
    // Only reachable for an EMPTY boundary list, which means the server reported
    // a workflow with no history — in which case there is no event to place and
    // this loop does not run. Placing the attempt under a guessed run would be
    // the papering-over the module note refuses, so it is not placed at all.
    return;
  }
  const slotKey = `${runId}\u0000${attempt}`;
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
 * rows of a continue-as-new chain that share a coordinate.
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
    runOrder.get(row.runId) ?? Number.POSITIVE_INFINITY;
  const byGeneration = generationRank(left) - generationRank(right);
  return (
    byActivity || left.attempt - right.attempt || (Number.isNaN(byGeneration) ? 0 : byGeneration)
  );
}
