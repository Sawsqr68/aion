import { describe, expect, test } from 'bun:test';

import type { WorkflowPage } from '@/lib/api';
import type { Namespace, WorkflowFilter, WorkflowId, WorkflowSummary } from '@/types';

import { UPDATE_CHECK_WORKFLOW_TYPE } from './contract';
import { fetchCheckRunStatus, hasRecordedSince } from './followCheck';

const NAMESPACE = 'default' as Namespace;

/** Enough accumulated history that bounded-vs-unbounded reads differ loudly. */
const HISTORY = 51;

function summary(index: number, status: WorkflowSummary['status']): WorkflowSummary {
  const startedAt = new Date(Date.UTC(2026, 7, 11, 1, 15, index)).toISOString();
  return {
    workflow_id: `wf-${index}` as WorkflowId,
    workflow_type: UPDATE_CHECK_WORKFLOW_TYPE,
    status,
    started_at: startedAt,
    ended_at: null,
  } as WorkflowSummary;
}

/**
 * A WIRE-FAITHFUL fake of `/workflows/list` as it actually behaves —
 * measured in the r3 review: the route ignores the client's page limit
 * entirely (docketed as #194), so the response carries EVERY summary the
 * filter matches, ascending by start time, with NO truncation. (The r2-era
 * fake truncated at a page limit; that wire does not exist, and a fixture
 * modelling it plants the opposite — and dangerous — belief that list
 * responses are bounded.)
 *
 * The fake also RECORDS each call's filter and the size of the page it
 * served, so tests can pin the two things the floor is actually for: the
 * request carries the floor, and the served page stays ~one row instead of
 * the whole of history.
 */
function wireFaithfulClient(records: WorkflowSummary[]) {
  const observed: { filter: WorkflowFilter; servedRows: number }[] = [];
  return {
    observed,
    queryWorkflows: (filter: WorkflowFilter): Promise<WorkflowPage<WorkflowSummary>> => {
      const items = records
        .filter(
          (record) =>
            (filter.workflow_type === null || record.workflow_type === filter.workflow_type) &&
            // The store's documented reading: started at or after.
            (filter.started_after === null || record.started_at >= filter.started_after)
        )
        .sort((a, b) => (a.started_at < b.started_at ? -1 : a.started_at > b.started_at ? 1 : 0));
      observed.push({ filter, servedRows: items.length });
      return Promise.resolve({ items, nextCursor: null, hasMore: false });
    },
  };
}

describe('fetchCheckRunStatus (the floor, on the wire that exists)', () => {
  /**
   * THE FLOOR PIN, on the true mechanism (r3 Major 1): with a full history
   * of older checks behind it, the follow's request carries the floor and
   * the served page is exactly the followed run — ~one row per 30-second
   * poll instead of the namespace's entire update-check history — and the
   * run resolves. The literal prior text (`started_after: null`, the
   * type-only scan) fails this by name: its request carries no floor and
   * its page is the whole of history.
   */
  test('the request carries the floor, the page stays one row, the run resolves', async () => {
    const older = Array.from({ length: HISTORY - 1 }, (_, index) => summary(index, 'Completed'));
    const followed = summary(HISTORY - 1, 'Completed');
    const client = wireFaithfulClient([...older, followed]);

    const floor = followed.started_at;
    const status = await fetchCheckRunStatus(client, NAMESPACE, followed.workflow_id, floor);

    expect(status).toBe('Completed');
    expect(client.observed).toHaveLength(1);
    const call = client.observed[0];
    if (call === undefined) {
      throw new Error('the observed-call recording is broken');
    }
    expect(call.filter.started_after).toBe(floor);
    expect(call.servedRows).toBe(1);
  });

  /**
   * CONTROL — the fixture distinguishes: on this wire a type-only query
   * (exactly what the pre-floor follow sent) returns the WHOLE history,
   * newest included. This is what makes the pin above able to fail: a
   * floorless request would resolve the run too, but only by hauling all
   * {@link HISTORY} rows — which the `servedRows` assertion refuses.
   */
  test('control: a type-only query on this wire returns the whole history', async () => {
    const older = Array.from({ length: HISTORY - 1 }, (_, index) => summary(index, 'Completed'));
    const followed = summary(HISTORY - 1, 'Completed');
    const client = wireFaithfulClient([...older, followed]);

    const page = await client.queryWorkflows({
      workflow_type: UPDATE_CHECK_WORKFLOW_TYPE,
      status: null,
      started_after: null,
      started_before: null,
      parent: null,
    });
    expect(page.items).toHaveLength(HISTORY);
    expect(page.items.some((item) => item.workflow_id === followed.workflow_id)).toBe(true);
  });

  test('a run not yet projected into visibility is null — the stated pending state', async () => {
    const client = wireFaithfulClient([]);
    const status = await fetchCheckRunStatus(
      client,
      NAMESPACE,
      'wf-ghost' as WorkflowId,
      new Date(Date.UTC(2026, 7, 11, 1, 15, 0)).toISOString()
    );
    expect(status).toBeNull();
  });

  test('a terminal failed status comes back as itself', async () => {
    const failed = summary(3, 'Failed');
    const client = wireFaithfulClient([failed]);
    const status = await fetchCheckRunStatus(
      client,
      NAMESPACE,
      failed.workflow_id,
      failed.started_at
    );
    expect(status).toBe('Failed');
  });
});

describe('hasRecordedSince (the concluded exit, r4 Major 2)', () => {
  const FLOOR = '2026-08-11T01:15:00.000Z';

  /**
   * All four arms of the decision the follow's concluded exit turns on. The
   * r4 review proved this logic was off every test's path by replacing the
   * whole return expression with `return recordedAt <= floor;` (comparison
   * inverted, both NaN guards deleted) with 912 tests green. Under that
   * exact mutant the first two arms die by name: "after" answers false
   * (inverted), and "before" answers TRUE — the panel declaring a days-old
   * answer current, the precise production failure the review described.
   * The "equal" arm pins the `>=` boundary against a strict-`>` slip, and
   * the unparseable arms pin the fail-safe DIRECTION (never conclude on
   * garbage) — honestly noted: for `>=`, NaN comparisons already answer
   * false, so the explicit guards are defensive statement rather than extra
   * behaviour, and these arms exist so a future rewrite cannot change that
   * direction unnoticed.
   */
  test('recorded strictly after the floor concludes', () => {
    expect(hasRecordedSince('2026-08-11T01:15:30.000Z', FLOOR)).toBe(true);
  });

  test('recorded strictly before the floor does NOT conclude — a stale answer is never declared current', () => {
    expect(hasRecordedSince('2026-08-11T01:14:30.000Z', FLOOR)).toBe(false);
  });

  test('recorded exactly at the floor concludes — the >= boundary the wire itself uses', () => {
    expect(hasRecordedSince(FLOOR, FLOOR)).toBe(true);
  });

  test('unparseable on either side never concludes — fail-safe, the follow keeps waiting', () => {
    expect(hasRecordedSince('not-a-timestamp', FLOOR)).toBe(false);
    expect(hasRecordedSince('2026-08-11T01:15:30.000Z', 'not-a-timestamp')).toBe(false);
    expect(hasRecordedSince(null, FLOOR)).toBe(false);
    expect(hasRecordedSince(undefined, FLOOR)).toBe(false);
  });
});
