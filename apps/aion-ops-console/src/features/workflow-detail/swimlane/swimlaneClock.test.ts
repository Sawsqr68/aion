import { expect, test } from 'bun:test';

import {
  advanceSwimlaneTimeFrame,
  createSwimlaneTimeFrame,
  SWIMLANE_TIME_BUCKET_MS,
} from './swimlaneClock';

test('swimlane clock samples preserve frame identity within one elapsed-time bucket', () => {
  const frame = createSwimlaneTimeFrame(SWIMLANE_TIME_BUCKET_MS + 100);

  expect(advanceSwimlaneTimeFrame(frame, SWIMLANE_TIME_BUCKET_MS + 999)).toBe(frame);
});

test('swimlane clock advances once the display bucket changes', () => {
  const frame = createSwimlaneTimeFrame(SWIMLANE_TIME_BUCKET_MS + 100);
  const next = advanceSwimlaneTimeFrame(frame, SWIMLANE_TIME_BUCKET_MS * 2);

  expect(next).not.toBe(frame);
  expect(next.nowMs).toBe(SWIMLANE_TIME_BUCKET_MS * 2);
  expect(next.bucket).toBe(2);
});
