import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import type { AionSocketError, ConnectionStatus } from '@/lib/api';
import type { AionTranscriptStreamManager, TranscriptTarget } from '@/lib/api/transcript-stream';
import { createConfiguredTranscriptReader, createConfiguredTranscriptStream } from '@/lib/config';
import type { ActivityEvent, Namespace, WorkflowId } from '@/types';

import { backfillEntries, type TranscriptEntry } from '../lib/entries';
import { TRANSCRIPT_LIVE_TAIL_SIZE } from '../lib/transcriptFoldBuffer';
import { startTranscriptSession, type TranscriptBackfillState } from './transcriptSession';

export const TRANSCRIPT_HISTORY_RETENTION_SIZE = 1500;

export type {
  TranscriptEntry,
  TranscriptEventEntry,
  TranscriptNoteRunEntry,
  TranscriptStreamEntry,
  TranscriptToolEntry,
  TranscriptToolStreamEntry,
} from '../lib/entries';
export { backfillEntries, foldTranscriptEvent } from '../lib/entries';

export type UseTranscriptOptions = {
  target: TranscriptTarget | null;
  createManager?: (
    target: TranscriptTarget,
    initialAfterSeq?: number
  ) => AionTranscriptStreamManager;
  fetchRetained?: (target: TranscriptTarget) => Promise<ActivityEvent[]>;
  fetchEarlier?: (target: TranscriptTarget, beforeSeq: number) => Promise<ActivityEvent[]>;
};

export type UseTranscriptResult = {
  entries: TranscriptEntry[];
  status: ConnectionStatus;
  socketError: AionSocketError | null;
  backfillState: TranscriptBackfillState;
  backfillError: Error | null;
  hasEarlier: boolean;
  isLoadingEarlier: boolean;
  loadEarlier: () => Promise<void>;
};

export function targetKey(target: TranscriptTarget | null): string {
  if (target === null) {
    return '';
  }
  return [
    target.namespace,
    target.workflowId,
    String(target.activityId),
    String(target.attempt),
  ].join('\0');
}

async function defaultFetchRetained(target: TranscriptTarget): Promise<ActivityEvent[]> {
  const window = await createConfiguredTranscriptReader({
    namespace: target.namespace,
  }).fetchTranscript({ ...target, last: TRANSCRIPT_LIVE_TAIL_SIZE });
  return window.events;
}

async function defaultFetchEarlier(
  target: TranscriptTarget,
  beforeSeq: number
): Promise<ActivityEvent[]> {
  const fromSeq = Math.max(0, beforeSeq - TRANSCRIPT_LIVE_TAIL_SIZE);
  if (fromSeq === beforeSeq) {
    return [];
  }
  const window = await createConfiguredTranscriptReader({
    namespace: target.namespace,
  }).fetchTranscript({
    ...target,
    fromSeq,
    limit: beforeSeq - fromSeq,
  });
  return window.events;
}

function defaultCreateManager(
  target: TranscriptTarget,
  initialAfterSeq?: number
): AionTranscriptStreamManager {
  return createConfiguredTranscriptStream(target, undefined, initialAfterSeq);
}

export function useTranscript(options: UseTranscriptOptions): UseTranscriptResult {
  const { target } = options;
  const createManager = options.createManager ?? defaultCreateManager;
  const fetchRetained = options.fetchRetained ?? defaultFetchRetained;
  const fetchEarlier = options.fetchEarlier ?? defaultFetchEarlier;
  const [liveEntries, setLiveEntries] = useState<TranscriptEntry[]>([]);
  const [earlierEntries, setEarlierEntries] = useState<TranscriptEntry[]>([]);
  const [status, setStatus] = useState<ConnectionStatus>('disconnected');
  const [socketError, setSocketError] = useState<AionSocketError | null>(null);
  const [backfillState, setBackfillState] = useState<TranscriptBackfillState>('ready');
  const [backfillError, setBackfillError] = useState<Error | null>(null);
  const [hasEarlier, setHasEarlier] = useState(false);
  const [isLoadingEarlier, setIsLoadingEarlier] = useState(false);
  const deltaCounter = useRef(0);
  const earliestLoadedSeq = useRef<number | null>(null);
  const key = targetKey(target);

  const fetchRetainedForSession = useCallback(
    async (sessionTarget: TranscriptTarget) => {
      const events = await fetchRetained(sessionTarget);
      const earliest = earliestStoreSequence(events);
      earliestLoadedSeq.current = earliest;
      setHasEarlier(earliest !== null && earliest > 0);
      return events;
    },
    [fetchRetained]
  );

  const loadEarlier = useCallback(async () => {
    const beforeSeq = earliestLoadedSeq.current;
    if (key === '' || beforeSeq === null || beforeSeq === 0 || isLoadingEarlier) {
      return;
    }
    setIsLoadingEarlier(true);
    try {
      const resolved = parseTargetKey(key);
      const events = await fetchEarlier(resolved, beforeSeq);
      const earliest = earliestStoreSequence(events);
      if (events.length === 0 || earliest === null) {
        setHasEarlier(false);
        return;
      }
      const page = backfillEntries(events).entries;
      const retainedCount = Math.min(
        TRANSCRIPT_HISTORY_RETENTION_SIZE,
        earlierEntries.length + page.length
      );
      setEarlierEntries((current) =>
        mergeTranscriptEntries(page, current).slice(-TRANSCRIPT_HISTORY_RETENTION_SIZE)
      );
      earliestLoadedSeq.current = earliest;
      setHasEarlier(earliest > 0 && retainedCount < TRANSCRIPT_HISTORY_RETENTION_SIZE);
    } finally {
      setIsLoadingEarlier(false);
    }
  }, [earlierEntries.length, fetchEarlier, isLoadingEarlier, key]);

  useEffect(() => {
    if (key === '') {
      setLiveEntries([]);
      setEarlierEntries([]);
      setStatus('disconnected');
      setSocketError(null);
      setBackfillState('ready');
      setBackfillError(null);
      setHasEarlier(false);
      setIsLoadingEarlier(false);
      earliestLoadedSeq.current = null;
      return;
    }
    const resolved = parseTargetKey(key);
    setLiveEntries([]);
    setEarlierEntries([]);
    setStatus('disconnected');
    setSocketError(null);
    setHasEarlier(false);
    setIsLoadingEarlier(false);
    earliestLoadedSeq.current = null;
    deltaCounter.current = 0;
    setBackfillError(null);
    setBackfillState('loading');

    const session = startTranscriptSession({
      createManager,
      deltaCounter,
      fetchRetained: fetchRetainedForSession,
      onBackfill: (state, error) => {
        setBackfillError(error);
        setBackfillState(state);
      },
      onEntries: setLiveEntries,
      onSocketError: setSocketError,
      onStatus: setStatus,
      target: resolved,
    });

    return session.stop;
  }, [key, createManager, fetchRetainedForSession]);

  const entries = useMemo(
    () => mergeTranscriptEntries(earlierEntries, liveEntries),
    [earlierEntries, liveEntries]
  );

  return useMemo(
    () => ({
      entries,
      status,
      socketError,
      backfillState,
      backfillError,
      hasEarlier,
      isLoadingEarlier,
      loadEarlier,
    }),
    [
      entries,
      status,
      socketError,
      backfillState,
      backfillError,
      hasEarlier,
      isLoadingEarlier,
      loadEarlier,
    ]
  );
}

export function parseTargetKey(key: string): TranscriptTarget {
  const [namespace, workflowId, activityId, attempt] = key.split('\0');
  return {
    namespace: (namespace ?? '') as Namespace,
    workflowId: (workflowId ?? '') as WorkflowId,
    activityId: Number(activityId),
    attempt: Number(attempt),
  };
}

function earliestStoreSequence(events: readonly ActivityEvent[]): number | null {
  for (const event of events) {
    if (typeof event.store_seq === 'number') {
      return event.store_seq;
    }
  }
  return null;
}

function mergeTranscriptEntries(
  earlier: readonly TranscriptEntry[],
  later: readonly TranscriptEntry[]
): TranscriptEntry[] {
  const byKey = new Map<string, TranscriptEntry>();
  const orderedKeys: string[] = [];
  for (const entry of [...earlier, ...later]) {
    const current = byKey.get(entry.key);
    if (current === undefined) {
      orderedKeys.push(entry.key);
      byKey.set(entry.key, entry);
    } else if (current.type === 'tool' && entry.type === 'tool') {
      byKey.set(entry.key, {
        ...entry,
        call: entry.call ?? current.call,
        result: entry.result ?? current.result,
      });
    } else {
      byKey.set(entry.key, entry);
    }
  }
  return orderedKeys.flatMap((entryKey) => {
    const entry = byKey.get(entryKey);
    return entry === undefined ? [] : [entry];
  });
}
