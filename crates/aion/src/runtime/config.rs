//! Builder-supplied scheduler configuration for the embedded runtime.

use std::time::Duration;

/// Configuration used when constructing the embedded BEAM runtime.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RuntimeConfig {
    /// Optional scheduler thread count supplied by the engine builder.
    ///
    /// `None` is passed through to beamr so the embedded runtime applies its own
    /// runtime-aware default.
    pub thread_count: Option<usize>,

    /// Optional JIT compilation threshold supplied by the engine builder.
    ///
    /// `None` is passed through to beamr so the embedded runtime applies its own
    /// default (`beamr::jit::profiler::DEFAULT_JIT_THRESHOLD`, 1000 recorded
    /// calls per `(module, function, arity)` at the version this builds against).
    /// Nothing here invents a value.
    ///
    /// # Why this is reachable at all
    ///
    /// beamr counts calls per MFA and compiles once the count reaches the
    /// threshold. Until this field existed the engine held the scheduler and
    /// never named the threshold, so the compile point was unreachable from
    /// aion — there was no way to move the moment of compilation, and therefore
    /// no way to test whether a fault tracks it.
    ///
    /// That is what this is for: a *proportionality* instrument. Halving the
    /// threshold should halve the call count at which a compile-correlated fault
    /// appears, which predicts a number rather than explaining one. A fault that
    /// moves with the threshold is caused by compilation; a fault that does not
    /// is not, however well the arithmetic happened to fit.
    ///
    /// # 🔴 What a large value does, and what it does NOT do
    ///
    /// This is **not** a JIT off-switch, and must not be documented as one.
    /// `beamr::jit::profiler::JitProfiler::record_call` increments with
    /// `saturating_add(1)` and then compiles unless `new_count < threshold`
    /// (`jit/profiler.rs:253-262`). At `u32::MAX` the counter saturates at
    /// `u32::MAX` and the comparison stops being true, so the MFA compiles —
    /// after 4,294,967,295 recorded calls to that one MFA.
    ///
    /// Both halves of that sentence are load-bearing. It is a real horizon, not
    /// a semantic quibble: at the ~5 calls per durable wait measured on this
    /// engine that is ~859 million waits, so a large value genuinely defers
    /// compilation past any workload this engine will run. And it is genuinely
    /// not "off": the compile still happens on the other side of that horizon,
    /// so anything relying on it never happening is relying on the workload, not
    /// on the configuration. beamr's own scheduler-level disable is the answer
    /// to "off", and it is a separate beamr release; this field cannot stand in
    /// for it.
    ///
    /// The threshold is stored in an `AtomicU32` read by `current_threshold`
    /// (`jit/profiler.rs:109`, `:189-191`). beamr can retune it at runtime via
    /// `tune_threshold`, but only when an embedder drives that call, and aion
    /// never does — so a value set here is the value that governs.
    pub jit_threshold: Option<u32>,

    /// Bounded readiness and retry policy for live signal mailbox delivery.
    pub signal_delivery: SignalDeliveryConfig,

    /// Unbounded backoff ladder for durable completion retries.
    pub completion_retry: CompletionRetryConfig,

    /// Whether the durable-outbox fan-out dispatch path is enabled.
    pub outbox_enabled: bool,
}

/// Backoff ladder for the process-exit completion retry.
///
/// Separate from [`SignalDeliveryConfig`] because the two policies answer
/// different questions. Signal delivery is a *bounded* enqueue ladder: it asks
/// "has the process body materialized yet", gives up after
/// `max_enqueue_attempts`, and its millisecond ceiling is sized for a wait
/// measured in scheduler ticks. A completion retry is *unbounded* — there is no
/// attempt count at which abandoning a finished run's terminal event becomes the
/// right answer, so only the epoch close ends it — and it sleeps between
/// **durable store round-trips against a store that is already failing**.
///
/// Borrowing the first for the second is how a value chosen for one job silently
/// governs another. There is no attempt count here on purpose; what there is, is
/// an interval, and it is now stated in one place where it can be changed.
///
/// # The fields are private, and that is the whole gate
///
/// 🔴 A value of this type is a ladder that CAN CLIMB. That is not a convention
/// a caller is asked to honour, it is the only thing [`Self::try_new`] returns —
/// and `try_new` plus [`Default`] are the only ways to obtain one. An earlier
/// revision left the fields `pub` and put the check in `EngineBuilder::build`,
/// which meant the invariant held for engines built through that builder and
/// nowhere else: [`crate::RuntimeHandle::new`] is `pub` and reads
/// `completion_retry` straight out of a [`RuntimeConfig`] a caller assembled by
/// hand. There is now no second door to hold shut, because there is no way to
/// name a degenerate ladder at all.
///
/// 🔴 If this type ever gains `serde::Deserialize` — and the server-config lane
/// (`docs/design/aion-authoring/BRIEF-ENGINE-CONFIG-SERVER-SURFACE.md`) is the
/// lane that will want it — a derived impl reintroduces exactly the hole this
/// removed, because a derive writes the fields directly. Deserialization must go
/// through [`Self::try_new`], not around it.
///
/// # The closure is held by these two examples, not by the paragraph above
///
/// Privacy is what makes the constructor the only door, and nothing in a unit
/// test can observe privacy — a test that could name a degenerate ladder would
/// not compile, so it would be a NON-RUN rather than a failure. These two
/// doctests are the mechanism. They run from OUTSIDE the crate, which is the
/// embedder's position, and they are identical but for the one line under test:
/// if the fields are ever made `pub` again the second stops failing and
/// `cargo test` says so.
///
/// The control — the constructor is reachable and the snippet around it is
/// sound, so the refusal below cannot be an artefact of a broken example:
///
/// ```
/// use aion::CompletionRetryConfig;
/// use std::time::Duration;
///
/// let ladder = CompletionRetryConfig::try_new(
///     Duration::from_millis(1),
///     Duration::from_secs(30),
/// )?;
/// assert_eq!(ladder.initial_backoff(), Duration::from_millis(1));
/// # Ok::<(), aion::InvalidCompletionRetryLadder>(())
/// ```
///
/// The closure — the same snippet with the constructor call replaced by a struct
/// literal naming a ladder that cannot climb:
///
/// ```compile_fail
/// use aion::CompletionRetryConfig;
/// use std::time::Duration;
///
/// let ladder = CompletionRetryConfig {
///     initial_backoff: Duration::ZERO,
///     max_backoff: Duration::ZERO,
/// };
/// assert_eq!(ladder.initial_backoff(), Duration::ZERO);
/// ```
///
/// 🔴 The bound on that second example, measured rather than assumed. Making
/// both fields `pub` again turns it red — the observed failure is rustdoc's
/// "Test compiled successfully, but it's marked `compile_fail`" — so it does
/// hold the closure. What it cannot do is verify WHY the compile failed:
/// `compile_fail` passes when the snippet fails for any reason at all. An
/// earlier revision of this paragraph claimed a `compile_fail,E0451` annotation
/// pinned the reason; that was measured and is FALSE on this toolchain — a
/// snippet edited to fail on an unresolved import still passed with the code
/// attached, so the annotation was decoration reading as a gate and has been
/// removed rather than left to mislead.
///
/// What stands in its place is the shared shape of the two examples. They differ
/// in one expression, so a rename of the type, the constructor or the accessor
/// breaks the CONTROL as well, and the control is an ordinary doctest that must
/// compile and run. The residue is narrow and named: a hand-edit that breaks
/// only the failing copy would go unnoticed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompletionRetryConfig {
    /// Sleep before the second attempt.
    initial_backoff: Duration,

    /// Upper bound the exponential ladder settles at.
    max_backoff: Duration,
}

/// Why a proposed completion-retry ladder was refused.
///
/// Both arms describe the same outcome — an interval that cannot climb, driving
/// a retry with no attempt budget into a hot loop against a store that is by
/// definition already failing — reached through different fields. Refusing a
/// value that cannot work is not the same as inventing one that does: nothing
/// here supplies a floor, a cap or a budget.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum InvalidCompletionRetryLadder {
    /// The floor is zero, so doubling can never lift it.
    #[error(
        "initial_backoff must be non-zero: the backoff ladder doubles from it, so a zero start \
         can never climb and the unbounded retry becomes a hot loop against a failing store"
    )]
    ZeroInitialBackoff,

    /// The ceiling sits below the floor, so the ladder ratchets down.
    #[error(
        "max_backoff ({max_backoff:?}) must be at least initial_backoff ({initial_backoff:?}): \
         the ladder clamps to the ceiling as soon as doubling passes it, so a ceiling below the \
         floor makes the interval ratchet DOWN — and a zero ceiling drives it to zero on the \
         first advance, which is the same hot loop against a failing store that a zero \
         initial_backoff would cause"
    )]
    CeilingBelowFloor {
        /// The floor the caller proposed.
        initial_backoff: Duration,

        /// The ceiling the caller proposed, which is below it.
        max_backoff: Duration,
    },
}

impl Default for CompletionRetryConfig {
    /// 1 ms initial, **30 s ceiling**, and **no attempt budget** — ruled, with
    /// reasoning, on 2026-08-06.
    ///
    /// Full record and the argument behind every clause:
    /// `docs/design/aion-authoring/RULING-COMPLETION-RETRY-BOUND-2026-08-06.md`.
    /// The reasoning is reproduced here because the numbers without it read as
    /// arbitrary defaults, and it was exactly that appearance — a ladder that
    /// looked inherited and was actually introduced — that made a ruling
    /// necessary.
    ///
    /// # Where the numbers came from, and what was NOT inherited with them
    ///
    /// 1 ms / 8 ms were [`SignalDeliveryConfig`]'s values, kept verbatim when
    /// this knob was split out of that struct so no intermediate revision
    /// altered behaviour. 🔴 That is not the same as "inherited":
    /// **nothing at HEAD retries a completion at all**, so measured against
    /// the tree this lands on the ladder is a value this change INTRODUCES.
    ///
    /// And the bound was not borrowed with the numbers.
    /// `SignalDeliveryConfig` pairs its ladder with `max_enqueue_attempts: 8` —
    /// it gives up. At an 8 ms ceiling with no budget this retry was roughly
    /// **125 attempts per second, indefinitely**, against a store that is by
    /// definition unwell, all on the single-worker engine-task executor. That is
    /// a runaway which also starves everything else on that executor.
    ///
    /// The store cost of that runaway is **one to six history reads per
    /// attempt, depending on how far the attempt gets before it dies**, and both
    /// magnitudes below are derived from that range rather than from a single
    /// number. 🔴 Two earlier revisions of this paragraph gave a single number
    /// and then a one-to-three range; both were arrived at by reading the first
    /// branch and stopping. The list below is the enumeration, and the rule it
    /// exists to serve is that a cost is counted along every reachable path, not
    /// along the one that comes to mind.
    ///
    /// - **One**, on the fail-fast path. `handle_process_exit_attempt`'s first
    ///   fallible call past the epoch gate is the history read
    ///   (`lifecycle/completion.rs:382`), so an attempt that fails *there* has
    ///   issued one and only one.
    /// - **Two or three**, on the recorded-then-bookkeeping-failed path. Past
    ///   the append, `upsert_workflow_visibility` reads history
    ///   (`lifecycle/visibility.rs:25`, skipped only for `TimedOut`) and
    ///   `reconcile_terminal_registry` reads it again
    ///   (`lifecycle/completion.rs:546`).
    /// - **Four to six**, on the `ContinuedAsNew` branch — which is the ordinary
    ///   continue-as-new completion, not an edge case, so under a sick store
    ///   EVERY attempt of that run's loop pays it. Past the two above,
    ///   `start_continuation_replacement` reads history to check for an existing
    ///   replacement (`lifecycle/completion.rs:571`), then
    ///   `start_workflow_with_options` reads again in `workflow_identity`
    ///   (`lifecycle/start_admission.rs:122`, whenever the id is supplied — and
    ///   here it always is) and once more for the new run's own visibility
    ///   upsert (`lifecycle/start.rs:190` → `lifecycle/visibility.rs:25`).
    ///
    /// Every one of those can fail `StoreError::Backend`, which
    /// `completion_retry`'s `store_error_is_transient` classifies **retryable**,
    /// so the attempt spends its reads and the next one starts over from the
    /// top. That the deepest of them is reachable is not inferred: the same
    /// module's `TerminalWriterHeld` classification is justified by naming this
    /// exact chain — `start_continuation_replacement` →
    /// `start_workflow_with_options` → `registry.insert` — which sits *past* all
    /// six.
    ///
    /// 🔴 THE O5 PROOF GUARDS THE FIRST NUMBER AND CANNOT SEE ANY OF THE
    /// OTHERS. Its fixture makes the FIRST read fail, so the loop under it never
    /// reaches the post-append reads at all; its attempt-count assertion is
    /// invariant to how many reads a bookkeeping or continuation failure costs.
    /// It is a real guard on the fail-fast cost and no guard whatsoever on the
    /// rest, and an earlier revision of this paragraph offered it as a guard on
    /// both. That is also the shape
    /// `crate::store_faults::FlakyStore::fail_reads_after` exists to reach — a
    /// budget that can skip past the append — so the deeper numbers are
    /// testable; they are simply not tested by O5.
    ///
    /// So: 125 attempts per second is 125 reads per second on the fail-fast
    /// path, and **up to 750** on the continue-as-new one.
    ///
    /// # The ceiling: 30 s
    ///
    /// At 30 s a stuck retry costs two attempts a minute, and therefore **two
    /// to twelve** reads a minute by the range above, while the 1 ms start still
    /// recovers instantly from a transient blip. 30 s is the value already ruled
    /// for DR-001's R4, so it is precedent rather than a fresh invention.
    ///
    /// # No attempt budget, and that is DELIBERATE
    ///
    /// 🔴 Read this before "fixing" the missing budget. A completion is truth
    /// trying to land in the record. `SignalDeliveryConfig`'s budget is sound
    /// BECAUSE delivery is re-drivable — something upstream tries again. **A
    /// completion has no re-driver.** If this loop gives up, the completion
    /// does not land late, it never lands, and the workflow wedges silently —
    /// converting a transient store outage into permanent silent data loss.
    /// That is a worse failure than the runaway a budget would prevent, and the
    /// runaway is already cured by the ceiling. The epoch close stays the only
    /// terminator.
    ///
    /// 🔴 The condition on ever adding one: whoever wants a budget owes the
    /// design **a place to PUT the abandoned completion** first. Until that
    /// place exists, giving up is a silent fallback, and this repository does
    /// not ship those.
    ///
    /// # The price of no-budget is loudness
    ///
    /// A retry loop ruled unbounded must never be invisible, so
    /// `lifecycle::completion_retry` states itself at `warn` once the ladder
    /// reaches this ceiling, carrying attempt count, elapsed time and the last
    /// error classification. The rate is one line per ceiling-interval attempt
    /// — which is a CONSEQUENCE of the ceiling, not a throttle. There is no
    /// separate rate limiter and there must not be one; that would be an
    /// invented cap.
    ///
    /// # Still not reachable from `aion server`
    ///
    /// Grepping `aion-server` and `aion-cli` for `completion_retry` /
    /// `CompletionRetryConfig` returns nothing — nor for `signal_delivery` /
    /// [`SignalDeliveryConfig`], which is the same shape. Only an embedder
    /// calling [`crate::EngineBuilder`] can change either. That gap is a
    /// separate ruled lane
    /// (`docs/design/aion-authoring/BRIEF-ENGINE-CONFIG-SERVER-SURFACE.md`),
    /// deliberately not a rider on this change. Until it lands, **this default
    /// is the shipped answer and it cannot be answered by configuration.**
    ///
    /// # Why this does not go through [`CompletionRetryConfig::try_new`]
    ///
    /// `Default::default` cannot fail, and this module owns the private fields,
    /// so the literal is written directly rather than unwrapping a `Result` —
    /// this crate does not `unwrap` in library code, and a fallback on error
    /// would be a silent one. That makes these two literals the only values in
    /// the program that reach the ladder without passing the constructor, so
    /// `the_default_ladder_is_one_the_constructor_would_accept` puts them back
    /// through it. The gap is closed by that test, not by care.
    fn default() -> Self {
        Self {
            initial_backoff: Duration::from_millis(1),
            max_backoff: Duration::from_secs(30),
        }
    }
}

impl CompletionRetryConfig {
    /// Create an explicit completion-retry backoff ladder, or refuse one that
    /// cannot climb.
    ///
    /// This is the only public constructor, and it is fallible for the reason
    /// stated on the type: the ladder governs a retry with no attempt budget, so
    /// an interval that cannot climb is not a fast retry, it is an unterminated
    /// hot loop against a store that is already unwell.
    ///
    /// Any non-zero interval the caller chooses is accepted as given. The only
    /// pairs refused are the ones that contradict themselves.
    ///
    /// # Errors
    ///
    /// [`InvalidCompletionRetryLadder::ZeroInitialBackoff`] when the floor is
    /// zero — `0 * 2 == 0`, so the ladder can never leave it.
    ///
    /// [`InvalidCompletionRetryLadder::CeilingBelowFloor`] when the ceiling is
    /// below the floor. `sleep_backoff` assigns the ceiling whenever doubling
    /// passes it, so such a ladder ratchets DOWN as the outage lengthens, which
    /// is the opposite of what a backoff is for; a zero ceiling is the extreme
    /// of that case and reaches zero on the first advance. A ceiling EQUAL to
    /// the floor is legitimate — that is a fixed interval — and is accepted.
    pub fn try_new(
        initial_backoff: Duration,
        max_backoff: Duration,
    ) -> Result<Self, InvalidCompletionRetryLadder> {
        if initial_backoff.is_zero() {
            return Err(InvalidCompletionRetryLadder::ZeroInitialBackoff);
        }
        if max_backoff < initial_backoff {
            return Err(InvalidCompletionRetryLadder::CeilingBelowFloor {
                initial_backoff,
                max_backoff,
            });
        }
        Ok(Self {
            initial_backoff,
            max_backoff,
        })
    }

    /// Sleep before the second attempt. Never zero.
    #[must_use]
    pub const fn initial_backoff(self) -> Duration {
        self.initial_backoff
    }

    /// Upper bound the exponential ladder settles at. Never below
    /// [`Self::initial_backoff`].
    #[must_use]
    pub const fn max_backoff(self) -> Duration {
        self.max_backoff
    }
}

/// Bounded signal delivery retry policy supplied by engine configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SignalDeliveryConfig {
    /// Maximum time to wait for a just-spawned process body to materialize.
    pub ready_timeout: Duration,

    /// Maximum number of mailbox enqueue attempts after the ready gate.
    pub max_enqueue_attempts: u32,

    /// Initial sleep between failed enqueue attempts.
    pub initial_backoff: Duration,

    /// Upper bound for exponential backoff between enqueue attempts.
    pub max_backoff: Duration,
}

impl Default for SignalDeliveryConfig {
    fn default() -> Self {
        Self::new(
            Duration::from_millis(50),
            8,
            Duration::from_millis(1),
            Duration::from_millis(8),
        )
    }
}

impl SignalDeliveryConfig {
    /// Create an explicit signal delivery policy.
    #[must_use]
    pub const fn new(
        ready_timeout: Duration,
        max_enqueue_attempts: u32,
        initial_backoff: Duration,
        max_backoff: Duration,
    ) -> Self {
        Self {
            ready_timeout,
            max_enqueue_attempts,
            initial_backoff,
            max_backoff,
        }
    }

    /// Bound for draining the single cleanup worker after every registered
    /// process has been force-unblocked.
    ///
    /// A zero process-readiness bound is valid for refusal tests, but is not a
    /// useful thread-shutdown observation window. Use at least the standard
    /// readiness window and scale it by every job the worker can own (one
    /// running plus the bounded queue) rather than imposing an unrelated
    /// timeout constant.
    pub(crate) fn cleanup_shutdown_timeout(self) -> Duration {
        self.ready_timeout
            .max(Self::default().ready_timeout)
            .saturating_mul(self.max_enqueue_attempts.max(1).saturating_add(1))
    }
}

impl RuntimeConfig {
    /// Create runtime configuration from the builder-supplied scheduler count.
    #[must_use]
    pub fn new(thread_count: Option<usize>) -> Self {
        Self {
            thread_count,
            jit_threshold: None,
            signal_delivery: SignalDeliveryConfig::default(),
            completion_retry: CompletionRetryConfig::default(),
            outbox_enabled: false,
        }
    }

    /// Override the JIT compilation threshold passed to beamr's scheduler.
    ///
    /// Read [`Self::jit_threshold`] before using this. In particular a large
    /// value defers compilation past any realistic workload but does not
    /// disable the JIT, and must not be described as doing so.
    #[must_use]
    pub const fn with_jit_threshold(mut self, jit_threshold: Option<u32>) -> Self {
        self.jit_threshold = jit_threshold;
        self
    }

    /// Override the signal delivery retry policy.
    #[must_use]
    pub const fn with_signal_delivery(mut self, signal_delivery: SignalDeliveryConfig) -> Self {
        self.signal_delivery = signal_delivery;
        self
    }

    /// Override the durable completion-retry backoff ladder.
    #[must_use]
    pub const fn with_completion_retry(mut self, completion_retry: CompletionRetryConfig) -> Self {
        self.completion_retry = completion_retry;
        self
    }

    /// Override whether the durable-outbox fan-out dispatch path is enabled.
    #[must_use]
    pub const fn with_outbox_enabled(mut self, enabled: bool) -> Self {
        self.outbox_enabled = enabled;
        self
    }
}

#[cfg(test)]
mod completion_retry_ladder_tests {
    use super::{CompletionRetryConfig, InvalidCompletionRetryLadder};
    use std::time::Duration;

    /// The two literals in `Default::default` are the only values in the
    /// program that reach the ladder without passing the constructor. This puts
    /// them back through it.
    ///
    /// It is not a restatement of the constructor's arithmetic — it never names
    /// 1 ms or 30 s. It reads whatever the default holds and asks the gate
    /// whether that pair is nameable, so a future change to the ruled numbers is
    /// checked by this test rather than escaping it.
    #[test]
    fn the_default_ladder_is_one_the_constructor_would_accept() {
        let shipped = CompletionRetryConfig::default();
        let through_the_gate =
            CompletionRetryConfig::try_new(shipped.initial_backoff(), shipped.max_backoff());
        assert_eq!(
            through_the_gate,
            Ok(shipped),
            "the shipped default must be a ladder the constructor would have accepted, or \
             `Default` is a second door into the type with different rules"
        );
    }

    /// A zero floor cannot be NAMED — not "is refused later by whoever happens
    /// to check", but cannot be brought into existence.
    ///
    /// Zero is not a fast retry: `sleep_backoff` doubles from the current value
    /// and `0 * 2 == 0`, and the completion retry has no attempt budget, so the
    /// ladder can never climb and nothing else would ever stop it.
    ///
    /// The `Ok` half is the control. Both halves use the same ceiling, so the
    /// only difference between them is the floor — without it a refusal here
    /// would not be attributable to the zero at all.
    #[test]
    fn a_zero_initial_backoff_cannot_be_named() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(
            CompletionRetryConfig::try_new(Duration::ZERO, Duration::from_millis(8)),
            Err(InvalidCompletionRetryLadder::ZeroInitialBackoff),
            "a zero initial backoff must be refused by the constructor"
        );

        let accepted =
            CompletionRetryConfig::try_new(Duration::from_millis(1), Duration::from_millis(8))?;
        assert_eq!(accepted.initial_backoff(), Duration::from_millis(1));
        assert_eq!(accepted.max_backoff(), Duration::from_millis(8));
        Ok(())
    }

    /// 🔴 The CEILING arm refuses too — the arm the floor's mutation never
    /// drives.
    ///
    /// `try_new` has two conditions, and a mutation that deletes the second one
    /// leaves the test above green, because it supplies a non-zero floor and so
    /// never reaches the second condition: A MUTATION ONLY MEASURES THE BRANCH
    /// THE TEST EXECUTES.
    ///
    /// Two shapes, because they fail differently and only one is obvious. A zero
    /// ceiling drives the interval to zero on the first advance — the same hot
    /// loop against a failing store that a zero floor causes. A ceiling merely
    /// *below* the floor is subtler: the ladder clamps to the ceiling as soon as
    /// doubling passes it, so the wait ratchets DOWN as the outage lengthens.
    ///
    /// The control is a ceiling EQUAL to the floor, which is a legitimate ladder
    /// — a fixed interval — and must be accepted. Without it, a constructor that
    /// refused everything would satisfy the two assertions above.
    #[test]
    fn a_ceiling_below_the_floor_cannot_be_named() {
        let floor = Duration::from_millis(8);

        assert_eq!(
            CompletionRetryConfig::try_new(floor, Duration::ZERO),
            Err(InvalidCompletionRetryLadder::CeilingBelowFloor {
                initial_backoff: floor,
                max_backoff: Duration::ZERO,
            }),
            "a zero max_backoff must be refused — it drives the interval to zero on the first \
             advance"
        );

        assert_eq!(
            CompletionRetryConfig::try_new(floor, Duration::from_millis(4)),
            Err(InvalidCompletionRetryLadder::CeilingBelowFloor {
                initial_backoff: floor,
                max_backoff: Duration::from_millis(4),
            }),
            "a max_backoff below initial_backoff must be refused — the interval would ratchet \
             DOWN as the outage lengthens"
        );

        assert!(
            CompletionRetryConfig::try_new(floor, floor).is_ok(),
            "a ceiling equal to the floor is a fixed interval and a legitimate ladder — if this \
             is refused the two refusals above are not attributable to the ceiling being LOW"
        );
    }
}
