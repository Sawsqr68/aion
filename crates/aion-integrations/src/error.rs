//! The harness-neutral error taxonomy for the integration seam.
//!
//! [`HarnessError`] is the single error type every [`crate::AgentHarness`] /
//! [`crate::AgentSession`] method returns. It is **harness-neutral**: no variant names a
//! concrete harness, and only the transport/protocol variants reference the notion of a wire
//! at all (as generic descriptions, never a specific protocol type). An adapter maps its own
//! failures onto these variants; callers above the adapter branch on the variant alone.

/// The neutral error taxonomy for the harness-integration seam.
///
/// Every arm is harness-neutral. [`Self::CapabilityNotSupported`] is the first-class outcome an
/// observability-only harness returns from [`crate::AgentSession::intervene`] for any command —
/// it is a legitimate, gated rejection, not an internal failure.
#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum HarnessError {
    /// The requested intervention primitive is not in the harness's advertised capability set.
    ///
    /// This is the first-class rejection an observability-only harness (empty capability set)
    /// returns for *every* command, and the rejection any harness returns for a primitive it did
    /// not advertise. It is a normal, expected outcome of capability gating — not a fault.
    #[error("capability not supported: {primitive}")]
    CapabilityNotSupported {
        /// A neutral label naming the unsupported primitive (e.g. `"pause_resume"`).
        primitive: String,
    },
    /// The command targets a stale or unknown activity attempt and is a no-op.
    ///
    /// A command addressed to a superseded attempt (a later attempt is now live) or to a session
    /// that has already reached its terminal result is dropped without effect.
    #[error("stale target: {detail}")]
    StaleTarget {
        /// Human-readable detail describing why the target is stale.
        detail: String,
    },
    /// The underlying transport failed (spawn/connect failure, broken pipe, EOF, I/O error).
    ///
    /// Neutral: it describes *that* the transport failed and carries the detail, never *which*
    /// transport. An adapter maps its own I/O failures here.
    #[error("transport error: {detail}")]
    Transport {
        /// Human-readable description of the transport failure.
        detail: String,
    },
    /// A message was received that violates the wire protocol contract.
    ///
    /// Malformed framing, an undecodable envelope, a response that correlates to no outstanding
    /// request, or a terminal result delivered on the wrong message kind. This signals a bug in
    /// the peer or the adapter, distinct from an ordinary transport outage.
    #[error("protocol error: {detail}")]
    Protocol {
        /// Human-readable description of the protocol violation.
        detail: String,
    },
    /// The harness reported an application-level failure while running the agent.
    ///
    /// The agent ran but ended in failure (a non-success exit, an error result, a rejected run).
    /// Distinct from [`Self::Transport`] (the channel broke) and [`Self::Protocol`] (a malformed
    /// message): here the channel and framing were sound and the harness *reported* failure.
    #[error("harness reported failure: {detail}")]
    Harness {
        /// Human-readable description of the reported failure.
        detail: String,
    },
    /// The harness cannot be launched as CONFIGURED, so no run was started.
    ///
    /// A DETERMINISTIC refusal, and its own variant for the same reason
    /// [`Self::Contract`] is: the channel never opened, so [`Self::Transport`] is wrong
    /// and would tell an operator a story about a flaky pipe; no frame was exchanged, so
    /// [`Self::Protocol`] is wrong; and nothing ran, so [`Self::Harness`] is wrong. What
    /// is wrong is a value somebody WROTE — an environment pass-through entry that is a
    /// `KEY=VALUE` pair instead of a name, a declaration that omits the variable the
    /// program is looked up on. The next attempt reads the same document and meets the
    /// same wall, so retrying spends a worker's attempt budget to learn nothing. The
    /// worker maps this variant to a TERMINAL activity failure.
    #[error("harness configuration refusal: {detail}")]
    Configuration {
        /// Human-readable description naming the setting and what is wrong with it.
        detail: String,
    },
    /// The run completed, but its native outcome cannot satisfy the canonical
    /// agent-outcome contract (`AgentOutcome { text, stop_reason, session_id }`).
    ///
    /// A DETERMINISTIC refusal, and that is the whole reason it is its own variant: the channel
    /// was sound ([`Self::Transport`] is wrong), the frames were well-formed ([`Self::Protocol`]
    /// is wrong — and a protocol fault CAN be a transient peer flake, which this never is), and
    /// the run did not fail ([`Self::Harness`] is wrong). The run's *configuration* produces an
    /// outcome the seam excludes — e.g. a structured output where the contract's `text` demands a
    /// String — so re-running it re-spends a whole agent run to hit the same wall. The worker
    /// maps this variant to a TERMINAL activity failure; every other variant stays retryable.
    #[error("agent-outcome contract refusal: {detail}")]
    Contract {
        /// Human-readable description naming the contract and what was found.
        detail: String,
    },
}

impl HarnessError {
    /// Builds a [`Self::CapabilityNotSupported`] naming the unsupported primitive.
    #[must_use]
    pub fn capability_not_supported(primitive: impl Into<String>) -> Self {
        Self::CapabilityNotSupported {
            primitive: primitive.into(),
        }
    }

    /// Builds a [`Self::StaleTarget`] with a detail message.
    #[must_use]
    pub fn stale_target(detail: impl Into<String>) -> Self {
        Self::StaleTarget {
            detail: detail.into(),
        }
    }

    /// Builds a [`Self::Transport`] with a detail message.
    #[must_use]
    pub fn transport(detail: impl Into<String>) -> Self {
        Self::Transport {
            detail: detail.into(),
        }
    }

    /// Builds a [`Self::Protocol`] with a detail message.
    #[must_use]
    pub fn protocol(detail: impl Into<String>) -> Self {
        Self::Protocol {
            detail: detail.into(),
        }
    }

    /// Builds a [`Self::Harness`] with a detail message.
    #[must_use]
    pub fn harness(detail: impl Into<String>) -> Self {
        Self::Harness {
            detail: detail.into(),
        }
    }

    /// Builds a [`Self::Contract`] with a detail message naming the canonical
    /// agent-outcome contract and what was found instead.
    #[must_use]
    pub fn contract(detail: impl Into<String>) -> Self {
        Self::Contract {
            detail: detail.into(),
        }
    }

    /// Builds a [`Self::Configuration`] with a detail message naming the setting that
    /// makes the harness unlaunchable.
    #[must_use]
    pub fn configuration(detail: impl Into<String>) -> Self {
        Self::Configuration {
            detail: detail.into(),
        }
    }

    /// Whether this error is DETERMINISTIC — a property of how the run is
    /// configured, so retrying re-spends a whole agent run to hit the same
    /// wall — as opposed to potentially transient (a provider-overload burst,
    /// a one-off malformed frame, a dropped pipe, a superseded attempt).
    ///
    /// This is THE retry-classification decision for the seam, made here in
    /// the defining crate with an EXHAUSTIVE match — legal despite
    /// `#[non_exhaustive]` — so adding a variant is a compile error at this
    /// site and its classification is decided on purpose, never defaulted by
    /// a caller's wildcard arm. The worker maps `true` to a terminal activity
    /// failure and `false` to a retryable one.
    ///
    /// Per variant:
    /// - [`Self::Contract`]: deterministic by definition — the run completed
    ///   and its configured outcome shape cannot satisfy the agent-outcome
    ///   contract; the next attempt is configured identically.
    /// - [`Self::Configuration`]: deterministic by definition — the launch was
    ///   refused by a value in the document, and the next attempt reads the
    ///   same document. A refusal that presented as a transient transport
    ///   failure would tell the operator the wrong story AND spend the whole
    ///   attempt budget confirming it.
    /// - [`Self::Transport`]: a broken channel can heal.
    /// - [`Self::Protocol`]: a malformed frame CAN be a one-off peer flake
    ///   (truncated stream, interleaved write), so it stays retryable even
    ///   though some protocol faults are in fact permanent.
    /// - [`Self::Harness`]: the run failed; overload and timeouts recur or
    ///   do not — that judgement belongs to the retry policy.
    /// - [`Self::CapabilityNotSupported`] / [`Self::StaleTarget`]: gating and
    ///   staleness outcomes on the intervention path; when they surface from
    ///   a result path at all they describe a racing world, not a fixed one.
    #[must_use]
    pub fn is_deterministic(&self) -> bool {
        match self {
            Self::Contract { .. } | Self::Configuration { .. } => true,
            Self::CapabilityNotSupported { .. }
            | Self::StaleTarget { .. }
            | Self::Transport { .. }
            | Self::Protocol { .. }
            | Self::Harness { .. } => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::HarnessError;

    fn assert_send_sync_static<T: Send + Sync + 'static>() {}

    #[test]
    fn harness_error_is_send_sync_static() {
        assert_send_sync_static::<HarnessError>();
    }

    #[test]
    fn capability_not_supported_names_the_primitive() {
        let error = HarnessError::capability_not_supported("pause_resume");
        assert_eq!(error.to_string(), "capability not supported: pause_resume");
        assert!(matches!(error, HarnessError::CapabilityNotSupported { .. }));
    }

    #[test]
    fn each_constructor_renders_its_class() {
        assert_eq!(
            HarnessError::stale_target("attempt 2 superseded").to_string(),
            "stale target: attempt 2 superseded"
        );
        assert_eq!(
            HarnessError::transport("broken pipe").to_string(),
            "transport error: broken pipe"
        );
        assert_eq!(
            HarnessError::protocol("no matching id").to_string(),
            "protocol error: no matching id"
        );
        assert_eq!(
            HarnessError::harness("exit code 1").to_string(),
            "harness reported failure: exit code 1"
        );
        assert_eq!(
            HarnessError::contract("output is a JSON object").to_string(),
            "agent-outcome contract refusal: output is a JSON object"
        );
    }

    /// The retry-classification decision, pinned in the crate that makes it:
    /// exactly the contract refusal is deterministic; every class that can be
    /// transient stays non-deterministic. (The match inside `is_deterministic`
    /// is exhaustive, so a new variant fails compilation there — this test
    /// pins the ANSWERS, the compiler pins the completeness.)
    #[test]
    fn only_the_contract_refusal_is_deterministic() {
        assert!(HarnessError::contract("output is a JSON object").is_deterministic());
        for transient in [
            HarnessError::transport("broken pipe"),
            HarnessError::protocol("invalid JSON frame"),
            HarnessError::harness("run stopped without completing"),
            HarnessError::stale_target("attempt 2 superseded"),
            HarnessError::capability_not_supported("pause_resume"),
        ] {
            assert!(
                !transient.is_deterministic(),
                "{transient:?} can be transient and must not classify deterministic"
            );
        }
    }
}
