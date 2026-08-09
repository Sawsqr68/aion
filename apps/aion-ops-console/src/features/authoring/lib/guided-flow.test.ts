import { describe, expect, test } from 'bun:test';

import { createAuthoringFacade, GuidedFlowRefusedError } from './facade';

const deployment = {
  deployment_id: 'deploy-1',
  document_path: 'demo.awl',
  content_hash: 'a'.repeat(64),
  package_id: 'package-1',
  workflow_type: 'demo',
  task_queue: 'jobs',
  workflow_id: null,
  run_id: null,
};

describe('guided deploy boundary', () => {
  // Catches: the save→deploy hash threading drifting from the server routes,
  // or the availability read losing its request shape. These are the legs the
  // header Deploy action and the run panel stand on.
  test('wires save, deploy, and the worker-availability read', async () => {
    const requests: string[] = [];
    const facade = createAuthoringFacade(async (input, init) => {
      const path = String(input);
      requests.push(`${init?.method ?? 'GET'} ${path}`);
      if (path.includes('/documents/')) {
        return Response.json({ source: 'deployed', content_hash: 'a'.repeat(64) });
      }
      if (path === '/awl/deploy') {
        return Response.json({
          deployment,
          steps: [
            { step: 'check', detail: 'green' },
            { step: 'compile', detail: 'direct BEAM compiled' },
            { step: 'package', detail: 'built' },
            { step: 'deploy', detail: 'loaded' },
          ],
        });
      }
      return Response.json({
        available: true,
        task_queue: 'jobs',
        connected_workers: 1,
        scaffold_hint: null,
      });
    });

    const hash = await facade.saveDocument('demo.awl', 'deployed');
    const deployed = await facade.deploy('demo.awl', hash);
    const worker = await facade.workerAvailability('default', deployed.deployment.taskQueue);

    expect(worker.available).toBe(true);
    expect(deployed.deployment.contentHash).toBe('a'.repeat(64));
    expect(deployed.steps.map((step) => step.step)).toEqual([
      'check',
      'compile',
      'package',
      'deploy',
    ]);
    expect(requests).toEqual([
      'PUT /awl/documents/demo.awl',
      'POST /awl/deploy',
      'POST /awl/workers/availability',
    ]);
  });

  test('preserves a typed server refusal instead of advancing', async () => {
    const facade = createAuthoringFacade(async () =>
      Response.json(
        { error_type: 'RevisionMismatch', message: 'saved revision changed' },
        { status: 422 }
      )
    );
    await expect(facade.deploy('demo.awl', 'old')).rejects.toBeInstanceOf(GuidedFlowRefusedError);
    try {
      await facade.deploy('demo.awl', 'old');
    } catch (error) {
      expect((error as GuidedFlowRefusedError).code).toBe('RevisionMismatch');
    }
  });
});
