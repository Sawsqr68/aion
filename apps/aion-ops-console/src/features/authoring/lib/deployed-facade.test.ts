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
