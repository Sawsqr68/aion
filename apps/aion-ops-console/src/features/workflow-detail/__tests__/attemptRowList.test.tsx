import { expect, test } from 'bun:test';
import { renderToStaticMarkup } from 'react-dom/server';

import type { AttemptNavigatorRow } from '../lib/attemptNavigator';
import { AttemptRowList } from '../swimlane/AttemptNavigator';

const RUN = '00000000-0000-0000-0000-0000000000a1';
/** The next generation of the same chain — same coordinate, different run. */
const SUCCESSOR_RUN = '00000000-0000-0000-0000-0000000000a2';

const rows: AttemptNavigatorRow[] = [
  {
    key: `${RUN}:3:1`,
    runId: RUN,
    activityId: '3',
    activityType: 'agent',
    attempt: 1,
    isLegacy: false,
    status: 'completed',
  },
  {
    key: `${RUN}:3:2`,
    runId: RUN,
    activityId: '3',
    activityType: 'agent',
    attempt: 2,
    isLegacy: false,
    status: 'started',
  },
];

test('attempt rows badge retained transcripts from the enumeration', () => {
  const markup = renderToStaticMarkup(
    <AttemptRowList
      onSelect={() => {}}
      retainedKeys={new Set([`${RUN}:3:1`])}
      rows={rows}
      selectedKey={null}
    />
  );

  // The badge appears exactly for the enumerated row: rows stay
  // timeline-derived; the retained enumeration is a badge source only.
  expect(markup).toContain(`data-testid="retained:${RUN}:3:1"`);
  expect(markup).toContain('retained');
  expect(markup).not.toContain(`retained:${RUN}:3:2`);
});

test('rows from a mid-chain tail window render their own generations, not a disclaimer', () => {
  // The rows a tail window produces once the boundaries come from the server.
  // Under the old window-scanning derivation this exact input was unreachable —
  // a window with no `WorkflowStarted` gave every row `runId: null`, and both
  // rows rendered "run unknown". They now carry two DIFFERENT runs, so the
  // assertion below is a positive statement about what an operator sees, not the
  // absence of a string.
  const tailRows: AttemptNavigatorRow[] = [
    {
      key: `${RUN}:3:1`,
      runId: RUN,
      activityId: '3',
      activityType: 'agent',
      attempt: 1,
      isLegacy: false,
      status: 'completed',
    },
    {
      key: `${SUCCESSOR_RUN}:3:1`,
      runId: SUCCESSOR_RUN,
      activityId: '3',
      activityType: 'agent',
      attempt: 1,
      isLegacy: false,
      status: 'completed',
    },
  ];
  const markup = renderToStaticMarkup(
    <AttemptRowList
      onSelect={() => {}}
      retainedKeys={new Set([`${SUCCESSOR_RUN}:3:1`])}
      rows={tailRows}
      selectedKey={null}
    />
  );

  // Two rows for one coordinate, each addressable in its own right: the badge
  // lands on the successor's row alone, which is only possible because the two
  // rows carry distinct keys rather than a shared unresolved one.
  expect(markup).toContain(`data-testid="retained:${SUCCESSOR_RUN}:3:1"`);
  expect(markup).not.toContain(`retained:${RUN}:3:1`);
  expect(markup).not.toContain('unresolved-run');
  expect(markup).not.toContain('run unknown');
});
