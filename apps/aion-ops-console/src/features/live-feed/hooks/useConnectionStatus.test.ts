import { expect, mock, test } from 'bun:test';
import * as react from 'react';

import { createAionEventWebSocketManager } from '@/lib/api';

/**
 * Calling the hook outside a render needs `useSyncExternalStore` to resolve
 * synchronously — but `mock.module` is GLOBAL and outlives this file, so the
 * rest of `react` is re-exported untouched. Replacing the whole module here
 * would leave every later-loaded test file without `useState`/`useRef`.
 */
mock.module('react', () => ({
  ...react,
  useSyncExternalStore: (
    subscribe: (notify: () => void) => () => void,
    getSnapshot: () => string
  ) => {
    const cleanup = subscribe(() => undefined);
    cleanup();
    return getSnapshot();
  },
}));

const namespace = 'default';
const workflowId = '00000000-0000-0000-0000-000000000001';

test('useConnectionStatus is exported as a hook function', async () => {
  const { useConnectionStatus } = await import('./useConnectionStatus');

  expect(typeof useConnectionStatus).toBe('function');
  // A manager nothing has subscribed to is idle, not disconnected: quiet is
  // not a fault.
  expect(useConnectionStatus()).toBe('idle');
});

test('manager status store reports connect, drop, and reconnect sequence consumed by the hook', () => {
  const scheduler = new FakeScheduler();
  const socketFactory = new FakeSocketFactory();
  const manager = createAionEventWebSocketManager({
    webSocketImpl: socketFactory.ctor,
    scheduler,
    reconnect: { initialDelayMs: 5, maxAttempts: 2 },
  });
  const statuses = [manager.getStatus()];
  const cleanup = manager.onStatusChange(() => {
    statuses.push(manager.getStatus());
  });
  const afterCleanupStatuses: string[] = [];

  manager.subscribe({ kind: 'workflow', namespace, workflowId }, () => undefined);
  const firstSocket = socketFactory.sockets[0] as FakeSocket;
  firstSocket.open();
  firstSocket.drop();
  scheduler.runNext();
  const secondSocket = socketFactory.sockets[1] as FakeSocket;
  secondSocket.open();
  cleanup();
  manager.onStatusChange((status) => afterCleanupStatuses.push(status));
  secondSocket.drop();

  // The sequence starts idle (nothing subscribed yet); a subscription opening
  // its first socket stays idle until the socket actually connects.
  expect(statuses).toEqual(['idle', 'connected', 'reconnecting', 'connected']);
  expect(afterCleanupStatuses).toEqual(['reconnecting']);
});

class FakeSocketFactory {
  readonly sockets: FakeSocket[] = [];
  readonly ctor: new (
    url: string
  ) => FakeSocket;

  constructor() {
    const sockets = this.sockets;

    this.ctor = class extends FakeSocket {
      constructor(url: string) {
        super(url);
        sockets.push(this);
      }
    };
  }
}

class FakeSocket {
  readyState = 0;
  onopen: ((event: Event) => void) | null = null;
  onmessage: ((event: { data: unknown }) => void) | null = null;
  onclose: ((event: { wasClean: boolean }) => void) | null = null;
  onerror: ((event: Event) => void) | null = null;

  constructor(readonly url: string) {}

  send(): void {}

  close(): void {
    this.readyState = 3;
    this.onclose?.({ wasClean: true });
  }

  open(): void {
    this.readyState = 1;
    this.onopen?.({} as Event);
  }

  drop(): void {
    this.readyState = 3;
    this.onclose?.({ wasClean: false });
  }
}

class FakeScheduler {
  private readonly callbacks: Array<() => void> = [];

  setTimeout(callback: () => void): ReturnType<typeof setTimeout> {
    this.callbacks.push(callback);
    return this.callbacks.length as unknown as ReturnType<typeof setTimeout>;
  }

  clearTimeout(): void {
    this.callbacks.shift();
  }

  runNext(): void {
    this.callbacks.shift()?.();
  }
}
