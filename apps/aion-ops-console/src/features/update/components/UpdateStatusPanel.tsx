import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { RefreshCw, TriangleAlert, X } from 'lucide-react';
import { useEffect, useState } from 'react';

import type { BuildIdentityResponse, StartWorkflowResult, UpdateStatusResponse } from '@/lib/api';
import { QUERY_STALE_TIME_MS } from '@/lib/config';
import { formatInstant } from '@/lib/time';
import type { WorkflowId, WorkflowStatus } from '@/types';

import { hasRecordedSince } from '../lib/followCheck';
import { compareForUpdate, type UpdateComparison } from '../lib/semver';

/**
 * Run statuses that end the started-check follow (nothing more will change
 * for THIS run). `ContinuedAsNew` is terminal for the followed run even
 * though a successor continues — unreachable for the update-check document,
 * but a follow that omitted it would poll forever if it ever appeared
 * (r2-m3, the same never-resolves shape as r1's m6).
 */
const TERMINAL_RUN_STATUSES: readonly WorkflowStatus[] = [
  'Completed',
  'Failed',
  'Cancelled',
  'TimedOut',
  'ContinuedAsNew',
];

function isTerminalRunStatus(status: WorkflowStatus | null | undefined): boolean {
  return status != null && TERMINAL_RUN_STATUSES.includes(status);
}

/**
 * The update pill and its panel: "does a newer `aion-cli` exist?" — slice one,
 * know and show. No install, no upgrade, no restart lives here.
 *
 * Mirrors the What's-new panel deliberately: a launcher in the header cluster
 * and a NON-MODAL floating `complementary` landmark beside it. Two reads feed
 * it:
 *
 * - `GET /update-status` — POLLED on the console's one freshness cadence
 *   ({@link QUERY_STALE_TIME_MS}) even while the panel is closed, because the
 *   pill must appear without a gesture once a check (run from anywhere) has
 *   found something newer.
 * - `GET /build` — fetched when the panel opens, the console's first-ever
 *   `/build` client read: the installed line renders what the SERVER says it
 *   runs, never a value baked into this bundle.
 *
 * The pill appears ONLY when `latest_known` is present and semver-greater
 * than `installed`. A malformed version on either side is its own loud state
 * ({@link UpdateComparison}), never a silent absence — a pill that hides
 * itself on garbage is indistinguishable from "up to date".
 *
 * "Check now" is THE operator action: it starts the `update_check` workflow
 * through the ordinary start spine, and the server-run declared `curl` does
 * the rest (output on the run's transcript). Nothing here fires without that
 * click, and this slice performs no install and no restart — knowing that a
 * version exists and becoming it are different acts.
 */
export function UpdateStatusPanel({
  defaultOpen = false,
  fetchUpdateStatus,
  fetchBuild,
  startCheck,
  fetchCheckStatus,
  checkDisabledReason = null,
}: {
  /** Open on first render — the launcher toggle owns it afterwards. */
  defaultOpen?: boolean;
  /** Fetches `GET /update-status`; injected so tests need no live server. */
  fetchUpdateStatus: () => Promise<UpdateStatusResponse>;
  /** Fetches `GET /build`; injected so tests need no live server. */
  fetchBuild: () => Promise<BuildIdentityResponse>;
  /** Starts one `update_check` run — the explicit operator act. */
  startCheck: () => Promise<StartWorkflowResult>;
  /**
   * The started run's projected status, for the follow (r1 m6): "check
   * started" must resolve to completed-or-failed, never stand forever. The
   * `startedAfter` floor — captured by the panel just before the start — is
   * what keeps each poll's response ~one row on a list wire that is
   * otherwise UNBOUNDED (r3 correction of r2 Major 1; see
   * `lib/followCheck.ts` for the measured mechanism).
   */
  fetchCheckStatus: (
    workflowId: WorkflowId,
    startedAfter: string
  ) => Promise<WorkflowStatus | null>;
  /** Non-null disables "check now" and states why (e.g. no namespace). */
  checkDisabledReason?: string | null;
}) {
  const [open, setOpen] = useState(defaultOpen);
  const queryClient = useQueryClient();

  const statusQuery = useQuery({
    queryKey: ['update-status'],
    queryFn: fetchUpdateStatus,
    // The pill must notice a check completed elsewhere (another tab, the CLI)
    // without a gesture; the poll rides the app's one freshness cadence.
    refetchInterval: QUERY_STALE_TIME_MS,
  });
  const buildQuery = useQuery({
    queryKey: ['build'],
    queryFn: fetchBuild,
    enabled: open,
    // The identity of a running binary cannot change while it runs.
    staleTime: Number.POSITIVE_INFINITY,
  });
  const checkMutation = useMutation({
    // The follow's time floor is captured HERE, immediately before the start
    // request leaves. The list wire returns EVERY matching summary today
    // (the page limit is ignored — measured in the r3 review, docketed as
    // #194), so the floor is what keeps each 30-second follow poll at ~one
    // row instead of the namespace's whole update-check history; see
    // `lib/followCheck.ts` for the full mechanism.
    mutationFn: async () => {
      const startedAfter = new Date().toISOString();
      const started = await startCheck();
      return { ...started, startedAfter };
    },
    onSuccess: () => {
      // The run takes as long as its curl takes; the poll picks the recorded
      // result up on the next tick, and this immediate invalidation covers
      // the case where it already landed.
      void queryClient.invalidateQueries({ queryKey: ['update-status'] });
    },
  });
  // The follow (r1 m6): a started check is polled to a TERMINAL status on the
  // console's one cadence, so "check started" always resolves — completed, or
  // a loud failure — instead of standing as the last word forever.
  const startedCheck = checkMutation.data ?? null;
  const status = statusQuery.data;
  // The pending state's exit (r3-m3): if a check has been RECORDED since the
  // floor, the question the follow exists to answer is answered. The follow
  // then concludes instead of polling forever. The decision itself lives in
  // `lib/followCheck.ts::hasRecordedSince` — exported and four-arm pinned
  // (r4 Major 2): the comparison DIRECTION is what the pins hold (inverting
  // it dies on two arms by name); the NaN guards are defensive statement
  // rather than pinned behaviour — for `>=`, NaN comparisons already answer
  // false — and the unparseable arms pin that fail-safe direction against a
  // rewrite (r5-m1, wording aligned with followCheck.test.ts).
  const recordedSinceStart =
    startedCheck !== null && hasRecordedSince(status?.checked_at, startedCheck.startedAfter);
  const runQuery = useQuery({
    queryKey: ['update-check-run', startedCheck?.workflowId ?? null],
    queryFn: () => {
      if (startedCheck === null) {
        throw new Error('the run follow is enabled only after a start');
      }
      return fetchCheckStatus(startedCheck.workflowId, startedCheck.startedAfter);
    },
    enabled: startedCheck !== null,
    refetchInterval: (query) =>
      isTerminalRunStatus(query.state.data) || recordedSinceStart ? false : QUERY_STALE_TIME_MS,
  });
  const runStatus = runQuery.data ?? null;
  useEffect(() => {
    // The moment the followed run completes, refresh the served status so the
    // panel shows the recorded answer without waiting out a poll tick.
    if (runStatus === 'Completed') {
      void queryClient.invalidateQueries({ queryKey: ['update-status'] });
    }
  }, [runStatus, queryClient]);
  const comparison: UpdateComparison | null =
    status && status.latest_known !== null
      ? compareForUpdate(status.installed, status.latest_known)
      : null;

  const launcher = (
    <button
      aria-expanded={open}
      className={`inline-flex items-center gap-2 self-start rounded-md border px-3 py-1.5 text-foreground text-xs outline-none hover:bg-surface-base focus-visible:ring-2 focus-visible:ring-accent-primary sm:self-end ${
        open ? 'border-accent-primary' : 'border-border'
      } bg-surface-elevated`}
      data-slot="update-launcher"
      onClick={() => setOpen((current) => !current)}
      type="button"
    >
      <RefreshCw aria-hidden="true" className="size-3.5" />
      Updates
      {comparison?.state === 'newer' && status?.latest_known !== null && (
        <span
          className="rounded-full bg-accent-primary px-2 py-0.5 font-semibold text-[10px] text-accent-primary-foreground"
          data-slot="update-available-pill"
        >
          {status?.latest_known} available
        </span>
      )}
      {comparison?.state === 'malformed' && (
        <span
          className="inline-flex items-center gap-1 text-status-warning"
          data-slot="update-version-malformed"
          title={comparison.reason}
        >
          <TriangleAlert aria-hidden="true" className="size-3.5" />
          version unreadable
        </span>
      )}
    </button>
  );

  if (!open) {
    return launcher;
  }

  return (
    <>
      {launcher}
      <aside
        aria-label="Update status"
        className="fixed top-6 right-6 z-40 flex max-h-[80vh] w-[26rem] max-w-[calc(100vw-3rem)] flex-col overflow-hidden rounded-xl border border-border bg-surface-elevated shadow-xl"
        data-slot="update-panel"
      >
        <header className="flex items-center justify-between gap-2 border-border border-b px-4 py-2.5">
          <span className="font-semibold text-foreground text-xs uppercase tracking-wide">
            Updates
          </span>
          <button
            aria-label="Close the update panel"
            className="rounded-md p-1 text-muted-foreground outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-accent-primary"
            onClick={() => setOpen(false)}
            type="button"
          >
            <X aria-hidden="true" className="size-4" />
          </button>
        </header>
        <div className="flex flex-col gap-3 overflow-y-auto px-4 py-3 text-sm">
          <InstalledLine query={buildQuery} />
          <LatestLine comparison={comparison} status={status ?? null} statusQuery={statusQuery} />
          <div className="flex flex-col gap-2">
            <button
              className="inline-flex items-center gap-2 self-start rounded-md border border-border bg-surface-base px-3 py-1.5 text-foreground text-xs outline-none hover:bg-surface-elevated focus-visible:ring-2 focus-visible:ring-accent-primary disabled:cursor-not-allowed disabled:opacity-50"
              data-slot="update-check-now"
              disabled={checkDisabledReason !== null || checkMutation.isPending}
              onClick={() => checkMutation.mutate()}
              title={checkDisabledReason ?? undefined}
              type="button"
            >
              <RefreshCw aria-hidden="true" className="size-3.5" />
              {checkMutation.isPending ? 'Starting the check…' : 'Check now'}
            </button>
            {checkDisabledReason !== null && (
              <p className="text-muted-foreground text-xs">{checkDisabledReason}</p>
            )}
            {checkMutation.isSuccess && (
              <CheckRunLine
                recordedSinceStart={recordedSinceStart}
                runError={runQuery.isError ? runQuery.error : null}
                status={runStatus}
                workflowId={checkMutation.data.workflowId}
              />
            )}
            {checkMutation.isError && (
              <p className="text-status-error text-xs" data-slot="update-check-error" role="alert">
                The check did not start: {describeError(checkMutation.error)}
              </p>
            )}
          </div>
          <p className="border-border border-t pt-2 text-muted-foreground text-xs">
            The server checks only when you ask — no timer, no check on startup. Checking only
            reports what is published; it installs nothing and restarts nothing.
          </p>
        </div>
      </aside>
    </>
  );
}

/**
 * The started check's lifecycle line (r1 m6): started → running → completed
 * or a LOUD failure naming the run. Never a claim that stands forever.
 * Exported for its own state tests (mutation state cannot be seeded from a
 * static render).
 */
export function CheckRunLine({
  workflowId,
  status,
  runError,
  recordedSinceStart = false,
}: {
  workflowId: WorkflowId;
  status: WorkflowStatus | null;
  runError: unknown;
  /**
   * Whether a check has been RECORDED at-or-after this run's start floor —
   * the pending state's exit (r3-m3): a recording answers the question the
   * follow exists to answer, so following stops even though this run's own
   * status was not read first (r4-m1: what is known is "not read", never
   * "never appeared" — the run may be one fetch away from visible).
   */
  recordedSinceStart?: boolean;
}) {
  if (runError !== null && runError !== undefined) {
    return (
      <p className="text-status-error text-xs" data-slot="update-check-follow-error" role="alert">
        Check started (workflow {workflowId}), but its run could not be followed:{' '}
        {describeError(runError)}. Open the workflow list to see how it ended.
      </p>
    );
  }
  if (status === 'Completed') {
    return (
      <p className="text-muted-foreground text-xs" data-slot="update-check-completed">
        Check completed (workflow {workflowId}). The recorded result is shown above.
      </p>
    );
  }
  if (status === 'Failed' || status === 'Cancelled' || status === 'TimedOut') {
    return (
      <p className="text-status-error text-xs" data-slot="update-check-failed" role="alert">
        The check did not succeed: workflow {workflowId} ended {status}. Its curl output and errors
        are on the run's transcript; nothing was recorded.
      </p>
    );
  }
  if (status === 'ContinuedAsNew') {
    // Unreachable for the update-check document (it never continues as new),
    // but a status the follow must terminate on rather than poll forever —
    // and unexpected enough to say so loudly (r2-m3).
    return (
      <p className="text-status-error text-xs" data-slot="update-check-continued" role="alert">
        Workflow {workflowId} ended ContinuedAsNew — an update check never continues as new. Inspect
        the run; this line stops following here.
      </p>
    );
  }
  if (status === null && recordedSinceStart) {
    // The pending state's exit (r3-m3): a check HAS been recorded since this
    // run started — the answer the check exists to produce is above. The
    // follow concludes rather than waiting on. Worded to what IS known
    // (r4-m1): polling stops the moment the recording lands, so this run's
    // own status was simply not read before the follow concluded — it may
    // well be visible in the workflow list right now, and often is.
    return (
      <p className="text-muted-foreground text-xs" data-slot="update-check-concluded">
        A check was recorded since this one started — the result above is current. This run
        (workflow {workflowId}) is no longer being followed; open the workflow list for its own
        status and transcript.
      </p>
    );
  }
  if (status === null) {
    // Its own STATED state, not an indefinite "following" (r2 Major 1): the
    // run is not yet in the time-filtered page — normally because visibility
    // has not projected it yet, or (documented in lib/followCheck.ts) because
    // a console clock ahead of the server excluded it.
    return (
      <p className="text-muted-foreground text-xs" data-slot="update-check-pending">
        Check started (workflow {workflowId}); waiting for its run to appear in the workflow list…
        If this line never moves on, the run is still visible in the workflow list — open it there
        for the transcript.
      </p>
    );
  }
  return (
    <p className="text-muted-foreground text-xs" data-slot="update-check-started">
      Check running (workflow {workflowId}, {status}). Its curl output streams onto the run's
      transcript; this line follows the run until it ends.
    </p>
  );
}

function InstalledLine({
  query,
}: {
  query: { data?: BuildIdentityResponse | undefined; isError: boolean; error: unknown };
}) {
  if (query.isError) {
    return (
      <p className="text-status-error text-xs" role="alert">
        The server's build identity could not be read: {describeError(query.error)}
      </p>
    );
  }
  if (!query.data) {
    return <p className="text-muted-foreground text-xs">Reading the server's build identity…</p>;
  }
  return (
    <p data-slot="update-installed">
      <span className="text-muted-foreground">Installed:</span>{' '}
      <span className="font-mono">{query.data.version}</span>
    </p>
  );
}

function LatestLine({
  comparison,
  status,
  statusQuery,
}: {
  comparison: UpdateComparison | null;
  status: UpdateStatusResponse | null;
  statusQuery: { isError: boolean; error: unknown };
}) {
  if (statusQuery.isError) {
    return (
      <p className="text-status-error text-xs" role="alert">
        The update status could not be read: {describeError(statusQuery.error)}
      </p>
    );
  }
  if (!status) {
    return <p className="text-muted-foreground text-xs">Reading the update status…</p>;
  }
  if (status.latest_known === null || status.checked_at === null) {
    return (
      <p className="text-muted-foreground" data-slot="update-never-checked">
        This server has never checked for updates.
      </p>
    );
  }
  return (
    <div className="flex flex-col gap-1">
      <p data-slot="update-latest">
        <span className="text-muted-foreground">Latest published:</span>{' '}
        <span className="font-mono">{status.latest_known}</span>
        {comparison?.state === 'not-newer' && (
          <span className="text-muted-foreground"> — up to date</span>
        )}
      </p>
      <p className="text-muted-foreground text-xs">Checked {formatInstant(status.checked_at)}</p>
      {comparison?.state === 'malformed' && (
        <p className="text-status-warning text-xs" data-slot="update-malformed-detail" role="alert">
          These versions cannot be compared: {comparison.reason}
        </p>
      )}
    </div>
  );
}

/** An error's message without swallowing non-`Error` throwables. */
function describeError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
