import type { ApiClient } from '@/lib/api';
import type { Namespace, WorkflowId, WorkflowStatus } from '@/types';

import { UPDATE_CHECK_WORKFLOW_TYPE } from './contract';

/**
 * The started check run's projected status, read through the ordinary
 * workflow list query — TIME-FILTERED, and the filter is the load-bearing
 * part (r2 Major 1, mechanism CORRECTED in the r3 review).
 *
 * # What the wire actually does (measured, r3 review)
 *
 * `/workflows/list` IGNORES the client's page limit today: the request DTO
 * carries only namespace + filter, so the response holds EVERY matching
 * summary — unbounded, and growing with every check ever run (docketed
 * estate-wide as #194; not this feature's to fix). Two consequences the
 * `started_after` floor cures:
 *
 * - **Response size.** A type-only follow would download the namespace's
 *   entire update-check history every 30 seconds, forever, for as long as
 *   the panel is open. With the floor — the instant captured just before
 *   the start — the response is ~one row.
 * - **Precision.** `find(id)` over the time-bounded page is exact; over the
 *   unbounded set it is a linear scan of history.
 *
 * And should #194 ever make the limit real, the floor is also what keeps
 * the followed run on page one of an ascending listing — the fix is correct
 * under BOTH wires. (The r2 review's original rationale asserted the
 * truncated wire as present fact; the r3 review measured the opposite.
 * List responses are NOT page-bounded today — do not build on the belief
 * that they are.)
 *
 * The floor is the CONSOLE's clock; `started_after` compares against the
 * SERVER's recorded start time. A console clock running ahead of the server
 * could therefore exclude the run — which degrades to the VISIBLE
 * "waiting for the run to appear" state, never to a false "following" and
 * never to a wrong status. `null` here means exactly that stated state: the
 * run is not in the time-filtered page (not yet projected into visibility,
 * or excluded by clock skew).
 */
export async function fetchCheckRunStatus(
  client: Pick<ApiClient, 'queryWorkflows'>,
  namespace: Namespace,
  workflowId: WorkflowId,
  startedAfter: string
): Promise<WorkflowStatus | null> {
  const page = await client.queryWorkflows(
    {
      workflow_type: UPDATE_CHECK_WORKFLOW_TYPE,
      status: null,
      started_after: startedAfter,
      started_before: null,
      parent: null,
    },
    {},
    { namespace }
  );
  return page.items.find((summary) => summary.workflow_id === workflowId)?.status ?? null;
}

/**
 * Has a check been RECORDED at-or-after `floor`? The decision the follow's
 * concluded exit turns on (r3-m3), lifted here so it is testable in
 * isolation (r4 Major 2: as an inline expression it sat off every test's
 * path — the reviewer inverted the comparison and deleted both NaN guards
 * and 912 tests stayed green).
 *
 * `checkedAt` is the served `/update-status` timestamp (or absent);
 * `floor` is the instant captured just before the operator's start.
 * At-or-after (`>=`) matches the wire's own `started_after` reading. Both
 * sides are parsed defensively: an unparseable timestamp on EITHER side
 * answers false — fail-safe, the follow then simply keeps waiting rather
 * than declaring a stale answer current.
 */
export function hasRecordedSince(checkedAt: string | null | undefined, floor: string): boolean {
  if (checkedAt == null) {
    return false;
  }
  const recordedAt = Date.parse(checkedAt);
  const floorAt = Date.parse(floor);
  return !Number.isNaN(recordedAt) && !Number.isNaN(floorAt) && recordedAt >= floorAt;
}
