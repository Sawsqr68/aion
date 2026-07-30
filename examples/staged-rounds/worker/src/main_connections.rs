//! The driven-agent ROLE table and its grouping into liminal CONNECTIONS.
//!
//! THE ROUTING CONSTRAINT THIS EXISTS FOR: the server routes an activity by
//! (namespace, `task_queue`, node) and by NOTHING else — never by activity
//! type. So two activities the document pins to the SAME node must be served
//! by ONE connection: two connections on one node would let the server land
//! `remediate` on the connection that only knows `fix_integration`, and the
//! activity would fail with no handler.
//!
//! `../awl/staged_rounds.awl` pins both `remediate` and `fix_integration` to
//! node `remediation` — deliberately, because both RESUME the planner's session
//! so the persistent coordinator answers for its own decomposition. The role
//! table below therefore stays one entry per ACTIVITY (it mirrors the
//! document's action table, which is what the pins are checked against) and
//! [`connections`] folds it into one entry per NODE, refusing loudly if two
//! roles sharing a node disagree about anything the connection owns.

use staged_rounds_worker::harness::{AgentActivity, ExtractFn};
use staged_rounds_worker::profiles::Profiles;
use staged_rounds_worker::prompts::{self, AssembleFn};
use staged_rounds_worker::{harness, schemas};

/// One driven-agent role: ONE activity type, the node its connection
/// registers, the output schema and profile doctrine its session runs with,
/// the prompt assembly and context extraction for its turns, its Norn tool
/// deny-list, and its connection concurrency.
///
/// One entry per activity type, in the document's action order.
pub(super) struct Role {
    pub(super) activity_type: &'static str,
    pub(super) node: &'static str,
    pub(super) output_schema: &'static str,
    pub(super) profile: String,
    pub(super) assemble: AssembleFn,
    /// Derives the per-run workspace root, session suffix, and post-run
    /// mechanical plan from each activity input (the arg template cannot
    /// express per-item — let alone per-item-per-lens — values).
    pub(super) extract: ExtractFn,
    /// The comma-separated Norn tool deny-list this role's sessions run
    /// with (`--disallowed-tools`), or `None` for no deny-list. `Some` for
    /// the planner and reviewer (read-only roles); `None` for the developer,
    /// the remediator, and the integration fixer (they must write).
    pub(super) disallowed_tools: Option<&'static str>,
    /// This role's connection `max_concurrency` — the developer and
    /// reviewer take the operator's `--max-parallel` (the genuine
    /// parallelism knob); the coordinator roles stay narrow.
    pub(super) max_concurrency: usize,
}

/// One liminal CONNECTION: everything the connection itself owns, plus every
/// activity type it serves.
pub(super) struct Connection {
    pub(super) node: &'static str,
    pub(super) output_schema: &'static str,
    pub(super) profile: String,
    pub(super) disallowed_tools: Option<&'static str>,
    pub(super) max_concurrency: usize,
    /// The activity types this one connection serves, in role-table order.
    pub(super) activities: Vec<AgentActivity>,
}

/// The FIVE driven-agent roles, in the document's action order. Each roots its
/// Norn session at the worktree its extractor reads from the activity input:
/// the planner at the repo, the developer and reviewer at the item's own
/// worktree, and the remediator and integration fixer at the integration
/// worktree.
///
/// `review_item` and `fix_integration` reuse the reviewer and remediator
/// profiles and schemas respectively — see [`connections`] for why that is a
/// requirement, not a coincidence, in the `fix_integration` case.
pub(super) fn roles(profiles: Profiles, max_parallel: usize, coordinator: usize) -> Vec<Role> {
    vec![
        Role {
            activity_type: "planner",
            node: "planner",
            output_schema: schemas::PLAN,
            profile: profiles.planner,
            assemble: prompts::planner,
            extract: harness::planner_context,
            disallowed_tools: Some(super::READ_ONLY_DISALLOWED_TOOLS),
            max_concurrency: coordinator,
        },
        Role {
            activity_type: "dev_item",
            node: "developer",
            output_schema: schemas::ITEM_REPORT,
            profile: profiles.developer,
            assemble: prompts::dev_item,
            extract: harness::dev_item_context,
            disallowed_tools: None,
            max_concurrency: max_parallel,
        },
        Role {
            activity_type: "review_item",
            node: "reviewer",
            output_schema: schemas::ITEM_VERDICT,
            profile: profiles.reviewer,
            assemble: prompts::review_item,
            extract: harness::review_item_context,
            disallowed_tools: Some(super::READ_ONLY_DISALLOWED_TOOLS),
            max_concurrency: max_parallel,
        },
        Role {
            activity_type: "remediate",
            node: "remediation",
            output_schema: schemas::REMEDIATION,
            profile: profiles.remediator.clone(),
            assemble: prompts::remediate,
            extract: harness::remediate_context,
            disallowed_tools: None,
            max_concurrency: coordinator,
        },
        Role {
            activity_type: "fix_integration",
            node: "remediation",
            output_schema: schemas::REMEDIATION,
            profile: profiles.remediator,
            assemble: prompts::fix_integration,
            extract: harness::fix_integration_context,
            disallowed_tools: None,
            max_concurrency: coordinator,
        },
    ]
}

/// Fold the role table into one [`Connection`] per node, in first-appearance
/// order.
///
/// A node's connection owns exactly one composed Norn command — one
/// `--output-schema`, one `--append-system-prompt` profile, one
/// `--disallowed-tools` value — and one `max_concurrency`. Those are per-
/// CONNECTION facts, so two roles sharing a node MUST agree on all four; if
/// they ever disagree, one role's session would silently run with the other's
/// doctrine or schema. That is refused here, by name, at startup.
///
/// # Errors
///
/// A message naming the node and the disagreeing setting when two roles on one
/// node do not agree, or when the table is empty.
pub(super) fn connections(roles: Vec<Role>) -> Result<Vec<Connection>, String> {
    if roles.is_empty() {
        return Err("the agent role table is empty — the worker would serve no \
                    agent activity at all"
            .to_owned());
    }
    let mut connections: Vec<Connection> = Vec::new();
    for role in roles {
        let activity = AgentActivity {
            activity_type: role.activity_type,
            assemble: role.assemble,
            extract: role.extract,
        };
        if let Some(existing) = connections
            .iter_mut()
            .find(|connection| connection.node == role.node)
        {
            require_agreement(existing, &role)?;
            existing.activities.push(activity);
            continue;
        }
        connections.push(Connection {
            node: role.node,
            output_schema: role.output_schema,
            profile: role.profile,
            disallowed_tools: role.disallowed_tools,
            max_concurrency: role.max_concurrency,
            activities: vec![activity],
        });
    }
    Ok(connections)
}

/// Require a role joining an existing node's connection to agree with it on
/// every per-connection setting.
fn require_agreement(existing: &Connection, joining: &Role) -> Result<(), String> {
    let node = existing.node;
    let first = existing
        .activities
        .first()
        .map_or("(none)", |activity| activity.activity_type);
    let mismatch = |setting: &str| {
        format!(
            "activities `{first}` and `{}` are both pinned to node {node:?}, so they \
             share ONE connection — but they disagree on {setting}, which the \
             connection owns. One of them would silently run with the other's \
             {setting}.",
            joining.activity_type
        )
    };
    if existing.output_schema != joining.output_schema {
        return Err(mismatch("--output-schema"));
    }
    if existing.profile != joining.profile {
        return Err(mismatch("the profile doctrine (--append-system-prompt)"));
    }
    if existing.disallowed_tools != joining.disallowed_tools {
        return Err(mismatch("--disallowed-tools"));
    }
    if existing.max_concurrency != joining.max_concurrency {
        return Err(mismatch("max_concurrency"));
    }
    Ok(())
}
