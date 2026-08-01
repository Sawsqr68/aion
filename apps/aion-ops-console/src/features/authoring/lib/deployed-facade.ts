import { type CheckResult, parseCheckResult } from './facade-check';
import {
  expectArray,
  expectBoolean,
  expectNumber,
  expectRecord,
  expectString,
  nullableString,
} from './facade-values';

/**
 * What a deployed version carries by way of authored AWL source.
 *
 * Absence has three distinguishable forms and none of them is an error: a
 * package can have been authored in Gleam, a version can be loaded from an
 * operator file the server never persisted, and a persisted archive can refuse
 * to read back. Collapsing them into one "no source" would tell the operator a
 * healthy Gleam workflow and a corrupt archive are the same thing.
 */
export type DeployedSourceState =
  | { state: 'available'; documentName: string; schemaCount: number }
  | { state: 'absent' }
  | { state: 'not_persisted' }
  | { state: 'unreadable'; reason: string };

export type DeployedVersion = {
  workflowType: string;
  contentHash: string;
  loaded: boolean;
  routeActive: boolean;
  loadedAt: string | null;
  deployedAt: string | null;
  source: DeployedSourceState;
};

export type DeployedSchema = { path: string; text: string | null; byteLength: number };

export type DeployedDocument = {
  workflowType: string;
  contentHash: string;
  documentName: string;
  source: string;
  schemas: DeployedSchema[];
  projection: CheckResult;
};

/** The server mounts no deployed surface (its deploy surface is switched off). */
export class DeployedSurfaceUnavailableError extends Error {
  constructor() {
    super('deployed workflow surface not available');
    this.name = 'DeployedSurfaceUnavailableError';
  }
}

/** A deployed read was refused, carrying the server's own error type. */
export class DeployedReadRefusedError extends Error {
  constructor(
    readonly code: string,
    message: string
  ) {
    super(message);
    this.name = 'DeployedReadRefusedError';
  }
}

type Fetch = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;

/**
 * The read-only deployed-workflow facade.
 *
 * Deliberately a SEPARATE object from the authoring facade rather than more
 * methods on it. Every method here is a GET, and there is no save, format,
 * gesture, or layout write anywhere on it — so a deployed view holding this
 * facade has no mutation to reach, by construction rather than by a disabled
 * button. The server's two deployed routes carry no other method either.
 */
export function createDeployedFacade(fetchImpl: Fetch = globalThis.fetch.bind(globalThis)) {
  async function read(path: string): Promise<unknown> {
    const response = await fetchImpl(path, { method: 'GET' });
    if (response.ok) return response.json();
    const failure: unknown = await response.json().catch(() => null);
    const refusal = refusalFrom(failure);
    // A 404 with no refusal body is the unmounted route family — the operator's
    // deploy surface is off. A 404 WITH one is a real answer about a real
    // version (unknown version, or no archived source), and must not be
    // flattened into "the feature is missing".
    if (refusal === null) {
      if (response.status === 404) throw new DeployedSurfaceUnavailableError();
      throw new DeployedReadRefusedError(
        'DeployedReadFailed',
        `Deployed workflow request failed (${response.status})`
      );
    }
    throw refusal;
  }

  return {
    async listDeployedVersions(): Promise<DeployedVersion[]> {
      return expectArray(await read('/awl/deployed'), 'deployed versions').map(
        parseDeployedVersion
      );
    },
    async loadDeployedDocument(
      workflowType: string,
      contentHash: string
    ): Promise<DeployedDocument> {
      return parseDeployedDocument(
        await read(
          `/awl/deployed/${encodeURIComponent(workflowType)}/${encodeURIComponent(contentHash)}`
        )
      );
    },
  };
}

export const deployedFacade = createDeployedFacade();

function refusalFrom(failure: unknown): DeployedReadRefusedError | null {
  if (failure === null || typeof failure !== 'object') return null;
  const record = failure as Record<string, unknown>;
  if (typeof record.message !== 'string') return null;
  const code = typeof record.error_type === 'string' ? record.error_type : 'DeployedReadFailed';
  return new DeployedReadRefusedError(code, record.message);
}

function parseDeployedVersion(value: unknown): DeployedVersion {
  const record = expectRecord(value);
  return {
    workflowType: expectString(record.workflow_type, 'workflow_type'),
    contentHash: expectString(record.content_hash, 'content_hash'),
    loaded: expectBoolean(record.loaded, 'loaded'),
    routeActive: expectBoolean(record.route_active, 'route_active'),
    loadedAt: nullableString(record.loaded_at ?? null, 'loaded_at'),
    deployedAt: nullableString(record.deployed_at ?? null, 'deployed_at'),
    source: parseSourceState(record.source),
  };
}

function parseSourceState(value: unknown): DeployedSourceState {
  const record = expectRecord(value);
  const state = expectString(record.state, 'source.state');
  if (state === 'available') {
    return {
      state,
      documentName: expectString(record.document_name, 'source.document_name'),
      schemaCount: expectNumber(record.schema_count, 'source.schema_count'),
    };
  }
  if (state === 'absent' || state === 'not_persisted') return { state };
  if (state === 'unreadable') {
    return { state, reason: expectString(record.reason, 'source.reason') };
  }
  // An unrecognised state is a server this console does not understand. It is
  // reported, never quietly folded into "absent" — that would render a fault as
  // a healthy Gleam package.
  throw new Error(`Invalid authoring response: unknown deployed source state \`${state}\``);
}

function parseDeployedDocument(value: unknown): DeployedDocument {
  const record = expectRecord(value);
  return {
    workflowType: expectString(record.workflow_type, 'workflow_type'),
    contentHash: expectString(record.content_hash, 'content_hash'),
    documentName: expectString(record.document_name, 'document_name'),
    source: expectString(record.source, 'source'),
    schemas: expectArray(record.schemas, 'schemas').map(parseDeployedSchema),
    projection: parseCheckResult(record.projection),
  };
}

function parseDeployedSchema(value: unknown): DeployedSchema {
  const record = expectRecord(value);
  return {
    path: expectString(record.path, 'schemas.path'),
    text: nullableString(record.text ?? null, 'schemas.text'),
    byteLength: expectNumber(record.byte_length, 'schemas.byte_length'),
  };
}
