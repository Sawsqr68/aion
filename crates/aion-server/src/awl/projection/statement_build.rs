//! Projection of checker-owned positional statement topology.

use std::collections::BTreeMap;

use aion_awl::semantic::{StatementInfo, StatementKind};
use aion_awl::{ChildDecl, TypeRef};

use super::statement_types::{
    ProjectionFork, ProjectionStatementEdge, ProjectionStatementGraph, ProjectionStatementKind,
    ProjectionStatementNode,
};

pub(super) fn statement_graph(
    statements: &[StatementInfo],
    children: &BTreeMap<&str, &ChildDecl>,
) -> ProjectionStatementGraph {
    statement_graph_with_order(statements, children, true)
}

fn statement_graph_with_order(
    statements: &[StatementInfo],
    children: &BTreeMap<&str, &ChildDecl>,
    ordered: bool,
) -> ProjectionStatementGraph {
    let nodes = statements
        .iter()
        .map(|statement| statement_node(statement, children))
        .collect::<Vec<_>>();
    let edges = if ordered {
        nodes
            .windows(2)
            .map(|pair| ProjectionStatementEdge {
                id: format!("flow:{}:{}", pair[0].id, pair[1].id),
                source: pair[0].id.clone(),
                target: pair[1].id.clone(),
            })
            .collect()
    } else {
        Vec::new()
    };
    ProjectionStatementGraph { nodes, edges }
}

fn statement_node(
    statement: &StatementInfo,
    children: &BTreeMap<&str, &ChildDecl>,
) -> ProjectionStatementNode {
    let kind = projection_kind(statement.kind);
    let detail = child_detail(statement, children).or_else(|| statement.detail.clone());
    let graph = has_nested_graph(statement.kind).then(|| {
        statement_graph_with_order(
            &statement.children,
            children,
            statement.kind != StatementKind::NamedFork,
        )
    });
    ProjectionStatementNode {
        id: format!("statement:{}:{}", statement.span.start, statement.span.end),
        kind,
        label: statement.label.clone(),
        detail,
        span: statement.span.into(),
        fork: statement.fork.as_ref().map(|fork| ProjectionFork {
            binding: fork.binding.clone(),
            collection: fork.collection.clone(),
            sequential: fork.sequential,
            result: fork.result.clone(),
        }),
        graph,
    }
}

fn child_detail(
    statement: &StatementInfo,
    children: &BTreeMap<&str, &ChildDecl>,
) -> Option<String> {
    if !matches!(
        statement.kind,
        StatementKind::ChildCall | StatementKind::SpawnChild | StatementKind::ChildStage
    ) {
        return None;
    }
    children
        .get(statement.label.as_str())
        .map(|child| match &statement.detail {
            Some(detail) => format!("{} · {detail}", child_signature(child)),
            None => child_signature(child),
        })
}

const fn has_nested_graph(kind: StatementKind) -> bool {
    matches!(
        kind,
        StatementKind::Pipe
            | StatementKind::ParallelFork
            | StatementKind::SequentialFork
            | StatementKind::NamedFork
            | StatementKind::Loop
    )
}

const fn projection_kind(kind: StatementKind) -> ProjectionStatementKind {
    match kind {
        StatementKind::ActionCall => ProjectionStatementKind::ActionCall,
        StatementKind::ChildCall => ProjectionStatementKind::ChildCall,
        StatementKind::SubflowCall => ProjectionStatementKind::SubflowCall,
        StatementKind::UnresolvedCall => ProjectionStatementKind::UnresolvedCall,
        StatementKind::SpawnChild => ProjectionStatementKind::SpawnChild,
        StatementKind::InvalidSpawn => ProjectionStatementKind::InvalidSpawn,
        StatementKind::Pipe => ProjectionStatementKind::Pipe,
        StatementKind::ActionStage => ProjectionStatementKind::ActionStage,
        StatementKind::ChildStage => ProjectionStatementKind::ChildStage,
        StatementKind::UnresolvedStage => ProjectionStatementKind::UnresolvedStage,
        StatementKind::FieldStage => ProjectionStatementKind::FieldStage,
        StatementKind::CombinatorStage => ProjectionStatementKind::CombinatorStage,
        StatementKind::Wait => ProjectionStatementKind::Wait,
        // A send gets its OWN projected kind rather than folding into a
        // neighbour. The console's statement topology is how an operator sees
        // what a step actually does, and a durable send is exactly the kind of
        // outward effect they must be able to see; collapsing it into another
        // kind would make it invisible on the canvas while it still executed.
        StatementKind::Send => ProjectionStatementKind::Send,
        // Distinct on the canvas for the same reason a send is: an answer is
        // the operator-visible surface of the run, and collapsing it into
        // another kind would hide the one statement that makes a query
        // answerable while it still executed.
        StatementKind::Answer => ProjectionStatementKind::Answer,
        StatementKind::Sleep => ProjectionStatementKind::Sleep,
        StatementKind::ParallelFork => ProjectionStatementKind::ParallelFork,
        StatementKind::SequentialFork => ProjectionStatementKind::SequentialFork,
        StatementKind::NamedFork => ProjectionStatementKind::NamedFork,
        StatementKind::Loop => ProjectionStatementKind::Loop,
        StatementKind::Route => ProjectionStatementKind::Route,
        StatementKind::SubStep => ProjectionStatementKind::Substep,
        StatementKind::Distribute => ProjectionStatementKind::Distribute,
        StatementKind::Sequence => ProjectionStatementKind::Sequence,
        StatementKind::Collect => ProjectionStatementKind::Collect,
    }
}

pub(super) fn child_signature(child: &ChildDecl) -> String {
    let parameters = child
        .params
        .iter()
        .map(|parameter| format!("{}: {}", parameter.name, type_text(&parameter.ty)))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{}({parameters}) -> {}",
        child.name,
        type_text(&child.returns)
    )
}

fn type_text(ty: &TypeRef) -> String {
    match ty {
        TypeRef::Named { name, .. } => name.clone(),
        TypeRef::List { inner, .. } => format!("[{}]", type_text(inner)),
        TypeRef::Optional { inner, .. } => format!("{}?", type_text(inner)),
    }
}
