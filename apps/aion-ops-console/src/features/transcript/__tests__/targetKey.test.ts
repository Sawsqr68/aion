import { describe, expect, test } from 'bun:test';
import type { TranscriptTarget } from '@/lib/api/transcript-stream';
import type { ActivityId, Namespace, RunId, WorkflowId } from '@/types';
import { parseTargetKey, targetKey } from '../hooks/useTranscript';

/**
 * Guards the `targetKey` <-> `parseTargetKey` round-trip. This pair serializes a
 * transcript target into a stable string for the effect dependency, then recovers
 * it inside the effect. A join/split delimiter mismatch here silently corrupts the
 * recovered target (blank namespace / NaN attempt), so the socket subscribes to a
 * nonexistent target and the panel shows an empty transcript forever — a failure
 * that leaves every other test green. The delimiter is NUL, which can never appear
 * in these identifier fields, so a namespace or workflow id that contains a space
 * must still round-trip exactly.
 */
describe('transcript target key round-trip', () => {
  test('round-trips a normal target', () => {
    const target: TranscriptTarget = {
      namespace: 'default' as Namespace,
      workflowId: 'wf-0192' as WorkflowId,
      runId: '00000000-0000-0000-0000-0000000000a1' as RunId,
      activityId: 7 as ActivityId,
      attempt: 2,
    };
    expect(parseTargetKey(targetKey(target))).toEqual(target);
  });

  test('round-trips fields containing a space (NUL-delimiter robustness)', () => {
    // A space-delimited join would mis-split these; the NUL delimiter must not.
    const target: TranscriptTarget = {
      namespace: 'team alpha' as Namespace,
      workflowId: 'order saga 9' as WorkflowId,
      runId: '00000000-0000-0000-0000-0000000000a2' as RunId,
      activityId: 3 as ActivityId,
      attempt: 1,
    };
    expect(parseTargetKey(targetKey(target))).toEqual(target);
  });

  test('two generations of one chain produce DIFFERENT keys', () => {
    // The whole point of the run axis: everything but the run is identical, so a
    // run-less key would fuse the two sessions onto one socket and one buffer.
    const base = {
      namespace: 'default' as Namespace,
      workflowId: 'wf-0192' as WorkflowId,
      activityId: 0 as ActivityId,
      attempt: 1,
    };
    const first: TranscriptTarget = {
      ...base,
      runId: '00000000-0000-0000-0000-0000000000a1' as RunId,
    };
    const second: TranscriptTarget = {
      ...base,
      runId: '00000000-0000-0000-0000-0000000000a2' as RunId,
    };
    expect(targetKey(first)).not.toBe(targetKey(second));
    expect(parseTargetKey(targetKey(second))).toEqual(second);
  });
});
