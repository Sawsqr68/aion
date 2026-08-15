import { expect, test } from 'bun:test';

import { requireWorkflowStatus } from './client-normalize';

test('workflow status normalization accepts paused workflows', () => {
  expect(requireWorkflowStatus('Paused')).toBe('Paused');
});
