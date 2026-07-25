import type { ChildTimelineState } from './laneTree';

/**
 * Inactive child projections are individually history-windowed but still sizable.
 * Keep a small navigation tail while always retaining every currently visible path.
 */
export const CHILD_TIMELINE_RETENTION_SIZE = 32;

/** Add or refresh one path, then enforce the inactive-path retention window. */
export function mergeChildTimelineWindow(
  current: ReadonlyMap<string, ChildTimelineState>,
  path: string,
  state: ChildTimelineState,
  visiblePaths: ReadonlySet<string>
): ReadonlyMap<string, ChildTimelineState> {
  if (current.get(path) === state) {
    return retainChildTimelineWindow(current, visiblePaths);
  }

  const next = new Map(current);
  // Map insertion order is the recency index used when inactive paths are trimmed.
  next.delete(path);
  next.set(path, state);
  return retainChildTimelineWindow(next, visiblePaths);
}

/**
 * Retain all visible paths plus the newest inactive paths. Return the original map
 * when it already satisfies the policy so unchanged child frames are React no-ops.
 */
export function retainChildTimelineWindow(
  current: ReadonlyMap<string, ChildTimelineState>,
  visiblePaths: ReadonlySet<string>
): ReadonlyMap<string, ChildTimelineState> {
  let inactiveCount = 0;
  for (const path of current.keys()) {
    if (!visiblePaths.has(path)) {
      inactiveCount += 1;
    }
  }

  let removeCount = inactiveCount - CHILD_TIMELINE_RETENTION_SIZE;
  if (removeCount <= 0) {
    return current;
  }

  const next = new Map(current);
  for (const path of current.keys()) {
    if (removeCount === 0) {
      break;
    }
    if (!visiblePaths.has(path)) {
      next.delete(path);
      removeCount -= 1;
    }
  }
  return next;
}

/** Collapse discards the exact branch because its loaders and live sockets unmount. */
export function withoutChildTimelineBranch(
  current: ReadonlyMap<string, ChildTimelineState>,
  path: string
): ReadonlyMap<string, ChildTimelineState> {
  let next: Map<string, ChildTimelineState> | undefined;
  for (const candidate of current.keys()) {
    if (candidate === path || candidate.startsWith(`${path}>`)) {
      next ??= new Map(current);
      next.delete(candidate);
    }
  }
  return next ?? current;
}
