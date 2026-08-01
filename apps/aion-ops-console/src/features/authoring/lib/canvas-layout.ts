import dagre from 'dagre';

import type {
  GraphProjection,
  ProjectionEdge,
  ProjectionStatementGraph,
  ProjectionStep,
  ProjectionSubstepGraph,
} from './projection-types';

/** Base box of one embedded graph node. */
export const EMBED_NODE_WIDTH = 200;
export const EMBED_NODE_HEIGHT = 88;
/** Square box of a pure-decision diamond node. */
export const DECISION_NODE_SIZE = 160;
/** Padding around an embedded graph inside its node. */
export const EMBED_PADDING = 16;
/** Height added by each collapsed scoped-substep section. */
export const EMBEDDED_SECTION_HEADER_HEIGHT = 36;
/** Lateral distance between parallel edge siblings. */
export const PARALLEL_EDGE_GAP = 24;
/** Clear space between a node boundary and a back/self-loop edge gutter. */
export const EDGE_GUTTER_CLEARANCE = 32;
/** Font size shared by embedded edge labels and their width estimate. */
export const EMBED_EDGE_LABEL_FONT_SIZE = 10;

export type NodeSize = { width: number; height: number };
export type PlacedNode = { x: number; y: number; width: number; height: number };
export type EmbeddedLayout = {
  width: number;
  height: number;
  nodeLeft: number;
  nodeRight: number;
  nodes: Record<string, PlacedNode>;
};

/**
 * The drawn size of one step node, growing when the node is an expanded
 * subflow or substep owner (its graph renders in place) and squaring when it
 * is a pure decision diamond. `path` is the node's expansion path — the
 * `/`-joined chain of step names from the canvas root.
 */
export function nodeSize(
  step: ProjectionStep,
  path: string,
  expanded: ReadonlySet<string>,
  base: NodeSize
): NodeSize {
  if (step.kind === 'decision') {
    return { width: DECISION_NODE_SIZE, height: DECISION_NODE_SIZE };
  }
  let width = base.width;
  let height =
    base.height +
    step.substeps.length * EMBEDDED_SECTION_HEADER_HEIGHT +
    statementGraphHeight(step.body) +
    (step.failure === null ? 0 : statementGraphHeight(step.failure));
  const subflow = step.subflow?.graph;
  if (subflow !== null && subflow !== undefined && expanded.has(path)) {
    const inner = layoutEmbedded(subflow, expanded, path);
    width = Math.max(width, inner.width + EMBED_PADDING * 2);
    height += inner.height + EMBED_PADDING;
  }
  for (const scoped of step.substeps) {
    const scopedPath = substepGraphPath(path, scoped);
    if (!expanded.has(scopedPath)) continue;
    const inner = layoutEmbedded(scoped.graph, expanded, scopedPath);
    width = Math.max(width, inner.width + EMBED_PADDING * 2);
    height += inner.height + EMBED_PADDING;
  }
  return { width, height };
}

/** Conservative vertical extent of an always-visible positional statement graph. */
export function statementGraphHeight(graph: ProjectionStatementGraph): number {
  if (graph.nodes.length === 0) return 0;
  const sectionChrome = 44;
  const connectorHeight = 20;
  const nodesHeight = graph.nodes.reduce((height, node) => {
    const nodeChrome = node.detail === null && node.fork === null ? 38 : 54;
    const nested = node.graph === null ? 0 : statementGraphHeight(node.graph) + 28;
    return height + nodeChrome + nested;
  }, 0);
  return sectionChrome + nodesHeight + (graph.nodes.length - 1) * connectorHeight;
}

/** Expansion key for one statement-list-scoped substep graph. */
export function substepGraphPath(path: string, scoped: ProjectionSubstepGraph): string {
  return `${path}/@${scoped.scope}-${scoped.index}`;
}

/** Human-readable label for one statement-list-scoped substep graph. */
export function substepScopeLabel(scoped: ProjectionSubstepGraph): string {
  if (scoped.scope === 'body') return 'Body substeps';
  if (scoped.scope === 'failure') return 'Failure substeps';
  const occurrence = scoped.index + 1;
  return `${scoped.scope === 'fork' ? 'Fork' : 'Loop'} ${occurrence} substeps`;
}

/** Dagre's placement result read back in local coordinates, before any gutter shift. */
type PlacedGraph = {
  nodes: Record<string, PlacedNode>;
  /** `+Infinity` when the graph has no steps — callers must treat it as 0. */
  minX: number;
  maxY: number;
  rawWidth: number;
};

/** Space reserved outside the node box so routed edges never cross a node. */
type EdgeGutters = { left: number; right: number; vertical: number };

/** Run dagre top-to-bottom and convert its centre points to top-left node boxes. */
function placeGraphNodes(
  graph: GraphProjection,
  expanded: ReadonlySet<string>,
  pathPrefix: string
): PlacedGraph {
  const dagreGraph = new dagre.graphlib.Graph()
    .setDefaultEdgeLabel(() => ({}))
    .setGraph({ rankdir: 'TB', nodesep: 32, ranksep: 48 });
  const sizes = new Map<string, NodeSize>();
  for (const step of graph.steps) {
    const size = nodeSize(step, `${pathPrefix}/${step.name}`, expanded, {
      width: EMBED_NODE_WIDTH,
      height: EMBED_NODE_HEIGHT,
    });
    sizes.set(step.name, size);
    dagreGraph.setNode(step.name, size);
  }
  for (const edge of graph.edges) dagreGraph.setEdge(edge.source, edge.target);
  dagre.layout(dagreGraph);

  const nodes: Record<string, PlacedNode> = {};
  let minX = Number.POSITIVE_INFINITY;
  let maxX = Number.NEGATIVE_INFINITY;
  let maxY = 0;
  for (const step of graph.steps) {
    const placed = dagreGraph.node(step.name);
    const size = sizes.get(step.name) ?? { width: EMBED_NODE_WIDTH, height: EMBED_NODE_HEIGHT };
    const node = {
      x: placed.x - size.width / 2,
      y: placed.y - size.height / 2,
      width: size.width,
      height: size.height,
    };
    nodes[step.name] = node;
    minX = Math.min(minX, node.x);
    maxX = Math.max(maxX, node.x + node.width);
    maxY = Math.max(maxY, node.y + node.height);
  }
  return { nodes, minX, maxY, rawWidth: Number.isFinite(minX) ? maxX - minX : 0 };
}

/**
 * Back edges route down the left gutter and self-loops down the right. Each must
 * clear the node boundary plus its own parallel-sibling lane and its label width;
 * a graph with neither kind of edge reserves nothing.
 */
function edgeGutters(edges: readonly ProjectionEdge[]): EdgeGutters {
  const siblingOffsets = parallelEdgeOffsets(edges);
  let left = 0;
  let right = 0;
  let needsVertical = false;
  for (const edge of edges) {
    const selfLoop = edge.source === edge.target;
    if (!(edge.back || selfLoop)) continue;
    needsVertical = true;
    const pathDistance = EDGE_GUTTER_CLEARANCE + edgeLaneSpread(siblingOffsets.get(edge.id) ?? 0);
    const required = pathDistance + embeddedEdgeLabelWidth(edgeLabel(edge));
    if (selfLoop) {
      right = Math.max(right, required);
    } else {
      left = Math.max(left, required);
    }
  }
  return { left, right, vertical: needsVertical ? EDGE_GUTTER_CLEARANCE : 0 };
}

/**
 * Lays out one embedded graph with dagre (top-to-bottom), returning top-left
 * positions in local coordinates plus the drawn extent. Nested expanded
 * graphs grow their owner nodes recursively.
 */
export function layoutEmbedded(
  graph: GraphProjection,
  expanded: ReadonlySet<string>,
  pathPrefix: string
): EmbeddedLayout {
  const { nodes, minX, maxY, rawWidth } = placeGraphNodes(graph, expanded, pathPrefix);
  const gutters = edgeGutters(graph.edges);

  const shiftX = gutters.left - (Number.isFinite(minX) ? minX : 0);
  for (const node of Object.values(nodes)) {
    node.x += shiftX;
    node.y += gutters.vertical;
  }
  const nodeRight = gutters.left + rawWidth;
  return {
    width: Math.ceil(nodeRight + gutters.right),
    height: Math.ceil(maxY + gutters.vertical * 2),
    nodeLeft: gutters.left,
    nodeRight,
    nodes,
  };
}

/** The label a control edge is drawn with — back edges carry `×N`. */
export function edgeLabel(edge: {
  label: string | null;
  back: boolean;
  visits: string | null;
}): string | null {
  if (edge.back) {
    const bound = edge.visits === null ? null : `×${edge.visits}`;
    if (edge.label === null) return bound;
    return bound === null ? edge.label : `${edge.label} ${bound}`;
  }
  return edge.label;
}

/** Conservative rendered-width bound for one semibold embedded edge label. */
export function embeddedEdgeLabelWidth(label: string | null): number {
  return label === null ? 0 : Array.from(label).length * EMBED_EDGE_LABEL_FONT_SIZE;
}

/**
 * Stable lateral offsets for edges sharing an endpoint pair. Sorting by wire
 * id makes the assignment independent of response ordering.
 */
export function parallelEdgeOffsets(
  edges: readonly { id: string; source: string; target: string }[]
): ReadonlyMap<string, number> {
  const groups = new Map<string, { id: string }[]>();
  for (const edge of edges) {
    const key = `${edge.source}\u0000${edge.target}`;
    const group = groups.get(key) ?? [];
    group.push(edge);
    groups.set(key, group);
  }
  const offsets = new Map<string, number>();
  for (const group of groups.values()) {
    group.sort((left, right) => left.id.localeCompare(right.id));
    for (const [index, edge] of group.entries()) {
      offsets.set(edge.id, (index - (group.length - 1) / 2) * PARALLEL_EDGE_GAP);
    }
  }
  return offsets;
}

/** Extra gutter distance that keeps centered sibling offsets on unique lanes. */
export function edgeLaneSpread(siblingOffset: number): number {
  return Math.abs(siblingOffset) * 2 + (siblingOffset < 0 ? PARALLEL_EDGE_GAP / 2 : 0);
}

/**
 * Route a bottom handle to a top handle without crossing either endpoint node:
 * leave below the source, traverse a lateral lane, then enter from above.
 */
export function lateralEdgePath(
  from: { x: number; y: number },
  to: { x: number; y: number },
  laneX: number
): string {
  const sourceOuterY = from.y + EDGE_GUTTER_CLEARANCE;
  const targetOuterY = to.y - EDGE_GUTTER_CLEARANCE;
  return `M ${from.x} ${from.y} C ${from.x} ${sourceOuterY}, ${laneX} ${sourceOuterY}, ${laneX} ${sourceOuterY} L ${laneX} ${targetOuterY} C ${laneX} ${targetOuterY}, ${to.x} ${targetOuterY}, ${to.x} ${to.y}`;
}
