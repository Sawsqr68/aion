//! Operator advice for Norn's degraded-session refusal.
//!
//! ## The defect this closes
//!
//! An assistant session parked on its control signal for 28 days. The engine
//! held it perfectly — a live status query answered, the signal was recorded,
//! the workflow woke and dispatched the next round. What failed was the AGENT
//! session underneath it: in 28 days the provider had moved to a new epoch, so
//! Norn refused to reopen the session:
//!
//! ```text
//! open_session failed: session '<id>-assistant' is a fresh-epoch projection;
//! explicit resume approval is required
//! ```
//!
//! The round retried to its cap and the workflow failed. Norn documents
//! [`ALLOW_DEGRADED_SESSION_FLAG`] for exactly this, and it was verified by
//! hand against that real session: it resumed AND retained its conversation.
//! The worker never passed it, and the operator's only signal was a raw
//! storage string with no route out.
//!
//! This module is that route out, promoted from the assistant's example worker
//! so EVERY composition root that drives a [`NornHarness`] — the built-in
//! `aion worker agent` verb serving a document that declares `kind norn`, and
//! hand-written workers alike —
//! surfaces the same teaching failure. This is a durability hole in the AGENT
//! layer, not the engine: any durable agent that parks longer than a provider
//! epoch hits it.
//!
//! ## What this does, and what it deliberately does not
//!
//! [`AdvisingHarness`] decorates a composed harness so the refusal TEACHES:
//! the surfaced failure keeps Norn's own words and appends the restart flag —
//! spelled the way THIS deployment spells it, which is why the opt-in
//! instruction is a caller-supplied parameter and never invented here — plus
//! the tradeoff in one line so the decision is informed.
//!
//! It does NOT decide for the operator. Resuming degraded means the exact
//! provider transcript could not be replayed; that is a real fidelity tradeoff
//! and not any worker's to assume. The flag is opt-in with no default, and
//! once given the advice goes quiet.
//!
//! [`NornHarness`]: crate::NornHarness

use std::sync::Arc;

use aion_integrations::{
    ActivityEvent, AgentRunSpec, DynAgentHarness, DynAgentSession, HarnessError,
    InterventionCapabilities, InterventionCommand, Payload,
};
use async_trait::async_trait;
use futures::stream::BoxStream;

/// Norn's flag for resuming a session whose exact provider transcript can no
/// longer be replayed.
pub const ALLOW_DEGRADED_SESSION_FLAG: &str = "--allow-degraded-session";

/// Substrings identifying Norn's degraded-session refusal. Both must appear:
/// the condition (`fresh-epoch projection`) and the demand (`explicit resume
/// approval is required`). Requiring both keeps the advice off unrelated
/// storage errors that happen to mention an epoch.
const DEGRADED_SESSION_MARKERS: [&str; 2] = [
    "fresh-epoch projection",
    "explicit resume approval is required",
];

/// Whether `detail` is Norn's degraded-session refusal.
#[must_use]
pub fn is_degraded_session_refusal(detail: &str) -> bool {
    DEGRADED_SESSION_MARKERS
        .iter()
        .all(|marker| detail.contains(marker))
}

/// Whether a composed Norn argument list already carries
/// [`ALLOW_DEGRADED_SESSION_FLAG`].
///
/// This is how a composition root derives the opt-in state from what it
/// actually composed rather than tracking it in a second place that could
/// drift: the flag either reaches the spawned `norn`'s argv or it does not,
/// and the advice must go quiet exactly when it does. The match is exact,
/// argument for argument — no prefix or substring reading, because
/// `--allow-degraded-sessions` is a different (nonexistent) flag.
#[must_use]
pub fn argv_allows_degraded_session<I>(args: I) -> bool
where
    I: IntoIterator,
    I::Item: AsRef<str>,
{
    args.into_iter()
        .any(|arg| arg.as_ref() == ALLOW_DEGRADED_SESSION_FLAG)
}

/// The operator advice for a failed agent round, or `None` when the failure is
/// not the degraded-session refusal or the opt-in was already given.
///
/// A durable session can park for weeks — one real run parked 28 days and the
/// engine resumed it perfectly — and in that time the agent provider can move
/// to a new epoch. When it has, Norn refuses to resume without explicit
/// approval, and the operator sees a raw storage string with no route out.
/// This is the route out: name the flag, and state the tradeoff in one line so
/// the decision is informed.
///
/// `opt_in` is the exact restart instruction as THIS deployment spells it —
/// for the built-in `aion worker agent` verb, adding
/// `"--allow-degraded-session"` to the worker document's `harness` section
/// `args` list and redeploying; a worker's own flag for a hand-written
/// composition root. It is a parameter with no default because the spelling
/// belongs to the deployment, which this crate cannot know.
///
/// The advice is withheld once the opt-in has been given
/// (`allow_degraded_session`): at that point Norn has refused for some other
/// reason and pointing at a flag already in the argv would send the operator
/// in a circle.
#[must_use]
pub fn degraded_session_advice(
    detail: &str,
    allow_degraded_session: bool,
    opt_in: &str,
) -> Option<String> {
    if allow_degraded_session || !is_degraded_session_refusal(detail) {
        return None;
    }
    Some(format!(
        "{detail}\n\nOPERATOR ACTION: this session outlived the agent provider's \
         epoch, so norn will not resume it without approval. Restart this worker \
         with {opt_in} to allow it. The tradeoff: the session resumes from a fresh \
         provider epoch because its exact provider transcript cannot be replayed, \
         so provider-side fidelity is reduced — the conversation itself is \
         retained. Without the flag the round fails on every retry and the \
         workflow fails with it."
    ))
}

/// Rewrite a harness failure to carry the operator advice when it is the
/// degraded-session refusal. Every other failure passes through untouched —
/// advice attached to failures it does not explain is noise that trains
/// operators to skip the message.
fn advise(error: HarnessError, allow_degraded_session: bool, opt_in: &str) -> HarnessError {
    match error {
        HarnessError::Harness { detail } => {
            match degraded_session_advice(&detail, allow_degraded_session, opt_in) {
                Some(advised) => HarnessError::Harness { detail: advised },
                None => HarnessError::Harness { detail },
            }
        }
        HarnessError::Transport { detail } => {
            match degraded_session_advice(&detail, allow_degraded_session, opt_in) {
                Some(advised) => HarnessError::Transport { detail: advised },
                None => HarnessError::Transport { detail },
            }
        }
        // Policy classification is adapter-local and must survive this prose-only
        // decorator unchanged; reclassifying here would discard the structured seam.
        policy_refused @ HarnessError::PolicyRefused { .. } => policy_refused,
        other => other,
    }
}

/// Wraps a composed harness so its failures carry the degraded-session advice.
///
/// The refusal can surface either at session open (`start_dyn`) or at the
/// terminal result (`wait_result`, where Norn's JSON-RPC `run/execute` error
/// arrives), so both are covered. Everything else is forwarded verbatim: this
/// decorator adds words to failures and changes nothing else.
pub struct AdvisingHarness {
    /// The composed harness this decorates.
    inner: Arc<dyn DynAgentHarness>,
    /// Whether the operator gave the degraded-resume opt-in.
    allow_degraded_session: bool,
    /// The restart instruction, as this deployment's command line spells it.
    opt_in: String,
}

impl AdvisingHarness {
    /// Wrap `inner`, recording whether the operator gave the opt-in and how
    /// this deployment spells it.
    ///
    /// Derive `allow_degraded_session` from the argv actually composed
    /// ([`argv_allows_degraded_session`]) rather than a separate record of
    /// intent, so the advice cannot fall out of step with what the spawned
    /// `norn` receives.
    #[must_use]
    pub fn new(
        inner: Arc<dyn DynAgentHarness>,
        allow_degraded_session: bool,
        opt_in: impl Into<String>,
    ) -> Self {
        Self {
            inner,
            allow_degraded_session,
            opt_in: opt_in.into(),
        }
    }
}

#[async_trait]
impl DynAgentHarness for AdvisingHarness {
    async fn start_dyn(
        &self,
        spec: AgentRunSpec,
    ) -> Result<Box<dyn DynAgentSession>, HarnessError> {
        match self.inner.start_dyn(spec).await {
            Ok(session) => Ok(Box::new(AdvisingSession {
                inner: session,
                allow_degraded_session: self.allow_degraded_session,
                opt_in: self.opt_in.clone(),
            })),
            Err(error) => Err(advise(error, self.allow_degraded_session, &self.opt_in)),
        }
    }
}

/// The session half of [`AdvisingHarness`]: forwards everything, and advises
/// on the terminal result's failure.
struct AdvisingSession {
    /// The decorated session.
    inner: Box<dyn DynAgentSession>,
    /// Whether the operator gave the degraded-resume opt-in.
    allow_degraded_session: bool,
    /// The restart instruction, as this deployment's command line spells it.
    opt_in: String,
}

#[async_trait(?Send)]
impl DynAgentSession for AdvisingSession {
    fn capabilities(&self) -> &InterventionCapabilities {
        self.inner.capabilities()
    }

    fn events(&mut self) -> BoxStream<'static, ActivityEvent> {
        self.inner.events()
    }

    async fn intervene(&self, cmd: InterventionCommand) -> Result<(), HarnessError> {
        self.inner.intervene(cmd).await
    }

    async fn wait_result(self: Box<Self>) -> Result<Payload, HarnessError> {
        let this = *self;
        this.inner
            .wait_result()
            .await
            .map_err(|error| advise(error, this.allow_degraded_session, &this.opt_in))
    }
}

#[cfg(test)]
mod tests {
    //! Tests for the degraded-session recognition, the advice, and the
    //! decorator — the decorator driven through a fake harness so the whole
    //! path (start, terminal result, untouched forwarding) is exercised
    //! without Norn.
    //!
    //! The negative controls carry the weight here. A flag only ever observed
    //! to fire is untested in the case that matters, and advice that fires on
    //! every failure teaches nothing.

    use std::sync::Arc;

    use aion_integrations::{
        ActivityEvent, ActivityId, AgentRunSpec, ContentType, DynAgentHarness, DynAgentSession,
        HarnessError, InterventionCapabilities, InterventionCommand, InterventionKind, Payload,
        RunId, WorkflowId,
    };
    use async_trait::async_trait;
    use futures::stream::{self, BoxStream};

    use super::{
        ALLOW_DEGRADED_SESSION_FLAG, AdvisingHarness, argv_allows_degraded_session,
        degraded_session_advice, is_degraded_session_refusal,
    };

    /// The exact refusal recorded in durable history for the 28-day-parked
    /// assistant run — the evidence this whole path was built from, verbatim.
    const RECORDED_REFUSAL: &str = "run/execute failed (code -32603): agent error: session error: \
         storage error: open_session failed: session \
         'f9b062c2-d260-43f9-8374-d3210330f567-assistant' is a fresh-epoch \
         projection; explicit resume approval is required";

    /// The restart instruction the built-in `aion worker agent` verb supplies.
    ///
    /// It is a sentence about a DOCUMENT rather than a flag: the launch settings moved
    /// into the worker document's `harness` section, so the route out of a degraded
    /// session is an edit and a redeploy, not a different command line.
    const OPT_IN: &str =
        "the worker document's `harness` section: add \"--allow-degraded-session\" to its `args`";

    #[test]
    fn the_recorded_refusal_is_recognised() {
        assert!(
            is_degraded_session_refusal(RECORDED_REFUSAL),
            "the refusal recorded on the real run must be recognised"
        );
    }

    #[test]
    fn unrelated_failures_are_not_mistaken_for_it() {
        for detail in [
            "harness reported failure: exit code 1",
            "storage error: open_session failed: no such session",
            // Names the condition but makes no approval demand.
            "session is a fresh-epoch projection",
            // Makes the demand but for a different condition.
            "explicit resume approval is required for forked sessions",
            "",
        ] {
            assert!(
                !is_degraded_session_refusal(detail),
                "must not claim the degraded-session condition for: {detail}"
            );
        }
    }

    #[test]
    fn the_composed_argv_is_what_decides_the_opt_in() {
        assert!(argv_allows_degraded_session([
            "--session-id",
            "run-assistant",
            ALLOW_DEGRADED_SESSION_FLAG,
        ]));
        // The negative control, and the exact-match rule: a different flag
        // that merely CONTAINS the spelling must not count as the opt-in.
        assert!(!argv_allows_degraded_session([
            "--session-id",
            "run-assistant",
        ]));
        assert!(!argv_allows_degraded_session(["--allow-degraded-sessions"]));
        assert!(!argv_allows_degraded_session(Vec::<String>::new()));
    }

    #[test]
    fn the_advice_names_the_deployments_own_opt_in_and_states_the_tradeoff() {
        let advice = degraded_session_advice(RECORDED_REFUSAL, false, OPT_IN);
        assert!(advice.is_some(), "the recorded refusal must produce advice");
        let advice = advice.unwrap_or_default();
        assert!(
            advice.contains(OPT_IN),
            "the advice must carry the caller's restart instruction verbatim: {advice}"
        );
        assert!(
            advice.contains("fidelity"),
            "the advice must state the tradeoff, not just the escape hatch: {advice}"
        );
        assert!(
            advice.contains(RECORDED_REFUSAL),
            "the advice must PRECEDE, not replace, the underlying refusal: {advice}"
        );
    }

    #[test]
    fn a_hand_written_workers_spelling_is_carried_verbatim_too() {
        // The instruction is a parameter precisely because the spelling
        // belongs to the deployment: the example worker's own flag must
        // survive the promotion unchanged.
        let advice =
            degraded_session_advice(RECORDED_REFUSAL, false, "--norn-allow-degraded-session")
                .unwrap_or_default();
        assert!(
            advice.contains("--norn-allow-degraded-session"),
            "the worker's own spelling must appear verbatim: {advice}"
        );
    }

    #[test]
    fn the_advice_is_withheld_once_the_opt_in_was_given() {
        // Pointing at a flag that is already in the argv sends the operator
        // in a circle; at that point Norn refused for some other reason.
        assert_eq!(
            degraded_session_advice(RECORDED_REFUSAL, true, OPT_IN),
            None,
            "advice must not repeat a flag the operator already passed"
        );
    }

    #[test]
    fn unrelated_failures_are_passed_through_unadvised() {
        assert_eq!(
            degraded_session_advice("harness reported failure: exit code 1", false, OPT_IN),
            None,
            "only the degraded-session refusal gets this advice"
        );
    }

    /// What the fake harness should do.
    #[derive(Clone)]
    enum Behaviour {
        /// Fail at session open.
        FailStart(HarnessError),
        /// Open, then fail at the terminal result.
        FailResult(HarnessError),
        /// Open and succeed.
        Succeed,
    }

    struct FakeHarness {
        behaviour: Behaviour,
    }

    #[async_trait]
    impl DynAgentHarness for FakeHarness {
        async fn start_dyn(
            &self,
            _spec: AgentRunSpec,
        ) -> Result<Box<dyn DynAgentSession>, HarnessError> {
            match self.behaviour.clone() {
                Behaviour::FailStart(error) => Err(error),
                other => Ok(Box::new(FakeSession {
                    behaviour: other,
                    capabilities: InterventionCapabilities::default(),
                })),
            }
        }
    }

    struct FakeSession {
        behaviour: Behaviour,
        capabilities: InterventionCapabilities,
    }

    #[async_trait(?Send)]
    impl DynAgentSession for FakeSession {
        fn capabilities(&self) -> &InterventionCapabilities {
            &self.capabilities
        }

        fn events(&mut self) -> BoxStream<'static, ActivityEvent> {
            Box::pin(stream::empty())
        }

        async fn intervene(&self, _cmd: InterventionCommand) -> Result<(), HarnessError> {
            Err(HarnessError::capability_not_supported("inject_message"))
        }

        async fn wait_result(self: Box<Self>) -> Result<Payload, HarnessError> {
            match self.behaviour.clone() {
                Behaviour::FailResult(error) => Err(error),
                _ => Ok(Payload::new(ContentType::Json, b"\"done\"".to_vec())),
            }
        }
    }

    fn spec() -> AgentRunSpec {
        AgentRunSpec::new(
            WorkflowId::new_v4(),
            RunId::new_v4(),
            ActivityId::from_sequence_position(1),
            1,
            "assistant",
            Payload::new(ContentType::Json, b"\"prompt\"".to_vec()),
        )
    }

    fn cancel_command() -> InterventionCommand {
        InterventionCommand {
            workflow_id: WorkflowId::new_v4(),
            run_id: RunId::new_v4(),
            activity_id: ActivityId::from_sequence_position(1),
            attempt: 1,
            issued_by: None,
            issued_at: chrono::Utc::now(),
            kind: InterventionKind::Cancel {
                reason: "operator".to_owned(),
            },
        }
    }

    fn wrapped(behaviour: Behaviour, allow_degraded_session: bool) -> AdvisingHarness {
        AdvisingHarness::new(
            Arc::new(FakeHarness { behaviour }),
            allow_degraded_session,
            OPT_IN,
        )
    }

    /// Run the decorated harness to its terminal result, returning the failure
    /// detail if it failed.
    async fn run_to_failure(harness: &AdvisingHarness) -> Option<String> {
        match harness.start_dyn(spec()).await {
            Err(error) => Some(error.to_string()),
            Ok(session) => session.wait_result().await.err().map(|e| e.to_string()),
        }
    }

    #[tokio::test]
    async fn a_refusal_at_session_open_is_advised() {
        let harness = wrapped(
            Behaviour::FailStart(HarnessError::harness(RECORDED_REFUSAL)),
            false,
        );
        let detail = run_to_failure(&harness).await.unwrap_or_default();
        assert!(
            detail.contains(OPT_IN),
            "an open-time refusal must teach: {detail}"
        );
        assert!(
            detail.contains("fresh-epoch projection"),
            "the advice must keep norn's own words: {detail}"
        );
    }

    #[tokio::test]
    async fn a_refusal_at_the_terminal_result_is_advised() {
        // This is where the real one arrived: norn accepted the session open
        // and reported the storage failure on `run/execute`.
        let harness = wrapped(
            Behaviour::FailResult(HarnessError::harness(RECORDED_REFUSAL)),
            false,
        );
        let detail = run_to_failure(&harness).await.unwrap_or_default();
        assert!(
            detail.contains(OPT_IN),
            "the terminal-result refusal must teach: {detail}"
        );
    }

    #[tokio::test]
    async fn a_transport_refusal_is_advised_too() {
        let harness = wrapped(
            Behaviour::FailStart(HarnessError::transport(RECORDED_REFUSAL)),
            false,
        );
        let detail = run_to_failure(&harness).await.unwrap_or_default();
        assert!(
            detail.contains(OPT_IN),
            "the refusal must teach whichever variant carries it: {detail}"
        );
    }

    #[tokio::test]
    async fn once_the_opt_in_is_given_the_advice_goes_quiet() {
        let harness = wrapped(
            Behaviour::FailResult(HarnessError::harness(RECORDED_REFUSAL)),
            true,
        );
        let detail = run_to_failure(&harness).await.unwrap_or_default();
        assert!(
            !detail.contains(OPT_IN),
            "advice must not repeat a flag already in the argv: {detail}"
        );
        assert!(
            detail.contains("fresh-epoch projection"),
            "the underlying refusal must still surface: {detail}"
        );
    }

    #[tokio::test]
    async fn unrelated_failures_pass_through_untouched() {
        // Advice attached to failures it does not explain trains operators to
        // skip the message.
        let original = "harness reported failure: exit code 1";
        let harness = wrapped(
            Behaviour::FailResult(HarnessError::harness(original)),
            false,
        );
        let detail = run_to_failure(&harness).await.unwrap_or_default();
        assert_eq!(detail, format!("harness reported failure: {original}"));
    }

    #[tokio::test]
    async fn a_capability_rejection_is_never_rewritten() {
        // `CapabilityNotSupported` is a first-class gated rejection, not a
        // fault: rewriting it would corrupt a normal control-plane answer.
        let harness = wrapped(Behaviour::Succeed, false);
        let session = harness.start_dyn(spec()).await.ok();
        assert!(session.is_some(), "the fake harness must open");
        let Some(session) = session else {
            unreachable!("guarded immediately above")
        };
        let rejected = session.intervene(cancel_command()).await;
        assert!(
            matches!(rejected, Err(HarnessError::CapabilityNotSupported { .. })),
            "interventions must forward verbatim, got {rejected:?}"
        );
    }

    #[tokio::test]
    async fn a_successful_run_forwards_its_payload() {
        let harness = wrapped(Behaviour::Succeed, false);
        assert_eq!(
            run_to_failure(&harness).await,
            None,
            "the decorator must not disturb a successful run"
        );
    }
}
