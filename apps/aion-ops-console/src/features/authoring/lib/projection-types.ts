export type SourceSpan = {
  start: number;
  end: number;
  line: number;
  column: number;
};

export type SemanticDeclaration = {
  name: string;
  kind:
    | 'workflow'
    | 'input'
    | 'signal'
    | 'outcome'
    | 'type'
    | 'field'
    | 'variant'
    | 'worker'
    | 'action'
    | 'const'
    | 'subflow'
    | 'child'
    | 'parameter'
    | 'step'
    | 'binding';
  documentation: string | null;
  span: SourceSpan;
};

export type SemanticEntry = {
  span: SourceSpan;
  type: string | null;
  declaration: SemanticDeclaration | null;
};

export type ProjectionStepKind =
  | 'plain'
  | 'distribute'
  | 'sequence'
  | 'collect'
  | 'subflow_call'
  | 'decision';
export type ProjectionDistribution = { binding: string; collection: string };
export type ProjectionCollect = { binding: string; tolerant: boolean; result: string };
export type ProjectionSubflow = { name: string; graph: GraphProjection | null };
export type ProjectionSubstepScope = 'body' | 'failure' | 'fork' | 'loop';
export type ProjectionSubstepGraph = {
  scope: ProjectionSubstepScope;
  index: number;
  graph: GraphProjection;
};
export type ProjectionStatementKind =
  | 'action_call'
  | 'child_call'
  | 'subflow_call'
  | 'unresolved_call'
  | 'spawn_child'
  | 'invalid_spawn'
  | 'pipe'
  | 'action_stage'
  | 'child_stage'
  | 'unresolved_stage'
  | 'field_stage'
  | 'combinator_stage'
  | 'wait'
  | 'sleep'
  | 'parallel_fork'
  | 'sequential_fork'
  | 'named_fork'
  | 'loop'
  | 'route'
  | 'substep'
  | 'distribute'
  | 'sequence'
  | 'collect';
export type ProjectionFork = {
  binding: string | null;
  collection: string | null;
  sequential: boolean;
  result: string | null;
};
export type ProjectionStatementNode = {
  id: string;
  kind: ProjectionStatementKind;
  label: string;
  detail: string | null;
  span: SourceSpan;
  fork: ProjectionFork | null;
  graph: ProjectionStatementGraph | null;
};
export type ProjectionStatementEdge = { id: string; source: string; target: string };
export type ProjectionStatementGraph = {
  nodes: ProjectionStatementNode[];
  edges: ProjectionStatementEdge[];
};
export type ProjectionStep = {
  name: string;
  documentation: string;
  span: SourceSpan;
  kind: ProjectionStepKind;
  region: string | null;
  distribution: ProjectionDistribution | null;
  collect: ProjectionCollect | null;
  subflow: ProjectionSubflow | null;
  substeps: ProjectionSubstepGraph[];
  visits: string | null;
  decision: boolean;
  waits: boolean;
  activities: string[];
  body: ProjectionStatementGraph;
  failure: ProjectionStatementGraph | null;
};
export type ProjectionEdge = {
  id: string;
  source: string;
  target: string;
  kind: 'route' | 'fall_through' | 'after';
  label: string | null;
  back: boolean;
  visits: string | null;
};
export type ProjectionChildCall = {
  id: string;
  parentStep: string;
  name: string;
  signature: string;
  span: SourceSpan;
};
export type GraphProjection = {
  steps: ProjectionStep[];
  edges: ProjectionEdge[];
  childCalls: ProjectionChildCall[];
};
export type StudioField = { name: string; type: string };
export type StudioType = {
  name: string;
  kind: 'record' | 'enum' | 'schema';
  fields: StudioField[];
  variants: string[];
};
export type StudioAction = {
  name: string;
  params: StudioField[];
  returnType: string;
};
export type StudioWorker = { name: string; actions: StudioAction[] };
export type StudioProjection = {
  builtins: string[];
  types: StudioType[];
  workers: StudioWorker[];
};
export type SemanticIndex = {
  entries: SemanticEntry[];
  graph: GraphProjection;
  studio: StudioProjection;
};

export type LayoutPosition = { x: number; y: number };
export type LayoutRecord = { positions: Record<string, LayoutPosition> };
