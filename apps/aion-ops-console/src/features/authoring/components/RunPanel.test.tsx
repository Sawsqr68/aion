import { describe, expect, test } from 'bun:test';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import type { ReactNode } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { MemoryRouter } from 'react-router';
import { startWorkflowDraftStoreFor } from '@/features/actions/lib/startWorkflowDraft';
import type { Namespace } from '@/types';

import type { DeploymentRecord } from '../lib/facade';
import { AvailabilityLine, nextOpenState, RunPanel, type RunPanelProps } from './RunPanel';

const DEPLOYMENT: DeploymentRecord = {
  deploymentId: 'dep-1',
  documentPath: 'hello.awl',
  contentHash: 'hash-abc',
  packageId: 'hello_flow$abc',
  workflowType: 'hello_flow',
  taskQueue: 'default',
  workflowId: null,
  runId: null,
};

function wrap(node: ReactNode): string {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return renderToStaticMarkup(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter>{node}</MemoryRouter>
    </QueryClientProvider>
  );
}

function render(overrides: Partial<RunPanelProps>): string {
  const props: RunPanelProps = {
    deployGranted: false,
    initialWorkflowType: undefined,
    lastDeployment: null,
    namespace: null,
    path: 'hello.awl',
    ...overrides,
  };
  return wrap(<RunPanel {...props} />);
}

describe('nextOpenState (the launcher toggle, pure)', () => {
  // Catches: the launcher wired one-way (open-only) or the close intents
  // flipping a closed panel open — static markup cannot click, so the
  // transition IS the tested toggle path.
  test('toggle flips both ways; close only ever closes', () => {
    expect(nextOpenState(false, 'toggle')).toBe(true);
    expect(nextOpenState(true, 'toggle')).toBe(false);
    expect(nextOpenState(true, 'close')).toBe(false);
    expect(nextOpenState(false, 'close')).toBe(false);
  });
});

describe('RunPanel', () => {
  // Non-modal by ruling: closed means ONLY the launcher exists — no hidden
  // panel in the tree, no backdrop anywhere.
  test('closed by default: the launcher renders, the panel does not', () => {
    const markup = render({});
    expect(markup).toContain('data-slot="run-panel-launcher"');
    expect(markup).toContain('aria-expanded="false"');
    expect(markup).not.toContain('data-slot="run-panel"');
  });

  test('open, it is an aside hosting the real start form', () => {
    const markup = render({ defaultOpen: true, namespace: 'orders' as Namespace });
    expect(markup).toContain('data-slot="run-panel"');
    expect(markup).toContain('<aside');
    expect(markup).toContain('aria-label="Run workflow"');
    expect(markup).toContain('Close the run panel');
    // The hosted StartWorkflowForm, with the selected namespace (so no
    // free-form namespace field) and its own submit.
    expect(markup).toContain('Workflow type');
    expect(markup).toContain('Start workflow');
    // No backdrop, no dialog semantics — the editor stays interactive.
    expect(markup).not.toContain('aria-modal');
  });

  // Catches: the launcher disclosing an id that is not the panel's — the
  // aria-controls wiring must name the aside it actually opens.
  test('the launcher aria-controls the id the open aside carries', () => {
    const markup = render({ defaultOpen: true, namespace: 'orders' as Namespace });
    const controls = markup.match(/aria-controls="([^"]+)"/);
    expect(controls).not.toBeNull();
    expect(markup).toContain(`id="${controls?.[1] ?? ''}"`);
  });

  // Catches: the embedded form reading (or clobbering) the Actions page
  // draft — its field state must come from the per-document authoring scope.
  test('the form reads the document-scoped authoring draft, not the Actions draft', () => {
    const authoring = startWorkflowDraftStoreFor('authoring:hello.awl');
    authoring.getState().setDraft({ workflowType: 'scoped_flow' });
    try {
      const markup = render({ defaultOpen: true, namespace: 'orders' as Namespace });
      expect(markup).toContain('value="scoped_flow"');
    } finally {
      authoring.getState().clearDraft();
    }
  });

  // Catches: two documents sharing one authoring draft — the scope carries
  // the path, so another document's panel must not see this one's draft.
  test('a different document does not see this document’s draft', () => {
    const authoring = startWorkflowDraftStoreFor('authoring:hello.awl');
    authoring.getState().setDraft({ workflowType: 'scoped_flow' });
    try {
      const markup = render({
        defaultOpen: true,
        namespace: 'orders' as Namespace,
        path: 'other.awl',
      });
      expect(markup).not.toContain('value="scoped_flow"');
    } finally {
      authoring.getState().clearDraft();
    }
  });

  // Catches: an availability line invented without a deploy this session —
  // there is no queue to ask about until one landed.
  test('no deploy this session, no availability line', () => {
    const markup = render({ defaultOpen: true, namespace: 'orders' as Namespace });
    expect(markup).not.toContain('data-slot="run-panel-availability"');
  });

  test('a deploy this session asks about its queue (in-flight on first paint)', () => {
    const markup = render({
      defaultOpen: true,
      lastDeployment: DEPLOYMENT,
      namespace: 'orders' as Namespace,
      availabilityClient: {
        workerAvailability: () =>
          Promise.resolve({
            available: true,
            taskQueue: 'default',
            connectedWorkers: 2,
            scaffoldHint: null,
          }),
      },
    });
    expect(markup).toContain('data-slot="run-panel-availability"');
    expect(markup).toContain('Checking what serves task queue default…');
  });

  test('a deploy without a selected namespace says why it cannot check', () => {
    const markup = render({ defaultOpen: true, lastDeployment: DEPLOYMENT });
    expect(markup).toContain('Select a namespace to check what serves task queue default.');
  });
});

describe('AvailabilityLine', () => {
  const namespace = 'orders' as Namespace;

  test('a served queue reports the worker count', () => {
    const markup = renderToStaticMarkup(
      <AvailabilityLine
        availability={{
          available: true,
          taskQueue: 'default',
          connectedWorkers: 2,
          scaffoldHint: null,
        }}
        error={null}
        namespace={namespace}
        pending={false}
        taskQueue="default"
      />
    );
    expect(markup).toContain('Task queue default is served — 2 connected workers.');
  });

  test('one worker is singular', () => {
    const markup = renderToStaticMarkup(
      <AvailabilityLine
        availability={{
          available: true,
          taskQueue: 'default',
          connectedWorkers: 1,
          scaffoldHint: null,
        }}
        error={null}
        namespace={namespace}
        pending={false}
        taskQueue="default"
      />
    );
    expect(markup).toContain('1 connected worker.');
  });

  // Catches: the scaffold hint (the server saying HOW to serve the queue)
  // flattened into a generic "unavailable".
  test('an unserved queue renders the scaffold hint verbatim', () => {
    const markup = renderToStaticMarkup(
      <AvailabilityLine
        availability={{
          available: false,
          taskQueue: 'default',
          connectedWorkers: 0,
          scaffoldHint: 'scaffold a worker with `aion awl scaffold`',
        }}
        error={null}
        namespace={namespace}
        pending={false}
        taskQueue="default"
      />
    );
    expect(markup).toContain('scaffold a worker with `aion awl scaffold`');
  });

  test('an unserved queue without a hint still says nothing serves it', () => {
    const markup = renderToStaticMarkup(
      <AvailabilityLine
        availability={{
          available: false,
          taskQueue: 'default',
          connectedWorkers: 0,
          scaffoldHint: null,
        }}
        error={null}
        namespace={namespace}
        pending={false}
        taskQueue="default"
      />
    );
    expect(markup).toContain('Nothing serves task queue default yet.');
  });

  // Catches: a failed availability check rendered as nothing — the line
  // informs, and a failure to inform must say so.
  test('a failed check renders the error text, never silence', () => {
    const markup = renderToStaticMarkup(
      <AvailabilityLine
        availability={null}
        error={new Error('availability endpoint unreachable')}
        namespace={namespace}
        pending={false}
        taskQueue="default"
      />
    );
    expect(markup).toContain('Availability check failed: availability endpoint unreachable');
  });

  // Catches: `String(error)` on a thrown plain object — "[object Object]"
  // is a line that says nothing; the JSON says what was thrown.
  test('a non-Error thrown value renders as JSON, not "[object Object]"', () => {
    const markup = renderToStaticMarkup(
      <AvailabilityLine
        availability={null}
        error={{ code: 'queue_gone', detail: 7 }}
        namespace={namespace}
        pending={false}
        taskQueue="default"
      />
    );
    expect(markup).not.toContain('[object Object]');
    expect(markup).toContain('queue_gone');
    // HTML-escaped JSON: quotes render as &quot;.
    expect(markup).toContain('detail');
  });

  // Catches: the stringify fallback itself failing silently — a value JSON
  // cannot express must still be NAMED.
  test('a value JSON cannot express is named unrenderable, never blank', () => {
    const cyclic: Record<string, unknown> = {};
    cyclic.self = cyclic;
    const markup = renderToStaticMarkup(
      <AvailabilityLine
        availability={null}
        error={cyclic}
        namespace={namespace}
        pending={false}
        taskQueue="default"
      />
    );
    expect(markup).toContain('Availability check failed: unrenderable error value');
  });

  test('no namespace explains itself instead of pretending to check', () => {
    const markup = renderToStaticMarkup(
      <AvailabilityLine
        availability={null}
        error={null}
        namespace={null}
        pending={false}
        taskQueue="default"
      />
    );
    expect(markup).toContain('Select a namespace to check what serves task queue default.');
  });
});
