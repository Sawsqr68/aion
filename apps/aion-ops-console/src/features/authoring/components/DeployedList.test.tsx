import { expect, test } from 'bun:test';
import { renderToStaticMarkup } from 'react-dom/server';

import type { DeployedVersion } from '../lib/deployed-facade';
import { DeployedList } from './DeployedList';

const HASH = 'abcdef0123456789'.repeat(4);

function version(overrides: Partial<DeployedVersion> = {}): DeployedVersion {
  return {
    workflowType: 'dev_brief',
    contentHash: HASH,
    loaded: true,
    routeActive: true,
    loadedAt: '2026-08-01T00:00:00+00:00',
    deployedAt: '2026-08-01T00:00:00+00:00',
    source: { state: 'available', documentName: 'dev_brief.awl', schemaCount: 0 },
    ...overrides,
  };
}

test('a deployed row names the workflow type, the content hash, and its archived document', () => {
  const html = renderToStaticMarkup(
    <DeployedList onSelect={() => undefined} selected={null} versions={[version()]} />
  );
  expect(html).toContain('dev_brief');
  // Short form on screen, full hash reachable — the version IS the hash, so it
  // must be readable without truncation somewhere.
  expect(html).toContain('abcdef01…23456789');
  expect(html).toContain(HASH);
  expect(html).toContain('AWL source: dev_brief.awl');
});

test('an available row with schemas counts them', () => {
  const html = renderToStaticMarkup(
    <DeployedList
      onSelect={() => undefined}
      selected={null}
      versions={[
        version({ source: { state: 'available', documentName: 'dev_brief.awl', schemaCount: 2 } }),
      ]}
    />
  );
  expect(html).toContain('AWL source: dev_brief.awl (+2 schemas)');
});

/**
 * 🔴 THE ABSENCE PIN. The three ways a version can have no readable source are
 * three DIFFERENT statements, and none of them is an error or a silence. A
 * single "no source" label would tell an operator that a healthy Gleam workflow
 * and a corrupt archive are the same thing.
 */
test('each absence state states its own reason and opens nothing', () => {
  const html = renderToStaticMarkup(
    <DeployedList
      onSelect={() => undefined}
      selected={null}
      versions={[
        version({ workflowType: 'gleam_flow', source: { state: 'absent' } }),
        version({ workflowType: 'operator_file', source: { state: 'not_persisted' } }),
        version({
          workflowType: 'broken',
          source: { state: 'unreadable', reason: 'archive contents are not a zip' },
        }),
      ]}
    />
  );
  expect(html).toContain('authored in Gleam, or deployed before source archiving');
  expect(html).toContain('loaded from an operator-supplied file');
  expect(html).toContain('Archive could not be read: archive contents are not a zip');
  // Three rows, three disabled buttons: a version with nothing to read does not
  // pretend the row leads somewhere.
  expect(html.split('disabled=""').length - 1).toBe(3);
});

/** The readable row is the control: it is NOT disabled. */
test('a version carrying archived source is selectable', () => {
  const html = renderToStaticMarkup(
    <DeployedList onSelect={() => undefined} selected={null} versions={[version()]} />
  );
  expect(html).not.toContain('disabled=""');
});

test('the selected version carries the current marker', () => {
  const html = renderToStaticMarkup(
    <DeployedList
      onSelect={() => undefined}
      selected={{ workflowType: 'dev_brief', contentHash: HASH }}
      versions={[version()]}
    />
  );
  expect(html).toContain('aria-current="true"');
});

test('an unavailable deployed surface says exactly that, and lists nothing', () => {
  const html = renderToStaticMarkup(
    <DeployedList onSelect={() => undefined} selected={null} unavailable versions={[version()]} />
  );
  expect(html).toContain('deployed workflow surface not available');
  expect(html).not.toContain('dev_brief');
});

test('an empty deployment set says so rather than showing an error', () => {
  const html = renderToStaticMarkup(
    <DeployedList onSelect={() => undefined} selected={null} versions={[]} />
  );
  expect(html).toContain('Nothing is deployed on this server yet.');
});

/**
 * The list runs no checks, so it claims no health. This is the same fabricated
 * claim the workspace document list carried until #130.
 */
test('the deployed list claims no check state it has not measured', () => {
  const html = renderToStaticMarkup(
    <DeployedList onSelect={() => undefined} selected={null} versions={[version()]} />
  );
  expect(html).not.toContain('deploys green');
  expect(html).not.toContain('data-status="healthy"');
});

test('an empty list caused by withheld sourceless versions says that, not "nothing deployed"', () => {
  const markup = renderToStaticMarkup(
    <DeployedList onSelect={() => {}} selected={null} sourcelessCount={3} versions={[]} />
  );

  expect(markup).toContain('3 deployed versions carry no archived AWL source');
  expect(markup).not.toContain('Nothing is deployed on this server yet');
});

test('a single withheld sourceless version is counted in the singular', () => {
  const markup = renderToStaticMarkup(
    <DeployedList onSelect={() => {}} selected={null} sourcelessCount={1} versions={[]} />
  );

  expect(markup).toContain('1 deployed version carries no archived AWL source');
});
