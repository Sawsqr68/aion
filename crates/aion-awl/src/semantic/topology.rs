use crate::Span;

/// Checker-owned classification of one executable statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatementInfo {
    /// Source span of this exact call site or construct.
    pub span: Span,
    /// The statement shape selected by checker name resolution.
    pub kind: StatementKind,
    /// Short source-shaped label retained by the checker traversal.
    pub label: String,
    /// Optional secondary label such as a result binding.
    pub detail: Option<String>,
    /// Collection fan-out metadata, present only for fork statements.
    pub fork: Option<ForkInfo>,
    /// Nested statements for forks, loops, and pipe stages, in execution order.
    pub children: Vec<StatementInfo>,
}

/// Checker-retained collection or named-branch fork metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForkInfo {
    /// Collection item binding; absent for named forks.
    pub binding: Option<String>,
    /// Canonical collection expression; absent for named forks.
    pub collection: Option<String>,
    /// Whether collection items execute one at a time in input order.
    pub sequential: bool,
    /// Joined result binding, when the fork writes one.
    pub result: Option<String>,
}

/// Checker-owned statement vocabulary consumed by graph projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatementKind {
    /// A direct call to a worker action.
    ActionCall,
    /// A direct call to a child workflow.
    ChildCall,
    /// A direct call to an inline subflow.
    SubflowCall,
    /// A direct call whose target did not resolve.
    UnresolvedCall,
    /// A fire-and-forget child workflow start.
    SpawnChild,
    /// A parsed `spawn` whose target is not a child workflow.
    InvalidSpawn,
    /// A pipe containing nested stage facts.
    Pipe,
    /// A worker-action pipe stage.
    ActionStage,
    /// A child-workflow pipe stage.
    ChildStage,
    /// An unresolved pipe call stage.
    UnresolvedStage,
    /// A deterministic field-projection pipe stage.
    FieldStage,
    /// A deterministic combinator pipe stage.
    CombinatorStage,
    /// A durable signal wait.
    Wait,
    /// A durable signal send to another workflow.
    Send,
    /// A durable timer.
    Sleep,
    /// A collection fork whose branches execute in parallel.
    ParallelFork,
    /// A collection fork whose branches execute sequentially.
    SequentialFork,
    /// A heterogeneous named-branch fork.
    NamedFork,
    /// A bounded loop.
    Loop,
    /// An explicit route.
    Route,
    /// A nested `step` declaration.
    SubStep,
    /// A parallel per-item region opener.
    Distribute,
    /// An in-order per-item region opener.
    Sequence,
    /// A per-item region closer.
    Collect,
}
