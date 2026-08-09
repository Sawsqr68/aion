import { useQuery } from '@tanstack/react-query';
import { useCallback } from 'react';

import {
  type DeployedDocument,
  type DeployedVersion,
  deployedFacade,
} from '@/features/authoring/lib/deployed-facade';

/** The two deployed reads this hook needs; injectable for tests. */
export type StartSchemaClient = Pick<
  typeof deployedFacade,
  'listDeployedVersions' | 'loadDeployedDocument'
>;

export type UseStartInputSchemaOptions = {
  /** The workflow type the operator has entered (trimmed). */
  workflowType: string;
  /**
   * Whether the resolved caller may read the deployed surface. `GET
   * /awl/deployed/...` sits behind the deploy grant, so without it every
   * schema read would be a guaranteed refusal — the hook stays idle and the
   * start form keeps its raw JSON editor, exactly as for a schema-less type.
   */
  deployGranted: boolean;
  /** Injected client (tests); defaults to the shared deployed facade. */
  client?: StartSchemaClient | undefined;
};

export type StartInputSchemaResult = {
  /**
   * The route-active deployed version's start-input schema, or `null` when
   * there is none to offer: no deployed version of this type, a legacy
   * archive with no recorded contract, an older server, a caller without the
   * deploy grant, or a deployed surface that is switched off. All of those
   * mean the same thing to the start form — raw JSON is the whole surface.
   */
  schema: Record<string, unknown> | null;
  /** True while a schema might still arrive; the caller holds a placeholder. */
  isLoading: boolean;
};

/**
 * Whether the schema lookup runs at all: only for a non-empty workflow type,
 * and only for a caller holding the deploy grant (without it every deployed
 * read is a guaranteed refusal — the hook must not hammer a surface it is
 * certain to be refused on).
 */
export function startSchemaEnabled(workflowType: string, deployGranted: boolean): boolean {
  return deployGranted && workflowType.length > 0;
}

/**
 * The version whose schema governs a start of `workflowType`: the ROUTE-ACTIVE
 * deployed version of that type — the code a start will actually run — never
 * merely the newest deploy.
 */
export function routeActiveVersion(
  list: DeployedVersion[],
  workflowType: string
): DeployedVersion | null {
  return (
    list.find((version) => version.workflowType === workflowType && version.routeActive) ?? null
  );
}

/** The minimal query facts {@link resolveStartSchema} decides from. */
export type StartSchemaQueryFacts = {
  enabled: boolean;
  versions: { isPending: boolean; isError: boolean };
  /** The route-active version selected from the list, or null. */
  active: DeployedVersion | null;
  document: { isPending: boolean; isError: boolean; data: DeployedDocument | undefined };
};

/**
 * The whole degradation ladder, pure. Every rung is a DECISION, not an
 * accident of hook ordering:
 *
 * - disabled → idle (no schema, not loading — the raw editor, immediately);
 * - versions list failed → no schema (the raw editor still works; the server
 *   remains the input's authority, so a schema is an enhancement, never a
 *   gate on starting);
 * - versions list pending → loading (a schema might still arrive; the caller
 *   must not flash the raw editor first);
 * - no route-active version of the type → no schema;
 * - document failed → no schema; document pending → loading;
 * - document loaded → its `input_schema` (which is itself null for a
 *   pre-contract archive).
 */
export function resolveStartSchema(facts: StartSchemaQueryFacts): StartInputSchemaResult {
  if (!facts.enabled) return { schema: null, isLoading: false };
  if (facts.versions.isError) return { schema: null, isLoading: false };
  if (facts.versions.isPending) return { schema: null, isLoading: true };
  if (facts.active === null) return { schema: null, isLoading: false };
  if (facts.document.isError) return { schema: null, isLoading: false };
  if (facts.document.isPending || facts.document.data === undefined) {
    return { schema: null, isLoading: true };
  }
  return { schema: facts.document.data.inputSchema, isLoading: false };
}

/**
 * Resolve the start-input JSON Schema for a workflow type. A thin react-query
 * shell over {@link startSchemaEnabled}, {@link routeActiveVersion}, and
 * {@link resolveStartSchema} — every decision lives in those pure functions.
 */
export function useStartInputSchema({
  workflowType,
  deployGranted,
  client = deployedFacade,
}: UseStartInputSchemaOptions): StartInputSchemaResult {
  const enabled = startSchemaEnabled(workflowType, deployGranted);

  // Stable select: re-created only when the type changes, not per render, so
  // react-query's memoized selection does not churn.
  const selectActive = useCallback(
    (list: DeployedVersion[]) => routeActiveVersion(list, workflowType),
    [workflowType]
  );

  const versions = useQuery<DeployedVersion[], Error, DeployedVersion | null>({
    enabled,
    queryKey: ['actions', 'start-schema', 'versions'],
    queryFn: () => client.listDeployedVersions(),
    select: selectActive,
    retry: false,
  });

  const active = versions.data ?? null;
  const document = useQuery<DeployedDocument>({
    enabled: enabled && active !== null,
    queryKey: ['actions', 'start-schema', 'document', active?.workflowType, active?.contentHash],
    queryFn: () =>
      client.loadDeployedDocument(active?.workflowType ?? '', active?.contentHash ?? ''),
    // A deployed version is immutable (named by content hash): once its
    // document is fetched it can never go stale.
    staleTime: Number.POSITIVE_INFINITY,
    retry: false,
  });

  return resolveStartSchema({
    enabled,
    versions: { isPending: versions.isPending, isError: versions.isError },
    active,
    document: {
      isPending: document.isPending,
      isError: document.isError,
      data: document.data,
    },
  });
}
