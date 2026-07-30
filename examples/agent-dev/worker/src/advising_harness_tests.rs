//! Tests for the advising decorator, driven through a fake harness so the
//! whole path — start, terminal result, and the untouched forwarding — is
//! exercised without norn.

use std::sync::Arc;

use aion_integrations::{
    ActivityEvent, ActivityId, AgentRunSpec, ContentType, DynAgentHarness, DynAgentSession,
    HarnessError, InterventionCapabilities, InterventionCommand, InterventionKind, Payload,
    WorkflowId,
};
use async_trait::async_trait;
use futures::stream::{self, BoxStream};

use super::AdvisingHarness;

/// The refusal recorded in durable history for the 28-day-parked run.
const RECORDED_REFUSAL: &str = "run/execute failed (code -32603): agent error: session error: \
     storage error: open_session failed: session 'run-assistant' is a \
     fresh-epoch projection; explicit resume approval is required";

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
        ActivityId::from_sequence_position(1),
        1,
        "assistant",
        Payload::new(ContentType::Json, b"\"prompt\"".to_vec()),
    )
}

fn cancel_command() -> InterventionCommand {
    InterventionCommand {
        workflow_id: WorkflowId::new_v4(),
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
    AdvisingHarness::new(Arc::new(FakeHarness { behaviour }), allow_degraded_session)
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
        detail.contains("--norn-allow-degraded-session"),
        "an open-time refusal must teach: {detail}"
    );
    assert!(
        detail.contains("fresh-epoch projection"),
        "the advice must keep norn's own words: {detail}"
    );
}

#[tokio::test]
async fn a_refusal_at_the_terminal_result_is_advised() {
    // This is where the real one arrived: norn accepted the session open and
    // reported the storage failure on `run/execute`.
    let harness = wrapped(
        Behaviour::FailResult(HarnessError::harness(RECORDED_REFUSAL)),
        false,
    );
    let detail = run_to_failure(&harness).await.unwrap_or_default();
    assert!(
        detail.contains("--norn-allow-degraded-session"),
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
        detail.contains("--norn-allow-degraded-session"),
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
        !detail.contains("--norn-allow-degraded-session"),
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
    // `CapabilityNotSupported` is a first-class gated rejection, not a fault:
    // rewriting it would corrupt a normal control-plane answer.
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
