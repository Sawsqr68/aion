//! The per-role activity context extractors: what the harness wrapper derives
//! from ONE activity input BEFORE the run — the worktree the session is rooted
//! at, the per-run session suffix, and the mechanical git step owed after a
//! successful turn.
//!
//! Resolved before the run, always loudly: an input that cannot name its
//! workspace must fail BEFORE an expensive agent turn, never after, and never
//! with a fallback that would unconfine the session or lose a mechanical
//! commit.
//!
//! SESSION TOPOLOGY, in one place:
//!
//! - `planner` → suffix `planner`, rooted at the repo (read-only).
//! - `dev_item` → suffix `dev-<item id>`, rooted at the item worktree, owes
//!   the dev-work commit. Resumed across that item's feedback rounds.
//! - `review_item` → suffix `review-<item id>-<lens name>`, rooted at the item
//!   worktree, owes the verdict stamp. The LENS is part of the suffix because
//!   the panel runs every lens over one item CONCURRENTLY: a suffix naming
//!   only the item would collide them onto a single Norn session, and one lens
//!   would resume — and read — another lens's transcript, destroying exactly
//!   the blindness the panel depends on.
//! - `remediate` → suffix `planner`, rooted at the integration worktree, owes
//!   the merge conclusion. THE mechanic: the remediator RESUMES the planner's
//!   session, so the persistent coordinator judges its own plan's conflicts.
//! - `fix_integration` → suffix `planner` as well, rooted at the integration
//!   worktree, owes the integration-fix commit. Same reason: the coordinator
//!   answers for its own decomposition when the union of individually-green
//!   branches is red.

use serde::Deserialize;

use crate::lens::{require_lens_name, review_session_suffix};
use crate::types::ProvisionedItem;

/// Which mechanical git step (or payload rewrite) a role's session performs
/// after a successful turn, with the context it resolved from the activity
/// input BEFORE the run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PostRunPlan {
    /// Commit the dev round's work in the item worktree and assemble the
    /// `DevItemResult` payload around the agent's report (developer role).
    DevWork {
        /// The provisioned item coordinates from the activity input
        /// (boxed: the item rides its whole `WorkItem`, which dwarfs the
        /// other variants).
        work: Box<ProvisionedItem>,
    },
    /// Stamp the verdict's `item_id` and `lens` from the dispatched panel seat
    /// (reviewer role). The reviewer writes nothing, so there is no commit —
    /// but its IDENTITY is still the machinery's to assert, not the agent's.
    StampVerdict {
        /// The item the seat reviewed.
        item_id: String,
        /// The lens the seat reviewed through.
        lens: String,
    },
    /// Conclude the in-progress merge in the integration worktree
    /// (remediator role).
    ConcludeMerge {
        /// The integration worktree from the activity input.
        workspace_path: String,
    },
    /// Commit the repair made to the integration worktree after a red
    /// integration battery (`fix_integration` role).
    IntegrationFix {
        /// The integration worktree from the activity input.
        workspace_path: String,
    },
}

/// What a role's extractor derives from one activity input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoleContext {
    /// The worktree the session is rooted at (`--workspace-root`).
    pub workspace_root: String,
    /// The per-run session suffix (`--session-id <workflow_id>-<suffix>`).
    pub session_suffix: String,
    /// The mechanical step after a successful turn, when the role has one.
    pub plan: Option<PostRunPlan>,
}

/// The signature every role's context extractor shares.
pub type ExtractFn = fn(&str) -> Result<RoleContext, String>;

/// Planner / remediator / integration-fixer input slice: `workspace_path` at
/// the top level (the repo root, or the integration worktree).
#[derive(Debug, Deserialize)]
struct TopLevelWorkspace {
    #[serde(default)]
    workspace_path: String,
}

/// Dev-item input slice: the provisioned item under `work`.
#[derive(Debug, Deserialize)]
struct DevItemInput {
    work: ProvisionedItem,
}

/// Review-item input slice: the panel seat under `task` — its lens, and the
/// gated item's id and worktree (every other `GatedItem` field is ignored).
#[derive(Debug, Deserialize)]
struct ReviewItemInput {
    task: ReviewTaskSlice,
}

#[derive(Debug, Deserialize)]
struct ReviewTaskSlice {
    lens: LensSlice,
    work: ReviewWorkSlice,
}

#[derive(Debug, Default, Deserialize)]
struct LensSlice {
    #[serde(default)]
    name: String,
}

#[derive(Debug, Deserialize)]
struct ReviewWorkSlice {
    item: ReviewItemSlice,
    #[serde(default)]
    workspace_path: String,
}

#[derive(Debug, Deserialize)]
struct ReviewItemSlice {
    #[serde(default)]
    id: String,
}

fn require_non_blank(value: &str, what: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!(
            "agent activity input carries an empty {what} — the harness \
             cannot root or key the session on an unnamed value"
        ));
    }
    Ok(())
}

/// The planner's extractor: rooted at the input's `workspace_path` (the
/// repo), session suffix `planner`.
///
/// # Errors
///
/// A message when the input carries no non-empty `workspace_path`.
pub fn planner_context(context_json: &str) -> Result<RoleContext, String> {
    let parsed: TopLevelWorkspace = serde_json::from_str(context_json)
        .map_err(|error| format!("planner input does not parse: {error}"))?;
    require_non_blank(&parsed.workspace_path, "workspace_path")?;
    Ok(RoleContext {
        workspace_root: parsed.workspace_path,
        session_suffix: "planner".to_owned(),
        plan: None,
    })
}

/// The developer's extractor: rooted at `work.workspace_path`, session
/// suffix `dev-<item id>`, with the [`PostRunPlan::DevWork`] commit plan.
///
/// # Errors
///
/// A message when the input carries no provisioned item, or blank
/// coordinates.
pub fn dev_item_context(context_json: &str) -> Result<RoleContext, String> {
    let parsed: DevItemInput = serde_json::from_str(context_json)
        .map_err(|error| format!("dev_item input does not carry a provisioned item: {error}"))?;
    require_non_blank(&parsed.work.workspace_path, "work.workspace_path")?;
    require_non_blank(&parsed.work.item.id, "work.item.id")?;
    Ok(RoleContext {
        workspace_root: parsed.work.workspace_path.clone(),
        session_suffix: format!("dev-{}", parsed.work.item.id),
        plan: Some(PostRunPlan::DevWork {
            work: Box::new(parsed.work),
        }),
    })
}

/// The reviewer's extractor: rooted at `task.work.workspace_path`, session
/// suffix `review-<item id>-<lens name>` (the lens is load-bearing — see the
/// module doc), with the [`PostRunPlan::StampVerdict`] identity plan.
///
/// # Errors
///
/// A message when the input carries no panel seat, no item id, no worktree, or
/// a lens name that cannot key a verdict and name a session.
pub fn review_item_context(context_json: &str) -> Result<RoleContext, String> {
    let parsed: ReviewItemInput = serde_json::from_str(context_json)
        .map_err(|error| format!("review_item input does not carry a panel task: {error}"))?;
    require_non_blank(&parsed.task.work.workspace_path, "task.work.workspace_path")?;
    require_non_blank(&parsed.task.work.item.id, "task.work.item.id")?;
    require_lens_name(&parsed.task.lens.name)?;
    Ok(RoleContext {
        workspace_root: parsed.task.work.workspace_path,
        session_suffix: review_session_suffix(&parsed.task.work.item.id, &parsed.task.lens.name),
        plan: Some(PostRunPlan::StampVerdict {
            item_id: parsed.task.work.item.id,
            lens: parsed.task.lens.name,
        }),
    })
}

/// The remediator's extractor: rooted at the input's top-level
/// `workspace_path` (the integration worktree), session suffix `planner` —
/// THE mechanic: the remediator resumes the planner's session — with the
/// [`PostRunPlan::ConcludeMerge`] plan.
///
/// # Errors
///
/// A message when the input carries no non-empty `workspace_path`.
pub fn remediate_context(context_json: &str) -> Result<RoleContext, String> {
    let parsed: TopLevelWorkspace = serde_json::from_str(context_json)
        .map_err(|error| format!("remediate input does not parse: {error}"))?;
    require_non_blank(&parsed.workspace_path, "workspace_path")?;
    Ok(RoleContext {
        workspace_root: parsed.workspace_path.clone(),
        session_suffix: "planner".to_owned(),
        plan: Some(PostRunPlan::ConcludeMerge {
            workspace_path: parsed.workspace_path,
        }),
    })
}

/// The integration fixer's extractor: rooted at the input's top-level
/// `workspace_path` (the integration worktree), session suffix `planner` — it
/// RESUMES the planner's session for the same reason the remediator does, so
/// the coordinator answers for its own decomposition — with the
/// [`PostRunPlan::IntegrationFix`] commit plan.
///
/// # Errors
///
/// A message when the input carries no non-empty `workspace_path`.
pub fn fix_integration_context(context_json: &str) -> Result<RoleContext, String> {
    let parsed: TopLevelWorkspace = serde_json::from_str(context_json)
        .map_err(|error| format!("fix_integration input does not parse: {error}"))?;
    require_non_blank(&parsed.workspace_path, "workspace_path")?;
    Ok(RoleContext {
        workspace_root: parsed.workspace_path.clone(),
        session_suffix: "planner".to_owned(),
        plan: Some(PostRunPlan::IntegrationFix {
            workspace_path: parsed.workspace_path,
        }),
    })
}
