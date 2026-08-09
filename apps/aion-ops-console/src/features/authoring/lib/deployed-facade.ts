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

/**
 * The versions the deployed read surface lists: those whose archive carries
 * readable AWL source. A version with nothing to read is genuinely deployed,
 * but the operator ruled (2026-08-10) that a read surface must not list
 * entries that cannot be opened — the complete inventory lives on the
 * versions listing, not here.
 */
export function versionsWithSource(versions: readonly DeployedVersion[]): DeployedVersion[] {
  return versions.filter((version) => version.source.state === 'available');
}

export type DeployedSchema = { path: string; text: string | null; byteLength: number };

/**
 * One signal the archived contract commits, with its payload schema. The
 * server's `DeployedSignal.input_schema` is NOT optional — every committed
 * signal carries a schema — so there is no null here.
 */
export type DeployedSignal = {
  name: string;
  /** Payload schema, draft 2020-12, as committed into the version identity. */
  inputSchema: Record<string, unknown>;
};

export type DeployedDocument = {
  workflowType: string;
  contentHash: string;
  documentName: string;
  source: string;
  schemas: DeployedSchema[];
  projection: CheckResult;
  /**
   * JSON Schema for the workflow's start input. `null` covers two honest
   * absences that the console treats identically: a legacy pre-contract
   * archive (the server answers `input_schema: null`) and an older server
   * that does not send the field at all. In both cases there is no schema to
   * build a form from, so the raw-JSON start input remains the whole surface.
   */
  inputSchema: Record<string, unknown> | null;
  /**
   * The signals the archived contract commits to THIS entry — absence is
   * semantic, mirroring the server's `Option<Vec<DeployedSignal>>`:
   *
   * - `DeployedSignal[]` (possibly empty — the contract genuinely commits
   *   none) when the entry's contract carries a signal set;
   * - `null` when the contract commits no signal set for this entry: an
   *   additional workflow entry, a pre-contract archive, or an older server
   *   that does not send the field. Null is NOT the empty list: a console
   *   must never offer signals to an entry whose contract never accepted
   *   them, and must never claim "no signals" where nothing was committed.
   */
  signals: DeployedSignal[] | null;
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
    inputSchema: parseNullableSchema(record.input_schema, 'input_schema'),
    signals:
      record.signals === undefined || record.signals === null
        ? null
        : expectArray(record.signals, 'signals').map(parseDeployedSignal),
  };
}

/**
 * A JSON Schema field that may be absent (older server), `null` (legacy
 * pre-contract archive), or an object. A present non-object value is a server
 * this console does not understand and is reported, never folded into "no
 * schema" — that would silently strip a contract the server DID send.
 */
function parseNullableSchema(value: unknown, field: string): Record<string, unknown> | null {
  if (value === undefined || value === null) return null;
  return expectRecord(value, field);
}

function parseDeployedSignal(value: unknown): DeployedSignal {
  const record = expectRecord(value);
  return {
    name: expectString(record.name, 'signals.name'),
    // Per committed signal the schema is REQUIRED (the server's field is not
    // an Option); a missing or non-object schema is a malformed response.
    inputSchema: expectRecord(record.input_schema, 'signals.input_schema'),
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
