import { expect, test } from 'bun:test';
import { renderToStaticMarkup } from 'react-dom/server';

import { DocumentList } from './DocumentList';

test('document list renders the empty state', () => {
  const html = renderToStaticMarkup(
    <DocumentList documents={[]} onSelect={() => undefined} selectedPath={null} />
  );
  expect(html).toContain('Create a workflow to begin.');
});

test('document list renders the exact unconfigured workspace state', () => {
  const html = renderToStaticMarkup(
    <DocumentList documents={[]} onSelect={() => undefined} selectedPath={null} unconfigured />
  );
  expect(html).toContain('authoring workspace not configured');
  expect(html).not.toContain('Create a workflow to begin.');
});

test('document list exposes the New Workflow form when creation is configured', () => {
  const html = renderToStaticMarkup(
    <DocumentList
      documents={[]}
      onCreate={async () => undefined}
      onSelect={() => undefined}
      selectedPath={null}
    />
  );
  expect(html).toContain('New workflow');
  expect(html).toContain('id="new-workflow-name"');
  expect(html).toContain('Create workflow');
});

test('document rows expose name, path, and selected semantics — and claim no deploy state', () => {
  const html = renderToStaticMarkup(
    <DocumentList
      documents={[
        { name: 'checkout.awl', path: 'workflows/checkout.awl' },
        { name: 'refund.awl', path: 'workflows/refund.awl' },
      ]}
      onSelect={() => undefined}
      selectedPath="workflows/checkout.awl"
    />
  );

  expect(html).toContain('checkout.awl');
  expect(html).toContain('workflows/checkout.awl');
  expect(html).toContain('refund.awl');
  expect(html).toContain('workflows/refund.awl');
  expect(html).toContain('aria-current="true"');
  // The list runs no checks, so it may not claim per-document deploy state:
  // the old hardcoded "deploys green" badge asserted health nothing measured.
  expect(html).not.toContain('deploys green');
  expect(html).not.toContain('data-status="healthy"');
});
