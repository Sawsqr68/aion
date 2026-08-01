import type { ActivityEvent } from '@/types';

import { backfillEntries, foldTranscriptEvent, type TranscriptEntry } from './entries';
import { decodeToolDelta, streamKind } from './toolEvents';

export const TRANSCRIPT_LIVE_TAIL_SIZE = 500;

export type TranscriptFoldItem = {
  deltaId: number;
  event: ActivityEvent;
};

type PendingText = {
  event: ActivityEvent;
  fragments: string[];
  kind: ReturnType<typeof streamKind>;
};

type PendingTool = {
  event: ActivityEvent;
  fragments: string[];
  callId: string | null;
  name: string | null;
  kind: string | null;
};

/**
 * Mutable indexing stays private; consumers receive a bounded immutable array once
 * per delivery batch. Durable resume state remains owned by the socket manager.
 */
export class TranscriptFoldBuffer {
  private entries: TranscriptEntry[];
  private readonly maxEntries: number;
  private readonly streamIndexes = new Map<string, number>();
  private readonly toolStreamIndexes = new Map<string, number>();
  private readonly pendingText = new Map<string, PendingText>();
  private readonly pendingTools = new Map<string, PendingTool>();

  constructor(
    initialEvents: readonly ActivityEvent[] = [],
    maxEntries = TRANSCRIPT_LIVE_TAIL_SIZE
  ) {
    this.maxEntries = maxEntries;
    this.entries = trimEntries(backfillEntries(initialEvents).entries, maxEntries);
    this.reindex();
  }

  foldBatch(items: readonly TranscriptFoldItem[]): TranscriptEntry[] {
    for (const item of items) {
      if (this.queueDelta(item.event) || this.queueToolDelta(item.event)) {
        continue;
      }
      this.flushPending();
      this.entries = trimEntries(
        foldTranscriptEvent(this.entries, item.event, item.deltaId),
        this.maxEntries
      );
      this.reindex();
    }
    this.flushPending();
    return this.entries;
  }

  snapshot(): TranscriptEntry[] {
    this.flushPending();
    return this.entries;
  }

  private queueDelta(event: ActivityEvent): boolean {
    if (event.store_seq !== null || event.kind.kind !== 'Delta') {
      return false;
    }
    const key = `stream:${event.agent_id}:${event.kind.message_id}`;
    const index = this.streamIndexes.get(key);
    if (index === undefined) {
      return false;
    }
    const pending = this.pendingText.get(key) ?? {
      event,
      fragments: [],
      kind: streamKind(event),
    };
    pending.fragments.push(event.kind.text_fragment);
    pending.event = event;
    if (pending.kind !== 'thinking') {
      pending.kind = streamKind(event);
    }
    this.pendingText.set(key, pending);
    return true;
  }

  private queueToolDelta(event: ActivityEvent): boolean {
    if (event.store_seq !== null) {
      return false;
    }
    const delta = decodeToolDelta(event);
    if (delta === null) {
      return false;
    }
    const identity = delta.itemId || delta.callId;
    if (identity === '') {
      return false;
    }
    const key = `tool-stream:${identity}`;
    const index = this.toolStreamIndexes.get(key);
    if (index === undefined) {
      return false;
    }
    const pending = this.pendingTools.get(key) ?? {
      event,
      fragments: [],
      callId: null,
      name: null,
      kind: null,
    };
    pending.fragments.push(delta.argumentsDelta);
    pending.event = event;
    pending.callId = delta.callId ?? pending.callId;
    pending.name = delta.name ?? pending.name;
    pending.kind = delta.kind ?? pending.kind;
    this.pendingTools.set(key, pending);
    return true;
  }

  /**
   * Merge buffered text fragments into their stream rows. A pending merge only
   * lands when its index still points at a row of the expected type: a reindex
   * between batches can move or drop a row, and a stale index must be discarded
   * rather than overwrite an unrelated entry.
   */
  private mergePendingText(next: TranscriptEntry[]): void {
    for (const [key, pending] of this.pendingText) {
      const index = this.streamIndexes.get(key);
      if (index === undefined) {
        continue;
      }
      const current = next[index];
      if (current?.type !== 'stream') {
        continue;
      }
      next[index] = {
        ...current,
        text: current.text + pending.fragments.join(''),
        // Thinking is sticky: a row that started as reasoning stays reasoning.
        streamKind: current.streamKind === 'thinking' ? 'thinking' : pending.kind,
        event: pending.event,
      };
    }
  }

  /** Merge buffered argument fragments into their tool-stream rows; see above on staleness. */
  private mergePendingTools(next: TranscriptEntry[]): void {
    for (const [key, pending] of this.pendingTools) {
      const index = this.toolStreamIndexes.get(key);
      if (index === undefined) {
        continue;
      }
      const current = next[index];
      if (current?.type !== 'tool-stream') {
        continue;
      }
      next[index] = {
        ...current,
        callId: pending.callId ?? current.callId,
        name: pending.name ?? current.name,
        kind: pending.kind ?? current.kind,
        argumentsText: current.argumentsText + pending.fragments.join(''),
        event: pending.event,
      };
    }
  }

  private flushPending(): void {
    if (this.pendingText.size === 0 && this.pendingTools.size === 0) {
      return;
    }
    const next = [...this.entries];
    this.mergePendingText(next);
    this.mergePendingTools(next);
    this.pendingText.clear();
    this.pendingTools.clear();
    this.entries = next;
  }

  private reindex(): void {
    this.streamIndexes.clear();
    this.toolStreamIndexes.clear();
    for (const [index, entry] of this.entries.entries()) {
      if (entry.type === 'stream') {
        this.streamIndexes.set(entry.key, index);
      } else if (entry.type === 'tool-stream') {
        this.toolStreamIndexes.set(entry.key, index);
      }
    }
  }
}

function trimEntries(entries: TranscriptEntry[], maxEntries: number): TranscriptEntry[] {
  return entries.length <= maxEntries ? entries : entries.slice(-maxEntries);
}
