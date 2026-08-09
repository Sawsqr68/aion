/**
 * Deploying an AWL WORKER document as a durable managed-worker deployment.
 *
 * The server side is `PUT /worker-deployments/{name}` with a `builtin`
 * artifact: the server's own executable re-invoked with an operator-declared
 * argv tail. For a worker document that tail is the `aion worker awl` verb —
 * `["worker", "awl", "<document>", ...flags]` — which compiles the document
 * and serves the queue it declares.
 *
 * Everything the verb needs beyond the document is OPERATOR POLICY the AWL
 * worker refuses to invent (`--concurrency` and the three reconnect settings
 * have no defaults, by design). This module therefore appends exactly the
 * flags the operator supplied and no others; a setting left blank must come
 * from the server process's environment (`AION_WORKER_CONCURRENCY`,
 * `AION_RECONNECT_*`), and the spawned worker's own refusal names any that are
 * missing.
 */

import { expectRecord, expectString } from './facade-values';

/**
 * Whether a document's opening declaration is `worker <queue>` — the shape
 * `aion worker awl` serves (a workflow document is refused by name). Mirrors
 * the server's reading: doc comments and blank lines precede the declaration.
 */
export function isWorkerDocument(source: string): boolean {
  return firstDeclaration(source)?.[0] === 'worker';
}

/** The queue a worker document declares, or null when it is not one. */
export function workerQueueFromSource(source: string): string | null {
  const declaration = firstDeclaration(source);
  if (declaration === null || declaration[0] !== 'worker') return null;
  return declaration[1] ?? null;
}

function firstDeclaration(source: string): string[] | null {
  for (const line of source.split('\n')) {
    const trimmed = line.trim();
    if (trimmed.length === 0 || trimmed.startsWith('//')) continue;
    return trimmed.split(/\s+/);
  }
  return null;
}

/** Operator-supplied settings the verb argv carries. Blank strings are OMITTED. */
export type WorkerVerbSettings = {
  /** The document path AS THE SERVER RESOLVES IT when spawning the worker. */
  documentPath: string;
  identity: string;
  concurrency: string;
  reconnectInitialBackoff: string;
  reconnectMaxBackoff: string;
  reconnectMaxAttempts: string;
  endpoint: string;
};

/**
 * Build the `builtin` artifact argv tail for `aion worker awl <document>`.
 * Flag names are pinned to the CLI's own surface (`worker_awl/args.rs` plus
 * the global `--endpoint`); a blank setting contributes nothing.
 */
export function buildWorkerAwlVerb(settings: WorkerVerbSettings): string[] {
  const verb = ['worker', 'awl', settings.documentPath];
  const flag = (name: string, value: string) => {
    const trimmed = value.trim();
    if (trimmed.length > 0) verb.push(name, trimmed);
  };
  flag('--identity', settings.identity);
  flag('--concurrency', settings.concurrency);
  flag('--reconnect-initial-backoff', settings.reconnectInitialBackoff);
  flag('--reconnect-max-backoff', settings.reconnectMaxBackoff);
  flag('--reconnect-max-attempts', settings.reconnectMaxAttempts);
  flag('--endpoint', settings.endpoint);
  return verb;
}

/** Split an operator's comma/whitespace-separated namespace entry. */
export function parseNamespacesEntry(entry: string): string[] {
  return [...new Set(entry.split(/[\s,]+/).filter((name) => name.length > 0))].sort();
}

export type PutWorkerDeploymentRequest = {
  verb: string[];
  namespaces: string[];
  taskQueue: string;
  node: string | null;
  desired: 'running' | 'stopped';
};

/** Everything the deploy-worker form collects, as raw field text. */
export type WorkerDeployFormFields = {
  name: string;
  documentPath: string;
  namespacesEntry: string;
  node: string;
  desired: 'running' | 'stopped';
  identity: string;
  concurrency: string;
  reconnectInitialBackoff: string;
  reconnectMaxBackoff: string;
  reconnectMaxAttempts: string;
  endpoint: string;
};

/** The decided PUT, or the reason there is none to make. */
export type WorkerDeploySubmission =
  | { ok: true; name: string; request: PutWorkerDeploymentRequest }
  | { ok: false; reason: string };

/**
 * Turn the form's raw fields into the exact PUT the server deserializes —
 * the ONE place the submission is composed, so what the panel sends is what
 * these semantics say and nothing else:
 *
 * - the deployment name and document path are trimmed, and both are required;
 * - the task queue is READ FROM THE DOCUMENT (`queue`), never typed — a
 *   deployment must serve the queue its document declares;
 * - namespaces are split/deduped/sorted from the free-form entry;
 * - a blank node is `null` (no locality axis), not `""`;
 * - the verb is {@link buildWorkerAwlVerb} over the trimmed document path and
 *   the operator's settings (blank settings contribute no flag).
 */
export function buildWorkerDeploymentSubmission(
  fields: WorkerDeployFormFields,
  queue: string | null
): WorkerDeploySubmission {
  if (queue === null) {
    return { ok: false, reason: 'The document does not declare `worker <queue>`.' };
  }
  const name = fields.name.trim();
  if (name.length === 0) {
    return { ok: false, reason: 'A deployment name is required.' };
  }
  const documentPath = fields.documentPath.trim();
  if (documentPath.length === 0) {
    return { ok: false, reason: 'A document path is required.' };
  }
  const node = fields.node.trim();
  return {
    ok: true,
    name,
    request: {
      verb: buildWorkerAwlVerb({
        documentPath,
        identity: fields.identity,
        concurrency: fields.concurrency,
        reconnectInitialBackoff: fields.reconnectInitialBackoff,
        reconnectMaxBackoff: fields.reconnectMaxBackoff,
        reconnectMaxAttempts: fields.reconnectMaxAttempts,
        endpoint: fields.endpoint,
      }),
      namespaces: parseNamespacesEntry(fields.namespacesEntry),
      taskQueue: queue,
      node: node.length === 0 ? null : node,
      desired: fields.desired,
    },
  };
}

export type PutWorkerDeploymentResult = {
  /** `created`, or `replaced` when the name already existed. */
  outcome: 'created' | 'replaced';
  name: string;
  taskQueue: string;
  desired: string;
};

/** A refused worker-deployment write, carrying the server's words verbatim. */
export class WorkerDeployRefusedError extends Error {
  constructor(
    readonly code: string,
    message: string
  ) {
    super(message);
    this.name = 'WorkerDeployRefusedError';
  }
}

type Fetch = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;

export function createWorkerDeploymentsClient(
  fetchImpl: Fetch = globalThis.fetch.bind(globalThis)
) {
  return {
    async putWorkerDeployment(
      name: string,
      request: PutWorkerDeploymentRequest
    ): Promise<PutWorkerDeploymentResult> {
      const response = await fetchImpl(`/worker-deployments/${encodeURIComponent(name)}`, {
        method: 'PUT',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({
          artifact: { type: 'builtin', verb: request.verb },
          namespaces: request.namespaces,
          task_queue: request.taskQueue,
          node: request.node,
          desired: request.desired,
        }),
      });
      if (!response.ok) throw await refusalFrom(response);
      const value = expectRecord(await response.json(), 'put response');
      const outcome = expectString(value.outcome, 'outcome');
      if (outcome !== 'created' && outcome !== 'replaced') {
        throw new Error(`Invalid authoring response: unknown put outcome \`${outcome}\``);
      }
      const deployment = expectRecord(value.deployment, 'deployment');
      return {
        outcome,
        name: expectString(deployment.name, 'deployment.name'),
        taskQueue: expectString(deployment.task_queue, 'deployment.task_queue'),
        desired: expectString(deployment.desired, 'deployment.desired'),
      };
    },
  };
}

export type WorkerDeploymentsClient = ReturnType<typeof createWorkerDeploymentsClient>;

export const workerDeploymentsClient = createWorkerDeploymentsClient();

async function refusalFrom(response: Response): Promise<WorkerDeployRefusedError> {
  const failure: unknown = await response.json().catch(() => null);
  if (failure !== null && typeof failure === 'object') {
    const record = failure as Record<string, unknown>;
    if (typeof record.message === 'string') {
      return new WorkerDeployRefusedError(
        typeof record.code === 'string' ? record.code : 'unknown',
        record.message
      );
    }
  }
  return new WorkerDeployRefusedError(
    'unknown',
    `Worker deployment request failed (${response.status})`
  );
}
