import { describe, expect, test } from 'bun:test';

import {
  buildWorkerAwlVerb,
  buildWorkerDeploymentSubmission,
  createWorkerDeploymentsClient,
  isWorkerDocument,
  parseNamespacesEntry,
  type WorkerDeployFormFields,
  WorkerDeployRefusedError,
  workerQueueFromSource,
  workflowTypeFromSource,
} from './worker-deploy';

const WORKER_DOC =
  '//! Ships briefs.\n\nworker briefs\n  action send(to: String) -> Receipt\n    run "mailer send"\n';
const WORKFLOW_DOC =
  '//! A flow.\nworkflow dev_brief\n  outcome done: type Summary, route success\n';

describe('worker document detection', () => {
  // Catches: the deploy affordance offered on a workflow document (which
  // `aion worker awl` refuses by name) or withheld from a worker document.
  test('a worker document is recognised through doc comments and blank lines', () => {
    expect(isWorkerDocument(WORKER_DOC)).toBe(true);
    expect(workerQueueFromSource(WORKER_DOC)).toBe('briefs');
  });

  test('a workflow document is not a worker document', () => {
    expect(isWorkerDocument(WORKFLOW_DOC)).toBe(false);
    expect(workerQueueFromSource(WORKFLOW_DOC)).toBeNull();
  });

  test('an empty or comment-only buffer declares nothing', () => {
    expect(isWorkerDocument('')).toBe(false);
    expect(isWorkerDocument('//! only prose\n// more prose\n')).toBe(false);
    expect(workerQueueFromSource('')).toBeNull();
  });

  // Catches: reading `worker` as a prefix — `workers x` is not a declaration.
  test('the declaration is the whole first token, not a prefix', () => {
    expect(isWorkerDocument('workers briefs\n')).toBe(false);
  });

  // Catches: the run panel seeded with a queue name, a worker name, or a
  // phantom type from a document that declares no workflow at all.
  test('a workflow document names its workflow type; nothing else does', () => {
    expect(workflowTypeFromSource(WORKFLOW_DOC)).toBe('dev_brief');
    expect(workflowTypeFromSource(WORKER_DOC)).toBeNull();
    expect(workflowTypeFromSource('')).toBeNull();
    expect(workflowTypeFromSource('//! prose only\n')).toBeNull();
    expect(workflowTypeFromSource('workflow\n')).toBeNull();
    expect(workflowTypeFromSource('workflows dev_brief\n')).toBeNull();
  });
});

describe('buildWorkerAwlVerb', () => {
  const blank = {
    documentPath: 'briefs-worker.awl',
    identity: '',
    concurrency: '',
    reconnectInitialBackoff: '',
    reconnectMaxBackoff: '',
    reconnectMaxAttempts: '',
    endpoint: '',
  };

  // Catches: verb drift — the argv tail must be exactly the `aion worker awl`
  // surface, document first, and a blank setting must contribute NOTHING
  // (no invented defaults anywhere in this stack).
  test('a bare deployment carries only the verb and document', () => {
    expect(buildWorkerAwlVerb(blank)).toEqual(['worker', 'awl', 'briefs-worker.awl']);
  });

  test('every supplied setting becomes its CLI flag, verbatim and in order', () => {
    expect(
      buildWorkerAwlVerb({
        documentPath: '/srv/aion/authoring/briefs-worker.awl',
        identity: 'fleet-7',
        concurrency: '4',
        reconnectInitialBackoff: '1',
        reconnectMaxBackoff: '30',
        reconnectMaxAttempts: '10',
        endpoint: 'http://127.0.0.1:7233',
      })
    ).toEqual([
      'worker',
      'awl',
      '/srv/aion/authoring/briefs-worker.awl',
      '--identity',
      'fleet-7',
      '--concurrency',
      '4',
      '--reconnect-initial-backoff',
      '1',
      '--reconnect-max-backoff',
      '30',
      '--reconnect-max-attempts',
      '10',
      '--endpoint',
      'http://127.0.0.1:7233',
    ]);
  });

  // Catches: whitespace-only entries smuggled in as empty flag values.
  test('whitespace-only settings are omitted like blanks', () => {
    expect(buildWorkerAwlVerb({ ...blank, concurrency: '   ' })).toEqual([
      'worker',
      'awl',
      'briefs-worker.awl',
    ]);
  });
});

describe('parseNamespacesEntry', () => {
  test('splits on commas and whitespace, dedupes, sorts', () => {
    expect(parseNamespacesEntry('orders, billing orders\n meridian')).toEqual([
      'billing',
      'meridian',
      'orders',
    ]);
    expect(parseNamespacesEntry('')).toEqual([]);
    expect(parseNamespacesEntry('  ,  ')).toEqual([]);
  });
});

const BLANK_FIELDS: WorkerDeployFormFields = {
  name: 'briefs-worker',
  documentPath: 'briefs-worker.awl',
  namespacesEntry: '',
  node: '',
  desired: 'running',
  identity: '',
  concurrency: '',
  reconnectInitialBackoff: '',
  reconnectMaxBackoff: '',
  reconnectMaxAttempts: '',
  endpoint: '',
};

describe('buildWorkerDeploymentSubmission (the panel submit, pure)', () => {
  // M9. Catches: the taskQueue-swap mutation — the queue MUST be the one the
  // document declares (passed in), never operator-typed or the name.
  test('the task queue is the document-declared queue, verbatim', () => {
    const submission = buildWorkerDeploymentSubmission(BLANK_FIELDS, 'briefs');
    expect(submission.ok).toBe(true);
    if (!submission.ok) return;
    expect(submission.request.taskQueue).toBe('briefs');
  });

  // M9. Catches: name/path trim mutations, and the untrimmed name reaching
  // the PUT path segment.
  test('name and document path are trimmed; the trimmed name keys the PUT', () => {
    const submission = buildWorkerDeploymentSubmission(
      { ...BLANK_FIELDS, name: '  briefs-worker  ', documentPath: '  a.awl  ' },
      'briefs'
    );
    expect(submission).toMatchObject({
      ok: true,
      name: 'briefs-worker',
      request: { verb: ['worker', 'awl', 'a.awl'] },
    });
  });

  // M9. Catches: the flag-swap mutation — every operator setting must land
  // under ITS OWN flag in the composed verb, in the canonical order.
  test('operator settings compose into the verb under their own flags', () => {
    const submission = buildWorkerDeploymentSubmission(
      {
        ...BLANK_FIELDS,
        identity: 'fleet-7',
        concurrency: '4',
        reconnectInitialBackoff: '1',
        reconnectMaxBackoff: '30',
        reconnectMaxAttempts: '10',
        endpoint: 'http://127.0.0.1:7233',
      },
      'briefs'
    );
    expect(submission.ok).toBe(true);
    if (!submission.ok) return;
    expect(submission.request.verb).toEqual([
      'worker',
      'awl',
      'briefs-worker.awl',
      '--identity',
      'fleet-7',
      '--concurrency',
      '4',
      '--reconnect-initial-backoff',
      '1',
      '--reconnect-max-backoff',
      '30',
      '--reconnect-max-attempts',
      '10',
      '--endpoint',
      'http://127.0.0.1:7233',
    ]);
  });

  // Catches: a blank node sent as "" instead of null, and the namespaces
  // entry reaching the wire unsplit.
  test('a blank node is null and namespaces are parsed from the entry', () => {
    const submission = buildWorkerDeploymentSubmission(
      { ...BLANK_FIELDS, namespacesEntry: 'orders, meridian', node: '  ' },
      'briefs'
    );
    expect(submission.ok).toBe(true);
    if (!submission.ok) return;
    expect(submission.request.node).toBeNull();
    expect(submission.request.namespaces).toEqual(['meridian', 'orders']);
    expect(submission.request.desired).toBe('running');
  });

  // Catches: a submission composed anyway when there is nothing valid to
  // send — each refusal names its reason.
  test('missing queue, name, or path refuse with the reason', () => {
    expect(buildWorkerDeploymentSubmission(BLANK_FIELDS, null)).toEqual({
      ok: false,
      reason: 'The document does not declare `worker <queue>`.',
    });
    expect(buildWorkerDeploymentSubmission({ ...BLANK_FIELDS, name: '  ' }, 'briefs')).toEqual({
      ok: false,
      reason: 'A deployment name is required.',
    });
    expect(
      buildWorkerDeploymentSubmission({ ...BLANK_FIELDS, documentPath: '' }, 'briefs')
    ).toEqual({ ok: false, reason: 'A document path is required.' });
  });
});

type Call = { url: string; init: RequestInit | undefined };

function stub(status: number, body: unknown, calls: Call[] = []) {
  const fetchImpl = (input: RequestInfo | URL, init?: RequestInit) => {
    calls.push({ url: String(input), init });
    return Promise.resolve(
      new Response(body === null ? null : JSON.stringify(body), {
        status,
        headers: { 'content-type': 'application/json' },
      })
    );
  };
  return { client: createWorkerDeploymentsClient(fetchImpl), calls };
}

const PUT_RESULT = {
  outcome: 'created',
  deployment: {
    name: 'briefs-worker',
    artifact: { type: 'builtin', verb: ['worker', 'awl', 'briefs-worker.awl'] },
    task_queue: 'briefs',
    desired: 'running',
  },
};

describe('the worker-deployments client', () => {
  // Catches: the PUT body drifting from the server's
  // `PutWorkerDeploymentRequest` shape (field names, artifact tag, verb).
  test('PUTs the exact request shape the server deserializes', async () => {
    const calls: Call[] = [];
    const { client } = stub(200, PUT_RESULT, calls);
    const result = await client.putWorkerDeployment('briefs-worker', {
      verb: ['worker', 'awl', 'briefs-worker.awl'],
      namespaces: ['meridian'],
      taskQueue: 'briefs',
      node: null,
      desired: 'running',
    });
    expect(calls[0]?.url).toBe('/worker-deployments/briefs-worker');
    expect(calls[0]?.init?.method).toBe('PUT');
    expect(JSON.parse(String(calls[0]?.init?.body))).toEqual({
      artifact: { type: 'builtin', verb: ['worker', 'awl', 'briefs-worker.awl'] },
      namespaces: ['meridian'],
      task_queue: 'briefs',
      node: null,
      desired: 'running',
    });
    expect(result).toEqual({
      outcome: 'created',
      name: 'briefs-worker',
      taskQueue: 'briefs',
      desired: 'running',
    });
  });

  test('the deployment name is URL-encoded', async () => {
    const calls: Call[] = [];
    const { client } = stub(404, null, calls);
    await client
      .putWorkerDeployment('odd/name', {
        verb: ['worker', 'awl', 'x.awl'],
        namespaces: [],
        taskQueue: 'q',
        node: null,
        desired: 'stopped',
      })
      .catch(() => undefined);
    expect(calls[0]?.url).toBe('/worker-deployments/odd%2Fname');
  });

  // Catches: a replaced outcome misread, or an unknown outcome coerced.
  test('a replaced outcome is carried; an unknown outcome is reported', async () => {
    const replaced = stub(200, { ...PUT_RESULT, outcome: 'replaced' });
    await expect(
      replaced.client.putWorkerDeployment('briefs-worker', {
        verb: ['worker', 'awl', 'x.awl'],
        namespaces: [],
        taskQueue: 'briefs',
        node: null,
        desired: 'running',
      })
    ).resolves.toMatchObject({ outcome: 'replaced' });

    const odd = stub(200, { ...PUT_RESULT, outcome: 'quantum' });
    await expect(
      odd.client.putWorkerDeployment('briefs-worker', {
        verb: ['worker', 'awl', 'x.awl'],
        namespaces: [],
        taskQueue: 'briefs',
        node: null,
        desired: 'running',
      })
    ).rejects.toThrow('unknown put outcome');
  });

  // Catches: the deploy-grant refusal (the shape every deploy route uses)
  // flattened into a generic error instead of the server's words.
  test('a deploy refusal keeps the server message and code verbatim', async () => {
    const { client } = stub(403, {
      code: 'namespace_denied',
      message: 'worker-deployment management requires the deployment-wide deploy grant',
    });
    const refusal = (await client
      .putWorkerDeployment('briefs-worker', {
        verb: ['worker', 'awl', 'x.awl'],
        namespaces: [],
        taskQueue: 'briefs',
        node: null,
        desired: 'running',
      })
      .catch((error: unknown) => error)) as WorkerDeployRefusedError;
    expect(refusal).toBeInstanceOf(WorkerDeployRefusedError);
    expect(refusal.code).toBe('namespace_denied');
    expect(refusal.message).toBe(
      'worker-deployment management requires the deployment-wide deploy grant'
    );
  });
});
