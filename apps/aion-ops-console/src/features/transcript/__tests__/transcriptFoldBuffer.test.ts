import { describe, expect, test } from 'bun:test';

import type { ActivityEvent } from '@/types';

import { TranscriptFoldBuffer } from '../lib/transcriptFoldBuffer';

function delta(messageId: string, fragment: string): ActivityEvent {
  return {
    workflow_id: '00000000-0000-0000-0000-000000000001',
    activity_id: 3,
    attempt: 1,
    agent_id: '00000000-0000-0000-0000-0000000000aa',
    agent_role: 'root',
    emitted_at: '2026-07-01T00:00:00Z',
    worker_seq: 1,
    store_seq: null,
    ephemeral: true,
    kind: { kind: 'Delta', message_id: messageId, text_fragment: fragment },
  };
}

describe('TranscriptFoldBuffer', () => {
  test('keeps a simulated multi-stream delta flood inside its entry cap', () => {
    const buffer = new TranscriptFoldBuffer([], 32);
    const items = Array.from({ length: 4000 }, (_, deltaId) => ({
      deltaId,
      event: delta(`message-${deltaId % 80}`, 'x'),
    }));

    const snapshot = buffer.foldBatch(items);

    expect(snapshot.length).toBeLessThanOrEqual(32);
    expect(snapshot.every((entry) => entry.type === 'stream')).toBe(true);
  });

  test('coalesces one token flood into one immutable snapshot row', () => {
    const buffer = new TranscriptFoldBuffer([], 32);
    const snapshot = buffer.foldBatch(
      Array.from({ length: 2000 }, (_, deltaId) => ({
        deltaId,
        event: delta('one-message', 'x'),
      }))
    );

    expect(snapshot).toHaveLength(1);
    expect(snapshot[0]?.type).toBe('stream');
    if (snapshot[0]?.type === 'stream') {
      expect(snapshot[0].text).toHaveLength(2000);
    }
  });
});
