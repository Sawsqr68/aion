import { describe, expect, test } from 'bun:test';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import type { ReactNode } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';

import { ApiError, type RenameWorkflowParams, type RequestOptions } from '@/lib/api';
import {
  DISPLAY_NAME_ATTRIBUTE,
  displayNameFromEvents,
  shortWorkflowId,
} from '@/lib/workflowIdentity';
import type { Event, Namespace, RunId, WorkflowId } from '@/types';

import { RenameControl } from '../components/RenameControl';
import { type RenameClient, submitRename } from '../hooks/useRename';

const NAMESPACE = 'default' as Namespace;
const WORKFLOW_ID = '3f2a1b4c-1111-2222-3333-444455556666' as WorkflowId;
const RUN_ID = '00000000-0000-0000-0000-0000000000a1' as RunId;
const OPTIONS: RequestOptions = { namespace: NAMESPACE };

function wrap(node: ReactNode): string {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });

  return renderToStaticMarkup(
    <QueryClientProvider client={queryClient}>{node}</QueryClientProvider>
  );
}

/** Records every rename call and answers with the name as the server would record it. */
function recordingClient(): RenameClient & { calls: RenameWorkflowParams[] } {
  const calls: RenameWorkflowParams[] = [];

  return {
    calls,
    rename: async (params: RenameWorkflowParams) => {
      calls.push(params);
      return { runId: RUN_ID, displayName: params.displayName.trim() };
    },
  };
}

function searchAttributesUpdated(seq: number, attributes: Record<string, string>): Event {
  return {
    type: 'SearchAttributesUpdated',
    data: {
      envelope: { seq, recorded_at: '2026-06-05T20:00:00Z', workflow_id: WORKFLOW_ID },
      workflow_id: WORKFLOW_ID,
      attributes: Object.fromEntries(
        Object.entries(attributes).map(([key, data]) => [key, { type: 'String' as const, data }])
      ),
    },
  };
}

describe('shortWorkflowId', () => {
  // Catches: a blanket truncation that would cut a non-UUID id and silently
  // drop the characters distinguishing one run from another.
  test('shortens a canonical UUID to its first group and returns anything else whole', () => {
    expect(shortWorkflowId(WORKFLOW_ID)).toBe('3f2a1b4c');
    expect(shortWorkflowId('workflow-1')).toBe('workflow-1');
    expect(shortWorkflowId('not-a-uuid-but-quite-long-indeed')).toBe(
      'not-a-uuid-but-quite-long-indeed'
    );
  });
});

describe('displayNameFromEvents', () => {
  test('an unnamed run projects no name', () => {
    expect(displayNameFromEvents([])).toBeNull();
    expect(displayNameFromEvents([searchAttributesUpdated(1, { customer: 'acme' })])).toBeNull();
  });

  // Catches: reading the FIRST recorded name. The current name is the last
  // write, exactly as the server folds it.
  test('the last recorded write wins, by sequence rather than array order', () => {
    const events = [
      searchAttributesUpdated(9, { [DISPLAY_NAME_ATTRIBUTE]: 'renamed' }),
      searchAttributesUpdated(2, { [DISPLAY_NAME_ATTRIBUTE]: 'original' }),
    ];

    expect(displayNameFromEvents(events)).toBe('renamed');
  });

  // Catches: a later update of OTHER attributes clearing the name — the
  // server's fold merges maps, so an absent key means unchanged.
  test('a later update that does not touch the name leaves it standing', () => {
    const events = [
      searchAttributesUpdated(2, { [DISPLAY_NAME_ATTRIBUTE]: 'nightly digest' }),
      searchAttributesUpdated(5, { customer: 'acme' }),
    ];

    expect(displayNameFromEvents(events)).toBe('nightly digest');
  });
});

describe('submitRename', () => {
  // Catches: the run being addressed by anything other than its id, or the
  // name being sent untrimmed and diverging from what the server records.
  test('success posts an id-addressed rename and resolves with the recorded name', async () => {
    const client = recordingClient();

    const result = await submitRename(
      client,
      { workflowId: WORKFLOW_ID, displayName: '  nightly digest  ' },
      OPTIONS
    );

    expect(client.calls).toEqual([{ workflowId: WORKFLOW_ID, displayName: 'nightly digest' }]);
    expect(result).toEqual({ runId: RUN_ID, displayName: 'nightly digest' });
  });

  test('an explicit run id pins the rename to one generation', async () => {
    const client = recordingClient();

    await submitRename(
      client,
      { workflowId: WORKFLOW_ID, displayName: 'x', runId: RUN_ID },
      OPTIONS
    );

    expect(client.calls[0]?.runId).toBe(RUN_ID);
  });

  // Catches: a blank rename spending a round trip on a request the engine
  // refuses, or worse, being treated as a no-op success.
  test('a blank name is refused before any request is made', async () => {
    const client = recordingClient();

    await expect(
      submitRename(client, { workflowId: WORKFLOW_ID, displayName: '   ' }, OPTIONS)
    ).rejects.toThrow('cannot be blank');
    expect(client.calls).toHaveLength(0);
  });

  // Catches: a server refusal being swallowed or reinterpreted as success.
  test('a server refusal propagates verbatim', async () => {
    const failing: RenameClient = {
      rename: async () => {
        throw new ApiError(404, 'no such workflow', 'not_found');
      },
    };

    try {
      await submitRename(failing, { workflowId: WORKFLOW_ID, displayName: 'x' }, OPTIONS);
      throw new Error('expected submitRename to reject');
    } catch (error) {
      expect(error).toBeInstanceOf(ApiError);
      expect((error as ApiError).code).toBe('not_found');
    }
  });
});

describe('RenameControl', () => {
  test('an unnamed run is offered a naming affordance, a named one a rename', () => {
    expect(
      wrap(<RenameControl displayName={null} namespace={NAMESPACE} workflowId={WORKFLOW_ID} />)
    ).toContain('Name run');
    expect(
      wrap(<RenameControl displayName="nightly" namespace={NAMESPACE} workflowId={WORKFLOW_ID} />)
    ).toContain('Rename');
  });

  // Catches: the editor seeding blank and silently discarding the run's name
  // when an operator opens it to make a small correction.
  test('the open editor is seeded with the name the run currently wears', () => {
    const markup = wrap(
      <RenameControl
        displayName="nightly digest"
        initialOpen
        namespace={NAMESPACE}
        workflowId={WORKFLOW_ID}
      />
    );

    expect(markup).toContain('value="nightly digest"');
    expect(markup).toContain('Save name');
  });

  // Catches: a disabled commit with no stated reason — the operator must be
  // told WHY the rename cannot be sent.
  test('an unscoped rename is disabled and says why', () => {
    const markup = wrap(
      <RenameControl displayName={null} initialOpen namespace={null} workflowId={WORKFLOW_ID} />
    );

    expect(markup).toContain('disabled');
    expect(markup).toContain('Select a namespace to scope the rename command.');
  });
});
