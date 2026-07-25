import { expect, test } from 'bun:test';

import {
  CHILD_TIMELINE_RETENTION_SIZE,
  mergeChildTimelineWindow,
  retainChildTimelineWindow,
  withoutChildTimelineBranch,
} from './childTimelineWindow';
import type { ChildTimelineState } from './laneTree';

const READY: ChildTimelineState = { status: 'ready', entries: [], isRunning: false };

test('child timeline retention keeps visible paths and only the newest inactive window', () => {
  const visiblePath = 'root>visible';
  const current = new Map<string, ChildTimelineState>([[visiblePath, READY]]);
  for (let index = 0; index < CHILD_TIMELINE_RETENTION_SIZE + 2; index += 1) {
    current.set(`root>inactive-${index}`, READY);
  }

  const retained = retainChildTimelineWindow(current, new Set([visiblePath]));

  expect(retained.size).toBe(CHILD_TIMELINE_RETENTION_SIZE + 1);
  expect(retained.has(visiblePath)).toBe(true);
  expect(retained.has('root>inactive-0')).toBe(false);
  expect(retained.has(`root>inactive-${CHILD_TIMELINE_RETENTION_SIZE + 1}`)).toBe(true);
});

test('child timeline updates preserve identity for unchanged state references', () => {
  const current = new Map<string, ChildTimelineState>([['root>child', READY]]);

  expect(mergeChildTimelineWindow(current, 'root>child', READY, new Set(['root>child']))).toBe(
    current
  );
});

test('collapsing a child removes its retained descendants and is a no-op for absent branches', () => {
  const current = new Map<string, ChildTimelineState>([
    ['root>child', READY],
    ['root>child>grandchild', READY],
    ['root>sibling', READY],
  ]);
  const next = withoutChildTimelineBranch(current, 'root>child');

  expect([...next.keys()]).toEqual(['root>sibling']);
  expect(withoutChildTimelineBranch(next, 'root>child')).toBe(next);
});
