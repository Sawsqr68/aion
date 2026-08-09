import { expect, test } from 'bun:test';
import { renderToStaticMarkup } from 'react-dom/server';

import type { AttemptNavigatorRow } from '../lib/attemptNavigator';
import { AttemptRowList } from '../swimlane/AttemptNavigator';

const RUN = '00000000-0000-0000-0000-0000000000a1';

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

test('a row whose generation the window cannot name says so, and is not badged', () => {
  // A tail window that begins mid-generation yields rows with no run. Such a row
  // cannot address a transcript stream, so the operator is told why rather than
  // being shown an empty panel with no explanation.
  const unresolved: AttemptNavigatorRow[] = [
    {
      key: '-:3:1',
      runId: null,
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
      retainedKeys={new Set<string>()}
      rows={unresolved}
      selectedKey={null}
    />
  );

  expect(markup).toContain('data-testid="unresolved-run:-:3:1"');
  expect(markup).toContain('run unknown');
});
