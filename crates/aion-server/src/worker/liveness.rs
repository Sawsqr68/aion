//! The transport-neutral vocabulary of the worker dead-man switch.
//!
//! The liveness probe measures ONE property — "the server can reach this
//! worker's dispatch path" — and it measures the same property over every
//! transport a worker may be delivered on. The wire differs (a serde ping on a
//! liminal connection, a protobuf ping on a gRPC task stream); the fact, the
//! probation that counts it, and the eligibility verdict that follows do not.
//!
//! These two types are what the transport halves share, and they live here
//! rather than inside either half so that neither can quietly grow a private
//! notion of "failed" that the other's operator lines describe differently.
//! That divergence is not hypothetical: the WARN this module's
//! [`PingFailure`] exists to keep honest once blamed a worker for a connection
//! the server had itself exhausted, because the two facts had been folded into
//! one string.

/// Why one liveness round produced no proof of dispatch reachability.
///
/// The two cases are different facts about different parties and an operator
/// must be able to act on the difference, so they are typed rather than folded
/// into one string.
#[derive(Debug)]
pub(in crate::worker) enum PingFailure {
    /// The server could not ASK. Push admission was refused, so no frame left
    /// the server and the worker was never probed. A dispatch on this channel
    /// would be refused identically. Says nothing about the worker.
    Unaskable(String),
    /// The server asked and got no usable answer. The push was admitted; the
    /// reply did not arrive in the cadence, did not decode, or echoed a
    /// sequence that was not the one asked.
    Unanswered(String),
}

/// Which wire one worker was probed over, for the operator-facing lines.
///
/// The probation, the tracker, and the verdict are transport-BLIND by design
/// (#197): a gRPC answer and a liminal answer are the same fact and are counted
/// the same way. This exists purely so an operator reading a withdrawal can
/// tell which link to go and look at — never so a decision can branch on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::worker) enum ProbedTransport {
    /// A liminal connection, addressed by its connection process id.
    Liminal {
        /// Liminal connection process id the ping was pushed on.
        pid: u64,
    },
    /// A gRPC bidirectional task stream, which has no server-side pid.
    Grpc,
}

impl ProbedTransport {
    /// The word an operator sees in the log line.
    pub(in crate::worker) const fn name(self) -> &'static str {
        match self {
            Self::Liminal { .. } => "liminal",
            Self::Grpc => "grpc",
        }
    }

    /// The connection pid, when the transport has one. `None` for gRPC, whose
    /// stream is owned by a tonic task and carries no such handle — recorded as
    /// an ABSENT field rather than a fabricated zero, so a reader filtering on
    /// `connection_pid` never matches a gRPC line by accident.
    pub(in crate::worker) const fn pid(self) -> Option<u64> {
        match self {
            Self::Liminal { pid } => Some(pid),
            Self::Grpc => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ProbedTransport;

    /// A gRPC line must carry NO connection pid. A fabricated zero would make
    /// every gRPC withdrawal look like it came from liminal connection 0, and
    /// an operator filtering their log by connection would be handed the wrong
    /// link to investigate.
    #[test]
    fn a_grpc_probe_reports_no_connection_pid() {
        assert_eq!(ProbedTransport::Grpc.pid(), None);
        assert_eq!(ProbedTransport::Grpc.name(), "grpc");
        assert_eq!(ProbedTransport::Liminal { pid: 7 }.pid(), Some(7));
        assert_eq!(ProbedTransport::Liminal { pid: 7 }.name(), "liminal");
    }
}
