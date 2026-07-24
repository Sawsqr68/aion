use std::collections::BTreeSet;

use aion_awl::semantic;

use super::super::build;
use super::super::statement_types::{
    ProjectionStatementGraph, ProjectionStatementKind, ProjectionStatementNode,
};
use super::super::types::{GraphProjection, ProjectionEdgeKind, ProjectionStep};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const REMEDIATION_PACKET: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/remediation-packet/awl/remediation_packet.awl"
));
const REPEATED_CHILD_CALLS: &str = include_str!("../fixtures/repeated_child_calls.awl");
const NAMED_FORK: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../aion-awl/tests/fixtures/rev2/dag-fork/valid/fork_named_branches.awl"
));

#[test]
fn remediation_packet_projects_forks_as_scoped_positional_topology() -> TestResult {
    let graph = project(REMEDIATION_PACKET)?;
    let select_targets = graph
        .edges
        .iter()
        .filter(|edge| edge.source == "select" && matches!(edge.kind, ProjectionEdgeKind::Route))
        .map(|edge| edge.target.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(select_targets, BTreeSet::from(["build_wave", "close"]));

    let build_wave = step(&graph, "build_wave")?;
    let build_kinds = build_wave
        .body
        .nodes
        .iter()
        .map(|node| node.kind)
        .collect::<Vec<_>>();
    assert_eq!(
        build_kinds,
        vec![
            ProjectionStatementKind::ParallelFork,
            ProjectionStatementKind::ActionCall,
            ProjectionStatementKind::ParallelFork,
        ]
    );
    assert_linear(&build_wave.body);

    let lane_fork = &build_wave.body.nodes[0];
    assert_fork(lane_fork, "entry", "wave.entries", false, "lane_results")?;
    let lane_branch = nested(lane_fork)?;
    assert_eq!(lane_branch.nodes.len(), 1);
    assert_eq!(
        lane_branch.nodes[0].kind,
        ProjectionStatementKind::ChildCall
    );
    assert_eq!(lane_branch.nodes[0].label, "lane_run");

    let watcher_fork = &build_wave.body.nodes[2];
    assert_fork(
        watcher_fork,
        "watcher",
        "config.meridian_watchers",
        false,
        "wave_heartbeats",
    )?;
    let watcher_branch = nested(watcher_fork)?;
    assert_eq!(watcher_branch.nodes.len(), 1);
    assert_eq!(
        watcher_branch.nodes[0].kind,
        ProjectionStatementKind::ActionCall
    );
    assert_eq!(watcher_branch.nodes[0].label, "run_command");

    let hold = step(&graph, "hold_ruling")?;
    let wait_position = hold
        .body
        .nodes
        .iter()
        .position(|node| node.kind == ProjectionStatementKind::Wait)
        .ok_or("missing positional wait node")?;
    assert_eq!(
        wait_position, 1,
        "wait remains between the first fork and action call"
    );
    assert_eq!(hold.body.nodes[wait_position].label, "wait ruling");
    assert_linear(&hold.body);

    let run_command_sites = graph
        .steps
        .iter()
        .flat_map(|step| matching_nodes(&step.body, ProjectionStatementKind::ActionCall))
        .filter(|node| node.label == "run_command")
        .collect::<Vec<_>>();
    assert_eq!(run_command_sites.len(), 5);
    let call_ids = run_command_sites
        .iter()
        .map(|node| node.id.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        call_ids.len(),
        5,
        "each action call site has a distinct node id"
    );

    let step_names = graph
        .steps
        .iter()
        .map(|step| step.name.as_str())
        .collect::<BTreeSet<_>>();
    assert!(
        graph
            .edges
            .iter()
            .all(|edge| step_names.contains(edge.source.as_str())
                && step_names.contains(edge.target.as_str())),
        "select outcomes remain step-control edges, not fork branch edges"
    );
    Ok(())
}

#[test]
fn sequential_fork_keeps_repeated_child_call_sites_distinct_and_wired() -> TestResult {
    let graph = project(REPEATED_CHILD_CALLS)?;
    let run_all = step(&graph, "run_all")?;
    assert_eq!(run_all.body.nodes.len(), 1);
    let fork = &run_all.body.nodes[0];
    assert_eq!(fork.kind, ProjectionStatementKind::SequentialFork);
    assert_fork(fork, "value", "values", true, "rows")?;
    let branch = nested(fork)?;
    assert_eq!(branch.nodes.len(), 2);
    assert!(
        branch
            .nodes
            .iter()
            .all(|node| node.kind == ProjectionStatementKind::ChildCall && node.label == "run_one")
    );
    assert_ne!(branch.nodes[0].id, branch.nodes[1].id);
    assert_linear(branch);
    assert_eq!(
        graph.child_calls.len(),
        2,
        "compatible summary keeps both sites"
    );
    Ok(())
}

#[test]
fn named_fork_members_remain_parallel_branch_roots() -> TestResult {
    let graph = project(NAMED_FORK)?;
    let gather = step(&graph, "gather")?;
    assert_eq!(gather.body.nodes.len(), 1);
    let fork = &gather.body.nodes[0];
    assert_eq!(fork.kind, ProjectionStatementKind::NamedFork);
    let branches = nested(fork)?;
    assert_eq!(
        branches
            .nodes
            .iter()
            .map(|node| node.label.as_str())
            .collect::<Vec<_>>(),
        vec!["fetch_profile", "fetch_history"]
    );
    assert!(
        branches.edges.is_empty(),
        "named branches do not fall through"
    );
    Ok(())
}

fn project(source: &str) -> Result<GraphProjection, Box<dyn std::error::Error>> {
    let document = aion_awl::parse(source)?;
    let analysis = semantic::analyze(&document);
    if !analysis.diagnostics().is_empty() {
        return Err(format!("fixture must check clean: {:?}", analysis.diagnostics()).into());
    }
    Ok(build::build(&document, analysis.step_kinds()))
}

fn step<'a>(graph: &'a GraphProjection, name: &str) -> Result<&'a ProjectionStep, String> {
    graph
        .steps
        .iter()
        .find(|step| step.name == name)
        .ok_or_else(|| format!("missing step `{name}`"))
}

fn nested(node: &ProjectionStatementNode) -> Result<&ProjectionStatementGraph, String> {
    node.graph
        .as_ref()
        .ok_or_else(|| format!("missing nested graph for `{}`", node.label))
}

fn assert_fork(
    node: &ProjectionStatementNode,
    binding: &str,
    collection: &str,
    sequential: bool,
    result: &str,
) -> TestResult {
    let fork = node.fork.as_ref().ok_or("missing fork metadata")?;
    assert_eq!(fork.binding.as_deref(), Some(binding));
    assert_eq!(fork.collection.as_deref(), Some(collection));
    assert_eq!(fork.sequential, sequential);
    assert_eq!(fork.result.as_deref(), Some(result));
    Ok(())
}

fn assert_linear(graph: &ProjectionStatementGraph) {
    assert_eq!(graph.edges.len(), graph.nodes.len().saturating_sub(1));
    for (edge, pair) in graph.edges.iter().zip(graph.nodes.windows(2)) {
        assert_eq!(edge.source, pair[0].id);
        assert_eq!(edge.target, pair[1].id);
    }
}

fn matching_nodes(
    graph: &ProjectionStatementGraph,
    kind: ProjectionStatementKind,
) -> Vec<&ProjectionStatementNode> {
    let mut found = Vec::new();
    for node in &graph.nodes {
        if node.kind == kind {
            found.push(node);
        }
        if let Some(nested) = &node.graph {
            found.extend(matching_nodes(nested, kind));
        }
    }
    found
}
