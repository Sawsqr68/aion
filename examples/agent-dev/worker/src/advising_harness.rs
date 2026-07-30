//! A harness decorator that attaches operator advice to norn's
//! degraded-session refusal.
//!
//! ## The defect this closes
//!
//! An assistant session parked on its control signal for 28 days. The engine
//! held it perfectly — a live status query answered, the signal was recorded,
//! the workflow woke and dispatched the next round. What failed was the AGENT
//! session underneath it: in 28 days the provider had moved to a new epoch, so
//! norn refused to reopen the session:
//!
//! ```text
//! open_session failed: session '<id>-assistant' is a fresh-epoch projection;
//! explicit resume approval is required
//! ```
//!
//! The round retried to its cap and the workflow failed. Norn documents
//! `--allow-degraded-session` for exactly this, and it was verified by hand
//! against that real session: it resumed AND retained its conversation. This
//! worker never passed it, and the operator's only signal was a raw storage
//! string with no route out.
//!
//! ## What this does, and what it deliberately does not
//!
//! It decorates the composed harness so the refusal TEACHES: the surfaced
//! failure keeps norn's own words and appends the flag to restart the worker
//! with, plus the tradeoff in one line — the same discipline as the
//! `--account` refusal in `main.rs`, not a second error idiom.
//!
//! It does NOT decide for the operator. Resuming degraded means the exact
//! provider transcript could not be replayed; that is a real fidelity
//! tradeoff and not this worker's to assume. The flag is opt-in with no
//! default, and once given the advice goes quiet.
//!
//! This is a durability hole in the AGENT layer, not the engine: any durable
//! agent that parks longer than a provider epoch hits it.

use std::sync::Arc;

use aion_integrations::{
    ActivityEvent, AgentRunSpec, DynAgentHarness, DynAgentSession, HarnessError,
    InterventionCapabilities, InterventionCommand, Payload,
};
use async_trait::async_trait;
use futures::stream::BoxStream;

use crate::norn_invocation::degraded_session_advice;

/// Rewrite a harness failure to carry operator advice when it is the
/// degraded-session refusal. Every other failure passes through untouched —
/// advice attached to failures it does not explain is noise that trains
/// operators to skip the message.
fn advise(error: HarnessError, allow_degraded_session: bool) -> HarnessError {
    match error {
        HarnessError::Harness { detail } => {
            match degraded_session_advice(&detail, allow_degraded_session) {
                Some(advised) => HarnessError::Harness { detail: advised },
                None => HarnessError::Harness { detail },
            }
        }
        HarnessError::Transport { detail } => {
            match degraded_session_advice(&detail, allow_degraded_session) {
                Some(advised) => HarnessError::Transport { detail: advised },
                None => HarnessError::Transport { detail },
            }
        }
        other => other,
    }
}

/// Wraps a composed harness so its failures carry the degraded-session advice.
///
/// The refusal can surface either at session open (`start_dyn`) or at the
/// terminal result (`wait_result`, where norn's JSON-RPC `run/execute` error
/// arrives), so both are covered. Everything else is forwarded verbatim: this
/// decorator adds words to failures and changes nothing else.
pub struct AdvisingHarness {
    /// The composed harness this decorates.
    inner: Arc<dyn DynAgentHarness>,
    /// Whether the operator gave the degraded-resume opt-in.
    allow_degraded_session: bool,
}

impl AdvisingHarness {
    /// Wrap `inner`, recording whether the operator gave the opt-in.
    #[must_use]
    pub fn new(inner: Arc<dyn DynAgentHarness>, allow_degraded_session: bool) -> Self {
        Self {
            inner,
            allow_degraded_session,
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
            })),
            Err(error) => Err(advise(error, self.allow_degraded_session)),
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
            .map_err(|error| advise(error, this.allow_degraded_session))
    }
}

#[cfg(test)]
#[path = "advising_harness_tests.rs"]
mod tests;
