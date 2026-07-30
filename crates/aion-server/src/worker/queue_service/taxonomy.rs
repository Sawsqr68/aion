//! The R1 four-way queue-service taxonomy and its typed dispatch failure.
//!
//! Every reason in [`QueueServiceReason`] is emitted by a real path at the
//! bridge seam — three from the selection census
//! ([`super::census::classify`]) and [`QueueServiceReason::Saturated`] from the
//! delivery-backpressure path ([`super::delivery`]). A variant that could not
//! fire would be a name that does not mean what it says.

use std::fmt;
use std::time::Duration;

use super::census::PoolCensus;

/// Failure-reason prefix classifying a dispatch failure as non-retryable.
const TERMINAL_PREFIX: &str = "terminal:";
/// Failure-reason prefix classifying a dispatch failure as retryable, matching
/// the vocabulary `aion`'s retry executor reads (`is_retryable_reason`).
const RETRYABLE_PREFIX: &str = "retryable:";
/// Stable tag identifying a queue-service refusal inside a dispatch failure.
const UNAVAILABLE_TAG: &str = "WORKER_UNAVAILABLE";
/// Separator between the machine-readable head and the human tail.
const TAIL_SEPARATOR: char = '—';
/// Field value standing in for an absent optional field.
const ABSENT: &str = "none";

/// Why a `(namespace, task_queue, activity_type)` address is not being served.
///
/// The names are R1's, verbatim, and are the strings that reach logs, dispatch
/// failures, and the queue-service accessor.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum QueueServiceReason {
    /// No deployed `.v4` contract declares this task queue at all. Structural:
    /// no worker can ever legitimately arrive for it, so it refuses
    /// unconditionally under every policy.
    NoQueueDeclaration,
    /// The queue is declared (or its declaration is unknowable) and no worker
    /// is connected for the pool at all.
    NoLivePollers,
    /// Workers are connected for the pool but none of them covers this
    /// activity type, or none of them sits on the dispatch's pinned node.
    PollersIncompatible,
    /// A compatible worker is connected and its intake is refusing work: the
    /// task could not be handed over before the schedule-to-start clock
    /// expired.
    Saturated,
}

impl QueueServiceReason {
    /// The canonical wire/log spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoQueueDeclaration => "NO_QUEUE_DECLARATION",
            Self::NoLivePollers => "NO_LIVE_POLLERS",
            Self::PollersIncompatible => "POLLERS_INCOMPATIBLE",
            Self::Saturated => "SATURATED",
        }
    }

    /// Whether the unservability is structural — provable from the deployed
    /// contract records alone, unchangeable by waiting or retrying.
    #[must_use]
    pub const fn is_structural(self) -> bool {
        matches!(self, Self::NoQueueDeclaration)
    }

    /// The operator-facing sentence for this reason at `address`: what has to
    /// be fixed, in words.
    ///
    /// This is the single source of that sentence. The dispatch refusal's human
    /// tail ([`WorkerUnavailable::explain`]) and the describe projection's
    /// `detail` both read it here, so a queue that is unserved cannot be
    /// explained one way in a refusal and another way in a read.
    #[must_use]
    pub fn explain(self, address: &ServiceAddress) -> String {
        let ServiceAddress {
            namespace,
            task_queue,
            activity_type,
            node,
        } = address;
        let node = node
            .as_ref()
            .map_or_else(String::new, |node| format!(" pinned to node `{node}`"));
        match self {
            Self::NoQueueDeclaration => format!(
                "no deployed contract declares task queue `{task_queue}`, \
                 so activity `{activity_type}` in namespace `{namespace}` \
                 can never be served{node}"
            ),
            Self::NoLivePollers => format!(
                "no worker is connected for `{namespace}`/`{task_queue}`, \
                 so activity `{activity_type}` is unserved{node}"
            ),
            Self::PollersIncompatible => format!(
                "workers are connected for `{namespace}`/`{task_queue}` but \
                 none serves activity `{activity_type}`{node}"
            ),
            Self::Saturated => format!(
                "a worker serving `{namespace}`/`{task_queue}` activity \
                 `{activity_type}`{node} would not accept the task before the \
                 schedule-to-start timeout"
            ),
        }
    }

    /// Read a reason back from its canonical spelling.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        [
            Self::NoQueueDeclaration,
            Self::NoLivePollers,
            Self::PollersIncompatible,
            Self::Saturated,
        ]
        .into_iter()
        .find(|reason| reason.as_str() == text)
    }
}

impl fmt::Display for QueueServiceReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Which of the two never-conflated service clocks expired.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExpiredClock {
    /// Time spent waiting for ANY compatible worker to exist.
    ServiceAvailability,
    /// Time spent between dispatch and a live compatible worker accepting the
    /// task into its intake.
    ScheduleToStart,
}

impl ExpiredClock {
    /// The canonical wire/log spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ServiceAvailability => "service_availability",
            Self::ScheduleToStart => "schedule_to_start",
        }
    }

    /// Read a clock back from its canonical spelling.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        [Self::ServiceAvailability, Self::ScheduleToStart]
            .into_iter()
            .find(|clock| clock.as_str() == text)
    }
}

impl fmt::Display for ExpiredClock {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// The pool address a dispatch could not be served at.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceAddress {
    /// Correctness/isolation boundary of the dispatch.
    pub namespace: String,
    /// Pool selector within the namespace.
    pub task_queue: String,
    /// Activity type the dispatch needs served.
    pub activity_type: String,
    /// Optional within-pool node pin carried by the dispatch.
    pub node: Option<String>,
}

/// A typed dispatch refusal: the taxonomy reason, which clock expired (if
/// any), how long the dispatch waited, and the live poller census behind the
/// verdict. Never a generic timeout, never a bare string.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerUnavailable {
    /// Taxonomy state of the queue at refusal time.
    pub reason: QueueServiceReason,
    /// The clock whose expiry produced the refusal, or `None` for a structural
    /// refusal (which needs no clock).
    pub clock: Option<ExpiredClock>,
    /// How long this dispatch waited before refusing.
    pub waited: Duration,
    /// Pool address that could not be served.
    pub address: ServiceAddress,
    /// Live poller census at refusal time.
    pub census: PoolCensus,
}

impl WorkerUnavailable {
    /// Whether the engine's retry executor should treat this failure as
    /// retryable.
    ///
    /// Structural unservability is terminal: no retry can conjure a deployment
    /// that declares the queue. Every other reason is a transient fleet
    /// condition (a worker may connect, backpressure may drain), so it carries
    /// the retryable prefix and the workflow's own declared retry policy — or
    /// its absence — decides what happens next. No retry budget is invented
    /// here.
    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        !self.reason.is_structural()
    }

    /// The failure string handed back at the engine dispatch seam.
    #[must_use]
    pub fn reason_string(&self) -> String {
        self.to_string()
    }

    /// Read the typed head back out of a dispatch failure string.
    ///
    /// Returns `None` for any failure that is not a queue-service refusal, so
    /// surfacing code can ask "is this a `WorkerUnavailable`?" without string
    /// sniffing.
    #[must_use]
    pub fn parse(failure: &str) -> Option<UnavailableSummary> {
        let body = failure
            .strip_prefix(TERMINAL_PREFIX)
            .or_else(|| failure.strip_prefix(RETRYABLE_PREFIX))?;
        let body = body.strip_prefix(UNAVAILABLE_TAG)?;
        let head = body.split(TAIL_SEPARATOR).next().unwrap_or(body);
        let mut reason = None;
        let mut clock = None;
        let mut waited = Duration::ZERO;
        let mut last_compatible_poller_age = None;
        for token in head.split_whitespace() {
            let Some((key, value)) = token.split_once('=') else {
                continue;
            };
            match key {
                "reason" => reason = QueueServiceReason::parse(value),
                "clock" => clock = ExpiredClock::parse(value),
                "waited_ms" => waited = parse_millis(value).unwrap_or_default(),
                "last_compatible_poller_age_ms" => {
                    last_compatible_poller_age = parse_millis(value);
                }
                _ => {}
            }
        }
        Some(UnavailableSummary {
            reason: reason?,
            clock,
            waited,
            last_compatible_poller_age,
        })
    }
}

impl fmt::Display for WorkerUnavailable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let prefix = if self.is_retryable() {
            RETRYABLE_PREFIX
        } else {
            TERMINAL_PREFIX
        };
        let clock = self.clock.map_or(ABSENT, ExpiredClock::as_str);
        let age = self
            .census
            .last_compatible_poller_age
            .map_or_else(|| ABSENT.to_owned(), |age| millis(age).to_string());
        write!(
            formatter,
            "{prefix}{UNAVAILABLE_TAG} reason={} clock={clock} waited_ms={} \
             last_compatible_poller_age_ms={age} workers_in_pool={} \
             workers_serving_activity={} compatible_workers={} \
             {TAIL_SEPARATOR} {}",
            self.reason,
            millis(self.waited),
            self.census.workers_in_pool,
            self.census.workers_serving_activity,
            self.census.compatible_workers,
            self.explain(),
        )
    }
}

impl WorkerUnavailable {
    /// The human tail of the failure string: what an operator has to fix.
    fn explain(&self) -> String {
        self.reason.explain(&self.address)
    }
}

/// The typed head of a queue-service refusal, recovered from its string form.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnavailableSummary {
    /// Taxonomy state recorded in the refusal.
    pub reason: QueueServiceReason,
    /// Clock that expired, if any.
    pub clock: Option<ExpiredClock>,
    /// How long the refused dispatch waited.
    pub waited: Duration,
    /// Age of the last compatible poller at refusal time, if one was ever seen.
    pub last_compatible_poller_age: Option<Duration>,
}

/// Milliseconds of a duration, saturating rather than wrapping.
pub(super) fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn parse_millis(value: &str) -> Option<Duration> {
    value.parse::<u64>().ok().map(Duration::from_millis)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn address() -> ServiceAddress {
        ServiceAddress {
            namespace: "default".to_owned(),
            task_queue: "general".to_owned(),
            activity_type: "greet".to_owned(),
            node: None,
        }
    }

    fn census() -> PoolCensus {
        PoolCensus {
            workers_in_pool: 0,
            workers_serving_activity: 0,
            compatible_workers: 0,
            last_compatible_poller_age: Some(Duration::from_millis(1_500)),
        }
    }

    #[test]
    fn structural_refusal_is_terminal_and_carries_no_clock() {
        let failure = WorkerUnavailable {
            reason: QueueServiceReason::NoQueueDeclaration,
            clock: None,
            waited: Duration::ZERO,
            address: address(),
            census: PoolCensus::default(),
        };
        assert!(!failure.is_retryable());
        let rendered = failure.reason_string();
        assert!(rendered.starts_with("terminal:"), "{rendered}");
        assert!(
            rendered.contains("reason=NO_QUEUE_DECLARATION"),
            "{rendered}"
        );
        assert!(rendered.contains("clock=none"), "{rendered}");
    }

    #[test]
    fn fleet_refusals_are_retryable_and_name_the_expired_clock() {
        let failure = WorkerUnavailable {
            reason: QueueServiceReason::NoLivePollers,
            clock: Some(ExpiredClock::ServiceAvailability),
            waited: Duration::from_secs(2),
            address: address(),
            census: census(),
        };
        assert!(failure.is_retryable());
        let rendered = failure.reason_string();
        assert!(rendered.starts_with("retryable:"), "{rendered}");
        assert!(
            rendered.contains("clock=service_availability"),
            "{rendered}"
        );
        assert!(rendered.contains("waited_ms=2000"), "{rendered}");
        assert!(
            rendered.contains("last_compatible_poller_age_ms=1500"),
            "{rendered}"
        );
    }

    #[test]
    fn the_two_clocks_render_distinctly() {
        let availability = ExpiredClock::ServiceAvailability.as_str();
        let schedule_to_start = ExpiredClock::ScheduleToStart.as_str();
        assert_ne!(availability, schedule_to_start);
        assert_eq!(
            ExpiredClock::parse(availability),
            Some(ExpiredClock::ServiceAvailability)
        );
        assert_eq!(
            ExpiredClock::parse(schedule_to_start),
            Some(ExpiredClock::ScheduleToStart)
        );
    }

    #[test]
    fn a_refusal_round_trips_back_to_its_typed_head() {
        let failure = WorkerUnavailable {
            reason: QueueServiceReason::Saturated,
            clock: Some(ExpiredClock::ScheduleToStart),
            waited: Duration::from_millis(750),
            address: ServiceAddress {
                node: Some("n1".to_owned()),
                ..address()
            },
            census: census(),
        };
        let parsed = WorkerUnavailable::parse(&failure.reason_string());
        assert_eq!(
            parsed,
            Some(UnavailableSummary {
                reason: QueueServiceReason::Saturated,
                clock: Some(ExpiredClock::ScheduleToStart),
                waited: Duration::from_millis(750),
                last_compatible_poller_age: Some(Duration::from_millis(1_500)),
            })
        );
    }

    #[test]
    fn an_ordinary_failure_is_not_read_as_a_queue_service_refusal() {
        assert_eq!(WorkerUnavailable::parse("lost:worker lost"), None);
        assert_eq!(WorkerUnavailable::parse("terminal:boom"), None);
        assert_eq!(WorkerUnavailable::parse("parked:server-draining"), None);
    }

    #[test]
    fn every_reason_round_trips_through_its_canonical_spelling() {
        for reason in [
            QueueServiceReason::NoQueueDeclaration,
            QueueServiceReason::NoLivePollers,
            QueueServiceReason::PollersIncompatible,
            QueueServiceReason::Saturated,
        ] {
            assert_eq!(QueueServiceReason::parse(reason.as_str()), Some(reason));
        }
    }
}
