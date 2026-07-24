import { createAnimationFrameBatch } from '@/features/live-feed/hooks/animationFrameBatch';
import type { AionSocketError, ConnectionStatus } from '@/lib/api';
import type {
  AionTranscriptStreamManager,
  TranscriptStreamListener,
  TranscriptTarget,
} from '@/lib/api/transcript-stream';
import type { ActivityEvent } from '@/types';

import type { TranscriptEntry } from '../lib/entries';
import { TranscriptFoldBuffer, type TranscriptFoldItem } from '../lib/transcriptFoldBuffer';

/**
 * The transcript session controller: the REST-backfill-first-then-WS-tail
 * sequencing extracted from {@link useTranscript} into a plain async function so
 * its runtime behavior is provable by DIRECT EXECUTION in tests (the SSR-only
 * suite never runs effects). The hook's effect is a thin adapter: it starts a
 * session wired to its `setState` sinks and returns {@link TranscriptSession.stop}
 * as the cleanup.
 *
 * Contract (the lane's headline sequencing):
 *
 * 1. `fetchRetained(target)` loads the durable retained transcript over REST.
 * 2. On success the folded backfill replaces the entry list, the delta counter is
 *    advanced past the consumed ids, backfill reports `ready`, and the WS tail
 *    attaches with `after_seq = lastSeq` (or a full replay when the retained
 *    transcript was empty — a pre-retention run, the honest fallback).
 * 3. On failure the error is surfaced VISIBLY (`error` state, never swallowed).
 *    No cursorless socket replay is started because it could remount all history.
 * 4. `stop()` cancels: a fetch settling after `stop()` neither mutates state nor
 *    attaches, and an attached manager's listeners are all unsubscribed.
 */

export type TranscriptBackfillState = 'loading' | 'ready' | 'error';

export type TranscriptSessionOptions = {
  /** The attempt target the session serves. */
  target: TranscriptTarget;
  /** WS manager factory; `initialAfterSeq` seeds the resume cursor. */
  createManager: (
    target: TranscriptTarget,
    initialAfterSeq?: number
  ) => AionTranscriptStreamManager;
  /** The retained-transcript REST fetch preceding the WS attach. */
  fetchRetained: (target: TranscriptTarget) => Promise<ActivityEvent[]>;
  /**
   * Monotonic id source for ephemeral-delta keys, owned by the caller so ids
   * stay unique across re-subscribes of the same mounted hook.
   */
  deltaCounter: { current: number };
  /** Entry-list sink (updater form, mirroring React's functional `setState`). */
  onEntries: (update: (current: TranscriptEntry[]) => TranscriptEntry[]) => void;
  /** Live socket status sink. */
  onStatus: (status: ConnectionStatus) => void;
  /** Typed live-socket error sink. */
  onSocketError: (error: AionSocketError | null) => void;
  /** Backfill lifecycle sink (`ready`/`error` + the visible failure, if any). */
  onBackfill: (state: TranscriptBackfillState, error: Error | null) => void;
};

export type TranscriptSession = {
  /**
   * Resolves once the backfill has settled AND the WS tail attached (or the
   * session was stopped first). Never rejects: the fetch failure is routed to
   * `onBackfill('error', …)`.
   */
  settled: Promise<void>;
  /** Cancel the session: suppress a late fetch and tear down the WS listeners. */
  stop: () => void;
};

export function startTranscriptSession(options: TranscriptSessionOptions): TranscriptSession {
  const { target, createManager, fetchRetained, deltaCounter } = options;

  let cancelled = false;
  let teardown: (() => void) | null = null;

  // Attach the live WS tail only after a bounded REST backfill establishes its cursor.
  const attach = (initialAfterSeq: number | undefined, foldBuffer: TranscriptFoldBuffer) => {
    const manager = createManager(target, initialAfterSeq);
    const batch = createAnimationFrameBatch<TranscriptFoldItem>((items) => {
      options.onEntries(() => foldBuffer.foldBatch(items));
    });

    const listener: TranscriptStreamListener = {
      onEvent: (event) => {
        const deltaId = deltaCounter.current;
        deltaCounter.current += 1;
        batch.push({ deltaId, event }, typeof event.store_seq === 'number');
      },
    };

    const unsubscribeStream = manager.subscribe(listener);
    const unsubscribeStatus = manager.onStatusChange(options.onStatus);
    const unsubscribeError = manager.onError(options.onSocketError);
    options.onStatus(manager.getStatus());
    teardown = () => {
      batch.cancel();
      unsubscribeStream();
      unsubscribeStatus();
      unsubscribeError();
    };
  };

  const settled = fetchRetained(target)
    .then((events) => {
      if (cancelled) {
        return;
      }
      const lastSeq = latestPersistedSequence(events);
      const foldBuffer = new TranscriptFoldBuffer(events);
      // Keep live entry keys unique past the backfill's consumed ids.
      deltaCounter.current = events.length;
      options.onEntries(() => foldBuffer.snapshot());
      options.onBackfill('ready', null);
      // An explicitly empty retained stream is the only cursorless attach: there
      // is no durable history to replay at the splice point.
      attach(lastSeq, foldBuffer);
    })
    .catch((error: unknown) => {
      if (cancelled) {
        return;
      }
      options.onBackfill('error', error instanceof Error ? error : new Error(String(error)));
    });

  return {
    settled,
    stop: () => {
      cancelled = true;
      teardown?.();
    },
  };
}

function latestPersistedSequence(events: readonly ActivityEvent[]): number | undefined {
  let latest: number | undefined;
  for (const event of events) {
    if (typeof event.store_seq === 'number') {
      latest = latest === undefined ? event.store_seq : Math.max(latest, event.store_seq);
    }
  }
  return latest;
}
