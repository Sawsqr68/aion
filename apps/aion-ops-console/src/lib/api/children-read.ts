import type { ActivityId, Namespace, RunId, WorkflowId, WorkflowStatus } from '@/types';

import { ApiError } from './api-error';
import { apiErrorFromResponse, readJson, requireWorkflowStatus } from './client-normalize';
import {
  type ApiCredentials,
  buildScopedHeaders,
  buildUrl,
  stripTrailingSlash,
} from './client-transport';

export const CHILDREN_ENDPOINT = '/workflows/children';

type FetchFn = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;

export type ChildLeg = {
  workflowId: WorkflowId;
  runId: RunId | null;
  workflowType: string;
  displayName: string | null;
  status: WorkflowStatus | null;
  currentActivityId: ActivityId | null;
  currentAttempt: number | null;
};

export type ChildrenReadOptions = {
  baseUrl?: string;
  fetchImpl?: FetchFn;
  credentials?: ApiCredentials;
};

/** Typed client for the server's history-derived direct-child projection. */
export class ChildrenReadClient {
  private readonly baseUrl: string;
  private readonly fetchImpl: FetchFn;
  private readonly credentials: ApiCredentials | undefined;

  constructor(options: ChildrenReadOptions = {}) {
    this.baseUrl = stripTrailingSlash(options.baseUrl ?? '');
    this.fetchImpl = options.fetchImpl ?? ((input, init) => globalThis.fetch(input, init));
    this.credentials = options.credentials;
  }

  async listChildren(
    namespace: Namespace,
    workflowId: WorkflowId,
    runId: RunId | null,
    signal?: AbortSignal
  ): Promise<ChildLeg[]> {
    const response = await this.fetchImpl(buildUrl(this.baseUrl, CHILDREN_ENDPOINT), {
      method: 'POST',
      headers: buildScopedHeaders(this.credentials),
      body: JSON.stringify({ namespace, workflow_id: workflowId, run_id: runId }),
      ...(signal === undefined ? {} : { signal }),
    });
    if (!response.ok) {
      const errorBody = await readJson(response).catch(() => null);
      throw apiErrorFromResponse(response.status, errorBody);
    }
    const body = await readJson(response);
    const children = isRecord(body) ? body.children : undefined;
    if (!Array.isArray(children)) {
      throw new ApiError(200, 'workflows/children response missing a children array');
    }
    return children.map(readChildLeg);
  }
}

function readChildLeg(value: unknown): ChildLeg {
  if (!isRecord(value)) {
    throw new ApiError(200, 'workflows/children child row is not an object');
  }
  const workflowId = nullableString(value.workflow_id);
  const runId = nullableString(value.run_id);
  const displayName = nullableString(value.display_name);
  const currentActivityId = nullableNumber(value.current_activity_id);
  const currentAttempt = nullableNumber(value.current_attempt);
  if (workflowId === null || typeof value.workflow_type !== 'string') {
    throw new ApiError(200, 'workflows/children child row has invalid identity or status');
  }
  const status = value.status === null ? null : requireWorkflowStatus(value.status);
  if ((runId === null) !== (status === null)) {
    throw new ApiError(200, 'workflows/children child row run_id and status disagree');
  }
  return {
    workflowId: workflowId as WorkflowId,
    runId: runId as RunId | null,
    workflowType: value.workflow_type,
    displayName,
    status,
    currentActivityId,
    currentAttempt,
  };
}

function nullableString(value: unknown): string | null {
  if (value === null) return null;
  if (typeof value === 'string') return value;
  throw new ApiError(200, 'workflows/children child row has an invalid string field');
}

function nullableNumber(value: unknown): number | null {
  if (value === null) return null;
  if (typeof value === 'number' && Number.isSafeInteger(value) && value >= 0) return value;
  throw new ApiError(200, 'workflows/children child row has an invalid numeric field');
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
