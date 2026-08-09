/**
 * The managed-worker surface client.
 *
 * Reads `GET /workers/managed` (the supervisor's join of durable deployment
 * records with live instance state) and drives the three lifecycle commands,
 * `POST /workers/managed/{name}/{start|stop|restart}`. Every refusal keeps the
 * server's own words: a 409 `invalid_state` on an uncommissioned server IS the
 * remedy message, and flattening it into "request failed" would strip the one
 * sentence that tells the operator what to do.
 */

import {
  expectArray,
  expectBoolean,
  expectNumber,
  expectRecord,
  expectString,
  nullableNumber,
  nullableString,
} from './wire';

/** Live supervision state, as serialized by the server (kebab-case tokens). */
export type ManagedWorkerState =
  | 'starting'
  | 'running'
  | 'backoff'
  | 'stopped'
  | 'failed'
  | 'uncommissioned';

/** Durable operator intent for a deployment. */
export type DesiredState = 'running' | 'stopped';

/** Identity of the binary a deployment named at deploy time. */
export type DeployedBinaryIdentity = {
  version: string;
  commit: string;
  dirty: string;
  contentHash: string;
};

/** Identity of the executable the current (or most recent) spawn actually ran. */
export type SpawnedBinary = {
  path: string;
  contentHash: string;
  version: string | null;
  commit: string | null;
  dirty: string | null;
};

/** How a managed worker process last ended. */
export type ManagedWorkerExit = {
  at: string;
  code: number | null;
  signal: number | null;
  requested: boolean;
};

/** One managed worker: what was asked for, and what is true. */
export type ManagedWorkerStatus = {
  name: string;
  taskQueue: string;
  desired: DesiredState;
  state: ManagedWorkerState;
  pid: number | null;
  processGroup: number | null;
  restarts: number;
  lastExit: ManagedWorkerExit | null;
  lastError: string | null;
  deployedBinary: DeployedBinaryIdentity;
  spawnBinary: SpawnedBinary | null;
};

/** The whole managed-worker surface for this server. */
export type ManagedWorkerReport = {
  commissioned: boolean;
  /** The server's remedy for an uncommissioned server, verbatim; else null. */
  remedy: string | null;
  workers: ManagedWorkerStatus[];
  /** Durable rows that are present but could not be decoded. */
  undecodable: string[];
};

export type ManagedWorkerAction = 'start' | 'stop' | 'restart';

/**
 * A refused managed-worker request. `message` is the server's message
 * verbatim; `code` is the stable wire code (`not_found`, `invalid_state`,
 * `deploy_denied`, ...) callers may branch on.
 */
export class ManagedWorkersRefusedError extends Error {
  constructor(
    readonly status: number,
    readonly code: string,
    readonly errorType: string | null,
    message: string
  ) {
    super(message);
    this.name = 'ManagedWorkersRefusedError';
  }
}

/**
 * The lifecycle routes are not mounted on this server.
 *
 * The router mounts `POST /workers/managed/{name}/start|stop|restart` only
 * under `[deploy].enabled` — with deploy off every such path is a BARE 404
 * with no wire-error body, while `GET /workers/managed` stays mounted. That
 * bodyless 404 is therefore a STRUCTURAL fact about the server, not an answer
 * about any worker: it must never be paraphrased into a status-number error,
 * and it is distinct from a 404 that CARRIES a `not_found` body (an unknown
 * worker name, rendered with the server's own message).
 */
export class ManagedWorkersSurfaceUnmountedError extends Error {
  constructor() {
    super(
      'This server does not mount worker lifecycle commands: it runs with [deploy] enabled=false, so it is not a deploy target. The fleet report stays readable; to start, stop, or restart managed workers, run the server with [deploy] enabled=true.'
    );
    this.name = 'ManagedWorkersSurfaceUnmountedError';
  }
}

type Fetch = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;

export function createManagedWorkersClient(fetchImpl: Fetch = globalThis.fetch.bind(globalThis)) {
  async function request(path: string, init?: RequestInit): Promise<unknown> {
    const response = await fetchImpl(path, init);
    if (response.ok) return response.json();
    throw await refusalFrom(response);
  }

  return {
    async report(): Promise<ManagedWorkerReport> {
      return parseReport(await request('/workers/managed'));
    },
    async act(name: string, action: ManagedWorkerAction): Promise<ManagedWorkerStatus> {
      const value = expectRecord(
        await request(`/workers/managed/${encodeURIComponent(name)}/${action}`, {
          method: 'POST',
        }),
        'action response'
      );
      return parseWorker(value.worker);
    },
  };
}

export type ManagedWorkersClient = ReturnType<typeof createManagedWorkersClient>;

export const managedWorkersClient = createManagedWorkersClient();

/**
 * A refusal keeps the server's own message and code. The branches are the
 * server's actual answer shapes:
 *
 * - a body carrying `message` is a wire error — the server's words, verbatim
 *   (a 404 WITH a `not_found` body is a real answer about a named worker);
 * - a BARE 404 with no wire body is the unmounted lifecycle surface
 *   (`[deploy] enabled=false` leaves those routes off the router entirely) —
 *   a structural condition, stated as one;
 * - anything else non-OK (a proxy page, an empty 5xx) is reported with its
 *   status — never swallowed.
 */
async function refusalFrom(response: Response): Promise<Error> {
  const failure: unknown = await response.json().catch(() => null);
  if (failure !== null && typeof failure === 'object') {
    const record = failure as Record<string, unknown>;
    if (typeof record.message === 'string') {
      return new ManagedWorkersRefusedError(
        response.status,
        typeof record.code === 'string' ? record.code : 'unknown',
        typeof record.error_type === 'string' ? record.error_type : null,
        record.message
      );
    }
  }
  if (response.status === 404) {
    return new ManagedWorkersSurfaceUnmountedError();
  }
  return new ManagedWorkersRefusedError(
    response.status,
    'unknown',
    null,
    `Managed-worker request failed (${response.status})`
  );
}

function parseReport(value: unknown): ManagedWorkerReport {
  const record = expectRecord(value, 'report');
  return {
    commissioned: expectBoolean(record.commissioned, 'commissioned'),
    remedy: nullableString(record.remedy, 'remedy'),
    workers: expectArray(record.workers, 'workers').map(parseWorker),
    undecodable: expectArray(record.undecodable, 'undecodable').map((name, index) =>
      expectString(name, `undecodable[${index}]`)
    ),
  };
}

function parseWorker(value: unknown): ManagedWorkerStatus {
  const record = expectRecord(value, 'worker');
  return {
    name: expectString(record.name, 'worker.name'),
    taskQueue: expectString(record.task_queue, 'worker.task_queue'),
    desired: parseDesired(record.desired),
    state: parseState(record.state),
    pid: nullableNumber(record.pid, 'worker.pid'),
    processGroup: nullableNumber(record.process_group, 'worker.process_group'),
    restarts: expectNumber(record.restarts, 'worker.restarts'),
    lastExit:
      record.last_exit === null || record.last_exit === undefined
        ? null
        : parseExit(record.last_exit),
    lastError: nullableString(record.last_error, 'worker.last_error'),
    deployedBinary: parseDeployedBinary(record.deployed_binary),
    spawnBinary:
      record.spawn_binary === null || record.spawn_binary === undefined
        ? null
        : parseSpawnBinary(record.spawn_binary),
  };
}

const WORKER_STATES: readonly ManagedWorkerState[] = [
  'starting',
  'running',
  'backoff',
  'stopped',
  'failed',
  'uncommissioned',
];

function parseState(value: unknown): ManagedWorkerState {
  const token = expectString(value, 'worker.state');
  const state = WORKER_STATES.find((candidate) => candidate === token);
  // An unrecognised state is a server this console does not understand —
  // reported, never folded into some "close enough" state.
  if (state === undefined) {
    throw new Error(`Invalid managed-worker response: unknown state \`${token}\``);
  }
  return state;
}

function parseDesired(value: unknown): DesiredState {
  const token = expectString(value, 'worker.desired');
  if (token !== 'running' && token !== 'stopped') {
    throw new Error(`Invalid managed-worker response: unknown desired state \`${token}\``);
  }
  return token;
}

function parseExit(value: unknown): ManagedWorkerExit {
  const record = expectRecord(value, 'worker.last_exit');
  return {
    at: expectString(record.at, 'last_exit.at'),
    code: nullableNumber(record.code, 'last_exit.code'),
    signal: nullableNumber(record.signal, 'last_exit.signal'),
    requested: expectBoolean(record.requested, 'last_exit.requested'),
  };
}

function parseDeployedBinary(value: unknown): DeployedBinaryIdentity {
  const record = expectRecord(value, 'worker.deployed_binary');
  return {
    version: expectString(record.version, 'deployed_binary.version'),
    commit: expectString(record.commit, 'deployed_binary.commit'),
    dirty: expectString(record.dirty, 'deployed_binary.dirty'),
    contentHash: expectString(record.content_hash, 'deployed_binary.content_hash'),
  };
}

function parseSpawnBinary(value: unknown): SpawnedBinary {
  const record = expectRecord(value, 'worker.spawn_binary');
  return {
    path: expectString(record.path, 'spawn_binary.path'),
    contentHash: expectString(record.content_hash, 'spawn_binary.content_hash'),
    version: nullableString(record.version, 'spawn_binary.version'),
    commit: nullableString(record.commit, 'spawn_binary.commit'),
    dirty: nullableString(record.dirty, 'spawn_binary.dirty'),
  };
}
