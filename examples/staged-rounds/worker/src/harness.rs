//! [`ProfiledNornHarness`] — the thin harness wrapper that assembles the
//! role prompt, derives the per-run session identity, and performs the
//! post-turn mechanical step, delegating everything else to the real
//! [`NornHarness`].
//!
//! WHY A WRAPPER: the per-turn prompt is the activity's per-run context
//! JSON, rendered by the activity's assembly function; the role's profile
//! markdown is the session's `--append-system-prompt` doctrine on the inner
//! harness (composed in `main.rs`). The wrapper intercepts exactly one seam:
//! it reads the spec's input payload, runs the activity's ONE assembly function
//! (`crate::prompts`), and hands the inner harness the same spec with its
//! input replaced by the assembled prompt as a JSON string.
//!
//! ONE CONNECTION MAY SERVE SEVERAL ACTIVITY TYPES. The server routes by
//! (namespace, `task_queue`, node) ONLY — never by activity type — so every
//! activity the document pins to one node must be served by ONE connection, or
//! the server will land an activity on a connection that has no handler for it.
//! `remediate` and `fix_integration` are both pinned to node `remediation`, so
//! this wrapper holds a TABLE of [`AgentActivity`] and dispatches on
//! `spec.activity_type`. A spec naming an activity this harness does not serve
//! is a loud protocol error, never a fallback to some default role.
//!
//! PER-RUN SESSION IDENTITY (the mechanic dev-brief's template could not
//! express): the norn arg template expands only `{workflow_id}` and
//! `{activity_type}`, so a per-ITEM (let alone per-item-per-LENS) session id
//! cannot ride the template. The wrapper therefore appends `--session-id
//! <workflow_id>-<suffix>` and `--resume-if-exists` per run in `start()`,
//! deriving the suffix from the activity's extractor over the input JSON (see
//! [`context`] for the whole session topology). `--workspace-root` is appended
//! the same way (dev-brief's proven per-run-append seam).
//!
//! THE POST-TURN SEAM (doctrine: agents do not run git, and agents do not
//! assert their own identity — the machinery does both): the developer role
//! commits the round's work and its result is ASSEMBLED around the agent's
//! report with the real coordinates and the real head; the reviewer role's
//! verdict is STAMPED with the panel seat that was dispatched; the remediator
//! concludes the in-progress merge it resolved; and the integration fixer's
//! repair is committed. See [`crate::commit`] and [`crate::agent_output`].

use aion_integration_norn::NornHarness;
use aion_integrations::contract::{AgentHarness, AgentSession};
use aion_integrations::{
    ActivityEvent, AgentRunSpec, ContentType, HarnessError, InterventionCapabilities,
    InterventionCommand, Payload,
};
use async_trait::async_trait;
use futures::stream::BoxStream;
use serde_json::Value;

use crate::agent_output;
use crate::commit;
use crate::prompts::AssembleFn;
use crate::shell::Shell;
use crate::types::ProvisionedItem;

#[path = "harness/context.rs"]
mod context;

pub use context::{
    ExtractFn, PostRunPlan, RoleContext, dev_item_context, fix_integration_context,
    planner_context, remediate_context, review_item_context,
};

/// One agent activity type a connection serves: the activity-type name the
/// server dispatches, the per-turn prompt assembly, and the context extractor.
///
/// Deliberately NOT `PartialEq`: two of its three fields are function
/// pointers, whose addresses are not guaranteed unique, so equality on them
/// would be a meaningless answer dressed up as a real one. Compare
/// `activity_type` — the identity that actually matters — and exercise the
/// function fields by CALLING them.
#[derive(Clone, Debug)]
pub struct AgentActivity {
    /// The dispatched activity-type name (must equal an `action` name in
    /// `../awl/staged_rounds.awl`).
    pub activity_type: &'static str,
    /// The per-turn prompt assembly function for this activity.
    pub assemble: AssembleFn,
    /// The context extractor for this activity.
    pub extract: ExtractFn,
}

/// A per-connection harness: the composed inner [`NornHarness`] (which already
/// carries the connection's profile as its `--append-system-prompt` doctrine
/// and its `--output-schema`), plus the table of activity types it serves.
#[derive(Clone, Debug)]
pub struct ProfiledNornHarness {
    inner: NornHarness,
    activities: Vec<AgentActivity>,
}

impl ProfiledNornHarness {
    /// Wrap a composed inner harness with the activity table this connection
    /// serves.
    #[must_use]
    pub fn new(inner: NornHarness, activities: Vec<AgentActivity>) -> Self {
        Self { inner, activities }
    }

    /// The served activity, or a message naming what was dispatched and what
    /// this connection actually serves. Never a fallback: routing an unknown
    /// activity onto some default role would run the wrong prompt, the wrong
    /// session, and the wrong post-turn step.
    ///
    /// # Errors
    ///
    /// A message when `activity_type` is not in this harness's table.
    pub fn activity(&self, activity_type: &str) -> Result<&AgentActivity, String> {
        self.activities
            .iter()
            .find(|activity| activity.activity_type == activity_type)
            .ok_or_else(|| {
                format!(
                    "this agent connection does not serve activity {activity_type:?} \
                     — it serves [{}]; the server routes by (namespace, task_queue, \
                     node) only, so a mismatch means the worker's node table and the \
                     document's action table have drifted",
                    self.activities
                        .iter()
                        .map(|activity| activity.activity_type)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })
    }

    /// Assemble the per-turn context prompt this harness would send for
    /// `context_json` on `activity_type` — exposed so tests exercise the exact
    /// production assembly path.
    ///
    /// # Errors
    ///
    /// A message when this connection does not serve `activity_type`.
    pub fn assembled_prompt(
        &self,
        activity_type: &str,
        context_json: &str,
    ) -> Result<String, String> {
        Ok((self.activity(activity_type)?.assemble)(context_json))
    }
}

#[async_trait]
impl AgentHarness for ProfiledNornHarness {
    type Session = ProfiledSession;

    async fn start(&self, mut spec: AgentRunSpec) -> Result<Self::Session, HarnessError> {
        // Which activity this connection was asked to run. A type it does not
        // serve is a loud protocol error before anything is spawned.
        let activity = self
            .activity(&spec.activity_type)
            .map_err(HarnessError::protocol)?;
        // The input payload is the workflow-encoded context JSON (an object).
        let context_json = std::str::from_utf8(spec.input.bytes())
            .map_err(|source| {
                HarnessError::protocol(format!("run input is not valid UTF-8: {source}"))
            })?
            .to_owned();
        // Resolve the workspace root, session suffix, and post-run plan BEFORE
        // the run — loudly, never with a fallback that would unconfine the
        // session or lose a mechanical step.
        let context = (activity.extract)(&context_json).map_err(HarnessError::protocol)?;
        let session_id = format!("{}-{}", spec.workflow_id, context.session_suffix);
        let prompt = (activity.assemble)(&context_json);
        // Re-encode as a JSON string so the inner harness's prompt
        // derivation unwraps it to the exact assembled text.
        spec.input = Payload::from_json(&Value::String(prompt)).map_err(|source| {
            HarnessError::protocol(format!("could not encode the assembled prompt: {source}"))
        })?;
        // Root THIS run's session at the input's worktree and key it to the
        // derived per-run session id. The inner harness is a template; a
        // per-run clone appends the resolved values.
        let inner = self
            .inner
            .clone()
            .with_arg("--workspace-root")
            .with_arg(context.workspace_root)
            .with_arg("--session-id")
            .with_arg(session_id)
            .with_arg("--resume-if-exists")
            .start(spec)
            .await?;
        Ok(ProfiledSession {
            inner,
            plan: context.plan,
        })
    }
}

// --- the wrapper session ------------------------------------------------------

/// The wrapper session: everything delegates to the inner Norn session; for
/// the roles that owe one, [`AgentSession::wait_result`] additionally performs
/// the post-turn mechanical step (see [`crate::commit`] and
/// [`crate::agent_output`]).
pub struct ProfiledSession {
    inner: <NornHarness as AgentHarness>::Session,
    plan: Option<PostRunPlan>,
}

#[async_trait]
impl AgentSession for ProfiledSession {
    fn capabilities(&self) -> &InterventionCapabilities {
        self.inner.capabilities()
    }

    fn events(&mut self) -> BoxStream<'static, ActivityEvent> {
        self.inner.events()
    }

    async fn intervene(&self, cmd: InterventionCommand) -> Result<(), HarnessError> {
        self.inner.intervene(cmd).await
    }

    async fn wait_result(self) -> Result<Payload, HarnessError> {
        let Self { inner, plan } = self;
        let payload = inner.wait_result().await?;
        match plan {
            None => Ok(payload),
            Some(PostRunPlan::DevWork { work }) => commit_and_assemble(*work, payload).await,
            Some(PostRunPlan::StampVerdict { item_id, lens }) => {
                stamp_verdict(&item_id, &lens, &payload)
            }
            Some(PostRunPlan::ConcludeMerge { workspace_path }) => {
                conclude(workspace_path, payload).await
            }
            Some(PostRunPlan::IntegrationFix { workspace_path }) => {
                commit_integration_fix(workspace_path, payload).await
            }
        }
    }
}

/// The developer's post-turn step: commit the round's work, then assemble
/// the `DevItemResult` payload around the agent's report with the real head
/// (reality wins — the agent never ran git). Any refusal is an activity
/// failure.
async fn commit_and_assemble(
    work: ProvisionedItem,
    payload: Payload,
) -> Result<Payload, HarnessError> {
    let workspace_path = work.workspace_path.clone();
    let item_id = work.item.id.clone();
    let outcome = blocking_commit(
        move || commit::commit_dev_work(&Shell::inherited(), &workspace_path, &item_id),
        "dev-work commit",
    )
    .await?;
    log_commit_outcome(&outcome, "dev work committed on the item branch");
    let assembled = agent_output::assemble_dev_item_result(payload.bytes(), &work, outcome.head())
        .map_err(HarnessError::harness)?;
    Ok(Payload::new(ContentType::Json, assembled))
}

/// The reviewer's post-turn step: stamp the verdict's `item_id` and `lens`
/// from the panel seat the machinery dispatched. No git — the reviewer is
/// read-only — but its IDENTITY is still the machinery's to assert.
fn stamp_verdict(item_id: &str, lens: &str, payload: &Payload) -> Result<Payload, HarnessError> {
    let stamped = agent_output::stamp_item_verdict(payload.bytes(), item_id, lens)
        .map_err(HarnessError::harness)?;
    tracing::info!(%item_id, %lens, "verdict stamped with the dispatched panel seat");
    Ok(Payload::new(ContentType::Json, stamped))
}

/// The remediator's post-turn step: conclude the in-progress merge (or
/// commit a dirty tree, or record a clean skip). The agent's report payload
/// passes through unchanged. Any refusal is an activity failure.
async fn conclude(workspace_path: String, payload: Payload) -> Result<Payload, HarnessError> {
    let outcome = tokio::task::spawn_blocking(move || {
        commit::conclude_merge(&Shell::inherited(), &workspace_path)
    })
    .await
    .map_err(|join_error| {
        HarnessError::harness(format!(
            "merge-conclusion task did not complete: {join_error}"
        ))
    })?
    .map_err(|error| HarnessError::harness(format!("merge conclusion failed: {error}")))?;
    match &outcome {
        commit::ConcludeOutcome::Concluded { commit } => {
            tracing::info!(%commit, "in-progress merge concluded with the remediator's resolutions");
        }
        commit::ConcludeOutcome::Committed { commit } => {
            tracing::info!(%commit, "remediation fix committed (no merge was in progress)");
        }
        commit::ConcludeOutcome::Skipped { head } => {
            tracing::info!(%head, "remediation left the tree clean; nothing to conclude");
        }
    }
    Ok(payload)
}

/// The integration fixer's post-turn step: commit the repair it made to the
/// integration worktree. Without it the very next `verify_integration` — which
/// asserts a clean tree — would discard the repair and record a failure the
/// agent had already fixed. The report payload passes through unchanged.
async fn commit_integration_fix(
    workspace_path: String,
    payload: Payload,
) -> Result<Payload, HarnessError> {
    let outcome = blocking_commit(
        move || commit::commit_integration_fix(&Shell::inherited(), &workspace_path),
        "integration-fix commit",
    )
    .await?;
    log_commit_outcome(
        &outcome,
        "integration repair committed on the integration branch",
    );
    Ok(payload)
}

/// Run one blocking mechanical commit on the blocking pool (the worker drives
/// this session inside its async runtime), mapping both the join failure and
/// the commit failure to activity failures — neither is ever swallowed.
async fn blocking_commit<Body>(
    body: Body,
    what: &str,
) -> Result<commit::FixCommitOutcome, HarnessError>
where
    Body: FnOnce() -> Result<commit::FixCommitOutcome, String> + Send + 'static,
{
    tokio::task::spawn_blocking(body)
        .await
        .map_err(|join_error| {
            HarnessError::harness(format!("{what} task did not complete: {join_error}"))
        })?
        .map_err(|error| HarnessError::harness(format!("{what} failed: {error}")))
}

/// Log what a mechanical commit did — a skip is reported as loudly as a
/// commit, because "nothing to commit" is a real and meaningful outcome here.
fn log_commit_outcome(outcome: &commit::FixCommitOutcome, committed_message: &str) {
    match outcome {
        commit::FixCommitOutcome::Committed { commit, paths } => {
            tracing::info!(%commit, ?paths, "{committed_message}");
        }
        commit::FixCommitOutcome::Skipped { head, reason } => {
            tracing::info!(%head, %reason, "mechanical commit skipped");
        }
    }
}

#[cfg(test)]
#[path = "harness/tests.rs"]
mod tests;
