import type {
  ActivityId,
  InterventionCapabilities,
  InterventionKind,
  Namespace,
  RunId,
  WorkflowId,
} from '@/types';

import type { JsonRecord } from './client-normalize';
import type { ApiCredentials } from './client-transport';

/** Start-workflow inputs (camelCase); the body is built server-shaped by ApiClient. */
export type StartWorkflowParams = {
  workflowType: string;
  /** Plain JSON input, auto-wrapped server-side as an `application/json` payload. */
  input?: JsonRecord | undefined;
  /** Optional R-4 steered-start routing key. */
  routingKey?: string | undefined;
  /** Empty/absent selects the namespace's default activity task queue. */
  taskQueue?: string | undefined;
  /**
   * Optional operator-facing display name for the run being started (#211). A
   * LABEL over UUID identity, recorded durably on the start; absent starts the
   * run unnamed. What is recorded is a workflow-level attribute, so it reads
   * back for every run of the workflow. Nothing resolves a workflow by name.
   */
  displayName?: string | undefined;
};

/**
 * Rename inputs (camelCase) for `POST /workflows/rename` (#211).
 *
 * The TARGET is always an id: `workflowId` plus an optional `runId` (absent
 * targets the latest run, which the server resolves). `displayName` is the new
 * label — never a lookup key.
 */
export type RenameWorkflowParams = {
  workflowId: WorkflowId;
  displayName: string;
  /** Absent targets the workflow's latest run (server-resolved). */
  runId?: RunId | undefined;
};

/**
 * One addressable activity attempt and the controls its worker supports.
 *
 * `runId` is the generation axis and is REQUIRED: a continue-as-new chain reuses
 * one workflow id while ordinals and attempts restart per generation, so
 * `(activityId, attempt)` alone names a coordinate, not an attempt.
 */
export type AttemptCapabilities = {
  runId: RunId;
  activityId: ActivityId;
  attempt: number;
  capabilities: InterventionCapabilities;
};

/**
 * Target attempt plus the neutral intervention primitive.
 *
 * `runId` is REQUIRED — the server refuses a run-less body. Routing a steer by
 * `(workflow, activity, attempt)` alone would deliver it to whichever generation
 * happened to own the colliding coordinate, which is a control-plane fault, not
 * a display bug.
 */
export type InterveneParams = {
  workflowId: WorkflowId;
  runId: RunId;
  activityId: ActivityId;
  attempt: number;
  kind: InterventionKind;
};

export type FetchFn = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;

export type ApiClientOptions = {
  baseUrl?: string;
  fetchImpl?: FetchFn;
  credentials?: ApiCredentials;
};

export type RequestOptions = {
  namespace: Namespace;
  credentials?: ApiCredentials | undefined;
  signal?: AbortSignal | undefined;
};

export type WorkflowPageRequest = {
  cursor?: string | undefined;
  limit?: number | undefined;
};

/** Request-time authorization capabilities returned by `GET /whoami`. */
export type Capabilities = {
  subject: string;
  authEnabled: boolean;
  deployGranted: boolean;
  allNamespaces: boolean;
  namespaces: Namespace[];
};

/** Raw `/whoami` envelope (server snake_case). */
export type WhoAmIResponse = {
  subject?: unknown;
  auth_enabled?: unknown;
  deploy_granted?: unknown;
  all_namespaces?: unknown;
  namespaces?: unknown;
};

/** `GET /changelog` envelope: the changelog embedded in the running binary. */
export type ChangelogResponse = {
  markdown: string;
};

/**
 * `GET /build` envelope (#123): the running server's compiled-in identity.
 * Unestablished fields carry the literal string `"unknown"`, never an empty
 * value — the server stamps absence as a word so it reads as a measurement.
 */
export type BuildIdentityResponse = {
  version: string;
  commit: string;
  dirty: string;
  built_at: string;
};

/**
 * `GET /update-status` envelope (#189 slice one): the installed version
 * joined with the last completed MANUAL update check. `latest_known` and
 * `checked_at` are `null` until a check has ever completed — a fresh server
 * has honestly never checked, and no field fabricates otherwise.
 */
export type UpdateStatusResponse = {
  installed: string;
  latest_known: string | null;
  checked_at: string | null;
};

export type RequestBody = JsonRecord | undefined;
