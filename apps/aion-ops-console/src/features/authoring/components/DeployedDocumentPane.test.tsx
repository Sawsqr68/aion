import { expect, test } from 'bun:test';
import { renderToStaticMarkup } from 'react-dom/server';

import type { DeployedDocument } from '../lib/deployed-facade';
import { DeployedDocumentPane } from './DeployedDocumentPane';

const HASH = 'abcdef0123456789'.repeat(4);

function deployed(overrides: Partial<DeployedDocument> = {}): DeployedDocument {
  return {
    workflowType: 'dev_brief',
    contentHash: HASH,
    documentName: 'dev_brief.awl',
    source: '//! Deployed probe.\nworkflow dev_brief\n',
    schemas: [],
    projection: {
      ok: true,
      deploysGreen: true,
      steps: 3,
      diagnostics: [],
      semantic: null,
    },
    // Pre-contract shape: the server sends `input_schema: null, signals: null`
    // for these archives — never `signals: []`, which is a different fact (a
    // contract that commits an empty signal set).
    inputSchema: null,
    signals: null,
    ...overrides,
  };
}

/**
 * 🔴 THE UNAMBIGUITY PIN. The pane says what it is showing, and names the
 * artifact the way the deploy surface names it: workflow type plus content
 * hash. An operator must never have to guess whether the source in front of
 * them is their draft or what is running.
 */
test('the pane is labelled as the deployed artifact and identifies it by type and hash', () => {
  const html = renderToStaticMarkup(<DeployedDocumentPane document={deployed()} />);
  expect(html).toContain('Deployed · read-only');
  expect(html).toContain('dev_brief.awl');
  expect(html).toContain('dev_brief');
  expect(html).toContain('abcdef01…23456789');
  expect(html).toContain(HASH);
  expect(html).toContain('3 steps');
  expect(html).toContain('no imported schemas');
});

/**
 * 🔴 NO MUTATION AFFORDANCE IS REACHABLE. Every control the workspace editor
 * offers for changing or shipping a document is named here and asserted
 * ABSENT — Save, Format, the gesture-bearing Studio view, and the header's
 * Deploy button and Run launcher. The view switcher above is the survival
 * half: the pane does render its read surfaces, so this is not passing
 * because nothing rendered.
 */
test('the deployed pane exposes no save, format, studio, deploy, or run control', () => {
  const html = renderToStaticMarkup(<DeployedDocumentPane document={deployed()} />);
  for (const control of ['Save', 'Format', 'Studio']) {
    expect(html).not.toContain(control);
  }
  // Matched as markup, not bare words — the pane's own copy says "Deployed".
  expect(html).not.toContain('>Deploy</button>');
  expect(html).not.toContain('data-slot="run-panel-launcher"');
  expect(html).not.toContain('Modified');
  // Survival: the read-only view surfaces are all present.
  expect(html).toContain('Text view');
  expect(html).toContain('Canvas view');
  expect(html).toContain('Split view');
});

/** The archived schemas are named, so the operator sees what it was checked against. */
test('archived schemas are named in the footer', () => {
  const html = renderToStaticMarkup(
    <DeployedDocumentPane
      document={deployed({
        schemas: [
          { path: 'schemas/brief.schema.json', text: '{"type":"object"}', byteLength: 17 },
          { path: 'schemas/step.schema.json', text: null, byteLength: 4 },
        ],
      })}
    />
  );
  expect(html).toContain('2 archived schemas');
  expect(html).toContain('schemas/brief.schema.json');
  expect(html).toContain('schemas/step.schema.json');
});

/**
 * A deployed version whose archived source no longer checks green is reported
 * as it is. The pane states the projection it was given; it does not launder a
 * red into a green because the version is deployed.
 */
test('a projection with errors is reported, not smoothed over', () => {
  const html = renderToStaticMarkup(
    <DeployedDocumentPane
      document={deployed({
        projection: {
          ok: false,
          deploysGreen: false,
          steps: null,
          diagnostics: [
            {
              class: 'error',
              message: 'unknown step',
              line: 4,
              column: 2,
              range: { start: { line: 3, character: 1 }, end: { line: 3, character: 5 } },
            },
          ],
          semantic: null,
        },
      })}
    />
  );
  expect(html).toContain('1 errors');
  expect(html).toContain('projection unavailable');
  expect(html).not.toContain('deploys green');
});

/**
 * The contract strip distinguishes the three signal answers the server can
 * give. Catches: `null` (no committed signal set — additional entry,
 * pre-contract, older server) rendered as if it were the empty list, or the
 * committed-empty list rendered as absence — the exact absent-vs-empty
 * collapse blocker B1 grew from.
 */
test('the contract strip states committed schema and signals, absence as absence', () => {
  const preContract = renderToStaticMarkup(<DeployedDocumentPane document={deployed()} />);
  expect(preContract).toContain('no committed start schema');
  expect(preContract).toContain('no committed signal set');

  const committedEmpty = renderToStaticMarkup(
    <DeployedDocumentPane document={deployed({ inputSchema: { type: 'object' }, signals: [] })} />
  );
  expect(committedEmpty).toContain('start schema committed');
  expect(committedEmpty).toContain('signals: none committed');
  expect(committedEmpty).not.toContain('no committed signal set');

  const committed = renderToStaticMarkup(
    <DeployedDocumentPane
      document={deployed({
        inputSchema: { type: 'object' },
        signals: [
          { name: 'approve', inputSchema: { type: 'object' } },
          { name: 'escalate', inputSchema: { type: 'object' } },
        ],
      })}
    />
  );
  expect(committed).toContain('signals: approve, escalate');
});
