import type { TimelineEntry } from '../types';
import { layoutSwimlane, type SwimlaneLane } from './laneLayout';
import type { VisibleWorkflow } from './timeLayout';

export type ChildTimelineState =
  | { status: 'loading'; entries: readonly TimelineEntry[]; isRunning: true }
  | { status: 'error'; entries: readonly TimelineEntry[]; isRunning: false; message: string }
  | { status: 'ready'; entries: readonly TimelineEntry[]; isRunning: boolean };

export type WorkflowTreeNode = {
  path: string;
  workflowId: string;
  ancestry: readonly string[];
  depth: number;
  entries: readonly TimelineEntry[];
  isRunning: boolean;
};

export type LaneTreeRow =
  | {
      kind: 'lane';
      id: string;
      path: string;
      workflowId: string;
      depth: number;
      lane: SwimlaneLane;
      /** Non-null on the timer aggregate, or its first expanded member. */
      timerGroupExpanded: boolean | null;
    }
  | {
      kind: 'notice';
      id: string;
      path: string;
      workflowId: string;
      depth: number;
      notice: 'loading' | 'error' | 'empty' | 'cycle';
      message: string;
    };

export type ChildTimelineRequest = {
  path: string;
  workflowId: string;
  ancestry: readonly string[];
};

export type LaneTree = {
  rows: LaneTreeRow[];
  workflows: VisibleWorkflow[];
  requests: ChildTimelineRequest[];
};

export function childNodePath(parentPath: string, childWorkflowId: string): string {
  return `${parentPath}>${childWorkflowId}`;
}

export function timerGroupPath(workflowPath: string): string {
  return `${workflowPath}:timer-group`;
}

/** An id already in the branch's ancestry must never be fetched or recursed into. */
export function isWorkflowCycle(workflowId: string, ancestry: readonly string[]): boolean {
  return ancestry.includes(workflowId);
}

/** Every notice row is keyed `<path>:<notice>`, which is unique per branch position. */
function noticeRow(
  path: string,
  workflowId: string,
  depth: number,
  notice: 'loading' | 'error' | 'empty' | 'cycle',
  message: string
): LaneTreeRow {
  return { kind: 'notice', id: `${path}:${notice}`, path, workflowId, depth, notice, message };
}

/**
 * A child branch is drawn when it is not explicitly collapsed and is either
 * expanded by the operator or auto-expanded by id on first render.
 */
function isChildExpanded(
  childPath: string,
  childWorkflowId: string,
  expandedPaths: ReadonlySet<string>,
  initiallyExpandedChildIds: ReadonlySet<string>,
  collapsedPaths: ReadonlySet<string>
): boolean {
  if (collapsedPaths.has(childPath)) {
    return false;
  }
  return expandedPaths.has(childPath) || initiallyExpandedChildIds.has(childWorkflowId);
}

/** Either the branch is ready to recurse into, or it stands in as a single notice row. */
type ChildResolution =
  | { kind: 'notice'; row: LaneTreeRow }
  | { kind: 'ready'; entries: readonly TimelineEntry[]; isRunning: boolean };

/** An unfetched child reads as loading — the request for it is issued in the same pass. */
function resolveChildState(
  child: ChildTimelineState | undefined,
  childPath: string,
  childWorkflowId: string,
  depth: number
): ChildResolution {
  if (child === undefined || child.status === 'loading') {
    return {
      kind: 'notice',
      row: noticeRow(
        childPath,
        childWorkflowId,
        depth,
        'loading',
        `Loading ${childWorkflowId} history…`
      ),
    };
  }
  if (child.status === 'error') {
    return {
      kind: 'notice',
      row: noticeRow(childPath, childWorkflowId, depth, 'error', child.message),
    };
  }
  return { kind: 'ready', entries: child.entries, isRunning: child.isRunning };
}

/**
 * Flatten expanded workflow lanes into depth-indented rows. The tree only decides
 * row order; every row still uses the one shared track origin and axis in the view.
 */
export function flattenLaneTree(
  root: Omit<WorkflowTreeNode, 'path' | 'ancestry' | 'depth'>,
  children: ReadonlyMap<string, ChildTimelineState>,
  expandedPaths: ReadonlySet<string>,
  initiallyExpandedChildIds: ReadonlySet<string> = new Set(),
  collapsedPaths: ReadonlySet<string> = new Set()
): LaneTree {
  const rows: LaneTreeRow[] = [];
  const workflows: VisibleWorkflow[] = [];
  const requests: ChildTimelineRequest[] = [];

  visit({ ...root, path: root.workflowId, ancestry: [], depth: 0 });

  return { rows, workflows, requests };

  function visit(node: WorkflowTreeNode): void {
    workflows.push({
      workflowId: node.workflowId,
      entries: node.entries,
      isRunning: node.isRunning,
    });
    const timersExpanded = expandedPaths.has(timerGroupPath(node.path));
    const layout = layoutSwimlane(node.entries, { expandTimers: timersExpanded });

    if (layout.lanes.length === 0 && node.depth > 0) {
      rows.push(
        noticeRow(
          node.path,
          node.workflowId,
          node.depth,
          'empty',
          'No correlated child events yet.'
        )
      );
      return;
    }

    // Only the first timer lane carries the group toggle; the rest render bare.
    let hasTimerGroupToggle = false;
    for (const lane of layout.lanes) {
      const carriesTimerToggle = lane.kind === 'timer' && !hasTimerGroupToggle;
      rows.push({
        kind: 'lane',
        id: `${node.path}:${lane.id}`,
        path: node.path,
        workflowId: node.workflowId,
        depth: node.depth,
        lane,
        timerGroupExpanded: carriesTimerToggle ? timersExpanded : null,
      });
      if (carriesTimerToggle) {
        hasTimerGroupToggle = true;
      }
      visitChildLane(node, lane.childWorkflowId);
    }
  }

  /** Descend into (or post a notice for) the child workflow a lane correlates to. */
  function visitChildLane(node: WorkflowTreeNode, childWorkflowId: string | null): void {
    if (childWorkflowId === null) {
      return;
    }
    const childPath = childNodePath(node.path, childWorkflowId);
    const expanded = isChildExpanded(
      childPath,
      childWorkflowId,
      expandedPaths,
      initiallyExpandedChildIds,
      collapsedPaths
    );
    if (!expanded) {
      return;
    }

    const ancestry = [...node.ancestry, node.workflowId];
    const depth = node.depth + 1;
    if (isWorkflowCycle(childWorkflowId, ancestry)) {
      rows.push(
        noticeRow(
          childPath,
          childWorkflowId,
          depth,
          'cycle',
          `Workflow ${childWorkflowId} is already expanded above.`
        )
      );
      return;
    }

    requests.push({ path: childPath, workflowId: childWorkflowId, ancestry });
    const resolved = resolveChildState(children.get(childPath), childPath, childWorkflowId, depth);
    if (resolved.kind === 'notice') {
      rows.push(resolved.row);
      return;
    }

    visit({
      path: childPath,
      workflowId: childWorkflowId,
      ancestry,
      depth,
      entries: resolved.entries,
      isRunning: resolved.isRunning,
    });
  }
}
