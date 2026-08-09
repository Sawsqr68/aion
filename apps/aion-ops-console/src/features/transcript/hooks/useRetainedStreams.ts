import { useEffect, useMemo, useState } from 'react';

import type { RetainedStreamHead, TranscriptReadClient } from '@/lib/api';
import { createConfiguredTranscriptReader } from '@/lib/config';
import type { Namespace, RunId, WorkflowId } from '@/types';

/**
 * Retained-transcript stream enumeration (lane #230, consuming the lane-#229
 * `POST /workflows/transcripts` endpoint).
 *
 * Loads once per `(namespace, workflowId, runIds)` identity — a discrete REST
 * read, NOT a poll. The attempt rows themselves stay timeline-derived (every
 * started attempt is durably enumerated by `ActivityStarted`); this enumeration
 * only badges which rows have a RETAINED durable transcript. An empty list is the
 * honest answer for a pre-retention run; a failure is visible state
 * (`loadError`), never a throw into render and never swallowed.
 *
 * The endpoint enumerates ONE generation, so a workflow whose loaded history
 * spans several generations of a continue-as-new chain is enumerated once per
 * generation and the rows are concatenated — each row names its own run, so the
 * union stays unambiguous. If ANY generation's read fails the whole result is an
 * error: a partial union badged as complete would silently under-report which
 * attempts have a retained transcript.
 */

export type UseRetainedStreamsOptions = {
  /** The workflow to enumerate; `null` leaves the hook idle. */
  workflowId: WorkflowId | null;
  /**
   * The generations to enumerate; empty leaves the hook idle. A run is required
   * by the server on every request: a chain-wide enumeration would return several
   * generations' streams under pairwise-colliding coordinates, so there is no
   * run-less answer to give.
   */
  runIds: readonly RunId[];
  /** The namespace scope; `null` leaves the hook idle (no scope to authorize). */
  namespace: Namespace | null;
  /** Override the REST reader (tests); defaults to the configured client. */
  reader?: Pick<TranscriptReadClient, 'listStreams'>;
};

export type UseRetainedStreamsResult = {
  /** The retained streams, as enumerated by the server (may be empty). */
  streams: RetainedStreamHead[];
  loadState: 'idle' | 'loading' | 'ready' | 'error';
  loadError: Error | null;
};

export function useRetainedStreams(options: UseRetainedStreamsOptions): UseRetainedStreamsResult {
  const { workflowId, namespace } = options;
  // The effect must not re-run because the caller built a fresh array with the
  // same contents; the identity it depends on is the RUNS, not the array.
  const runsKey = options.runIds.join('\u0000');
  const [streams, setStreams] = useState<RetainedStreamHead[]>([]);
  const [loadState, setLoadState] = useState<UseRetainedStreamsResult['loadState']>('idle');
  const [loadError, setLoadError] = useState<Error | null>(null);

  useEffect(() => {
    const runIds = runsKey === '' ? [] : (runsKey.split('\u0000') as RunId[]);
    if (workflowId === null || runIds.length === 0 || namespace === null) {
      setStreams([]);
      setLoadState('idle');
      setLoadError(null);
      return;
    }

    let active = true;
    const reader = options.reader ?? createConfiguredTranscriptReader({ namespace });
    setStreams([]);
    setLoadState('loading');
    setLoadError(null);

    Promise.all(runIds.map((runId) => reader.listStreams(namespace, workflowId, runId)))
      .then((pages) => {
        if (!active) {
          return;
        }
        setStreams(pages.flat());
        setLoadState('ready');
      })
      .catch((error: unknown) => {
        if (!active) {
          return;
        }
        setLoadState('error');
        setLoadError(error instanceof Error ? error : new Error(String(error)));
      });

    return () => {
      active = false;
    };
  }, [workflowId, runsKey, namespace, options.reader]);

  return useMemo(() => ({ streams, loadState, loadError }), [streams, loadState, loadError]);
}
