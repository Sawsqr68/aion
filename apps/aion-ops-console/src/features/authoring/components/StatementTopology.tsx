import { cn } from '@/lib/utils';
import type {
  ProjectionStatementGraph,
  ProjectionStatementKind,
  ProjectionStatementNode,
} from '../lib/projection-types';

type StatementTopologyProps = {
  graph: ProjectionStatementGraph;
  label: string;
  onJumpTo: (byteOffset: number) => void;
};

/** Positional executable flow nested inside its owning authored step. */
export function StatementTopology({ graph, label, onJumpTo }: StatementTopologyProps) {
  if (graph.nodes.length === 0) return null;
  return (
    <section aria-label={label} className="border-border border-t px-3 py-3">
      <span className="mb-2 block font-semibold text-[10px] text-muted-foreground uppercase tracking-wide">
        {label}
      </span>
      <div className="flex flex-col items-stretch">
        {graph.nodes.map((node, index) => {
          const edge = graph.edges.find((candidate) => candidate.source === node.id);
          return (
            <div key={node.id}>
              <StatementNode node={node} onJumpTo={onJumpTo} />
              {index < graph.nodes.length - 1 && edge !== undefined && (
                <div
                  aria-label={`${node.label} flows to ${graph.nodes[index + 1]?.label ?? 'next statement'}`}
                  className="mx-auto h-5 w-px bg-muted-foreground/50"
                  data-edge-id={edge.id}
                  data-source={edge.source}
                  data-target={edge.target}
                  role="img"
                />
              )}
            </div>
          );
        })}
      </div>
    </section>
  );
}

function StatementNode({
  node,
  onJumpTo,
}: {
  node: ProjectionStatementNode;
  onJumpTo: (byteOffset: number) => void;
}) {
  const forkLabel = forkKindLabel(node.kind);
  const isContainer = node.graph !== null;
  return (
    <article
      className={cn(
        'rounded-lg border border-border bg-surface-base',
        isContainer && 'border-accent-primary/40 bg-accent-primary/5'
      )}
      data-statement-kind={node.kind}
    >
      <button
        className="nodrag w-full p-2 text-left outline-none focus-visible:ring-2 focus-visible:ring-accent-primary"
        onClick={() => onJumpTo(node.span.start)}
        type="button"
      >
        <span className="flex items-center justify-between gap-2">
          <span className="truncate font-mono font-semibold text-foreground text-xs">
            {node.label}
          </span>
          <span className="shrink-0 text-[10px] text-muted-foreground uppercase tracking-wide">
            {forkLabel ?? statementKindLabel(node.kind)}
          </span>
        </span>
        {node.detail !== null && (
          <span className="mt-1 block truncate font-mono text-[10px] text-muted-foreground">
            {node.detail}
          </span>
        )}
        {node.fork?.collection !== null && node.fork?.collection !== undefined && (
          <span className="mt-1 block text-[10px] text-accent-primary">
            ×N over {node.fork.collection}
            {node.fork.sequential ? ' · in order' : ' · parallel'}
          </span>
        )}
      </button>
      {node.graph !== null && (
        <div className="m-2 mt-0 rounded-md border border-border border-dashed bg-surface-elevated p-2">
          <StatementTopology
            graph={node.graph}
            label={forkLabel === null ? `${statementKindLabel(node.kind)} flow` : 'Branch flow'}
            onJumpTo={onJumpTo}
          />
        </div>
      )}
    </article>
  );
}

function forkKindLabel(kind: ProjectionStatementKind): string | null {
  if (kind === 'parallel_fork') return 'Parallel fan-out';
  if (kind === 'sequential_fork') return 'Sequential fan-out';
  if (kind === 'named_fork') return 'Named fork';
  return null;
}

function statementKindLabel(kind: ProjectionStatementKind): string {
  const labels: Record<ProjectionStatementKind, string> = {
    action_call: 'Action call',
    child_call: 'Child call',
    subflow_call: 'Subflow call',
    unresolved_call: 'Unresolved call',
    spawn_child: 'Spawn child',
    invalid_spawn: 'Invalid spawn',
    pipe: 'Pipe',
    action_stage: 'Action stage',
    child_stage: 'Child stage',
    unresolved_stage: 'Unresolved stage',
    field_stage: 'Field stage',
    combinator_stage: 'Combinator stage',
    wait: 'Wait',
    send: 'Send signal',
    sleep: 'Sleep',
    parallel_fork: 'Parallel fan-out',
    sequential_fork: 'Sequential fan-out',
    named_fork: 'Named fork',
    loop: 'Loop',
    route: 'Route',
    substep: 'Step',
    distribute: 'Distribute',
    sequence: 'Sequence',
    collect: 'Collect',
  };
  return labels[kind];
}
