import type { Event, TimerId } from '@/types';

import type { TimelineEntry } from '../types';
import { eventSequence, projectTimeline } from './timeline';

export type TimelineProjection = {
  events: Event[];
  timeline: TimelineEntry[];
};

export function updateTimelineProjection(
  previous: TimelineProjection | null,
  events: Event[]
): TimelineProjection {
  if (previous === null) {
    return { events, timeline: projectTimeline(events) };
  }
  if (previous.events === events) {
    return previous;
  }

  const changedEvents = sequenceDifference(previous.events, events);
  if (changedEvents.length === 0) {
    return { events, timeline: previous.timeline };
  }
  const affectedIds = new Set(changedEvents.map(timelineEntryId));
  const unaffected = previous.timeline.filter((entry) => !affectedIds.has(entry.id));
  const affectedEvents = new Map<string, Event[]>();
  for (const event of events) {
    const id = timelineEntryId(event);
    if (affectedIds.has(id)) {
      const grouped = affectedEvents.get(id) ?? [];
      grouped.push(event);
      affectedEvents.set(id, grouped);
    }
  }
  const rebuilt = [...affectedEvents.values()]
    .flatMap((group) => projectTimeline(group))
    .sort((left, right) => left.sequence - right.sequence);

  return { events, timeline: mergeTimelineEntries(unaffected, rebuilt) };
}

function sequenceDifference(previous: readonly Event[], next: readonly Event[]): Event[] {
  const changed: Event[] = [];
  let left = 0;
  let right = 0;
  while (left < previous.length || right < next.length) {
    const previousEvent = previous[left];
    const nextEvent = next[right];
    if (previousEvent === undefined) {
      if (nextEvent !== undefined) changed.push(nextEvent);
      right += 1;
    } else if (nextEvent === undefined) {
      changed.push(previousEvent);
      left += 1;
    } else {
      const previousSeq = eventSequence(previousEvent);
      const nextSeq = eventSequence(nextEvent);
      if (previousSeq === nextSeq) {
        if (previousEvent !== nextEvent) {
          changed.push(previousEvent, nextEvent);
        }
        left += 1;
        right += 1;
      } else if (previousSeq < nextSeq) {
        changed.push(previousEvent);
        left += 1;
      } else {
        changed.push(nextEvent);
        right += 1;
      }
    }
  }
  return changed;
}

function mergeTimelineEntries(
  left: readonly TimelineEntry[],
  right: readonly TimelineEntry[]
): TimelineEntry[] {
  const merged: TimelineEntry[] = [];
  let leftIndex = 0;
  let rightIndex = 0;
  while (leftIndex < left.length || rightIndex < right.length) {
    const leftEntry = left[leftIndex];
    const rightEntry = right[rightIndex];
    if (
      rightEntry === undefined ||
      (leftEntry !== undefined && leftEntry.sequence <= rightEntry.sequence)
    ) {
      if (leftEntry !== undefined) merged.push(leftEntry);
      leftIndex += 1;
    } else {
      merged.push(rightEntry);
      rightIndex += 1;
    }
  }
  return merged;
}

function timelineEntryId(event: Event): string {
  const sequence = eventSequence(event);
  switch (event.type) {
    case 'WorkflowStarted':
    case 'WorkflowCompleted':
    case 'WorkflowFailed':
    case 'WorkflowCancelled':
    case 'WorkflowTimedOut':
      return `lifecycle:${event.type}:${sequence}`;
    case 'ActivityScheduled':
    case 'ActivityStarted':
    case 'ActivityCompleted':
    case 'ActivityFailed':
    case 'ActivityCancelled':
      return `activity:${String(event.data.activity_id)}`;
    case 'TimerStarted':
    case 'TimerFired':
    case 'TimerCancelled':
    case 'WithTimeoutCompleted':
      return `timer:${stringifyTimerId(event.data.timer_id)}`;
    case 'SignalReceived':
      return `signal:received:${event.data.name}:${sequence}`;
    case 'SignalSent':
      return `signal:sent:${event.data.name}:${sequence}`;
    case 'ChildWorkflowStarted':
    case 'ChildWorkflowCompleted':
    case 'ChildWorkflowFailed':
    case 'ChildWorkflowCancelled':
      return `child:${event.data.child_workflow_id}`;
    default:
      return `event:${event.type}:${sequence}`;
  }
}

function stringifyTimerId(timerId: TimerId): string {
  return 'Named' in timerId ? timerId.Named : `anonymous-${timerId.Anonymous}`;
}
