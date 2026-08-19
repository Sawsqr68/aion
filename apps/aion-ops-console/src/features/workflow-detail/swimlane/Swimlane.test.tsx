import { expect, test } from 'bun:test';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import type { ReactNode } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';

import { DISPLAY_NAME_ATTRIBUTE } from '@/lib/workflowIdentity';
import type { Event, Payload } from '@/types';

import { projectTimeline } from '../lib/timeline';
import { buildRankIndex, layoutSwimlane } from './laneLayout';
import { Swimlane } from './Swimlane';
import { WorkflowDetailViewContent } from './WorkflowDetailView';

/**
 * A CANONICAL UUID, deliberately (#211 R2). These tests assert what the detail
 * header renders as the run's identity, and a short form is only ever a
 * truncation of a canonical UUID — with a non-UUID fixture id `shortWorkflowId`
 * returns the value whole, so every assertion below would have passed against a
 * header that had thrown the rest of a real id away.
 */
const WORKFLOW_ID = '3f2a1b4c-1111-2222-3333-444455556666';
const PAYLOAD: Payload = { content_type: 'Json', bytes: [123, 125] };

/**
 * The detail view's header carries the rename affordance (#211), a real
 * mutation — so it renders under a QueryClientProvider exactly as it does in
 * the app, rather than in a query-less environment the app never has.
 */
function renderDetail(node: ReactNode): string {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });

  return renderToStaticMarkup(
    <QueryClientProvider client={queryClient}>{node}</QueryClientProvider>
  );
}

function envelope(seq: number) {
  return {
    seq,
    recorded_at: `2026-06-29T00:00:${String(seq).padStart(2, '0')}Z`,
    workflow_id: WORKFLOW_ID,
  };
}

function workflowStarted(seq: number): Event {
  return {
    type: 'WorkflowStarted',
    data: {
      envelope: envelope(seq),
      workflow_type: 'checkout',
      input: PAYLOAD,
      run_id: '00000000-0000-0000-0000-0000000000a1',
      parent_run_id: null,
      parent_workflow_id: null,
      package_version: '1.0.0',
    },
  };
}

/** A recorded naming/rename: the display-name search attribute (#211). */
function renamedTo(seq: number, displayName: string): Event {
  return {
    type: 'SearchAttributesUpdated',
    data: {
      envelope: envelope(seq),
      workflow_id: WORKFLOW_ID,
      attributes: { [DISPLAY_NAME_ATTRIBUTE]: { type: 'String', data: displayName } },
    },
  };
}

function activityScheduled(seq: number, id: number, type: string): Event {
  return {
    type: 'ActivityScheduled',
    data: {
      envelope: envelope(seq),
      activity_id: id,
      activity_type: type,
      input: PAYLOAD,
      task_queue: 'default',
      node: null,
    },
  };
}

function activityStarted(seq: number, id: number): Event {
  return {
    type: 'ActivityStarted',
    data: { envelope: envelope(seq), activity_id: id, attempt: 1 },
  };
}

function activityFailed(seq: number, id: number, attempt: number): Event {
  return {
    type: 'ActivityFailed',
    data: {
      envelope: envelope(seq),
      activity_id: id,
      attempt,
      error: { kind: 'Retryable', message: 'boom', details: null },
    },
  };
}

function activityCompleted(seq: number, id: number): Event {
  return {
    type: 'ActivityCompleted',
    data: { envelope: envelope(seq), activity_id: id, attempt: 1, result: PAYLOAD },
  };
}

function timerStarted(seq: number, name: string): Event {
  return {
    type: 'TimerStarted',
    data: { envelope: envelope(seq), timer_id: { Named: name }, fire_at: '2026-06-29T01:00:00Z' },
  };
}

function timerFired(seq: number, name: string): Event {
  return { type: 'TimerFired', data: { envelope: envelope(seq), timer_id: { Named: name } } };
}

function workflowFailed(seq: number, message: string): Event {
  return {
    type: 'WorkflowFailed',
    data: { envelope: envelope(seq), error: { message, details: null } },
  };
}

function terminalActivityFailed(seq: number, id: number, attempt: number): Event {
  return {
    type: 'ActivityFailed',
    data: {
      envelope: envelope(seq),
      activity_id: id,
      attempt,
      error: { kind: 'Terminal', message: 'terminal boom', details: null },
    },
  };
}

function childStarted(seq: number, childId: string): Event {
  return {
    type: 'ChildWorkflowStarted',
    data: {
      envelope: envelope(seq),
      child_workflow_id: childId,
      workflow_type: 'receipt',
      input: PAYLOAD,
      package_version: '1.0.0',
    },
  };
}

test('buildRankIndex assigns dense ranks over all carried sequences', () => {
  const entries = projectTimeline([workflowStarted(1), timerStarted(5, 't1'), timerFired(9, 't1')]);
  const index = buildRankIndex(entries);

  expect(index.get(1)).toBe(0);
  expect(index.get(5)).toBe(1);
  expect(index.get(9)).toBe(2);
});

test('lane assignment yields one lane per correlation key plus lifecycle', () => {
  const layout = layoutSwimlane(
    projectTimeline([
      workflowStarted(1),
      activityScheduled(2, 10, 'charge'),
      activityCompleted(3, 10),
      timerStarted(4, 't1'),
      timerFired(5, 't1'),
    ])
  );

  const kinds = layout.lanes.map((lane) => lane.kind);
  expect(kinds).toContain('lifecycle');
  expect(kinds.filter((kind) => kind === 'activity')).toHaveLength(1);
  expect(kinds.filter((kind) => kind === 'timer')).toHaveLength(1);
});

test('attempt segmentation records one rank per ActivityFailed attempt', () => {
  const layout = layoutSwimlane(
    projectTimeline([
      activityScheduled(1, 10, 'charge'),
      activityStarted(2, 10),
      activityFailed(3, 10, 1),
      activityFailed(4, 10, 2),
      activityCompleted(5, 10),
    ])
  );

  const activityLane = layout.lanes.find((lane) => lane.kind === 'activity');
  const bar = activityLane?.bars[0];
  expect(bar?.attemptSequences).toEqual([3, 4]);
  expect(bar?.status).toBe('completed');
});

test('concurrent activity + timer overlap on the same rank span', () => {
  const layout = layoutSwimlane(
    projectTimeline([
      activityScheduled(1, 10, 'charge'),
      timerStarted(2, 't1'),
      activityCompleted(4, 10),
      timerFired(3, 't1'),
    ])
  );

  const activity = layout.lanes.find((lane) => lane.kind === 'activity')?.bars[0];
  const timer = layout.lanes.find((lane) => lane.kind === 'timer')?.bars[0];

  if (activity === undefined || timer === undefined) {
    throw new Error('expected both an activity and a timer lane');
  }

  // Their rank spans overlap (concurrent) rather than being sequential.
  expect(Math.max(activity.startRank, timer.startRank)).toBeLessThanOrEqual(
    Math.min(activity.endRank, timer.endRank)
  );
});

test('child bar carries a drill-link target and the swimlane renders it', () => {
  const entries = projectTimeline([childStarted(1, 'child-1')]);
  const layout = layoutSwimlane(entries);
  const childBar = layout.lanes.find((lane) => lane.kind === 'child')?.bars[0];

  expect(childBar?.childWorkflowId).toBe('child-1');

  const markup = renderToStaticMarkup(
    <Swimlane
      entries={entries}
      isRunning={false}
      onSelect={() => {}}
      selectedSequence={null}
      workflowId={WORKFLOW_ID}
    />
  );
  expect(markup).toContain('Workflow swimlane');
  // The child lane renders with its drill affordance (↳) and the bar references
  // the child workflow in its accessible label.
  expect(markup).toContain('↳');
  expect(markup).toContain('seq 1');
});

test('live-append extends an activity bar in place (endRank grows)', () => {
  const initial = layoutSwimlane(projectTimeline([activityScheduled(1, 10, 'charge')]));
  const initialBar = initial.lanes.find((lane) => lane.kind === 'activity')?.bars[0];
  expect(initialBar?.endRank).toBe(0);

  const extended = layoutSwimlane(
    projectTimeline([activityScheduled(1, 10, 'charge'), activityCompleted(2, 10)])
  );
  const extendedBar = extended.lanes.find((lane) => lane.kind === 'activity')?.bars[0];
  expect(extendedBar?.endRank).toBe(1);
  expect(extendedBar?.status).toBe('completed');
});

test('WorkflowDetailViewContent renders swimlane mode by default with no fabricated data', () => {
  const timeline = projectTimeline([workflowStarted(1), activityScheduled(2, 10, 'charge')]);
  const markup = renderDetail(
    <WorkflowDetailViewContent
      error={null}
      history={[]}
      isError={false}
      isLive={true}
      isLoading={false}
      isTerminal={false}
      namespace="default"
      terminalOutcome={null}
      timeline={timeline}
      workflowId={WORKFLOW_ID}
    />
  );

  expect(markup).toContain('Workflow swimlane');
  expect(markup).toContain('aria-pressed="true"');
});

/**
 * The workflow id the HEADER renders as text, or `null` when it renders none.
 *
 * Read out of the header's id element rather than searched for anywhere in the
 * markup: the id also appears in swimlane row titles and lane keys, so a bare
 * `toContain(WORKFLOW_ID)` would stay green with a header that shows no id at
 * all — or a truncated one.
 */
function renderedHeaderId(markup: string): string | null {
  return /data-testid="workflow-id"[^>]*>([^<]*)</.exec(markup)?.[1] ?? null;
}

// #211 R2. Catches: a header that shows a NAME ALONE, and a header that shows
// the id only in SHORT form. The name is a label over the run's identity, so
// the id is always beside it — in full, because this page is the only place the
// id appears as text and every CLI verb an operator types from here needs the
// whole UUID.
//
// The named and unnamed runs are separate tests on purpose: as two arms of one
// test the first failing assertion would abort before the second arm ran, so a
// header that dropped the id only for the run with no name would be reported by
// nothing.
test('a NAMED run reads as its name over the full id', () => {
  const history = [workflowStarted(1), renamedTo(2, 'nightly digest')];
  const named = renderDetail(
    <WorkflowDetailViewContent
      error={null}
      history={history}
      isError={false}
      isLive={true}
      isLoading={false}
      isTerminal={false}
      namespace="default"
      terminalOutcome={null}
      timeline={projectTimeline(history)}
      workflowId={WORKFLOW_ID}
    />
  );

  expect(named).toContain('>nightly digest</h1>');
  expect(renderedHeaderId(named)).toBe(WORKFLOW_ID);
  // A named run offers a rename, not a first naming.
  expect(named).toContain('Rename');
});

// The unnamed arm: a run with no name still reads as a workflow with an id —
// never as a blank heading, and never as a heading that has TRADED the id for a
// label it does not have.
test('an UNNAMED run reads as the fallback heading over the full id', () => {
  const unnamedHistory = [workflowStarted(1)];
  const unnamed = renderDetail(
    <WorkflowDetailViewContent
      error={null}
      history={unnamedHistory}
      isError={false}
      isLive={true}
      isLoading={false}
      isTerminal={false}
      namespace="default"
      terminalOutcome={null}
      timeline={projectTimeline(unnamedHistory)}
      workflowId={WORKFLOW_ID}
    />
  );

  expect(unnamed).toContain('>Workflow</h1>');
  expect(renderedHeaderId(unnamed)).toBe(WORKFLOW_ID);
  expect(unnamed).toContain('Name run');
});

// #211 boundary 1. Catches: the header depending solely on a fold over the
// LOADED history window, so a run named at start with more events than the
// window holds reads as unnamed. The server-projected summary is read from full
// history and supplies the name the window cannot reach.
test('the server-projected summary supplies a name the loaded window cannot reach', () => {
  // The naming event sits before the loaded window — history in view has none.
  const windowed = [workflowStarted(1)];
  const markup = renderDetail(
    <WorkflowDetailViewContent
      error={null}
      history={windowed}
      isError={false}
      isLive={true}
      isLoading={false}
      isTerminal={false}
      namespace="default"
      summaryDisplayName="named at start"
      terminalOutcome={null}
      timeline={projectTimeline(windowed)}
      workflowId={WORKFLOW_ID}
    />
  );

  expect(markup).toContain('>named at start</h1>');
  expect(renderedHeaderId(markup)).toBe(WORKFLOW_ID);
});

// Catches the inverse defect, which preferring the summary would introduce: a
// rename that arrived LIVE being masked by a summary fetched before it. Any
// rename lands at the head of history, so recorded history in view is the
// fresher reader and must win.
test('a rename in view supersedes a stale summary name', () => {
  const history = [workflowStarted(1), renamedTo(9, 'renamed just now')];
  const markup = renderDetail(
    <WorkflowDetailViewContent
      error={null}
      history={history}
      isError={false}
      isLive={true}
      isLoading={false}
      isTerminal={false}
      namespace="default"
      summaryDisplayName="stale name"
      terminalOutcome={null}
      timeline={projectTimeline(history)}
      workflowId={WORKFLOW_ID}
    />
  );

  expect(markup).toContain('>renamed just now</h1>');
  expect(renderedHeaderId(markup)).toBe(WORKFLOW_ID);
  expect(markup).not.toContain('stale name');
});

// Catches: a FAILED summary read being presented as the fact "this run has no
// name". It is said out loud — but only when it costs something, so a run whose
// name is already known from history stays quiet.
test('an unreadable summary is stated, and only when it actually costs a name', () => {
  const unnamed = [workflowStarted(1)];
  const silentOnKnownName = [workflowStarted(1), renamedTo(2, 'known from history')];
  const props = {
    error: null,
    isError: false,
    isLive: true,
    isLoading: false,
    isTerminal: false,
    namespace: 'default',
    summaryUnavailable: true,
    terminalOutcome: null,
    workflowId: WORKFLOW_ID,
  } as const;

  const noName = renderDetail(
    <WorkflowDetailViewContent {...props} history={unnamed} timeline={projectTimeline(unnamed)} />
  );
  expect(noName).toContain('name-unavailable');
  expect(noName).toContain('could not be loaded');

  const hasName = renderDetail(
    <WorkflowDetailViewContent
      {...props}
      history={silentOnKnownName}
      timeline={projectTimeline(silentOnKnownName)}
    />
  );
  expect(hasName).toContain('known from history');
  expect(hasName).not.toContain('name-unavailable');
});

// Catches: the header holding a name in local state, so a rename recorded
// elsewhere (the CLI, another console) never reaches this view. The header
// projects the LAST recorded write, always.
test('the header projects the last recorded name, not the first', () => {
  const history = [workflowStarted(1), renamedTo(2, 'first name'), renamedTo(3, 'second name')];
  const markup = renderDetail(
    <WorkflowDetailViewContent
      error={null}
      history={history}
      isError={false}
      isLive={true}
      isLoading={false}
      isTerminal={false}
      namespace="default"
      terminalOutcome={null}
      timeline={projectTimeline(history)}
      workflowId={WORKFLOW_ID}
    />
  );

  expect(markup).toContain('>second name</h1>');
  expect(renderedHeaderId(markup)).toBe(WORKFLOW_ID);
  // Scoped to the HEADING, in the same shape as the positive assertion above.
  // The previous form (`'first name ·'`) named a separator the heading has
  // never rendered, so it could not fail for any implementation — an inert
  // assertion reads as coverage while measuring nothing. A superseded name is
  // allowed to appear elsewhere in the markup (the timeline renders the
  // recorded rename events, and must), so a blanket absence check would be a
  // false red rather than a fix.
  expect(markup).not.toContain('>first name</h1>');
});

test('WorkflowDetailViewContent surfaces error state (no silent failure)', () => {
  const markup = renderDetail(
    <WorkflowDetailViewContent
      error={new Error('history failed')}
      history={[]}
      isError={true}
      isLive={false}
      isLoading={false}
      isTerminal={false}
      namespace="default"
      terminalOutcome={null}
      timeline={[]}
      workflowId={WORKFLOW_ID}
    />
  );

  expect(markup).toContain('history failed');
});

test('WorkflowDetailViewContent renders the loading skeleton while history is pending', () => {
  const markup = renderDetail(
    <WorkflowDetailViewContent
      error={null}
      history={[]}
      isError={false}
      isLive={false}
      isLoading={true}
      isTerminal={false}
      namespace="default"
      terminalOutcome={null}
      timeline={[]}
      workflowId={WORKFLOW_ID}
    />
  );

  expect(markup).toContain('Loading timeline');
});

test('WorkflowDetailViewContent renders the empty state for a history with no events', () => {
  const markup = renderDetail(
    <WorkflowDetailViewContent
      error={null}
      history={[]}
      isError={false}
      isLive={false}
      isLoading={false}
      isTerminal={false}
      namespace="default"
      terminalOutcome={null}
      timeline={[]}
      workflowId={WORKFLOW_ID}
    />
  );

  expect(markup).toContain('This workflow has no events yet');
});

test('WorkflowDetailViewContent renders the no-namespace state without an unscoped query', () => {
  const markup = renderDetail(
    <WorkflowDetailViewContent
      error={null}
      history={[]}
      isError={false}
      isLive={false}
      isLoading={false}
      isTerminal={false}
      namespace={null}
      terminalOutcome={null}
      timeline={[]}
      workflowId={WORKFLOW_ID}
    />
  );

  expect(markup).toContain('No namespace selected');
});

test('the reopen affordance appears for a terminal Failed run, not a fresh reopen event', () => {
  const failedHistory = [
    workflowStarted(1),
    activityScheduled(2, 10, 'charge'),
    terminalActivityFailed(3, 10, 1),
    workflowFailed(4, 'charge declined'),
  ];
  const withReopen = renderDetail(
    <WorkflowDetailViewContent
      error={null}
      history={failedHistory}
      isError={false}
      isLive={false}
      isLoading={false}
      isTerminal={true}
      namespace="default"
      terminalOutcome="failed"
      timeline={projectTimeline(failedHistory)}
      workflowId={WORKFLOW_ID}
    />
  );
  // The Reopen affordance is gated on the terminal Failed/Cancelled outcome, NOT
  // on an already-recorded WorkflowReopened event (the old inverted gate).
  expect(withReopen).toContain('reopen-open');
  // The failure context (failed step + reason) is surfaced where reopen is decided.
  expect(withReopen).toContain('failure-context');
  expect(withReopen).toContain('charge');
  expect(withReopen).toContain('charge declined');

  const cleanHistory = [workflowStarted(1), activityScheduled(2, 10, 'charge')];
  const withoutReopen = renderDetail(
    <WorkflowDetailViewContent
      error={null}
      history={cleanHistory}
      isError={false}
      isLive={true}
      isLoading={false}
      isTerminal={false}
      namespace="default"
      terminalOutcome={null}
      timeline={projectTimeline(cleanHistory)}
      workflowId={WORKFLOW_ID}
    />
  );
  // A running workflow shows neither the reopen affordance nor an empty failure panel.
  expect(withoutReopen).not.toContain('reopen-open');
  expect(withoutReopen).not.toContain('failure-context');
});

test('opening the reopen panel mounts ReopenDiff from the live history with an enabled commit', () => {
  const failedHistory = [
    workflowStarted(1),
    activityScheduled(2, 10, 'charge'),
    terminalActivityFailed(3, 10, 1),
    workflowFailed(4, 'charge declined'),
  ];
  const markup = renderDetail(
    <WorkflowDetailViewContent
      error={null}
      history={failedHistory}
      initialReopenOpen={true}
      isError={false}
      isLive={false}
      isLoading={false}
      isTerminal={true}
      namespace="default"
      terminalOutcome="failed"
      timeline={projectTimeline(failedHistory)}
      workflowId={WORKFLOW_ID}
    />
  );

  expect(markup).toContain('reopen-diff');
  expect(markup).toContain('Reopen preview');
  // The commit button is present and enabled (no stale "not yet available" note).
  expect(markup).toContain('reopen-commit');
  expect(markup).not.toContain('not yet available');
});
