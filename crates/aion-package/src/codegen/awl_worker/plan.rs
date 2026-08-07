//! The CONNECTION PLAN: what a worker process owes a queue, per node.
//!
//! The server routes an activity by (namespace × `task_queue` × node) and by
//! nothing else — never by activity type. Two consequences decide the whole
//! shape of a generated worker, and getting either wrong costs a first flight:
//!
//! * A process serving actions pinned to several nodes must open ONE
//!   CONNECTION PER NODE. Two connections on one node would let the server
//!   land an activity on the connection that holds no handler for it.
//! * Every connection owes the actions a dispatch could REACH it with, which
//!   is exactly [`aion_package::compatibility`]'s admission rule: an action
//!   pinned to a node reaches only that node's connection, while an UNPINNED
//!   action reaches every worker in the pool and is therefore owed by all of
//!   them.
//!
//! An action carrying a declarative `body` is executed by the SERVER itself,
//! so no worker serves it and none may advertise it. The filter here is the
//! same [`ActionContract::worker_owed`] the admission gate applies before
//! deciding what a connection owes; a generator that filtered differently
//! would emit a worker the gate refuses (advertising too much) or one that
//! parks a dispatch forever (advertising too little).

use crate::contract::{ActionContract, WorkerContract};

use super::error::AwlScaffoldError;

/// One connection the generated worker opens: the node it registers on and
/// every action it serves there, in the document's declaration order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connection {
    /// The node this connection advertises at registration, or `None` when
    /// the queue pins no action to any node — a connection carrying no
    /// locality is reachable by unpinned dispatches alone, which is then the
    /// whole of the queue.
    pub node: Option<String>,
    /// The actions served on this connection, in declaration order.
    pub actions: Vec<String>,
}

/// The whole plan for one queue: its connections and the servable actions
/// behind them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionPlan {
    /// The queue (the document's `worker` block name) every connection joins.
    pub task_queue: String,
    /// One entry per node, in first-declaration order.
    pub connections: Vec<Connection>,
    /// Every action needing an out-of-band worker, in declaration order —
    /// the union of the connections' action lists, and exactly the set of
    /// handler stubs the author owes.
    pub servable: Vec<ServableAction>,
}

/// One action the generated worker must serve: its name, its node pin, and
/// the wire schemas the document declares for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServableAction {
    /// Activity name, which is also the handler function's name.
    pub name: String,
    /// The node the document pins the action to, `None` when unpinned.
    pub node: Option<String>,
    /// The declared input schema, verbatim from the compiled contract.
    pub input_schema: serde_json::Value,
    /// The declared output schema, verbatim from the compiled contract.
    pub output_schema: serde_json::Value,
}

/// Derives the connection plan for one compiled worker contract.
///
/// # Errors
///
/// Returns [`AwlScaffoldError::NoServableAction`] when every declared action
/// carries a body (the server runs them all, so there is no worker to write),
/// or [`AwlScaffoldError::ActionNameNotAnIdentifier`] /
/// [`AwlScaffoldError::ActionNameUnnameable`] when an action name cannot name
/// a Rust handler function.
pub fn plan(contract: &WorkerContract) -> Result<ConnectionPlan, AwlScaffoldError> {
    let servable = servable_actions(contract)?;
    let plan = ConnectionPlan {
        task_queue: contract.task_queue.clone(),
        connections: connections(&servable),
        servable,
    };
    Ok(plan)
}

/// The actions a worker must serve: every declared action WITHOUT a body, in
/// declaration order, each validated as nameable by a Rust function.
fn servable_actions(contract: &WorkerContract) -> Result<Vec<ServableAction>, AwlScaffoldError> {
    let servable = contract
        .actions
        .iter()
        .filter(|action| action.worker_owed())
        .map(|action| servable_action(&contract.task_queue, action))
        .collect::<Result<Vec<_>, _>>()?;
    if servable.is_empty() {
        return Err(AwlScaffoldError::NoServableAction {
            task_queue: contract.task_queue.clone(),
        });
    }
    Ok(servable)
}

/// Validates one bodyless action and takes its declared surface.
fn servable_action(
    task_queue: &str,
    action: &ActionContract,
) -> Result<ServableAction, AwlScaffoldError> {
    validate_action_name(task_queue, &action.name)?;
    Ok(ServableAction {
        name: action.name.clone(),
        node: action.node.clone(),
        input_schema: action.input_schema.clone(),
        output_schema: action.output_schema.clone(),
    })
}

/// Refuses an action name that cannot name a Rust handler function.
///
/// AWL action names are `snake_case` identifiers, so this only ever fires on a
/// contract built by some other route — but the generated crate must COMPILE,
/// and a name that cannot be a function name would emit a crate that does not.
/// `self`, `crate`, and `super` are singled out because they are the three
/// keywords with no raw-identifier form.
fn validate_action_name(task_queue: &str, name: &str) -> Result<(), AwlScaffoldError> {
    if matches!(name, "self" | "crate" | "super" | "Self") {
        return Err(AwlScaffoldError::ActionNameUnnameable {
            task_queue: task_queue.to_owned(),
            action: name.to_owned(),
        });
    }
    let mut characters = name.chars();
    let starts = characters
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_');
    let continues = characters.all(|rest| rest.is_ascii_alphanumeric() || rest == '_');
    if starts && continues {
        return Ok(());
    }
    Err(AwlScaffoldError::ActionNameNotAnIdentifier {
        task_queue: task_queue.to_owned(),
        action: name.to_owned(),
    })
}

/// Folds the servable actions into one connection per NODE, in first-
/// declaration order, each carrying the actions a dispatch can reach it with.
///
/// A queue pinning nothing yields ONE connection carrying no locality: it is
/// reachable by every unpinned dispatch, which is then the whole queue. A
/// queue with pins yields no such extra connection — every node connection
/// already carries the unpinned actions, so a node-less one would add a
/// registration nothing needs.
fn connections(servable: &[ServableAction]) -> Vec<Connection> {
    let mut nodes: Vec<&str> = Vec::new();
    for action in servable {
        if let Some(node) = action.node.as_deref()
            && !nodes.contains(&node)
        {
            nodes.push(node);
        }
    }
    if nodes.is_empty() {
        return vec![Connection {
            node: None,
            actions: servable.iter().map(|action| action.name.clone()).collect(),
        }];
    }
    nodes
        .into_iter()
        .map(|node| Connection {
            node: Some(node.to_owned()),
            actions: servable
                .iter()
                .filter(|action| dispatch_can_reach(action.node.as_deref(), Some(node)))
                .map(|action| action.name.clone())
                .collect(),
        })
        .collect()
}

/// Whether a dispatch for an action pinned to `action_node` can REACH a
/// connection advertising `worker_node`.
///
/// This mirrors the admission gate's own rule
/// ([`crate::compatibility`]'s `dispatch_can_reach`) deliberately: the set of
/// actions a connection is GENERATED to serve must be exactly the set the
/// server DEMANDS of it. Serving fewer is a refused dial
/// (`WORKER_CONTRACT_MISMATCH`); serving more is a handler no dispatch can
/// ever reach.
fn dispatch_can_reach(action_node: Option<&str>, worker_node: Option<&str>) -> bool {
    match action_node {
        None => true,
        Some(pin) => worker_node == Some(pin),
    }
}
