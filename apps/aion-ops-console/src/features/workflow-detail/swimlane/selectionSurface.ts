import type { RunGeneration, WorkflowId } from '@/types';

import type { TimelineEntry } from '../types';
import type { SwimlaneSelection } from './Swimlane';

export type SelectionSurface = {
  workflowId: WorkflowId;
  timeline: readonly TimelineEntry[];
  /** The boundaries of `workflowId`'s own chain — never an ancestor's. */
  generations: readonly RunGeneration[];
  entry: TimelineEntry | null;
  clusterEntries: readonly TimelineEntry[];
  scopedSelection: SwimlaneSelection | null;
};

/** Resolve every workflow-id-driven surface from the exact bar click scope. */
export function resolveSelectionSurface(
  rootWorkflowId: WorkflowId,
  rootTimeline: readonly TimelineEntry[],
  rootGenerations: readonly RunGeneration[],
  selectedSequence: number | null,
  scopedSelection: SwimlaneSelection | null
): SelectionSurface {
  const activeScopedSelection =
    scopedSelection?.sequence === selectedSequence ? scopedSelection : null;
  const timeline = activeScopedSelection?.timeline ?? rootTimeline;
  // Resolved from the SAME source as the timeline, in the same breath: a scope
  // that took its events from a child must take its runs from that child too.
  const generations = activeScopedSelection?.generations ?? rootGenerations;
  const workflowId = activeScopedSelection?.workflowId ?? rootWorkflowId;
  const entry = timeline.find((candidate) => candidate.sequence === selectedSequence) ?? null;
  const clusterEntries =
    activeScopedSelection === null
      ? []
      : activeScopedSelection.clusterSequences.flatMap((sequence) => {
          const member = timeline.find((candidate) => candidate.sequence === sequence);
          return member === undefined ? [] : [member];
        });

  return {
    workflowId,
    timeline,
    generations,
    entry,
    clusterEntries,
    scopedSelection: activeScopedSelection,
  };
}
