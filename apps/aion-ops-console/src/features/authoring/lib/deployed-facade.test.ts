import { describe, expect, test } from 'bun:test';

import {
  createDeployedFacade,
  DeployedReadRefusedError,
  type DeployedSourceState,
  DeployedSurfaceUnavailableError,
} from './deployed-facade';

const PROJECTION = {
  ok: true,
  deploys_green: true,
  steps: 0,
  diagnostics: [],
  semantic: {
    entries: [],
    graph: { steps: [], edges: [], child_calls: [] },
    studio: { builtins: [], types: [], workers: [] },
  },
};

const DOCUMENT_SOURCE =
  '//! Deployed probe.\nworkflow deployed_probe\n  outcome done: type Summary, route success\n';

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
  return { facade: createDeployedFacade(fetchImpl), calls };
}

describe('the deployed facade parses every source state distinctly', () => {
  const cases: { name: string; payload: unknown; expected: DeployedSourceState }[] = [
    {
      name: 'available',
      payload: { state: 'available', document_name: 'dev_brief.awl', schema_count: 2 },
      expected: { state: 'available', documentName: 'dev_brief.awl', schemaCount: 2 },
    },
    { name: 'absent', payload: { state: 'absent' }, expected: { state: 'absent' } },
    {
      name: 'not_persisted',
      payload: { state: 'not_persisted' },
      expected: { state: 'not_persisted' },
    },
    {
      name: 'unreadable',
      payload: { state: 'unreadable', reason: 'archive read failed' },
      expected: { state: 'unreadable', reason: 'archive read failed' },
    },
  ];

  for (const item of cases) {
    test(item.name, async () => {
      const { facade } = stub(200, [
        {
          workflow_type: 'dev_brief',
          content_hash: 'a'.repeat(64),
          loaded: true,
          route_active: true,
          loaded_at: '2026-08-01T00:00:00+00:00',
          deployed_at: null,
          source: item.payload,
        },
      ]);
      const [version] = await facade.listDeployedVersions();
      expect(version?.source).toEqual(item.expected);
      expect(version?.deployedAt).toBeNull();
      expect(version?.loadedAt).toBe('2026-08-01T00:00:00+00:00');
    });
  }
});

/**
 * An unrecognised state is a server this console does not understand. Folding
 * it into "absent" would render a fault as a healthy Gleam package — the exact
 * class of fabricated claim the hardcoded "deploys green" badge was.
 */
test('an unrecognised source state is reported, never treated as absence', async () => {
  const { facade } = stub(200, [
    {
      workflow_type: 'dev_brief',
      content_hash: 'a'.repeat(64),
      loaded: true,
      route_active: true,
      loaded_at: null,
      deployed_at: null,
      source: { state: 'quantum' },
    },
  ]);
  await expect(facade.listDeployedVersions()).rejects.toThrow('unknown deployed source state');
});

/**
 * 🔴 THE READ-ONLY PIN AT THE FACADE, both halves. The document arrives
 * byte-identical (survival) — and every request the facade makes is a GET with
 * no body (absence). A facade that served the right bytes through a POST would
 * pass the first half alone.
 */
test('the deployed document round-trips verbatim and every request is a bodyless GET', async () => {
  const calls: Call[] = [];
  const { facade } = stub(
    200,
    {
      workflow_type: 'dev_brief',
      content_hash: 'b'.repeat(64),
      document_name: 'dev_brief.awl',
      source: DOCUMENT_SOURCE,
      schemas: [{ path: 'schemas/brief.schema.json', text: '{"type":"object"}', byte_length: 17 }],
      projection: PROJECTION,
    },
    calls
  );

  const document = await facade.loadDeployedDocument('dev_brief', 'b'.repeat(64));

  expect(document.source).toBe(DOCUMENT_SOURCE);
  expect(document.documentName).toBe('dev_brief.awl');
  expect(document.schemas).toEqual([
    { path: 'schemas/brief.schema.json', text: '{"type":"object"}', byteLength: 17 },
  ]);
  expect(document.projection.ok).toBe(true);
  expect(document.projection.deploysGreen).toBe(true);

  await facade.listDeployedVersions().catch(() => undefined);
  expect(calls.length).toBeGreaterThan(0);
  for (const call of calls) {
    expect(call.init?.method).toBe('GET');
    expect(call.init?.body ?? null).toBeNull();
  }
});

/**
 * The start-input contract fields, in the exact shapes
 * `crates/aion-server/src/awl/deployed/types.rs` serializes:
 *
 * - primary entry: `input_schema: <object>`, `signals: <array>` (POSSIBLY
 *   EMPTY — the contract genuinely commits none);
 * - additional entry: `input_schema: <object>`, `signals: null` (an
 *   `AdditionalWorkflowContract` carries no signal set);
 * - pre-contract archive: `input_schema: null`, `signals: null`;
 * - older server: both fields absent.
 *
 * Absence is semantic: `signals: null` must parse as null (no committed
 * signal set), NEVER as the empty list, and never crash the document read —
 * the crash was blocker B1, hidden by a fixture that sent `signals: []`
 * where the server sends `null` (right and plausible-wrong coinciding).
 */
describe('the deployed document start-input contract', () => {
  const base = {
    workflow_type: 'dev_brief',
    content_hash: 'b'.repeat(64),
    document_name: 'dev_brief.awl',
    source: DOCUMENT_SOURCE,
    schemas: [],
    projection: PROJECTION,
  };
  const load = (facade: ReturnType<typeof stub>['facade']) =>
    facade.loadDeployedDocument('dev_brief', 'b'.repeat(64));

  // Catches: a parser that drops or reshapes the schema/signals the server
  // DID send — the start form would silently lose its contract.
  test('a primary entry carries its schema and signal set verbatim', async () => {
    const inputSchema = {
      type: 'object',
      properties: { to: { type: 'string' } },
      required: ['to'],
    };
    const { facade } = stub(200, {
      ...base,
      input_schema: inputSchema,
      signals: [{ name: 'approve', input_schema: { type: 'object', properties: {} } }],
    });
    const document = await load(facade);
    expect(document.inputSchema).toEqual(inputSchema);
    expect(document.signals).toEqual([
      { name: 'approve', inputSchema: { type: 'object', properties: {} } },
    ]);
  });

  // Catches: absent-vs-empty collapsed. `signals: []` is a real answer — the
  // contract commits an empty signal set — and must stay a (present) empty
  // list, not become null.
  test('a primary entry with an empty committed signal set parses as the empty list', async () => {
    const { facade } = stub(200, { ...base, input_schema: { type: 'object' }, signals: [] });
    const document = await load(facade);
    expect(document.signals).toEqual([]);
    expect(document.signals).not.toBeNull();
  });

  // B1 executed proof, case 1: an ADDITIONAL entry — schema object, signals
  // null. Catches: expectArray rejecting the whole document (the blocker),
  // and null being faked into [].
  test('an additional entry (schema present, signals null) parses with signals null', async () => {
    const { facade } = stub(200, {
      ...base,
      input_schema: { type: 'object', properties: {} },
      signals: null,
    });
    const document = await load(facade);
    expect(document.inputSchema).toEqual({ type: 'object', properties: {} });
    expect(document.signals).toBeNull();
  });

  // B1 executed proof, case 2: a PRE-CONTRACT archive — both null.
  test('a pre-contract archive (both fields null) parses as no schema, signals null', async () => {
    const { facade } = stub(200, { ...base, input_schema: null, signals: null });
    const document = await load(facade);
    expect(document.inputSchema).toBeNull();
    expect(document.signals).toBeNull();
  });

  // Catches: a parser that REQUIRES the new fields — an older server would
  // break the deployed view entirely. Absent maps to the same semantic null
  // as a pre-contract archive: nothing was committed.
  test('an older server that omits both fields parses as null schema and null signals', async () => {
    const { facade } = stub(200, base);
    const document = await load(facade);
    expect(document.inputSchema).toBeNull();
    expect(document.signals).toBeNull();
  });

  // Catches: a present-but-malformed schema folded into "no schema" — that
  // would silently strip a contract the server DID send.
  test('a present non-object input_schema is reported, never read as absence', async () => {
    const { facade } = stub(200, { ...base, input_schema: 'not-a-schema' });
    await expect(load(facade)).rejects.toThrow('input_schema');
  });

  // Catches: a committed signal without its (non-optional) schema coerced
  // into something renderable instead of being reported.
  test('a signal missing its payload schema is reported', async () => {
    const { facade } = stub(200, {
      ...base,
      input_schema: { type: 'object' },
      signals: [{ name: 'approve' }],
    });
    await expect(load(facade)).rejects.toThrow('signals.input_schema');
  });
});

/** Path segments are encoded, so no workflow type can reshape the URL. */
test('the document URL encodes both segments', async () => {
  const calls: Call[] = [];
  const { facade } = stub(404, null, calls);
  await facade.loadDeployedDocument('a/b', 'c d').catch(() => undefined);
  expect(calls[0]?.url).toBe('/awl/deployed/a%2Fb/c%20d');
});

/**
 * A 404 with no refusal body is the unmounted route family — the operator's
 * deploy surface is off. A 404 WITH one is a real answer about a real version
 * and must not be flattened into "the feature is missing": the operator would
 * go looking at their config instead of at the workflow.
 */
test('a bodyless 404 is the unavailable surface; a refusal 404 keeps its own code', async () => {
  const unavailable = stub(404, null).facade;
  await expect(unavailable.listDeployedVersions()).rejects.toBeInstanceOf(
    DeployedSurfaceUnavailableError
  );

  const absent = stub(404, {
    code: 'invalid_input',
    error_type: 'DeployedAwlSourceAbsent',
    message: 'carries no archived AWL source: it was authored in Gleam, or deployed before …',
  }).facade;
  const refusal = await absent
    .loadDeployedDocument('gleam_flow', 'c'.repeat(64))
    .then(() => null)
    .catch((error: unknown) => error);
  expect(refusal).toBeInstanceOf(DeployedReadRefusedError);
  expect((refusal as DeployedReadRefusedError).code).toBe('DeployedAwlSourceAbsent');
  expect((refusal as DeployedReadRefusedError).message).toContain('Gleam');
});

/** A denial carries the server's own words, not a status number. */
test('a refused read surfaces the server message', async () => {
  const { facade } = stub(403, {
    code: 'deploy_denied',
    error_type: 'DeployDenied',
    message: 'subject `alice` is not authorized to deploy',
  });
  await expect(facade.listDeployedVersions()).rejects.toThrow('not authorized to deploy');
});

import type { DeployedVersion } from './deployed-facade';
import { versionsWithSource } from './deployed-facade';

test('versionsWithSource keeps only versions whose archive carries readable source', () => {
  const base: Omit<DeployedVersion, 'source' | 'contentHash'> = {
    workflowType: 'dev_brief',
    loaded: true,
    routeActive: false,
    loadedAt: null,
    deployedAt: null,
  };
  const versions: DeployedVersion[] = [
    {
      ...base,
      contentHash: 'a'.repeat(64),
      source: { state: 'available', documentName: 'dev_brief.awl', schemaCount: 0 },
    },
    { ...base, contentHash: 'b'.repeat(64), source: { state: 'absent' } },
    { ...base, contentHash: 'c'.repeat(64), source: { state: 'not_persisted' } },
    {
      ...base,
      contentHash: 'd'.repeat(64),
      source: { state: 'unreadable', reason: 'archive truncated' },
    },
  ];

  const kept = versionsWithSource(versions);

  expect(kept.map((version) => version.contentHash)).toEqual(['a'.repeat(64)]);
});
