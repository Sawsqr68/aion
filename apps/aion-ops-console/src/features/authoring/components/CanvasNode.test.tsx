import { describe, expect, test } from 'bun:test';
import { renderToStaticMarkup } from 'react-dom/server';
import { ReactFlowProvider } from 'reactflow';

import type { EditResult, GestureOperation } from '../lib/facade';
import type { ProjectionStep } from '../lib/projection-types';
import { type CanvasMode, proseEditAffordance } from './AuthoringCanvas';
import { type CanvasNodeData, StepCanvasNode } from './CanvasNode';

const shipStep: ProjectionStep = {
  name: 'ship',
  documentation: 'Ship the build.',
  activities: [],
  span: { start: 0, end: 4, line: 1, column: 1 },
  kind: 'plain',
  region: null,
  distribution: null,
  collect: null,
  subflow: null,
  substeps: [],
  visits: null,
  decision: false,
  waits: false,
  body: { nodes: [], edges: [] },
  failure: null,
};

async function refuseGesture(_operation: GestureOperation): Promise<EditResult> {
  throw new Error('Run mode is read-only');
}

/** Build the node data the canvas would hand this step in the given mode. */
function nodeDataFor(mode: CanvasMode): CanvasNodeData {
  return {
    kind: 'step',
    name: shipStep.name,
    label: shipStep.documentation,
    step: shipStep,
    expandedSubflows: new Set(),
    onToggleSubflow: () => undefined,
    onJumpTo: () => undefined,
    onActivate: () => undefined,
    proseEditing: false,
    ...proseEditAffordance(mode, shipStep.name, () => undefined, refuseGesture),
    onCancelProseEdit: () => undefined,
  };
}

function renderNode(mode: CanvasMode): string {
  return renderToStaticMarkup(
    <ReactFlowProvider>
      <StepCanvasNode
        data={nodeDataFor(mode)}
        dragging={false}
        id={shipStep.name}
        isConnectable={false}
        selected={false}
        type="step"
        xPos={0}
        yPos={0}
        zIndex={0}
      />
    </ReactFlowProvider>
  );
}

describe('canvas node mutating affordances follow the canvas mode', () => {
  /**
   * The control case. Absence only means something if the same assertions see
   * the affordance when it is supposed to be there.
   */
  test('the edit canvas offers prose editing and the callbacks that perform it', () => {
    const data = nodeDataFor('edit');
    expect(data.onBeginProseEdit).toBeDefined();
    expect(data.onSaveProse).toBeDefined();

    const html = renderNode('edit');
    expect(html).toContain('Edit prose');
    expect(html).toContain('aria-label="Edit prose for ship"');
    expect(html).toContain('aria-label="Step ship"');
  });

  /**
   * Run mode watches a live workflow: the gesture underneath is refused, so the
   * button must never be offered in the first place.
   */
  test('run mode renders the step and NO mutating affordance', () => {
    const data = nodeDataFor('run');
    expect(data.onBeginProseEdit).toBeUndefined();
    expect(data.onSaveProse).toBeUndefined();

    const html = renderNode('run');
    expect(html).toContain('aria-label="Step ship"');
    expect(html).not.toContain('Edit prose');
    expect(html).not.toContain('Save prose');
    expect(html).not.toContain('<textarea');
    expect(html).not.toContain('<form');
  });

  /** A deployed artifact came out of an archive; the same rule holds. */
  test('deployed mode renders the step and NO mutating affordance', () => {
    const data = nodeDataFor('deployed');
    expect(data.onBeginProseEdit).toBeUndefined();
    expect(data.onSaveProse).toBeUndefined();

    const html = renderNode('deployed');
    expect(html).toContain('aria-label="Step ship"');
    expect(html).not.toContain('Edit prose');
    expect(html).not.toContain('Save prose');
    expect(html).not.toContain('<textarea');
    expect(html).not.toContain('<form');
  });

  /**
   * The prose form is only reachable through the withheld callback, but pin the
   * node's own guard too: a node told it is editing still needs the callbacks.
   */
  test('the prose form renders only for a node that was given the edit callbacks', () => {
    const editing = renderToStaticMarkup(
      <ReactFlowProvider>
        <StepCanvasNode
          data={{ ...nodeDataFor('edit'), proseEditing: true }}
          dragging={false}
          id={shipStep.name}
          isConnectable={false}
          selected={false}
          type="step"
          xPos={0}
          yPos={0}
          zIndex={0}
        />
      </ReactFlowProvider>
    );
    expect(editing).toContain('Save prose');
    expect(editing).toContain('<textarea');
  });
});
