//! Connection dead-man switch for the liminal worker transport.
//!
//! # Why this exists
//!
//! The liminal push connection is a plain TCP socket read by a background
//! reader thread. A socket that is CLOSED surfaces promptly (the reader ends,
//! `recv_timeout` reports `Disconnected`). A socket that is merely DEAD — the
//! peer process gone without a FIN, a wedged forwarder, a half-open path — does
//! not: every read times out benignly, every write lands in the kernel send
//! buffer and returns `Ok`, and both sides sit in a blocking wait believing they
//! are connected. That is exactly what was observed live on 2026-07-29: a worker
//! finished an activity and blocked on a dead socket for 28 minutes while the
//! server pushed dispatches that went nowhere.
//!
//! Silence is therefore the only observable a worker has, and silence alone is
//! not evidence — an idle-but-healthy connection is silent too. The fix is a
//! liveness EXPECTATION: the server pushes a [`LivenessPing`] on a fixed cadence
//! and the worker answers with a [`LivenessPong`] on the same connection. One
//! exchange proves both directions at once — the ping's ARRIVAL proves the
//! server→worker leg to the worker, and the pong's arrival proves the
//! worker→server leg to the server. A worker that hears nothing for longer than
//! the window the server itself declared is looking at a dead link, and says so.
//!
//! # No knobs
//!
//! The worker declares no window of its own. The server carries its operational
//! contract — the operator's `worker.heartbeat_window`, the same value the
//! server's own connection-lease expiry uses — inside every ping
//! ([`LivenessPing::silence_window_ms`]). The worker's expectation is therefore
//! the server's expectation, always, with no second configuration surface and
//! no constant to drift.
//!
//! # Arming
//!
//! [`SilenceMonitor`] starts UNARMED and stays that way until the first ping
//! arrives. Before that first ping the worker has been told no window, so it
//! asserts nothing — a connection to a server that never pings behaves exactly
//! as it did before this module existed. From the first ping onward the window
//! is live and re-armed by every subsequent ping.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// Wire liveness ping the SERVER pushes on an established liminal connection.
///
/// Field-for-field mirror of `aion-server`'s
/// `liminal_liveness::LivenessPing` (same serde field names), the same
/// cross-crate contract the dispatch/response and intervention pairs pin.
///
/// `liveness_ping` is also the DEMUX discriminator: no other frame the server
/// pushes on this channel carries that field, and a ping carries none of the
/// required fields a `DispatchRequest` or `InterventionRequest` needs, so the
/// three decode disjointly.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LivenessPing {
    /// Monotonic ping sequence within this connection, echoed on the answer.
    pub liveness_ping: u64,
    /// How long this worker may hear NOTHING on this connection before it must
    /// declare the link dead — the server's own `worker.heartbeat_window`,
    /// carried on every ping so the worker never holds a second copy of it.
    pub silence_window_ms: u64,
}

/// Wire answer the worker replies with, echoing the ping's sequence.
///
/// Field-for-field mirror of `aion-server`'s `liminal_liveness::LivenessPong`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LivenessPong {
    /// The sequence of the ping being answered, echoed verbatim.
    pub liveness_pong: u64,
}

/// The worker's dead-man switch over one liminal connection.
///
/// Records when the connection last carried ANY inbound frame (a dispatch, an
/// intervention, or a liveness ping) and, once armed by a ping, answers whether
/// the connection has been silent past the server-declared window.
///
/// Lock-free by construction (two atomics over a fixed epoch), because it is
/// read and written from the serve loop through a shared `&self` and must never
/// be able to poison a lock on the transport's hot path.
#[derive(Debug)]
pub(crate) struct SilenceMonitor {
    /// Fixed origin for the two millisecond stamps below.
    epoch: Instant,
    /// Milliseconds since `epoch` at the last inbound frame.
    last_inbound_ms: AtomicU64,
    /// Server-declared silence window in milliseconds; `0` means UNARMED (no
    /// ping has arrived yet, so the worker asserts nothing).
    window_ms: AtomicU64,
}

impl SilenceMonitor {
    /// Builds an unarmed monitor whose silence clock starts now (connection
    /// establishment is itself inbound evidence).
    pub(crate) fn new() -> Self {
        Self {
            epoch: Instant::now(),
            last_inbound_ms: AtomicU64::new(0),
            window_ms: AtomicU64::new(0),
        }
    }

    /// Milliseconds elapsed since this monitor's epoch, saturating.
    fn now_ms(&self) -> u64 {
        u64::try_from(self.epoch.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    /// Record that a frame arrived on the connection.
    pub(crate) fn record_inbound(&self) {
        self.last_inbound_ms.store(self.now_ms(), Ordering::Relaxed);
    }

    /// Arm (or re-arm) the expectation with the window the server just declared.
    ///
    /// A zero window is ignored rather than disarming: `0` is this type's
    /// "never told" sentinel, and a server that sent one would otherwise be able
    /// to silently switch the dead-man switch off.
    pub(crate) fn arm(&self, window: Duration) {
        let window_ms = u64::try_from(window.as_millis()).unwrap_or(u64::MAX);
        if window_ms > 0 {
            self.window_ms.store(window_ms, Ordering::Relaxed);
        }
    }

    /// The server-declared window, or `None` while unarmed.
    pub(crate) fn window(&self) -> Option<Duration> {
        match self.window_ms.load(Ordering::Relaxed) {
            0 => None,
            millis => Some(Duration::from_millis(millis)),
        }
    }

    /// How long the connection has been silent, when that exceeds the armed
    /// window; `None` while unarmed or while the connection is inside it.
    pub(crate) fn silent_for(&self) -> Option<Duration> {
        self.silent_for_at(self.now_ms())
    }

    /// [`Self::silent_for`] against an explicit millisecond clock, so the
    /// verdict is unit-testable without sleeping.
    fn silent_for_at(&self, now_ms: u64) -> Option<Duration> {
        let window_ms = self.window_ms.load(Ordering::Relaxed);
        if window_ms == 0 {
            return None;
        }
        let silent_ms = now_ms.saturating_sub(self.last_inbound_ms.load(Ordering::Relaxed));
        (silent_ms > window_ms).then(|| Duration::from_millis(silent_ms))
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{LivenessPing, LivenessPong, SilenceMonitor};

    /// An unarmed monitor asserts NOTHING, however long the silence: a worker
    /// that has never been told a window must behave exactly as it did before
    /// the dead-man switch existed.
    #[test]
    fn an_unarmed_monitor_never_declares_death() {
        let monitor = SilenceMonitor::new();
        assert_eq!(monitor.window(), None);
        assert_eq!(monitor.silent_for_at(u64::MAX), None);
    }

    /// THE GATE: once armed, silence beyond the server-declared window is a
    /// declared death, and silence inside it is not.
    #[test]
    fn an_armed_monitor_declares_death_only_past_the_window() {
        let monitor = SilenceMonitor::new();
        monitor.arm(Duration::from_secs(1));
        assert_eq!(monitor.window(), Some(Duration::from_secs(1)));
        assert_eq!(
            monitor.silent_for_at(1_000),
            None,
            "silence exactly at the window is still inside it"
        );
        assert_eq!(
            monitor.silent_for_at(1_001),
            Some(Duration::from_millis(1_001)),
            "silence past the window is a declared death carrying its duration"
        );
    }

    /// An inbound frame resets the clock, so a busy connection is never
    /// declared dead.
    #[test]
    fn an_inbound_frame_resets_the_silence_clock() {
        let monitor = SilenceMonitor::new();
        monitor.arm(Duration::from_millis(50));
        monitor.record_inbound();
        assert_eq!(
            monitor.silent_for(),
            None,
            "a frame that just arrived leaves no silence to measure"
        );
    }

    /// A zero window never disarms an armed monitor — `0` is the "never told"
    /// sentinel, not an operator-declared value.
    #[test]
    fn a_zero_window_cannot_disarm_the_switch() {
        let monitor = SilenceMonitor::new();
        monitor.arm(Duration::from_millis(10));
        monitor.arm(Duration::ZERO);
        assert_eq!(monitor.window(), Some(Duration::from_millis(10)));
    }

    /// The ping/pong pair round-trips with stable field names — the cross-crate
    /// wire contract with `aion-server`'s mirror of these types.
    #[test]
    fn the_liveness_pair_round_trips_through_json() -> Result<(), serde_json::Error> {
        let ping = LivenessPing {
            liveness_ping: 7,
            silence_window_ms: 30_000,
        };
        let encoded = serde_json::to_string(&ping)?;
        assert_eq!(
            encoded, r#"{"liveness_ping":7,"silence_window_ms":30000}"#,
            "the ping's wire shape is the cross-crate contract"
        );
        assert_eq!(serde_json::from_str::<LivenessPing>(&encoded)?, ping);

        let answer = LivenessPong { liveness_pong: 7 };
        let encoded = serde_json::to_string(&answer)?;
        assert_eq!(encoded, r#"{"liveness_pong":7}"#);
        assert_eq!(serde_json::from_str::<LivenessPong>(&encoded)?, answer);
        Ok(())
    }
}
