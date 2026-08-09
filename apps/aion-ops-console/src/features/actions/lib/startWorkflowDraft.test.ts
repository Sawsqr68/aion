import { describe, expect, test } from 'bun:test';

import {
  EMPTY_START_WORKFLOW_DRAFT,
  startWorkflowDraftStore,
  startWorkflowDraftStoreFor,
} from './startWorkflowDraft';

describe('startWorkflowDraftStoreFor', () => {
  // Catches: the absent-scope path minting a NEW store — existing callers
  // must keep the exact store (and storage key) they always had.
  test('no scope is the original Actions store, identically', () => {
    expect(startWorkflowDraftStoreFor()).toBe(startWorkflowDraftStore);
    expect(startWorkflowDraftStoreFor(undefined)).toBe(startWorkflowDraftStore);
  });

  // Catches: a fresh store per call — two mounts of the same scope would
  // race two persistence bindings over one key.
  test('the same scope always resolves the same store', () => {
    expect(startWorkflowDraftStoreFor('authoring')).toBe(startWorkflowDraftStoreFor('authoring'));
  });

  // Catches: the clobber this scoping exists to prevent — an embedded form
  // writing through to the Actions page's draft (or another scope's).
  test('two scopes hold independent drafts, and the default store is untouched', () => {
    const before = startWorkflowDraftStore.getState().draft;
    const authoring = startWorkflowDraftStoreFor('authoring');
    const other = startWorkflowDraftStoreFor('elsewhere');

    authoring.getState().setDraft({ workflowType: 'hello_flow', inputText: '{"n": 1}' });
    other.getState().setDraft({ workflowType: 'other_flow' });

    expect(authoring.getState().draft.workflowType).toBe('hello_flow');
    expect(authoring.getState().draft.inputText).toBe('{"n": 1}');
    expect(other.getState().draft.workflowType).toBe('other_flow');
    expect(other.getState().draft.inputText).toBe('');
    expect(startWorkflowDraftStore.getState().draft).toEqual(before);

    // Clearing one scope leaves the other's draft standing.
    authoring.getState().clearDraft();
    expect(authoring.getState().draft).toEqual(EMPTY_START_WORKFLOW_DRAFT);
    expect(other.getState().draft.workflowType).toBe('other_flow');
    other.getState().clearDraft();
  });
});
