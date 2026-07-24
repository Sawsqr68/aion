import { describe, expect, test } from 'bun:test';
import { renderToStaticMarkup } from 'react-dom/server';

import type { ProjectionStatementGraph, ProjectionStatementNode } from '../lib/projection-types';
import { StatementTopology } from './StatementTopology';

const span = { start: 10, end: 20, line: 2, column: 3 };
const forkNode: ProjectionStatementNode = {
  id: 'statement:10:20',
  kind: 'parallel_fork',
  label: 'entry in wave.entries',
  detail: 'join → lane_results',
  span,
  fork: {
    binding: 'entry',
    collection: 'wave.entries',
    sequential: false,
    result: 'lane_results',
  },
  graph: {
    nodes: [
      {
        id: 'statement:12:18',
        kind: 'child_call',
        label: 'lane_run',
        detail: 'lane_run(lane_id: String) -> LaneResult',
        span,
        fork: null,
        graph: null,
      },
    ],
    edges: [],
  },
};
const waitNode: ProjectionStatementNode = {
  id: 'statement:30:40',
  kind: 'wait',
  label: 'wait ruling',
  detail: '→ rulings',
  span,
  fork: null,
  graph: null,
};

function render(graph: ProjectionStatementGraph) {
  return renderToStaticMarkup(
    <StatementTopology graph={graph} label="Body flow" onJumpTo={() => undefined} />
  );
}

describe('positional statement topology', () => {
  test('renders fork branch members and waits as wired nodes', () => {
    const html = render({
      nodes: [forkNode, waitNode],
      edges: [
        {
          id: 'flow:statement:10:20:statement:30:40',
          source: forkNode.id,
          target: waitNode.id,
        },
      ],
    });
    expect(html).toContain('data-statement-kind="parallel_fork"');
    expect(html).toContain('Parallel fan-out');
    expect(html).toContain('×N over wave.entries · parallel');
    expect(html).toContain('aria-label="Branch flow"');
    expect(html).toContain('data-statement-kind="child_call"');
    expect(html).toContain('lane_run');
    expect(html).toContain('data-statement-kind="wait"');
    expect(html).toContain('wait ruling');
    expect(html).toContain('data-source="statement:10:20"');
    expect(html).toContain('data-target="statement:30:40"');
  });

  test('distinguishes sequential collection fan-out', () => {
    const html = render({
      nodes: [
        {
          ...forkNode,
          kind: 'sequential_fork',
          fork: forkNode.fork === null ? null : { ...forkNode.fork, sequential: true },
        },
      ],
      edges: [],
    });
    expect(html).toContain('Sequential fan-out');
    expect(html).toContain('· in order');
  });
});
