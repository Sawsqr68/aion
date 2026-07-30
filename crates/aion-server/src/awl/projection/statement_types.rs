//! Additive wire vocabulary for executable statements inside projected steps.

use serde::Serialize;

use super::super::handlers::SourceSpan;

/// One ordered statement list, recursively nested by fork, loop, and pipe nodes.
#[derive(Debug, Serialize)]
pub struct ProjectionStatementGraph {
    /// Executable constructs in written order.
    pub nodes: Vec<ProjectionStatementNode>,
    /// Written-order control edges between sibling constructs.
    pub edges: Vec<ProjectionStatementEdge>,
}

/// One positional executable construct inside a step.
#[derive(Debug, Serialize)]
pub struct ProjectionStatementNode {
    /// Stable source-derived id within the owning workflow graph.
    pub id: String,
    /// Checker-owned construct classification.
    pub kind: ProjectionStatementKind,
    /// Short source-shaped label shown on the canvas.
    pub label: String,
    /// Optional secondary label such as a child signature or result binding.
    pub detail: Option<String>,
    /// Source span of this exact call site or construct.
    pub span: SourceSpan,
    /// Collection fan-out metadata, present only for fork nodes.
    pub fork: Option<ProjectionFork>,
    /// Nested branch, loop, or pipe-stage flow.
    pub graph: Option<ProjectionStatementGraph>,
}

/// One written-order edge between positional statement nodes.
#[derive(Debug, Serialize)]
pub struct ProjectionStatementEdge {
    /// Stable edge id within the statement list.
    pub id: String,
    /// Source statement id.
    pub source: String,
    /// Target statement id.
    pub target: String,
}

/// Collection or named-branch fork metadata.
#[derive(Debug, Serialize)]
pub struct ProjectionFork {
    /// Collection item binding; absent for named forks.
    pub binding: Option<String>,
    /// Canonical collection expression; absent for named forks.
    pub collection: Option<String>,
    /// Whether collection items execute one at a time in input order.
    pub sequential: bool,
    /// Joined result binding, when the fork writes one.
    pub result: Option<String>,
}

/// Checker-owned positional statement vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionStatementKind {
    /// Worker action call.
    ActionCall,
    /// Child workflow call.
    ChildCall,
    /// Inline subflow call.
    SubflowCall,
    /// Call with a checker diagnostic for its unresolved target.
    UnresolvedCall,
    /// Fire-and-forget child workflow start.
    SpawnChild,
    /// Parsed `spawn` with a checker-invalid target.
    InvalidSpawn,
    /// Pipe container whose stages are nested nodes.
    Pipe,
    /// Worker action pipe stage.
    ActionStage,
    /// Child workflow pipe stage.
    ChildStage,
    /// Unresolved pipe call stage.
    UnresolvedStage,
    /// Deterministic field pipe stage.
    FieldStage,
    /// Deterministic combinator pipe stage.
    CombinatorStage,
    /// Durable signal wait.
    Wait,
    /// Durable signal send.
    Send,
    /// Durable timer.
    Sleep,
    /// Parallel collection fan-out.
    ParallelFork,
    /// Sequential collection fan-out.
    SequentialFork,
    /// Heterogeneous named-branch fork.
    NamedFork,
    /// Bounded loop container.
    Loop,
    /// Explicit route.
    Route,
    /// Nested step at its statement position.
    Substep,
    /// Parallel per-item region opener.
    Distribute,
    /// Sequential per-item region opener.
    Sequence,
    /// Per-item region closer.
    Collect,
}
