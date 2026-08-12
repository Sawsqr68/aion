import { describe, expect, test } from 'bun:test';
import { renderToStaticMarkup } from 'react-dom/server';
import { createMemoryRouter, RouterProvider } from 'react-router';

import { ApiClient, type WorkflowPage } from '@/lib/api';
import { DISPLAY_NAME_ATTRIBUTE } from '@/lib/workflowIdentity';
import type { Event, Namespace, WorkflowFilter, WorkflowSummary } from '@/types';
import { affectsWorkflowSummaryProjection, patchWorkflowPage } from '../hooks/useLiveListUpdates';
import { queryWorkflowPage, workflowQueryKey } from '../hooks/useWorkflowQuery';
import {
  EMPTY_WORKFLOW_LIST_FILTER_STATE,
  type WorkflowListPaginationState,
  workflowFilterFromState,
} from '../types';
import { NewAboveBanner, WorkflowListBody } from './WorkflowList';
import { WorkflowRow } from './WorkflowRow';

const workflow: WorkflowSummary = {
  workflow_id: 'workflow-1',
  workflow_type: 'EmailDigest',
  status: 'Running',
  started_at: '2026-06-05T20:00:00Z',
  ended_at: null,
  parent: null,
};

const page: WorkflowPage<WorkflowSummary> = {
  items: [workflow],
  nextCursor: null,
  hasMore: false,
};

describe('WorkflowListBody', () => {
  test('renders rows for a returned workflow page', () => {
    const markup = renderWithRouter(
      <WorkflowListBody
        query={queryState({ data: page })}
        canGoPrevious={false}
        onPrevious={() => undefined}
        onNext={() => undefined}
      />
    );

    expect(markup).toContain('workflow-1');
    expect(markup).toContain('EmailDigest');
    expect(markup).toContain('Running');
    expect(markup).toContain('/workflows/workflow-1');
  });

  test('renders loading skeleton rows while initially loading', () => {
    const markup = renderToStaticMarkup(
      <WorkflowListBody
        query={queryState({ isPending: true })}
        canGoPrevious={false}
        onPrevious={() => undefined}
        onNext={() => undefined}
      />
    );

    expect(markup).toContain('Loading workflow summaries');
  });

  test('renders an empty state when no workflows match', () => {
    const markup = renderToStaticMarkup(
      <WorkflowListBody
        query={queryState({ data: { items: [], nextCursor: null, hasMore: false } })}
        canGoPrevious={false}
        onPrevious={() => undefined}
        onNext={() => undefined}
      />
    );

    expect(markup).toContain('No workflows match this filter');
  });

  test('renders an error state with a retry affordance', () => {
    const markup = renderToStaticMarkup(
      <WorkflowListBody
        query={queryState({ error: new Error('server unavailable'), isError: true })}
        canGoPrevious={false}
        onPrevious={() => undefined}
        onNext={() => undefined}
      />
    );

    expect(markup).toContain('Could not load workflows');
    expect(markup).toContain('server unavailable');
    expect(markup).toContain('Retry');
  });
});

describe('WorkflowRow', () => {
  const uuid = '3f2a1b4c-1111-2222-3333-444455556666';

  function renderRow(summary: WorkflowSummary): string {
    return renderWithRouter(
      <table>
        <tbody>
          <WorkflowRow workflow={summary} />
        </tbody>
      </table>
    );
  }

  test('links workflow rows to the detail route', () => {
    expect(renderRow(workflow)).toContain('/workflows/workflow-1');
  });

  // #211 R2. Catches: a named row showing the NAME ALONE. A name is a label,
  // so the id must stay visible and the link must stay id-addressed.
  test('a named row shows the name AND the short id, still linked by id', () => {
    const markup = renderRow({ ...workflow, workflow_id: uuid, display_name: 'nightly digest' });

    expect(markup).toContain('nightly digest');
    expect(markup).toContain('3f2a1b4c');
    expect(markup).toContain(`/workflows/${uuid}`);
  });

  // Catches: an unnamed row regressing from the bare UUID it has always shown.
  test('an unnamed row falls back to the bare id, unchanged', () => {
    const markup = renderRow({ ...workflow, workflow_id: uuid });

    expect(markup).toContain(uuid);
    expect(markup).toContain(`/workflows/${uuid}`);
  });

  // Catches: a blank name from a foreign writer rendering as empty space where
  // the id used to be — a row an operator could not address.
  test('a blank name is treated as unnamed, never as an empty label', () => {
    const markup = renderRow({ ...workflow, workflow_id: uuid, display_name: '   ' });

    expect(markup).toContain(uuid);
  });
});

describe('live workflow list updates', () => {
  test('patches status transitions and inserts newly matching started workflows', () => {
    const filter = workflowFilterFromState(EMPTY_WORKFLOW_LIST_FILTER_STATE);
    const completed = patchWorkflowPage(
      page,
      workflowCompleted(2, workflow.workflow_id),
      filter,
      50
    );

    expect(completed?.items[0]?.status).toBe('Completed');
    expect(completed?.items[0]?.ended_at).toBe('2026-06-05T20:02:00Z');

    const inserted = patchWorkflowPage(
      completed ?? page,
      workflowStarted(3, 'workflow-2', 'EmailDigest'),
      filter,
      50
    );

    expect(inserted?.items.map((item) => item.workflow_id)).toEqual(['workflow-2', 'workflow-1']);
    expect(inserted?.items[0]?.status).toBe('Running');
  });

  test('patches an existing row status without inserting or reordering', () => {
    const filter = workflowFilterFromState(EMPTY_WORKFLOW_LIST_FILTER_STATE);
    const patched = patchWorkflowPage(
      page,
      workflowCompleted(2, workflow.workflow_id),
      filter,
      50,
      { allowInsert: false }
    );

    expect(patched?.items.map((item) => item.workflow_id)).toEqual(['workflow-1']);
    expect(patched?.items[0]?.status).toBe('Completed');
  });

  test('projects every summary-changing lifecycle status and clears a reopened terminal', () => {
    const filter = workflowFilterFromState(EMPTY_WORKFLOW_LIST_FILTER_STATE);
    const pausedEvent = workflowPaused(2, workflow.workflow_id);
    const continuedEvent = workflowContinuedAsNew(3, workflow.workflow_id);
    const reopenedEvent = workflowReopened(4, workflow.workflow_id);
    const resumedEvent = workflowResumed(5, workflow.workflow_id);

    expect(
      [pausedEvent, continuedEvent, reopenedEvent, resumedEvent].every(
        affectsWorkflowSummaryProjection
      )
    ).toBeTrue();

    const paused = patchWorkflowPage(page, pausedEvent, filter, 50);
    expect(paused?.items[0]).toMatchObject({ status: 'Paused', ended_at: null });

    const continued = patchWorkflowPage(page, continuedEvent, filter, 50);
    expect(continued?.items[0]).toMatchObject({
      status: 'ContinuedAsNew',
      ended_at: '2026-06-05T20:03:00Z',
    });

    const reopened = patchWorkflowPage(continued ?? page, reopenedEvent, filter, 50);
    expect(reopened?.items[0]).toMatchObject({ status: 'Running', ended_at: null });
  });

  // #211. Catches: the live path patching status but NOT the name, leaving a
  // list where one column updates live and another silently does not — the
  // operator would have no way to tell which columns to trust.
  test('a rename arriving live patches the row name in place', () => {
    const filter = workflowFilterFromState(EMPTY_WORKFLOW_LIST_FILTER_STATE);
    const event = searchAttributesUpdated(2, workflow.workflow_id, {
      [DISPLAY_NAME_ATTRIBUTE]: 'nightly digest',
    });

    expect(affectsWorkflowSummaryProjection(event)).toBeTrue();

    const renamed = patchWorkflowPage(page, event, filter, 50);

    expect(renamed?.items).toHaveLength(1);
    expect(renamed?.items[0]?.display_name).toBe('nightly digest');
    // A rename is not a lifecycle transition: nothing else about the row moves.
    expect(renamed?.items[0]).toMatchObject({ status: 'Running', ended_at: null });

    // And it applies on a deeper page too — an in-place patch is not an insert.
    const deeper = patchWorkflowPage(page, event, filter, 50, { allowInsert: false });
    expect(deeper?.items[0]?.display_name).toBe('nightly digest');
  });

  // Catches: a rename for an off-page workflow being counted as a new row
  // above, or spending a refetch — it changes nothing the operator can see.
  test('a rename for a workflow not on the page leaves the page untouched', () => {
    const filter = workflowFilterFromState(EMPTY_WORKFLOW_LIST_FILTER_STATE);
    const event = searchAttributesUpdated(2, 'workflow-elsewhere', {
      [DISPLAY_NAME_ATTRIBUTE]: 'other run',
    });

    expect(patchWorkflowPage(page, event, filter, 50)).toBe(page);
    expect(patchWorkflowPage(page, event, filter, 50, { allowInsert: false })).toBe(page);
  });

  // #211 BOTH ARMS OF THE SELECTOR BYPASS, at the consumer.
  //
  // A rename now reaches EVERY status subscriber, because a label change has no
  // lifecycle meaning and the server-side selector stopped forcing it into the
  // `Running` bucket. A bypass on its own is only half the contract — it is
  // only safe because the consumer guards the other half. Both directions have
  // to bite, and the SAME event has to drive both, or this pins nothing:
  //
  //   arm 1 (the bypass earns its keep): a rename recorded on a COMPLETED run
  //     reaches a terminal-filtered list and the displayed name updates. Before
  //     the bypass this event never arrived at all, and the name shown went
  //     stale until a refetch.
  //   arm 2 (the guard holds): that very same event, delivered to a
  //     Running-filtered list that does not hold the workflow, inserts NOTHING.
  //     Delivering renames to every status bucket would otherwise let a
  //     completed run appear in a Running list.
  test('a rename on a completed run updates a terminal-filtered row and inserts into no other list', () => {
    const completedRun: WorkflowSummary = {
      ...workflow,
      workflow_id: 'workflow-done',
      status: 'Completed',
      ended_at: '2026-06-05T20:05:00Z',
    };
    const terminalPage: WorkflowPage<WorkflowSummary> = {
      items: [completedRun],
      nextCursor: null,
      hasMore: false,
    };
    const completedFilter = workflowFilterFromState({
      ...EMPTY_WORKFLOW_LIST_FILTER_STATE,
      status: 'Completed',
    });
    const runningFilter = workflowFilterFromState({
      ...EMPTY_WORKFLOW_LIST_FILTER_STATE,
      status: 'Running',
    });

    // ONE event drives both arms.
    const event = searchAttributesUpdated(3, completedRun.workflow_id, {
      [DISPLAY_NAME_ATTRIBUTE]: 'settled overnight',
    });

    // ARM 1 — the terminal-filtered consumer's row updates in place, and the
    // run stays terminal: a rename is not a lifecycle transition.
    const patched = patchWorkflowPage(terminalPage, event, completedFilter, 50);
    expect(patched?.items).toHaveLength(1);
    expect(patched?.items[0]?.display_name).toBe('settled overnight');
    expect(patched?.items[0]).toMatchObject({
      status: 'Completed',
      ended_at: '2026-06-05T20:05:00Z',
    });

    // ARM 2 — the same event against a Running-filtered list that does not hold
    // the workflow inserts nothing, on both the insert-allowed and
    // insert-suppressed paths. `toBe(page)` is the strong form: the cached page
    // object is returned IDENTICAL, so nothing re-renders and nothing is
    // miscounted as a new row above.
    expect(patchWorkflowPage(page, event, runningFilter, 50)).toBe(page);
    expect(patchWorkflowPage(page, event, runningFilter, 50, { allowInsert: false })).toBe(page);

    // CONTROL: arm 2 must be the RENAME guard biting, not the filter or an
    // empty page doing the work. A genuinely new matching start against the
    // same Running-filtered page DOES insert — so "nothing inserted" above is
    // attributable to the event being a rename and to nothing else.
    const started = workflowStarted(4, 'workflow-fresh', workflow.workflow_type);
    const insertedPage = patchWorkflowPage(page, started, runningFilter, 50);
    expect(insertedPage?.items[0]?.workflow_id).toBe('workflow-fresh');
  });

  // Catches: an attribute update with no name in it being treated as a rename,
  // and a re-recorded identical name churning the cache for no change.
  test('a non-naming attribute update, and a no-op rename, change nothing', () => {
    const filter = workflowFilterFromState(EMPTY_WORKFLOW_LIST_FILTER_STATE);
    const customerOnly = searchAttributesUpdated(2, workflow.workflow_id, { customer: 'acme' });

    expect(affectsWorkflowSummaryProjection(customerOnly)).toBeFalse();

    const named = patchWorkflowPage(
      page,
      searchAttributesUpdated(2, workflow.workflow_id, { [DISPLAY_NAME_ATTRIBUTE]: 'same' }),
      filter,
      50
    );
    const again = patchWorkflowPage(
      named ?? page,
      searchAttributesUpdated(3, workflow.workflow_id, { [DISPLAY_NAME_ATTRIBUTE]: 'same' }),
      filter,
      50
    );

    // The identical re-record returns the SAME page object: no re-render.
    expect(again).toBe(named);
  });

  test('off page 1, a newly-started workflow is NOT inserted into the cursor view', () => {
    const filter = workflowFilterFromState(EMPTY_WORKFLOW_LIST_FILTER_STATE);
    const result = patchWorkflowPage(
      page,
      workflowStarted(3, 'workflow-2', 'EmailDigest'),
      filter,
      50,
      { allowInsert: false }
    );

    // null signals the caller to surface a "new above" affordance rather than
    // corrupting the deeper page's cursor anchor (R3).
    expect(result).toBeNull();
  });
});

describe('NewAboveBanner', () => {
  test('is hidden when there are no new workflows above', () => {
    const markup = renderToStaticMarkup(<NewAboveBanner count={0} onShow={() => undefined} />);
    expect(markup).toBe('');
  });

  test('surfaces a count and a jump-to-top action when rows arrive above', () => {
    const markup = renderToStaticMarkup(<NewAboveBanner count={3} onShow={() => undefined} />);
    expect(markup).toContain('3 new workflows started above');
    expect(markup).toContain('Jump to top');
  });

  test('uses singular phrasing for a single new workflow', () => {
    const markup = renderToStaticMarkup(<NewAboveBanner count={1} onShow={() => undefined} />);
    expect(markup).toContain('1 new workflow started above');
  });
});

describe('workflow list query path', () => {
  test('query helper requests namespaced summary pages with generated filter and pagination', async () => {
    const namespace = 'default' as Namespace;
    const filter = workflowFilterFromState({
      ...EMPTY_WORKFLOW_LIST_FILTER_STATE,
      workflowType: 'EmailDigest',
      status: 'Failed',
      startedAfter: '2026-06-05T20:00',
    });
    const requests: unknown[] = [];
    const client = new ApiClient({
      fetchImpl: async (_input, init) => {
        requests.push(JSON.parse(String(init?.body)));
        return Response.json({ items: [workflow], next_cursor: null, has_more: false });
      },
    });

    const result = await queryWorkflowPage(client, namespace, filter, {
      cursor: 'cursor-1',
      limit: 25,
    });

    expect(requests).toEqual([
      {
        namespace,
        filter: {
          workflow_type: 'EmailDigest',
          status: 'Failed',
          started_after: '2026-06-05T20:00',
          started_before: null,
          parent: null,
        } satisfies WorkflowFilter,
        cursor: 'cursor-1',
        limit: 25,
      },
    ]);
    expect(result.items).toEqual([workflow]);
  });

  test('query key includes namespace, filter, and pagination', () => {
    const namespace = 'tenant-a' as Namespace;
    const filter = workflowFilterFromState(EMPTY_WORKFLOW_LIST_FILTER_STATE);
    const pageRequest = { cursor: undefined, limit: 50 };

    expect(workflowQueryKey(namespace, filter, pageRequest)).toEqual([
      'workflows',
      namespace,
      filter,
      pageRequest,
    ]);
  });
});

describe('workflow list filter and pagination state', () => {
  test('filter changes produce a new WorkflowFilter and reset pagination', () => {
    const previousPagination: WorkflowListPaginationState = {
      cursor: 'cursor-2',
      previousCursors: ['', 'cursor-1'],
    };
    const filter = workflowFilterFromState({
      ...EMPTY_WORKFLOW_LIST_FILTER_STATE,
      workflowType: 'EmailDigest',
      status: 'Failed',
      startedAfter: '2026-06-05T20:00',
    });
    const resetPagination = resetPaginationOnFilterChange(previousPagination);

    expect(filter).toEqual({
      workflow_type: 'EmailDigest',
      status: 'Failed',
      started_after: '2026-06-05T20:00',
      started_before: null,
      parent: null,
    } satisfies WorkflowFilter);
    expect(resetPagination).toEqual({ cursor: undefined, previousCursors: [] });
  });
});

type QueryStateOverrides = {
  data?: WorkflowPage<WorkflowSummary>;
  error?: Error;
  isError?: boolean;
  isFetching?: boolean;
  isPending?: boolean;
};

function queryState(overrides: QueryStateOverrides) {
  return {
    data: overrides.data,
    error: overrides.error ?? null,
    isError: overrides.isError ?? false,
    isFetching: overrides.isFetching ?? false,
    isPending: overrides.isPending ?? false,
    refetch: () => Promise.resolve({}),
  } as ReturnType<typeof import('../hooks/useWorkflowQuery').useWorkflowQuery>;
}

function workflowStarted(seq: number, workflowId: string, workflowType: string): Event {
  return {
    type: 'WorkflowStarted',
    data: {
      envelope: { seq, recorded_at: `2026-06-05T20:0${seq}:00Z`, workflow_id: workflowId },
      input: { content_type: 'Json', bytes: [123, 125] },
      workflow_type: workflowType,
      run_id: '00000000-0000-0000-0000-0000000000a1',
      parent_run_id: null,
      package_version: '1.0.0',
    },
  };
}

/** A recorded search-attribute update (a rename carries the display-name key). */
function searchAttributesUpdated(
  seq: number,
  workflowId: string,
  attributes: Record<string, string>
): Event {
  return {
    type: 'SearchAttributesUpdated',
    data: {
      envelope: { seq, recorded_at: `2026-06-05T20:0${seq}:00Z`, workflow_id: workflowId },
      workflow_id: workflowId,
      attributes: Object.fromEntries(
        Object.entries(attributes).map(([key, data]) => [key, { type: 'String' as const, data }])
      ),
    },
  };
}

function workflowCompleted(seq: number, workflowId: string): Event {
  return {
    type: 'WorkflowCompleted',
    data: {
      envelope: { seq, recorded_at: `2026-06-05T20:0${seq}:00Z`, workflow_id: workflowId },
      result: { content_type: 'Json', bytes: [123, 125] },
    },
  };
}

function workflowContinuedAsNew(seq: number, workflowId: string): Event {
  return {
    type: 'WorkflowContinuedAsNew',
    data: {
      envelope: { seq, recorded_at: `2026-06-05T20:0${seq}:00Z`, workflow_id: workflowId },
      input: { content_type: 'Json', bytes: [123, 125] },
      workflow_type: null,
      parent_run_id: '00000000-0000-0000-0000-0000000000a1',
    },
  };
}

function workflowReopened(seq: number, workflowId: string): Event {
  return {
    type: 'WorkflowReopened',
    data: {
      envelope: { seq, recorded_at: `2026-06-05T20:0${seq}:00Z`, workflow_id: workflowId },
      run_id: '00000000-0000-0000-0000-0000000000a1',
      reopened: [],
    },
  };
}

function workflowPaused(seq: number, workflowId: string): Event {
  return {
    type: 'WorkflowPaused',
    data: {
      envelope: { seq, recorded_at: `2026-06-05T20:0${seq}:00Z`, workflow_id: workflowId },
      run_id: '00000000-0000-0000-0000-0000000000a1',
      reason: null,
      operator: null,
    },
  };
}

function workflowResumed(seq: number, workflowId: string): Event {
  return {
    type: 'WorkflowResumed',
    data: {
      envelope: { seq, recorded_at: `2026-06-05T20:0${seq}:00Z`, workflow_id: workflowId },
      run_id: '00000000-0000-0000-0000-0000000000a1',
      operator: null,
    },
  };
}

function resetPaginationOnFilterChange(
  pagination: WorkflowListPaginationState
): WorkflowListPaginationState {
  void pagination;
  return { cursor: undefined, previousCursors: [] };
}

function renderWithRouter(children: React.ReactNode) {
  const router = createMemoryRouter([{ path: '*', element: children }]);

  return renderToStaticMarkup(<RouterProvider router={router} />);
}
