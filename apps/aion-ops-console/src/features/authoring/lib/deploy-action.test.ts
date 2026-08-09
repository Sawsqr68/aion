import { describe, expect, test } from 'bun:test';

import {
  type DeployOutcome,
  deploySuccessMessage,
  nextDeployOutcome,
  saveThenDeploy,
} from './deploy-action';
import type { DeploymentRecord, GuidedDeployResult } from './guided-facade';

const DEPLOYMENT: DeploymentRecord = {
  deploymentId: 'dep-1',
  documentPath: 'hello.awl',
  contentHash: 'hash-abc',
  packageId: 'hello_flow$abc',
  workflowType: 'hello_flow',
  taskQueue: 'default',
  workflowId: null,
  runId: null,
};

const RESULT: GuidedDeployResult = {
  deployment: DEPLOYMENT,
  steps: [{ step: 'deploy', detail: 'deployed hello_flow$abc' }],
};

describe('saveThenDeploy', () => {
  // Catches: deploying an unsaved buffer (the server verifies content_hash
  // against the SAVED document — RevisionMismatch otherwise), or deploying
  // under anything but the hash that exact save returned.
  test('saves first and deploys under the returned hash', async () => {
    const calls: string[] = [];
    const result = await saveThenDeploy(
      {
        saveDocument: (path, source) => {
          calls.push(`save:${path}:${source}`);
          return Promise.resolve('hash-abc');
        },
        deploy: (path, contentHash) => {
          calls.push(`deploy:${path}:${contentHash}`);
          return Promise.resolve(RESULT);
        },
      },
      'hello.awl',
      'workflow hello_flow\n',
      (saved) => calls.push(`saved:${saved}`)
    );
    expect(calls).toEqual([
      'save:hello.awl:workflow hello_flow\n',
      'saved:workflow hello_flow\n',
      'deploy:hello.awl:hash-abc',
    ]);
    expect(result).toEqual(RESULT);
  });

  // Catches: the editor left showing "Modified" for bytes already on disk —
  // the save landed even though the deploy was refused.
  test('onSaved fires even when the deploy is refused afterwards', async () => {
    const savedSources: string[] = [];
    const refusal = saveThenDeploy(
      {
        saveDocument: () => Promise.resolve('hash-abc'),
        deploy: () => Promise.reject(new Error('deploy grant required')),
      },
      'hello.awl',
      'workflow hello_flow\n',
      (saved) => savedSources.push(saved)
    );
    await expect(refusal).rejects.toThrow('deploy grant required');
    expect(savedSources).toEqual(['workflow hello_flow\n']);
  });

  // Catches: a failed save marked as saved, or a deploy attempted anyway.
  test('a refused save deploys nothing and marks nothing saved', async () => {
    let deployCalled = false;
    let onSavedCalled = false;
    const refusal = saveThenDeploy(
      {
        saveDocument: () => Promise.reject(new Error('workspace is read-only')),
        deploy: () => {
          deployCalled = true;
          return Promise.resolve(RESULT);
        },
      },
      'hello.awl',
      'workflow hello_flow\n',
      () => {
        onSavedCalled = true;
      }
    );
    await expect(refusal).rejects.toThrow('workspace is read-only');
    expect(deployCalled).toBe(false);
    expect(onSavedCalled).toBe(false);
  });
});

describe('deploySuccessMessage', () => {
  // The outcome line is the operator's receipt: WHAT shipped, WHERE, and AS
  // WHAT — the package id is the identity they can find again later.
  test('names the workflow type, task queue, and package identity', () => {
    expect(deploySuccessMessage(DEPLOYMENT)).toBe(
      'Deployed hello_flow to queue default (package hello_flow$abc)'
    );
  });
});

describe('nextDeployOutcome', () => {
  const SUCCESS: DeployOutcome = { kind: 'success', text: 'Deployed …' };
  const REFUSED: DeployOutcome = { kind: 'refused', text: 'deploy grant required' };

  test('a starting deploy clears whatever outcome stood', () => {
    expect(nextDeployOutcome(SUCCESS, { event: 'deploy-started' })).toBeNull();
    expect(nextDeployOutcome(REFUSED, { event: 'deploy-started' })).toBeNull();
    expect(nextDeployOutcome(null, { event: 'deploy-started' })).toBeNull();
  });

  test('a landed deploy is a success outcome carrying the receipt', () => {
    expect(nextDeployOutcome(null, { event: 'deploy-landed', deployment: DEPLOYMENT })).toEqual({
      kind: 'success',
      text: 'Deployed hello_flow to queue default (package hello_flow$abc)',
    });
  });

  test('a refused deploy carries the server text verbatim', () => {
    expect(
      nextDeployOutcome(null, { event: 'deploy-refused', text: 'deploy grant required' })
    ).toEqual(REFUSED);
  });

  // Catches: the checker wiping the outcome — the exact defect this state
  // exists to prevent. The checker completes on EVERY edit, and a refusal
  // must survive the operator editing toward its fix.
  test('a completed check changes nothing, whatever stands', () => {
    expect(nextDeployOutcome(SUCCESS, { event: 'check-completed' })).toBe(SUCCESS);
    expect(nextDeployOutcome(REFUSED, { event: 'check-completed' })).toBe(REFUSED);
    expect(nextDeployOutcome(null, { event: 'check-completed' })).toBeNull();
  });
});
