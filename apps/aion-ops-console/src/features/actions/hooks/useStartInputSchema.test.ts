import { describe, expect, test } from 'bun:test';

import type { DeployedDocument, DeployedVersion } from '@/features/authoring/lib/deployed-facade';

import {
  resolveStartSchema,
  routeActiveVersion,
  type StartSchemaQueryFacts,
  startSchemaEnabled,
} from './useStartInputSchema';

function version(overrides: Partial<DeployedVersion>): DeployedVersion {
  return {
    workflowType: 'dev_brief',
    contentHash: 'a'.repeat(64),
    loaded: true,
    routeActive: false,
    loadedAt: null,
    deployedAt: null,
    source: { state: 'absent' },
    ...overrides,
  };
}

describe('routeActiveVersion', () => {
  // Catches: consulting the wrong deployed version — the schema must come
  // from the version a start will RUN (route-active), never merely the newest
  // deploy of the type, and never another type's route-active version.
  test('picks the route-active version of the requested type only', () => {
    const versions = [
      version({ contentHash: 'b'.repeat(64), routeActive: false }),
      version({ contentHash: 'c'.repeat(64), routeActive: true }),
      version({ workflowType: 'other_flow', contentHash: 'd'.repeat(64), routeActive: true }),
    ];
    expect(routeActiveVersion(versions, 'dev_brief')?.contentHash).toBe('c'.repeat(64));
    expect(routeActiveVersion(versions, 'other_flow')?.contentHash).toBe('d'.repeat(64));
  });

  // Catches: falling back to an inactive version when nothing is routed — a
  // start of that type would not run it, so its schema must not govern.
  test('answers null when no version of the type is route-active', () => {
    expect(routeActiveVersion([version({ routeActive: false })], 'dev_brief')).toBeNull();
    expect(routeActiveVersion([], 'dev_brief')).toBeNull();
  });
});

describe('startSchemaEnabled', () => {
  // M7. Catches: `deployGranted` dropped from the enabled condition — the
  // deployed surface is deploy-gated, so an ungranted caller's lookup would
  // be a guaranteed 403 on every keystroke.
  test('an ungranted caller never enables the lookup', () => {
    expect(startSchemaEnabled('dev_brief', false)).toBe(false);
  });

  // Catches: an empty type firing a lookup, or a granted caller being denied.
  test('a granted caller with a typed workflow type enables it, and only then', () => {
    expect(startSchemaEnabled('dev_brief', true)).toBe(true);
    expect(startSchemaEnabled('', true)).toBe(false);
    expect(startSchemaEnabled('', false)).toBe(false);
  });
});

function documentWith(schema: Record<string, unknown> | null): DeployedDocument {
  return {
    workflowType: 'dev_brief',
    contentHash: 'a'.repeat(64),
    documentName: 'dev_brief.awl',
    source: 'workflow dev_brief\n',
    schemas: [],
    projection: { ok: true, deploysGreen: true, steps: 1, diagnostics: [], semantic: null },
    inputSchema: schema,
    signals: null,
  };
}

function facts(overrides: Partial<StartSchemaQueryFacts>): StartSchemaQueryFacts {
  return {
    enabled: true,
    versions: { isPending: false, isError: false },
    active: version({ routeActive: true }),
    document: { isPending: false, isError: false, data: documentWith({ type: 'object' }) },
    ...overrides,
  };
}

describe('resolveStartSchema', () => {
  // M7. Every rung of the degradation ladder, each against its inversion —
  // the five branches the review named as untestable while they lived inline
  // in the hook. Each case would fail if its branch were inverted or removed.
  test('disabled is idle: no schema, not loading', () => {
    expect(resolveStartSchema(facts({ enabled: false }))).toEqual({
      schema: null,
      isLoading: false,
    });
  });

  test('a failed versions list degrades to no schema, not to loading forever', () => {
    expect(resolveStartSchema(facts({ versions: { isPending: false, isError: true } }))).toEqual({
      schema: null,
      isLoading: false,
    });
  });

  test('a pending versions list is loading (the caller must hold a placeholder)', () => {
    expect(
      resolveStartSchema(facts({ versions: { isPending: true, isError: false }, active: null }))
    ).toEqual({ schema: null, isLoading: true });
  });

  test('no route-active version means no schema', () => {
    expect(resolveStartSchema(facts({ active: null }))).toEqual({
      schema: null,
      isLoading: false,
    });
  });

  test('a failed document read degrades to no schema', () => {
    expect(
      resolveStartSchema(facts({ document: { isPending: false, isError: true, data: undefined } }))
    ).toEqual({ schema: null, isLoading: false });
  });

  test('a pending document read is loading', () => {
    expect(
      resolveStartSchema(facts({ document: { isPending: true, isError: false, data: undefined } }))
    ).toEqual({ schema: null, isLoading: true });
  });

  test('a loaded document yields its schema — including a pre-contract null', () => {
    expect(resolveStartSchema(facts({}))).toEqual({
      schema: { type: 'object' },
      isLoading: false,
    });
    expect(
      resolveStartSchema(
        facts({ document: { isPending: false, isError: false, data: documentWith(null) } })
      )
    ).toEqual({ schema: null, isLoading: false });
  });
});
