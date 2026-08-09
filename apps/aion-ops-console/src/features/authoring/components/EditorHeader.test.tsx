import { describe, expect, test } from 'bun:test';
import { renderToStaticMarkup } from 'react-dom/server';

import { deployEligibility } from '../lib/deploy-eligibility';
import { EditorHeader, type EditorHeaderProps } from './EditorHeader';

const GREEN = { deploysGreen: true, diagnostics: [] };
const RED = {
  deploysGreen: false,
  diagnostics: [{ class: 'error' as const, message: 'unknown step `shp`', line: 3, column: 5 }],
};

function render(overrides: Partial<EditorHeaderProps>): string {
  const props: EditorHeaderProps = {
    deploy: deployEligibility({ workerDocument: false, deployGranted: true, check: GREEN }),
    // Dirty by default so Save is enabled: the disabled-attribute assertions
    // below then speak about Deploy alone.
    dirty: true,
    onDeploy: () => undefined,
    onFormat: () => undefined,
    onSave: () => undefined,
    onSelectViewMode: () => undefined,
    path: 'hello.awl',
    run: {
      deployGranted: true,
      initialWorkflowType: 'hello_flow',
      lastDeployment: null,
      namespace: null,
      path: 'hello.awl',
    },
    viewMode: 'text',
    workerDocument: false,
    working: null,
    ...overrides,
  };
  return renderToStaticMarkup(<EditorHeader {...props} />);
}

describe('EditorHeader', () => {
  // The ruled row: two separate buttons, Deploy then Run, after Format/Save.
  test('a green granted workflow document offers Format, Save, Deploy, and Run', () => {
    const markup = render({});
    expect(markup).toContain('Format');
    expect(markup).toContain('Save');
    expect(markup).toContain('>Deploy</button>');
    expect(markup).toContain('data-slot="run-panel-launcher"');
    // Ready means enabled: neither native nor aria disabling, and no reason.
    expect(markup).not.toContain('disabled=""');
    expect(markup).not.toContain('aria-disabled');
    expect(markup).not.toContain('data-slot="deploy-reason"');
    // Ship & Run is retired: a workflow document has no run view tab at all.
    expect(markup).not.toContain('aria-label="Deploy worker view"');
  });

  // Catches: workflow-deploy offered on a worker document — it ships through
  // DeployWorkerPanel — and a run affordance for something that is not a flow.
  test('a worker document hides both Deploy and Run', () => {
    const markup = render({
      deploy: deployEligibility({ workerDocument: true, deployGranted: true, check: GREEN }),
      workerDocument: true,
    });
    expect(markup).not.toContain('>Deploy</button>');
    expect(markup).not.toContain('data-slot="run-panel-launcher"');
    // The view switch still offers the worker's own deploy surface.
    expect(markup).toContain('Deploy worker');
  });

  test('no deploy grant hides Deploy but keeps Run — running needs no grant', () => {
    const markup = render({
      deploy: deployEligibility({ workerDocument: false, deployGranted: false, check: GREEN }),
    });
    expect(markup).not.toContain('>Deploy</button>');
    expect(markup).toContain('data-slot="run-panel-launcher"');
  });

  // Catches: a red document deployable anyway, or the reason unreachable —
  // aria-disabled keeps the button FOCUSABLE (native `disabled` would hide it
  // from keyboard/AT), aria-describedby points at the reason rendered as
  // VISIBLE text, and the title rides along for pointer hover.
  test('a red check aria-disables Deploy with the reason reachable three ways', () => {
    const markup = render({
      deploy: deployEligibility({ workerDocument: false, deployGranted: true, check: RED }),
    });
    expect(markup).toContain('aria-disabled="true"');
    expect(markup).not.toContain('disabled=""');
    expect(markup).toContain('title="unknown step `shp`"');
    // The describedby target exists and renders the reason as visible text.
    const described = markup.match(/aria-describedby="([^"]+)"/);
    expect(described).not.toBeNull();
    expect(markup).toContain(`id="${described?.[1] ?? ''}"`);
    expect(markup).toContain('data-slot="deploy-reason"');
    expect(markup).toContain('unknown step `shp`</span>');
  });

  test('a still-running check aria-disables Deploy with its own reason', () => {
    const markup = render({
      deploy: deployEligibility({ workerDocument: false, deployGranted: true, check: null }),
    });
    expect(markup).toContain('aria-disabled="true"');
    expect(markup).toContain('title="The document is still being checked"');
    expect(markup).toContain('The document is still being checked</span>');
  });

  test('an in-flight deploy reports itself and blocks re-entry', () => {
    const markup = render({ working: 'deploy' });
    expect(markup).toContain('Deploying…');
    expect(markup).not.toContain('>Deploy</button>');
    expect(markup).toContain('aria-disabled="true"');
  });

  // The run panel is CLOSED by default: only its launcher renders, and the
  // launcher says so. (Open-state markup is RunPanel's own test's business.)
  test('the run panel starts closed — a launcher, no panel', () => {
    const markup = render({});
    expect(markup).toContain('aria-expanded="false"');
    expect(markup).not.toContain('data-slot="run-panel"');
  });
});
