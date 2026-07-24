import type { Event, EventEnvelope } from '@/types';

import type { LifecycleOutcome } from '../types';
import { lifecycleOutcome, type TerminalLifecycleType } from './lifecycle';

export function eventSequence(event: Event): number {
  return event.data.envelope.seq;
}

export function eventRecordedAt(event: Event): string {
  return event.data.envelope.recorded_at;
}

export function eventEnvelope(event: Event): EventEnvelope {
  return event.data.envelope;
}

export function mergeEventsBySequence(history: readonly Event[], live: readonly Event[]): Event[] {
  const eventsBySequence = new Map<number, Event>();

  for (const event of [...history, ...live]) {
    const sequence = eventSequence(event);

    if (!eventsBySequence.has(sequence)) {
      eventsBySequence.set(sequence, event);
    }
  }

  return [...eventsBySequence.values()].sort(
    (left, right) => eventSequence(left) - eventSequence(right)
  );
}

/** Insert a small arriving batch into an already sorted, fixed-size sequence window. */
export function mergeEventWindowBySequence(
  current: readonly Event[],
  arriving: readonly Event[],
  limit: number
): Event[] {
  let next = [...current];
  let changed = false;

  for (const event of arriving) {
    const sequence = eventSequence(event);
    let low = 0;
    let high = next.length;
    while (low < high) {
      const middle = (low + high) >>> 1;
      const middleEvent = next[middle];
      if (middleEvent !== undefined && eventSequence(middleEvent) < sequence) {
        low = middle + 1;
      } else {
        high = middle;
      }
    }
    const existing = next[low];
    if (existing !== undefined && eventSequence(existing) === sequence) {
      continue;
    }
    next.splice(low, 0, event);
    changed = true;
  }

  if (next.length > limit) {
    next = next.slice(-limit);
    changed = true;
  }
  return changed ? next : (current as Event[]);
}

/** Preserve the public contract for unordered and overlapping event collections. */
export function terminalOutcomeForEvents(events: readonly Event[]): LifecycleOutcome | null {
  return terminalOutcomeForMergedEvents(mergeEventsBySequence(events, []));
}

/** Derive terminal state from an already sequence-sorted, deduplicated event array. */
export function terminalOutcomeForMergedEvents(
  mergedEvents: readonly Event[]
): LifecycleOutcome | null {
  // A later `WorkflowReopened` SUPERSEDES an earlier terminal event (AD-012): the
  // run is live again, so the projection must not keep reporting it terminal —
  // stale-terminal here freezes the status badge AND disables the live event
  // subscription while the reopened run is actually executing.
  let lifecycleEvent: TerminalOrReopenedEvent | undefined;
  for (let index = mergedEvents.length - 1; index >= 0; index -= 1) {
    const event = mergedEvents[index];
    if (
      event !== undefined &&
      (isTerminalWorkflowEvent(event) || event.type === 'WorkflowReopened')
    ) {
      lifecycleEvent = event;
      break;
    }
  }

  if (lifecycleEvent === undefined || lifecycleEvent.type === 'WorkflowReopened') {
    return null;
  }
  return lifecycleOutcome(lifecycleEvent);
}

type TerminalOrReopenedEvent =
  | Extract<Event, { type: TerminalLifecycleType }>
  | Extract<Event, { type: 'WorkflowReopened' }>;

export function isTerminalWorkflowEvent(
  event: Event
): event is Extract<Event, { type: TerminalLifecycleType }> {
  return (
    event.type === 'WorkflowCompleted' ||
    event.type === 'WorkflowFailed' ||
    event.type === 'WorkflowCancelled' ||
    event.type === 'WorkflowTimedOut'
  );
}
