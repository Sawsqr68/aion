import { describe, expect, test } from 'bun:test';

import {
  BLANK_DISPLAY_NAME_ERROR,
  EMPTY_START_WORKFLOW_DRAFT,
  type StartWorkflowDraft,
  startWorkflowDraftStore,
  startWorkflowDraftStoreFor,
  startWorkflowParamsFromDraft,
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

describe('startWorkflowParamsFromDraft', () => {
  function draft(overrides: Partial<StartWorkflowDraft> = {}): StartWorkflowDraft {
    return { ...EMPTY_START_WORKFLOW_DRAFT, workflowType: 'EmailDigest', ...overrides };
  }

  function ok(result: ReturnType<typeof startWorkflowParamsFromDraft>) {
    if (!result.ok) throw new Error(`expected params, got refusal: ${result.error}`);
    return result.params;
  }

  // Catches: an EMPTY field sending `displayName: ''`. Nothing typed is the
  // operator choosing nothing, which must reach the wire as ABSENCE — an empty
  // string is a value the server would have to interpret, and it refuses it.
  test('an empty display name field is omitted entirely, never sent as an empty string', () => {
    const params = ok(startWorkflowParamsFromDraft(draft({ displayName: '' }), {}));

    expect('displayName' in params).toBe(false);
    expect(params).toEqual({ workflowType: 'EmailDigest', input: {} });
  });

  // Catches: a whitespace-only name being silently dropped, which would start an
  // unnamed run while the operator believed they had named it. The server, and
  // every SDK, refuse a present-but-blank name; the console must give the same
  // answer rather than teach that blank means unnamed.
  test.each([
    '   ',
    '\t',
    '\n ',
    ' ',
  ])('a whitespace-only display name is refused, not quietly dropped (%j)', (blank) => {
    const result = startWorkflowParamsFromDraft(draft({ displayName: blank }), {});

    expect(result.ok).toBe(false);
    if (result.ok) throw new Error('unreachable');
    expect(result.error).toBe(BLANK_DISPLAY_NAME_ERROR);
  });

  // Catches: the name field being dropped, or sent untrimmed.
  test('a typed display name travels trimmed', () => {
    const params = ok(
      startWorkflowParamsFromDraft(draft({ displayName: '  nightly digest  ' }), { to: 'ops' })
    );

    expect(params.displayName).toBe('nightly digest');
    expect(params.input).toEqual({ to: 'ops' });
  });

  // Catches: the routing fields diverging from the omission rule, and the name
  // field being folded back INTO it. The two rules differ on purpose — the
  // server reads a blank routing key or task queue as "not selected" but
  // refuses a blank name — so this pins both halves in one place.
  test('routing fields trim-or-omit; the display name refuses a blank instead', () => {
    const filled = ok(
      startWorkflowParamsFromDraft(
        draft({ routingKey: ' shard-1 ', taskQueue: ' gpu ', displayName: ' named ' }),
        {}
      )
    );
    expect(filled).toEqual({
      workflowType: 'EmailDigest',
      input: {},
      routingKey: 'shard-1',
      taskQueue: 'gpu',
      displayName: 'named',
    });

    const blankRouting = ok(
      startWorkflowParamsFromDraft(draft({ routingKey: ' ', taskQueue: '', displayName: '' }), {})
    );
    expect(blankRouting).toEqual({ workflowType: 'EmailDigest', input: {} });

    const blankName = startWorkflowParamsFromDraft(
      draft({ routingKey: ' ', taskQueue: '', displayName: '\t' }),
      {}
    );
    expect(blankName.ok).toBe(false);
  });
});
