import { expect, test } from 'bun:test';

import type { Namespace, RunId, WorkflowId } from '@/types';

import { ApiError } from './api-error';
import { ChildrenReadClient } from './children-read';

const namespace = 'default' as Namespace;
const parentId = '00000000-0000-0000-0000-000000000001' as WorkflowId;
const parentRun = '00000000-0000-0000-0000-000000000002' as RunId;

function clientReturning(child: Record<string, unknown>) {
  return new ChildrenReadClient({
    fetchImpl: async () =>
      new Response(JSON.stringify({ children: [child] }), {
        status: 200,
        headers: { 'content-type': 'application/json' },
      }),
  });
}

function child(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    workflow_id: '00000000-0000-0000-0000-000000000003',
    run_id: '00000000-0000-0000-0000-000000000004',
    workflow_type: 'builder_leg',
    display_name: 'Builder',
    status: 'Running',
    current_activity_id: 7,
    current_attempt: 2,
    ...overrides,
  };
}

test('children decoder preserves typed status and explicit not-started state', async () => {
  const running = await clientReturning(child()).listChildren(namespace, parentId, parentRun);
  const pending = await clientReturning(
    child({ run_id: null, status: null, current_activity_id: null, current_attempt: null })
  ).listChildren(namespace, parentId, parentRun);

  expect(running[0]?.status).toBe('Running');
  expect(pending[0]?.runId).toBeNull();
  expect(pending[0]?.status).toBeNull();
});

test('children decoder rejects present wrong-typed nullable strings', async () => {
  for (const row of [child({ run_id: 42 }), child({ display_name: { value: 'Builder' } })]) {
    try {
      await clientReturning(row).listChildren(namespace, parentId, parentRun);
      throw new Error('decoder accepted a wrong-typed nullable string');
    } catch (error) {
      expect(error).toBeInstanceOf(ApiError);
      expect((error as Error).message).toContain('invalid string field');
    }
  }
});

test('children decoder rejects unknown or run-incoherent status', async () => {
  for (const row of [child({ status: 'Pending' }), child({ run_id: null })]) {
    await expect(
      clientReturning(row).listChildren(namespace, parentId, parentRun)
    ).rejects.toThrow();
  }
});
