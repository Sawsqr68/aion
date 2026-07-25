/** Running bars need coarse elapsed-time progress, not a full chart render each second. */
export const SWIMLANE_TIME_BUCKET_MS = 5_000;

export type SwimlaneTimeFrame = {
  bucket: number;
  nowMs: number;
};

export function createSwimlaneTimeFrame(nowMs: number): SwimlaneTimeFrame {
  return {
    bucket: Math.floor(nowMs / SWIMLANE_TIME_BUCKET_MS),
    nowMs,
  };
}

/** Preserve identity while the observed clock remains in the current display bucket. */
export function advanceSwimlaneTimeFrame(
  current: SwimlaneTimeFrame,
  observedNowMs: number
): SwimlaneTimeFrame {
  const bucket = Math.floor(observedNowMs / SWIMLANE_TIME_BUCKET_MS);
  return bucket === current.bucket ? current : { bucket, nowMs: observedNowMs };
}
