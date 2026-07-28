//! Hand-off of a dispatched task to a live compatible worker, bounded by the
//! schedule-to-start clock.
//!
//! This is where [`QueueServiceReason::Saturated`](super::QueueServiceReason::Saturated)
//! is really emitted, and the only place it can be: a saturated queue is by
//! definition one with compatible pollers connected, so selection SUCCEEDS and
//! the unservability shows up at intake. Without a configured
//! schedule-to-start timeout the behaviour is exactly what it was — one
//! `try_send`, and a full or closed intake fails the dispatch immediately —
//! because inventing a wait would be inventing a timeout.

use std::time::{Duration, Instant};

use tokio::sync::mpsc::error::TrySendError;

use crate::worker::registry::{WorkerMessage, WorkerTaskSender};

/// Cadence at which a saturated intake is re-offered the task.
///
/// This is the seam's PRE-EXISTING park cadence (the runtime-less branch of the
/// selection wait has slept exactly this long between re-selections since the
/// bridge was written), named once and reused rather than a second, newly
/// invented interval. Each sleep is clamped to the remaining schedule-to-start
/// budget, so the clock — not the cadence — decides when the refusal lands.
pub const PARK_POLL_INTERVAL: Duration = Duration::from_millis(500);

/// Why a task could not be handed to the selected worker.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeliveryRefusal {
    /// A compatible worker was live and its intake stayed full for the whole
    /// schedule-to-start budget: `SATURATED`.
    Saturated {
        /// How long the hand-off was attempted before giving up.
        waited: Duration,
    },
    /// The intake was full and no schedule-to-start clock is configured, so the
    /// dispatch fails at once exactly as it always has.
    Full,
    /// The worker's transport is gone.
    Closed,
    /// The server stopped accepting work (drain) while the hand-off retried.
    NotAccepting {
        /// The drain gate's own refusal reason, passed through untouched.
        reason: String,
    },
}

/// Offer `message` to `sender` until it is accepted or the schedule-to-start
/// clock expires.
///
/// `accepting` is re-consulted before every re-offer so a drain beginning
/// mid-hand-off still fails fast with the drain's own reason — the existing
/// drain contract is not weakened by the retry window.
///
/// # Errors
///
/// Returns the typed [`DeliveryRefusal`] describing why the hand-off failed.
pub fn deliver_within_schedule_to_start(
    sender: &WorkerTaskSender,
    message: WorkerMessage,
    schedule_to_start: Option<Duration>,
    accepting: &mut dyn FnMut() -> Result<(), String>,
) -> Result<(), DeliveryRefusal> {
    let started_at = Instant::now();
    let mut pending = message;
    loop {
        match sender.try_send(pending) {
            Ok(()) => return Ok(()),
            Err(TrySendError::Closed(_)) => return Err(DeliveryRefusal::Closed),
            Err(TrySendError::Full(returned)) => {
                let Some(budget) = schedule_to_start else {
                    return Err(DeliveryRefusal::Full);
                };
                let waited = started_at.elapsed();
                let Some(remaining) = budget.checked_sub(waited) else {
                    return Err(DeliveryRefusal::Saturated { waited });
                };
                if remaining.is_zero() {
                    return Err(DeliveryRefusal::Saturated { waited });
                }
                if let Err(reason) = accepting() {
                    return Err(DeliveryRefusal::NotAccepting { reason });
                }
                std::thread::sleep(remaining.min(PARK_POLL_INTERVAL));
                pending = returned;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task() -> WorkerMessage {
        WorkerMessage::ActivityTask(Box::default())
    }

    fn always_accepting() -> impl FnMut() -> Result<(), String> {
        || Ok(())
    }

    #[test]
    fn a_free_intake_accepts_the_task_immediately() -> Result<(), String> {
        let (sender, mut receiver) = tokio::sync::mpsc::channel(1);
        let mut accepting = always_accepting();
        deliver_within_schedule_to_start(&sender, task(), None, &mut accepting)
            .map_err(|refusal| format!("unexpected refusal: {refusal:?}"))?;
        receiver
            .try_recv()
            .map_err(|error| format!("task never reached the worker: {error}"))?;
        Ok(())
    }

    #[test]
    fn a_full_intake_with_no_clock_fails_at_once() {
        let (sender, _receiver) = tokio::sync::mpsc::channel(1);
        let mut accepting = always_accepting();
        assert!(
            sender.try_send(task()).is_ok(),
            "the fixture must fill the single intake slot"
        );
        let started = Instant::now();
        let refusal = deliver_within_schedule_to_start(&sender, task(), None, &mut accepting);
        assert_eq!(refusal, Err(DeliveryRefusal::Full));
        assert!(
            started.elapsed() < PARK_POLL_INTERVAL,
            "the unclocked path must not wait"
        );
    }

    #[test]
    fn a_closed_intake_is_never_reported_as_saturation() {
        let (sender, receiver) = tokio::sync::mpsc::channel(1);
        drop(receiver);
        let mut accepting = always_accepting();
        let refusal = deliver_within_schedule_to_start(
            &sender,
            task(),
            Some(Duration::from_secs(30)),
            &mut accepting,
        );
        assert_eq!(refusal, Err(DeliveryRefusal::Closed));
    }

    /// SATURATED really fires: a live, compatible, connected worker whose
    /// intake refuses the task for the whole schedule-to-start budget.
    #[test]
    fn an_intake_that_stays_full_past_the_clock_is_saturated() {
        let (sender, _receiver) = tokio::sync::mpsc::channel(1);
        let mut accepting = always_accepting();
        assert!(sender.try_send(task()).is_ok(), "fixture must fill intake");
        let budget = Duration::from_millis(300);
        let started = Instant::now();
        let refusal =
            deliver_within_schedule_to_start(&sender, task(), Some(budget), &mut accepting);
        let elapsed = started.elapsed();
        assert!(
            matches!(refusal, Err(DeliveryRefusal::Saturated { waited }) if waited >= budget),
            "expected saturation after at least {budget:?}, got {refusal:?}"
        );
        assert!(
            elapsed < Duration::from_secs(5),
            "saturation must be bounded by the clock, took {elapsed:?}"
        );
    }

    /// An intake that drains within the budget is served, not refused: the
    /// clock exists to bound the wait, not to shorten a recoverable one.
    #[test]
    fn an_intake_that_frees_up_within_the_clock_is_served() -> Result<(), String> {
        let (sender, mut receiver) = tokio::sync::mpsc::channel(1);
        let mut accepting = always_accepting();
        sender
            .try_send(task())
            .map_err(|error| format!("fixture fill failed: {error}"))?;
        let drainer = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            let taken = receiver.try_recv().is_ok();
            (receiver, taken)
        });
        let outcome = deliver_within_schedule_to_start(
            &sender,
            task(),
            Some(Duration::from_secs(5)),
            &mut accepting,
        );
        let (_receiver, taken) = drainer.join().map_err(|_| "drainer thread panicked")?;
        assert!(taken, "the fixture drainer never freed the intake slot");
        assert_eq!(outcome, Ok(()));
        Ok(())
    }

    /// A drain starting mid-hand-off still wins, with the drain's own reason.
    #[test]
    fn a_drain_during_the_retry_window_fails_with_the_drain_reason() {
        let (sender, _receiver) = tokio::sync::mpsc::channel(1);
        assert!(sender.try_send(task()).is_ok(), "fixture must fill intake");
        let mut accepting = || Err("server draining".to_owned());
        let refusal = deliver_within_schedule_to_start(
            &sender,
            task(),
            Some(Duration::from_secs(30)),
            &mut accepting,
        );
        assert_eq!(
            refusal,
            Err(DeliveryRefusal::NotAccepting {
                reason: "server draining".to_owned()
            })
        );
    }
}
