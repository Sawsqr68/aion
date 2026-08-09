import { describe, expect, test } from 'bun:test';
import { renderToStaticMarkup } from 'react-dom/server';
import { MemoryRouter } from 'react-router';

import { DeployWorkerPanel } from './DeployWorkerPanel';

const WORKER_DOC =
  '//! Ships briefs.\nworker briefs\n  action send(to: String) -> Receipt\n    run "mailer send"\n';

function render(path: string, buffer: string): string {
  return renderToStaticMarkup(
    <MemoryRouter>
      <DeployWorkerPanel buffer={buffer} path={path} />
    </MemoryRouter>
  );
}

describe('DeployWorkerPanel', () => {
  // Catches: the panel losing the operator-policy fields the verb is built
  // from, or inventing values for them (the help text IS the no-defaults
  // contract, stated to the operator).
  test('a worker document renders the deployment form with the queue it declares', () => {
    const markup = render('pipelines/briefs-worker.awl', WORKER_DOC);
    expect(markup).toContain('Deploy as managed worker');
    expect(markup).toContain('briefs');
    // Name prefilled as a reading of the path; document path prefilled verbatim.
    expect(markup).toContain('value="briefs-worker"');
    expect(markup).toContain('value="pipelines/briefs-worker.awl"');
    // The connection-settings contract is stated, not defaulted away.
    expect(markup).toContain('no defaults anywhere in the stack');
    expect(markup).toContain('--concurrency');
    expect(markup).toContain('--reconnect-initial-backoff');
    expect(markup).toContain('--reconnect-max-backoff');
    expect(markup).toContain('--reconnect-max-attempts');
    // The honest path warning: the server, not the studio, resolves the path.
    // (Matched around the apostrophe, which static markup HTML-escapes.)
    expect(markup).toContain('Path the SERVER resolves when spawning the worker');
    expect(markup).toContain('working directory');
    expect(markup).toContain('Deploy worker');
  });

  // Catches: offering the worker-deploy flow on a workflow document, which
  // `aion worker awl` would refuse at spawn — after the deployment was
  // already durably written.
  test('a workflow document is refused up front', () => {
    const markup = render('dev_brief.awl', 'workflow dev_brief\n');
    expect(markup).toContain('Not a worker document');
    expect(markup).not.toContain('Deploy as managed worker');
  });
});
