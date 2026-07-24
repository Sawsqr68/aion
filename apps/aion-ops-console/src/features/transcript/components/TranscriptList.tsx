import { useLayoutEffect, useMemo, useRef, useState } from 'react';

import { Button } from '@/components/ui';

import type { TranscriptEntry } from '../hooks/useTranscript';
import { TranscriptEntryRow } from './TranscriptEventRow';

const FOLLOW_THRESHOLD_PX = 48;
export const TRANSCRIPT_DOM_WINDOW_SIZE = 200;

export type TranscriptListProps = {
  entries: TranscriptEntry[];
  hasEarlier?: boolean;
  isLoadingEarlier?: boolean;
  onLoadEarlier?: () => Promise<void>;
};

/** A fixed-size DOM page over the bounded transcript source window. */
export function TranscriptList({
  entries,
  hasEarlier = false,
  isLoadingEarlier = false,
  onLoadEarlier,
}: TranscriptListProps) {
  const listRef = useRef<HTMLOListElement | null>(null);
  const seenTailKeysRef = useRef<Set<string> | null>(null);
  const [following, setFollowing] = useState(true);
  const [pageOffset, setPageOffset] = useState(0);
  const maxOffset = Math.max(0, entries.length - TRANSCRIPT_DOM_WINDOW_SIZE);
  const boundedOffset = Math.min(pageOffset, maxOffset);
  const end = entries.length - boundedOffset;
  const start = Math.max(0, end - TRANSCRIPT_DOM_WINDOW_SIZE);
  const visibleEntries = useMemo(() => entries.slice(start, end), [end, entries, start]);
  const seenTailKeys = seenTailKeysRef.current;

  const handleScroll = () => {
    const list = listRef.current;
    if (list === null || boundedOffset > 0) {
      return;
    }
    const distanceFromBottom = list.scrollHeight - list.scrollTop - list.clientHeight;
    setFollowing(distanceFromBottom <= FOLLOW_THRESHOLD_PX);
  };

  const jumpToLatest = () => {
    setPageOffset(0);
    const list = listRef.current;
    if (list !== null) {
      list.scrollTop = list.scrollHeight;
    }
    setFollowing(true);
  };

  const showEarlier = async () => {
    if (start > 0) {
      setPageOffset((current) => Math.min(maxOffset, current + TRANSCRIPT_DOM_WINDOW_SIZE));
      setFollowing(false);
      return;
    }
    if (hasEarlier && onLoadEarlier !== undefined) {
      await onLoadEarlier();
      // Pin to the oldest mounted page after the newly fetched prefix commits.
      setPageOffset(Number.MAX_SAFE_INTEGER);
      setFollowing(false);
    }
  };

  useLayoutEffect(() => {
    const list = listRef.current;
    if (!following || boundedOffset > 0 || list === null || entries.length === 0) {
      return;
    }
    list.scrollTop = list.scrollHeight;
  }, [boundedOffset, entries, following]);

  useLayoutEffect(() => {
    if (boundedOffset === 0) {
      seenTailKeysRef.current = new Set(visibleEntries.map((entry) => entry.key));
    }
  }, [boundedOffset, visibleEntries]);

  const canShowEarlier = start > 0 || hasEarlier;

  return (
    <div className="relative flex min-h-0 flex-1 flex-col">
      {canShowEarlier ? (
        <div className="flex justify-center pb-2">
          <Button
            className="h-7 px-3 text-xs"
            disabled={isLoadingEarlier}
            onClick={() => void showEarlier()}
            type="button"
            variant="ghost"
          >
            {isLoadingEarlier ? 'Loading earlier…' : 'Show earlier'}
          </Button>
        </div>
      ) : null}
      <ol
        className="max-h-[70vh] min-h-0 flex-1 overflow-y-auto"
        data-testid="transcript-scroll"
        onScroll={handleScroll}
        ref={listRef}
      >
        {visibleEntries.map((entry) => (
          <TranscriptEntryRow
            entry={entry}
            isNew={boundedOffset === 0 && seenTailKeys !== null && !seenTailKeys.has(entry.key)}
            key={entry.key}
          />
        ))}
      </ol>
      {!following || boundedOffset > 0 ? (
        <Button
          className="absolute right-3 bottom-3 h-7 px-3 text-xs shadow-md"
          onClick={jumpToLatest}
          type="button"
          variant="outline"
        >
          Jump to latest
        </Button>
      ) : null}
    </div>
  );
}
