import { describe, expect, test } from 'bun:test';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { renderToStaticMarkup } from 'react-dom/server';

import type { BuildIdentityResponse, UpdateStatusResponse } from '@/lib/api';
import type { WorkflowId } from '@/types';

import { CheckRunLine, UpdateStatusPanel } from './UpdateStatusPanel';

const BUILD: BuildIdentityResponse = {
  version: '0.13.7',
  commit: 'abcdef1234567890',
  dirty: 'false',
  built_at: '2026-08-01T00:00:00Z',
};

const NEVER_CHECKED: UpdateStatusResponse = {
  installed: '0.13.7',
  latest_known: null,
  checked_at: null,
};

const UPDATE_AVAILABLE: UpdateStatusResponse = {
  installed: '0.13.7',
  latest_known: '0.14.0',
  checked_at: '2026-08-11T01:00:00Z',
};

const PRERELEASE_AVAILABLE: UpdateStatusResponse = {
  installed: '0.13.7',
  latest_known: '0.14.0-rc.1',
  checked_at: '2026-08-11T01:00:00Z',
};

const UP_TO_DATE: UpdateStatusResponse = {
  installed: '0.13.7',
  latest_known: '0.13.7',
  checked_at: '2026-08-11T01:00:00Z',
};

const MALFORMED: UpdateStatusResponse = {
  installed: 'unknown',
  latest_known: '0.14.0',
  checked_at: '2026-08-11T01:00:00Z',
};

const UNPARSEABLE_TIME: UpdateStatusResponse = {
  installed: '0.13.7',
  latest_known: '0.13.7',
  checked_at: 'not-a-timestamp',
};

function panel(
  status: UpdateStatusResponse | null,
  open = false,
  disabledReason: string | null = null
) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  if (status !== null) {
    queryClient.setQueryData(['update-status'], status);
  }
  queryClient.setQueryData(['build'], BUILD);

  return renderToStaticMarkup(
    <QueryClientProvider client={queryClient}>
      <UpdateStatusPanel
        checkDisabledReason={disabledReason}
        defaultOpen={open}
        fetchBuild={() => Promise.resolve(BUILD)}
        fetchCheckStatus={() => Promise.resolve(null)}
        fetchUpdateStatus={() => Promise.resolve(status ?? NEVER_CHECKED)}
        startCheck={() => Promise.resolve({ workflowId: 'wf-1', runId: 'run-1' })}
      />
    </QueryClientProvider>
  );
}

describe('UpdateStatusPanel', () => {
  test('no pill before any check has ever run — absence is honest, not zero', () => {
    const markup = panel(NEVER_CHECKED);
    expect(markup).toContain('data-slot="update-launcher"');
    expect(markup).not.toContain('data-slot="update-available-pill"');
    expect(markup).not.toContain('data-slot="update-version-malformed"');
  });

  test('the pill appears ONLY when latest_known is semver-greater than installed', () => {
    expect(panel(UPDATE_AVAILABLE)).toContain('data-slot="update-available-pill"');
    expect(panel(UPDATE_AVAILABLE)).toContain('0.14.0 available');
    expect(panel(UP_TO_DATE)).not.toContain('data-slot="update-available-pill"');
  });

  test('a prerelease latest never raises the pill (r1 Major 1)', () => {
    const markup = panel(PRERELEASE_AVAILABLE);
    expect(markup).not.toContain('data-slot="update-available-pill"');
    expect(markup).not.toContain('data-slot="update-version-malformed"');
  });

  test('a malformed version is a LOUD state on the launcher, never a silent hide', () => {
    const markup = panel(MALFORMED);
    expect(markup).not.toContain('data-slot="update-available-pill"');
    expect(markup).toContain('data-slot="update-version-malformed"');
    expect(markup).toContain('version unreadable');
  });

  test('open, never checked: says so and renders installed from /build', () => {
    const markup = panel(NEVER_CHECKED, true);
    expect(markup).toContain('data-slot="update-panel"');
    expect(markup).toContain('data-slot="update-never-checked"');
    expect(markup).toContain('data-slot="update-installed"');
    expect(markup).toContain('0.13.7');
    expect(markup).toContain('data-slot="update-check-now"');
    // The manual-only promise is stated to the operator in plain language.
    expect(markup).toContain('The server checks only when you ask');
    expect(markup).toContain('installs nothing and restarts nothing');
  });

  test('open with a recorded check: renders latest, checked time, and up-to-date note', () => {
    const markup = panel(UP_TO_DATE, true);
    expect(markup).toContain('data-slot="update-latest"');
    expect(markup).toContain('up to date');
    expect(markup).toContain('Checked ');
  });

  test('an unparseable checked_at is shown raw and NAMED, never "Invalid Date" (r1 m12)', () => {
    const markup = panel(UNPARSEABLE_TIME, true);
    expect(markup).toContain('not-a-timestamp (unparseable timestamp)');
    expect(markup).not.toContain('Invalid Date');
  });

  test('open with malformed versions: the refusal is rendered with its reason', () => {
    const markup = panel(MALFORMED, true);
    expect(markup).toContain('data-slot="update-malformed-detail"');
    expect(markup).toContain('installed version');
    expect(markup).toContain('`unknown`');
  });

  test('a disabled check-now states its reason instead of guessing a namespace', () => {
    const markup = panel(NEVER_CHECKED, true, 'Select a namespace to run the check in.');
    expect(markup).toContain('disabled');
    expect(markup).toContain('Select a namespace to run the check in.');
  });
});

describe('CheckRunLine (the r1 m6 follow: started never stands forever)', () => {
  const workflowId = 'wf-42' as WorkflowId;

  test('not yet visible: a STATED pending state with the give-up pointer (r2 Major 1, r3-m3)', () => {
    const markup = renderToStaticMarkup(
      <CheckRunLine runError={null} status={null} workflowId={workflowId} />
    );
    expect(markup).toContain('data-slot="update-check-pending"');
    expect(markup).toContain('waiting for its run to appear');
    // The escape hatch is stated, not left to be discovered (r3-m3).
    expect(markup).toContain('open');
    expect(markup).toContain('workflow list');
    expect(markup).not.toContain('follows the run until it ends');
  });

  test('pending CONCLUDES once a check is recorded after the floor (r3-m3)', () => {
    const markup = renderToStaticMarkup(
      <CheckRunLine
        recordedSinceStart={true}
        runError={null}
        status={null}
        workflowId={workflowId}
      />
    );
    expect(markup).toContain('data-slot="update-check-concluded"');
    expect(markup).toContain('the result above is current');
    expect(markup).not.toContain('data-slot="update-check-pending"');
  });

  test('a running run says it is following, naming the status', () => {
    const markup = renderToStaticMarkup(
      <CheckRunLine runError={null} status="Running" workflowId={workflowId} />
    );
    expect(markup).toContain('data-slot="update-check-started"');
    expect(markup).toContain('follows the run until it ends');
    expect(markup).toContain('Running');
  });

  test('ContinuedAsNew is terminal for the follow and loud (r2-m3)', () => {
    const markup = renderToStaticMarkup(
      <CheckRunLine runError={null} status="ContinuedAsNew" workflowId={workflowId} />
    );
    expect(markup).toContain('data-slot="update-check-continued"');
    expect(markup).toContain('role="alert"');
    expect(markup).toContain('stops following here');
  });

  test('a completed run says so and points at the recorded result', () => {
    const markup = renderToStaticMarkup(
      <CheckRunLine runError={null} status="Completed" workflowId={workflowId} />
    );
    expect(markup).toContain('data-slot="update-check-completed"');
    expect(markup).toContain('wf-42');
  });

  test('a failed run is LOUD, names the run, and says nothing was recorded', () => {
    for (const status of ['Failed', 'Cancelled', 'TimedOut'] as const) {
      const markup = renderToStaticMarkup(
        <CheckRunLine runError={null} status={status} workflowId={workflowId} />
      );
      expect(markup).toContain('data-slot="update-check-failed"');
      expect(markup).toContain('role="alert"');
      expect(markup).toContain(status);
      expect(markup).toContain('nothing was recorded');
    }
  });

  test('a follow error is its own loud line, never a silent stall', () => {
    const markup = renderToStaticMarkup(
      <CheckRunLine
        runError={new Error('list query refused')}
        status={null}
        workflowId={workflowId}
      />
    );
    expect(markup).toContain('data-slot="update-check-follow-error"');
    expect(markup).toContain('list query refused');
  });
});
