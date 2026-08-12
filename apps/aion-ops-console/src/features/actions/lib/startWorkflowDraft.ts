import type { JsonRecord, StartWorkflowParams } from '@/lib/api';
import { createDraftStore } from '@/lib/state';
import { isBlankDisplayName } from '@/lib/workflowIdentity';

/** Raw string state of the start form's inputs (always strings, possibly blank). */
export type StartWorkflowDraft = {
  workflowType: string;
  namespaceEntry: string;
  inputText: string;
  routingKey: string;
  taskQueue: string;
  /**
   * Optional operator-facing name for the run (#211). An EMPTY field starts the
   * run unnamed, because the request omits the field entirely. A field holding
   * only whitespace is REFUSED rather than quietly treated as empty — the
   * server, the Rust SDK, the Python SDK, and the TypeScript SDK all refuse a
   * present-but-blank name, and the console is the one surface an operator
   * would otherwise learn the wrong rule from.
   */
  displayName: string;
};

export const EMPTY_START_WORKFLOW_DRAFT: StartWorkflowDraft = {
  workflowType: '',
  namespaceEntry: '',
  inputText: '',
  routingKey: '',
  taskQueue: '',
  displayName: '',
};

/** Either the request parameters, or the reason the draft cannot be submitted. */
export type StartWorkflowDraftResult =
  | { readonly ok: true; readonly params: StartWorkflowParams }
  | { readonly ok: false; readonly error: string };

/** The message the console shows for a whitespace-only display name. */
export const BLANK_DISPLAY_NAME_ERROR =
  'Display name must not be only whitespace — clear the field to start the run unnamed.';

/**
 * Build the start-workflow request parameters from a draft plus its parsed
 * input, or refuse the draft.
 *
 * `routingKey` and `taskQueue` follow the omission rule: trim, and a blank field
 * is OMITTED — never sent as an empty string, which would make the request claim
 * a selection it does not carry.
 *
 * `displayName` does NOT follow that rule, and the difference is deliberate. The
 * server reads a blank routing key or task queue as "not selected", but it
 * REFUSES a present-but-blank `display_name` with `invalid_input` rather than
 * reading it as unnamed; the Rust, Python, and TypeScript SDKs all refuse it at
 * their own boundary for the same reason. So an EMPTY field here means "no name"
 * and is omitted, while a field the operator has actually typed into — even if
 * only whitespace — is REFUSED. Silently dropping it would start an unnamed run
 * while the operator believed they had named it, and would teach, on the one
 * surface a person actually uses, an equivalence (blank = unnamed) that every
 * other surface in the system rejects.
 *
 * The refusal lives HERE rather than beside the caller so there is no second
 * path to a parameter set: a caller cannot obtain params without passing the
 * check. Pure, so both rules are testable without a DOM.
 */
export function startWorkflowParamsFromDraft(
  draft: StartWorkflowDraft,
  input: JsonRecord
): StartWorkflowDraftResult {
  if (draft.displayName.length > 0 && isBlankDisplayName(draft.displayName)) {
    return { ok: false, error: BLANK_DISPLAY_NAME_ERROR };
  }

  const routingKey = draft.routingKey.trim();
  const taskQueue = draft.taskQueue.trim();
  const displayName = draft.displayName.trim();

  return {
    ok: true,
    params: {
      workflowType: draft.workflowType.trim(),
      input,
      ...(routingKey.length === 0 ? {} : { routingKey }),
      ...(taskQueue.length === 0 ? {} : { taskQueue }),
      ...(displayName.length === 0 ? {} : { displayName }),
    },
  };
}

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
