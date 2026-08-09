import { createDraftStore } from '@/lib/state';

/** Raw string state of the start form's inputs (always strings, possibly blank). */
export type StartWorkflowDraft = {
  workflowType: string;
  namespaceEntry: string;
  inputText: string;
  routingKey: string;
  taskQueue: string;
};

export const EMPTY_START_WORKFLOW_DRAFT: StartWorkflowDraft = {
  workflowType: '',
  namespaceEntry: '',
  inputText: '',
  routingKey: '',
  taskQueue: '',
};

/** A half-filled start form survives navigating away; cleared on a confirmed start. */
export const startWorkflowDraftStore = createDraftStore({
  name: 'start-workflow',
  empty: EMPTY_START_WORKFLOW_DRAFT,
});

type StartWorkflowDraftStore = typeof startWorkflowDraftStore;

const scopedDraftStores = new Map<string, StartWorkflowDraftStore>();

/**
 * The draft store for a given scope. No scope is the Actions page's original
 * store — same storage key, so existing drafts survive and existing callers
 * see zero change. A named scope (the authoring run panel) gets its OWN
 * sessionStorage key, so an embedded start form and the Actions page never
 * clobber each other's half-filled drafts. Memoized per scope: a draft store
 * is an identity, and minting a second one for the same key would race two
 * persistence bindings against each other.
 */
export function startWorkflowDraftStoreFor(scope?: string): StartWorkflowDraftStore {
  if (scope === undefined) {
    return startWorkflowDraftStore;
  }
  const existing = scopedDraftStores.get(scope);
  if (existing !== undefined) {
    return existing;
  }
  const created = createDraftStore({
    name: `start-workflow:${scope}`,
    empty: EMPTY_START_WORKFLOW_DRAFT,
  });
  scopedDraftStores.set(scope, created);
  return created;
}
