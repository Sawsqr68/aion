import type { RenameMapping } from './facade-records';
import { parseLayout, parseRenameMapping } from './facade-records';
import { createGuidedFacade } from './guided-facade';

export type { RenameMapping } from './facade-records';

import type { AwlDiagnostic, CheckResult } from './facade-check';
import { parseCheckResult, parseDiagnostic } from './facade-check';
import { expectArray, expectBoolean, expectRecord, expectString } from './facade-values';
import type { LayoutRecord } from './projection-types';

// The check shapes and their parser live in `facade-check`, shared with the
// read-only deployed facade so both views of the server's `CheckResponse`
// are one type parsed one way.
export type { AwlDiagnostic, CheckResult, DiagnosticClass } from './facade-check';
export type {
  DeploymentRecord,
  GuidedDeployResult,
  GuidedStepResult,
  WorkerAvailability,
} from './guided-facade';

export type AwlDocument = { path: string; name: string };
export type WorkerRuntime = 'shell' | 'rust';
export type ScaffoldFiles = Record<string, string>;

export class GuidedFlowRefusedError extends Error {
  constructor(
    readonly code: string,
    message: string
  ) {
    super(message);
    this.name = 'GuidedFlowRefusedError';
  }
}

export type ActionParameter = { name: string; type: string };
export type RouteArgument = { name: string; expression: string };
export type GestureOperation =
  | { type: 'add_step'; name: string; prose?: string }
  | { type: 'add_type'; name: string; fields: ActionParameter[] }
  | { type: 'add_type_field'; type_name: string; name: string; field_type: string }
  | { type: 'remove_type_field'; type_name: string; name: string }
  | { type: 'add_enum_type'; name: string; variants: string[] }
  | {
      type: 'add_worker';
      name: string;
      action: { name: string; params: ActionParameter[]; return_type: string };
    }
  | { type: 'remove_worker'; name: string }
  | { type: 'remove_action'; worker: string; name: string }
  | {
      type: 'add_action';
      worker: string;
      name: string;
      params: ActionParameter[];
      return_type: string;
    }
  | {
      type: 'add_outcome_route';
      source: string;
      target: string;
      name: string;
      guard: { type: 'when'; expression: string } | { type: 'otherwise' };
      payload?: RouteArgument[];
    }
  | { type: 'add_fall_through'; source: string; target: string }
  | { type: 'edit_prose'; step: string; prose: string }
  | { type: 'rename_binding'; kind: 'step' | 'binding'; from: string; to: string }
  | { type: 'delete_step'; step: string };

export type EditResult = {
  source: string;
  diagnostics: AwlDiagnostic[];
  rename: RenameMapping | null;
};

export class GestureRefusedError extends Error {
  constructor(
    readonly code: string,
    message: string
  ) {
    super(message);
    this.name = 'GestureRefusedError';
  }
}

export class ScaffoldRefusedError extends Error {
  constructor(
    readonly code: string,
    message: string
  ) {
    super(message);
    this.name = 'ScaffoldRefusedError';
  }
}

export class AuthoringWorkspaceNotConfiguredError extends Error {
  constructor() {
    super('authoring workspace not configured');
    this.name = 'AuthoringWorkspaceNotConfiguredError';
  }
}

type Fetch = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;

export function createAuthoringFacade(fetchImpl: Fetch = globalThis.fetch.bind(globalThis)) {
  async function request(
    path: string,
    init?: RequestInit,
    documentEndpoint = false
  ): Promise<unknown> {
    const response = await fetchImpl(path, init);
    if (documentEndpoint && (response.status === 404 || response.status === 503)) {
      throw new AuthoringWorkspaceNotConfiguredError();
    }
    if (!response.ok) throw await refusalFrom(response);
    return response.json();
  }

  return {
    async check(source: string, path?: string): Promise<CheckResult> {
      return parseCheckResult(
        await request(
          '/awl/check',
          jsonInit('POST', path === undefined ? { source } : { source, path })
        )
      );
    },
    async format(source: string): Promise<string> {
      const value = expectRecord(await request('/awl/fmt', jsonInit('POST', { source })));
      return expectString(value.formatted, 'formatted');
    },
    async edit(source: string, operation: GestureOperation): Promise<EditResult> {
      const value = expectRecord(
        await request('/awl/edit', jsonInit('POST', { source, operation }))
      );
      if (!expectBoolean(value.ok, 'ok')) {
        const refused = expectRecord(value.refusal);
        throw new GestureRefusedError(
          expectString(refused.code, 'refusal.code'),
          expectString(refused.message, 'refusal.message')
        );
      }
      return {
        source: expectString(value.source, 'source'),
        diagnostics: expectArray(value.diagnostics, 'diagnostics').map(parseDiagnostic),
        rename: value.rename === undefined ? null : parseRenameMapping(value.rename),
      };
    },
    async scaffold(source: string, worker: string, runtime: WorkerRuntime): Promise<ScaffoldFiles> {
      const value = expectRecord(
        await request('/awl/scaffold', jsonInit('POST', { source, worker, runtime }))
      );
      if (!expectBoolean(value.ok, 'ok')) {
        const refusal = expectRecord(value.refusal);
        const code = expectString(refusal.code, 'refusal.code');
        const reason =
          typeof refusal.reason === 'string' ? refusal.reason : `Worker scaffold refused: ${code}`;
        throw new ScaffoldRefusedError(code, reason);
      }
      const files = expectRecord(value.files);
      return Object.fromEntries(
        Object.entries(files).map(([path, content]) => [
          path,
          expectString(content, `files.${path}`),
        ])
      );
    },
    /**
     * Creates a workspace document. With a `source`, the server stores those
     * bytes verbatim (the guide's "Try it" path); without one it synthesises
     * its placeholder workflow. Both forms are the same atomic create: an
     * existing document answers with a `DocumentExists` refusal, never an
     * overwrite.
     */
    async createDocument(name: string, source?: string): Promise<AwlDocument> {
      const value = expectRecord(
        await request(
          '/awl/documents',
          jsonInit('POST', source === undefined ? { name } : { name, source }),
          true
        )
      );
      return {
        path: expectString(value.path, 'path'),
        name: expectString(value.name, 'name'),
      };
    },
    async listDocuments(): Promise<AwlDocument[]> {
      const value = await request('/awl/documents', undefined, true);
      return expectArray(value, 'documents').map((entry) => {
        const record = expectRecord(entry);
        return { path: expectString(record.path, 'path'), name: expectString(record.name, 'name') };
      });
    },
    async loadDocument(path: string): Promise<string> {
      const value = expectRecord(
        await request(`/awl/documents/${encodeURIComponent(path)}`, undefined, true)
      );
      return expectString(value.source, 'source');
    },
    async saveDocument(path: string, source: string): Promise<string> {
      const value = expectRecord(
        await request(
          `/awl/documents/${encodeURIComponent(path)}`,
          jsonInit('PUT', { source }),
          true
        )
      );
      return expectString(value.content_hash, 'content_hash');
    },
    ...createGuidedFacade(request),
    async loadLayout(path: string): Promise<LayoutRecord> {
      return parseLayout(await request(`/awl/layout/${encodeURIComponent(path)}`, undefined, true));
    },
    async saveLayout(path: string, layout: LayoutRecord): Promise<LayoutRecord> {
      return parseLayout(
        await request(`/awl/layout/${encodeURIComponent(path)}`, jsonInit('PUT', layout), true)
      );
    },
  };
}

export const authoringFacade = createAuthoringFacade();

function jsonInit(method: string, body: unknown): RequestInit {
  return { method, headers: { 'content-type': 'application/json' }, body: JSON.stringify(body) };
}

/**
 * Builds the refusal for a non-OK authoring response.
 *
 * Lifted out of `request` so the transport does one thing: a body the server
 * shaped as a refusal keeps its own error type and message verbatim, and
 * anything else becomes a refusal naming the status. Both arms throw — a
 * failed request never returns a value.
 */
async function refusalFrom(response: Response): Promise<GuidedFlowRefusedError> {
  const failure: unknown = await response.json().catch(() => null);
  const record =
    failure !== null && typeof failure === 'object' ? (failure as Record<string, unknown>) : null;
  if (record !== null && typeof record.message === 'string') {
    return new GuidedFlowRefusedError(
      typeof record.error_type === 'string' ? record.error_type : 'AuthoringRefused',
      record.message
    );
  }
  return new GuidedFlowRefusedError(
    'AuthoringRefused',
    `Authoring request failed (${response.status})`
  );
}
