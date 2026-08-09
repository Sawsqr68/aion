import { describe, expect, test } from 'bun:test';

import { runSeedWorkflowType } from './run-seed';

const WORKFLOW_DOC = '//! A flow.\nworkflow dev_brief\n  outcome done: type Summary\n';
const WORKER_DOC = '//! Ships briefs.\nworker briefs\n  action send(to: String) -> Receipt\n';

describe('runSeedWorkflowType', () => {
  // Catches: the fallback order inverted — a landed deploy's type is the
  // authoritative name (it is what the server routes), and must win over
  // whatever the buffer currently declares.
  test('a deploy this session wins over the buffer declaration', () => {
    expect(runSeedWorkflowType({ workflowType: 'deployed_flow' }, WORKFLOW_DOC)).toBe(
      'deployed_flow'
    );
  });

  test('no deploy falls back to the document declaration', () => {
    expect(runSeedWorkflowType(null, WORKFLOW_DOC)).toBe('dev_brief');
  });

  // Catches: null leaking through as the seed — StartWorkflowForm's contract
  // is `string | undefined`, and a worker document declares no workflow.
  test('nothing derivable is undefined, never null', () => {
    expect(runSeedWorkflowType(null, WORKER_DOC)).toBeUndefined();
    expect(runSeedWorkflowType(null, '')).toBeUndefined();
    expect(runSeedWorkflowType(null, '//! prose only\n')).toBeUndefined();
  });
});
