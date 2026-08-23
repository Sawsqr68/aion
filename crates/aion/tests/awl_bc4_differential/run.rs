//! Runs one package through a fresh real engine and returns its durable event
//! trail together with a DETERMINISTIC disposition.
//!
//! Two bindings make the run trustworthy:
//!
//! - Splice binding (r2 finding 1): the caller passes the EXACT entry-module
//!   bytes it expects the engine to load; `run_package` refuses to run unless
//!   the package's entry beam equals them, immediately before `load_workflows`.
//!   The reference run is bound to the reference entry bytes and the direct run
//!   to the `select()` bytes, so swapping `reference.clone()` in for the direct
//!   package fails before execution — the oracle can never compare a package to
//!   itself. `oracle_self_test.rs` exercises exactly that mis-binding.
//!
//! - Park evidence (r2 finding 3): a run is `Completed`/`Failed`/`Cancelled`
//!   the moment its terminal event is recorded, and `Parked` only on POSITIVE
//!   evidence — the engine's visibility surface reports the workflow `Running`
//!   (non-terminal) AND the history carries an outstanding `TimerStarted` THE
//!   WORKFLOW ISSUED. A stability read is kept only as a supplementary guard.
//!   Bare signal waits expose no positive registration evidence through any
//!   engine surface, so signal-parking fixtures are excluded from the oracle
//!   upstream (see `covered.rs`), never inferred from silence.
//!
//!   "The workflow issued" is load-bearing and was NOT always enforced here.
//!   The engine arms a run-scoped deadline timer (`deadline:{run_id}`) for
//!   every entry carrying a declared timeout, and this differential's reference
//!   project declares `timeout_seconds = 60` on EVERY entry
//!   (`harness::write_workflow_toml`), so every run in the corpus records a
//!   deadline `TimerStarted` at seq 2 and carries it outstanding until its
//!   terminal retires it. Counting that guard as park evidence satisfied the
//!   timer half of the predicate for every fixture from its second event
//!   onward, which left `stable >= STABLE_READS` — three unchanged 20ms history
//!   reads — as the ONLY thing separating "parked" from "slow", and a loaded
//!   box hands 60ms of history silence to any run at all. That is how fixtures
//!   with no timer anywhere in their own lowering reached the report as
//!   `disposition=Parked` on one backend and `Completed` on the other. See
//!   [`park_evidence`].

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use aion::EngineBuilder;
use aion_core::{Event, Payload, TimerId, WorkflowFilter, WorkflowId, WorkflowStatus};
use aion_package::Package;
use aion_store::{EventStore, InMemoryStore};
use serde_json::Value;
use uuid::Uuid;

use crate::dispatcher::TypedDispatcher;
use crate::fixtures::ActionResult;

/// Hard upper bound on a single run. No fixture that reaches a terminal or a
/// durable-timer park needs anywhere near this long; exceeding it is `Stuck`.
const RUN_DEADLINE: Duration = Duration::from_secs(15);

/// History poll interval.
const POLL: Duration = Duration::from_millis(20);

/// Supplementary stability guard: consecutive unchanged reads that must
/// accompany the positive park evidence before a park is accepted.
const STABLE_READS: u32 = 3;

/// The recorded disposition of a run, derived entirely from durable history plus
/// the engine's visibility status.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Disposition {
    /// A terminal `WorkflowCompleted` was recorded.
    Completed,
    /// A terminal `WorkflowFailed` was recorded.
    Failed,
    /// A terminal `WorkflowCancelled` was recorded.
    Cancelled,
    /// Positive durable-timer park: visibility `Running`, a pending
    /// `TimerStarted`, no terminal event.
    Parked,
    /// Neither terminal nor positively parked within the deadline (infra).
    Stuck,
}

/// The fixed workflow id both backends run under. Giving the reference and
/// direct runs the SAME identity is what "the same workflow, two byte
/// productions" means: a fixture that routes its own `workflow.id` into its
/// output then produces the identical result on both sides, controlling identity
/// at the experiment's inputs rather than in the normalizer (decision 11). Each
/// run uses a fresh in-memory store, so one shared id never collides.
pub fn shared_workflow_id() -> WorkflowId {
    WorkflowId::new(Uuid::from_u128(0x00bc_4d1f_0000_0000_0000_0000_0000_0001))
}

/// The outcome of one run: its durable trail, disposition, and — as positive
/// park evidence — the pending durable timers observed at quiescence.
pub struct RunOutcome {
    /// The durable event trail (full when terminal, quiescent-partial when
    /// parked).
    pub trail: Vec<Event>,
    /// The disposition derived from history + visibility.
    pub disposition: Disposition,
    /// The `Display` forms of every timer still outstanding at quiescence —
    /// the full durable-timer evidence of the run, engine-armed deadline
    /// included, recorded so a park's timer identities compare across backends.
    pub pending_timers: Vec<String>,
}

/// Runs `package`'s `workflow_type` on `input` through a fresh engine, refusing
/// unless the package's entry beam equals `expected_entry` (splice binding), and
/// returns the recorded trail plus its disposition.
///
/// # Errors
///
/// Fails when the loaded package's entry beam does not match `expected_entry`,
/// or when the engine cannot be built/started or history cannot be read — every
/// such error is infrastructure, surfaced by the caller, never a disposition.
pub async fn run_package(
    package: Package,
    workflow_type: &str,
    input: &Value,
    action_results: HashMap<String, ActionResult>,
    expected_entry: &[u8],
) -> Result<RunOutcome, Box<dyn std::error::Error>> {
    // Splice binding: the package about to be loaded MUST carry the expected
    // production's entry bytes. This ties the differential's comparison to the
    // actual bytes each engine loads.
    let entry_module = package.manifest().entry_module.clone();
    let loaded_entry = package
        .beams()
        .get(&entry_module)
        .ok_or_else(|| format!("package has no entry beam `{entry_module}`"))?;
    if loaded_entry != expected_entry {
        return Err(format!(
            "splice binding violated: the package's entry beam `{entry_module}` \
             ({} bytes) is not the expected production ({} bytes) — refusing to \
             run a package the oracle did not bind",
            loaded_entry.len(),
            expected_entry.len()
        )
        .into());
    }

    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    let engine = EngineBuilder::new()
        .in_process_activity_serving()
        .store_arc(Arc::clone(&store))
        .in_memory_visibility()
        .scheduler_threads(1)
        .activity_dispatcher(Arc::new(TypedDispatcher::new(action_results)))
        .load_workflows(package)
        .build()
        .await?;
    let handle = engine
        .start_workflow_with_id(
            workflow_type,
            Payload::from_json(input)?,
            HashMap::new(),
            String::from("default"),
            Some(shared_workflow_id()),
            None,
        )
        .await?;

    let deadline = Instant::now() + RUN_DEADLINE;
    let mut previous: Option<Vec<Event>> = None;
    let mut stable = 0u32;
    let outcome = loop {
        let history = store.read_history(handle.workflow_id()).await?;
        if let Some(disposition) = terminal_disposition(&history) {
            break outcome(history, disposition);
        }
        let evidence = park_evidence(&history);
        stable = if previous.as_ref() == Some(&history) {
            stable + 1
        } else {
            0
        };
        // Positive park evidence: an outstanding timer THE WORKFLOW STARTED AND
        // visibility reports the workflow Running, confirmed by a supplementary
        // stable read. The engine's run deadline is excluded upstream, so a run
        // that merely lost the CPU for three polls can no longer present as a
        // park: with no workflow timer outstanding this branch is unreachable
        // however long the history stays silent, and the 15s `RUN_DEADLINE`
        // below classifies a genuinely wedged run as `Stuck` (infra), loudly.
        if !evidence.is_empty()
            && stable >= STABLE_READS
            && is_running(&engine, workflow_type, handle.workflow_id()).await?
        {
            break outcome(history, Disposition::Parked);
        }
        if Instant::now() >= deadline {
            break outcome(history, Disposition::Stuck);
        }
        previous = Some(history);
        tokio::time::sleep(POLL).await;
    };
    engine.shutdown()?;
    Ok(outcome)
}

/// Assembles a run outcome, recording the pending-timer identities as evidence.
fn outcome(trail: Vec<Event>, disposition: Disposition) -> RunOutcome {
    let pending_timers = pending_timers(&trail)
        .into_iter()
        .map(|timer| timer.to_string())
        .collect();
    RunOutcome {
        trail,
        disposition,
        pending_timers,
    }
}

/// Whether the engine's visibility surface reports this workflow as `Running`
/// (non-terminal) — the positive "actively parked, not finished" signal.
async fn is_running(
    engine: &aion::Engine,
    workflow_type: &str,
    workflow_id: &WorkflowId,
) -> Result<bool, Box<dyn std::error::Error>> {
    let filter = WorkflowFilter {
        workflow_type: Some(workflow_type.to_owned()),
        status: None,
        started_after: None,
        started_before: None,
        parent: None,
    };
    let summaries = engine.list_workflows(filter).await?;
    Ok(summaries.iter().any(|summary| {
        &summary.workflow_id == workflow_id && summary.status == WorkflowStatus::Running
    }))
}

/// Returns the terminal disposition when the history carries a terminal workflow
/// event, else `None`.
fn terminal_disposition(history: &[Event]) -> Option<Disposition> {
    history.iter().find_map(|event| match event {
        Event::WorkflowCompleted { .. } => Some(Disposition::Completed),
        Event::WorkflowFailed { .. } => Some(Disposition::Failed),
        Event::WorkflowCancelled { .. } => Some(Disposition::Cancelled),
        _ => None,
    })
}

/// Every durable timer still outstanding in `history`: a `TimerStarted` whose
/// id has not since been retired, in the order the timers were started.
///
/// Retirement is last-event-wins over BOTH retirement events — `TimerFired` and
/// `TimerCancelled` — which is the same law
/// [`aion::time::outstanding_deadline_timer`] applies to the deadline it
/// recovers. Reading only `TimerFired` reported a run's engine deadline as
/// outstanding even on a `Completed` trail, because a terminal transition
/// retires the deadline by CANCELLING it (`aion::time::retire_run_deadline`),
/// never by firing it.
pub fn pending_timers(history: &[Event]) -> Vec<TimerId> {
    let mut outstanding: Vec<TimerId> = Vec::new();
    for event in history {
        match event {
            Event::TimerStarted { timer_id, .. } => {
                if !outstanding.contains(timer_id) {
                    outstanding.push(timer_id.clone());
                }
            }
            Event::TimerFired { timer_id, .. } | Event::TimerCancelled { timer_id, .. } => {
                outstanding.retain(|held| held != timer_id);
            }
            _ => {}
        }
    }
    outstanding
}

/// The outstanding timers that are positive PARK evidence: the ones the
/// WORKFLOW started.
///
/// [`pending_timers`] answers "what durable timers are live", which is the right
/// question for the run RECORD and the wrong one for the park DECISION. The
/// engine's run-scoped deadline is live from a run's second event until its
/// terminal on every fixture this differential compiles (the reference project
/// declares `timeout_seconds = 60` on every entry), so it is outstanding exactly
/// as often on a run that completes in 3ms as on one that is genuinely parked.
/// It is a guard the ENGINE armed against the run, never something the workflow
/// is waiting for, so it carries no information about parking and is excluded
/// here.
///
/// Excluding it cannot hide a park a workflow authored: the `deadline:` prefix
/// is ENGINE-MINTED ONLY — the author NIF choke point `decode_timer_id_arg`
/// refuses any author-supplied timer name under it — so a `deadline:` timer in
/// history is always the per-run deadline and never a workflow timer
/// ([`aion::time::deadline`]). The fixtures that DO park keep every timer they
/// park on, and their recorded [`RunOutcome::pending_timers`] evidence is
/// unchanged: it still carries the deadline alongside the workflow's own timer,
/// so the two backends still compare their full timer sets.
pub fn park_evidence(history: &[Event]) -> Vec<TimerId> {
    pending_timers(history)
        .into_iter()
        .filter(|timer_id| !aion::time::is_deadline_timer(timer_id))
        .collect()
}

/// The park-evidence predicate, pinned at the exact histories that produced the
/// misclassification it now refuses.
///
/// 🔴 These are the red tests for a defect the differential itself could never
/// state: the oracle read "a timer is outstanding" as "the run is parked", and
/// because the reference project declares a workflow timeout on EVERY entry,
/// that was true of every run from its second event until its terminal. The
/// gate saw it twice, in opposite directions, on two fixtures whose lowerings
/// contain no timer at all — `flagship/valid/awl_hello` and
/// `header-types/valid/max_arity_record` — and both times the only outstanding
/// timer their histories could possibly have carried was the engine's deadline.
#[cfg(test)]
mod park_evidence_tests {
    use aion_core::{Event, EventEnvelope, RunId, TimerCancelCause, TimerId, WorkflowId};

    use super::{park_evidence, pending_timers, shared_workflow_id};

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    /// An envelope at `seq`, stamped from a fixed instant so a history built
    /// here is a pure function of its sequence.
    fn envelope(seq: u64, workflow_id: &WorkflowId) -> Result<EventEnvelope, TestError> {
        Ok(EventEnvelope {
            seq,
            recorded_at: "2026-08-24T00:00:00Z".parse::<chrono::DateTime<chrono::Utc>>()?,
            workflow_id: workflow_id.clone(),
        })
    }

    /// The error a history builder can produce: a timestamp that will not parse
    /// or an identifier that will not mint. Both are impossible for the literal
    /// inputs below, and neither is allowed to become an `unwrap`.
    #[derive(Debug, thiserror::Error)]
    enum TestError {
        /// The fixed timestamp literal failed to parse.
        #[error("fixed timestamp did not parse: {0}")]
        Timestamp(#[from] chrono::ParseError),
        /// A timer identifier failed to mint.
        #[error("timer id did not mint: {0}")]
        TimerId(#[from] aion_core::IdError),
    }

    /// The history every corpus run carries after `WorkflowStarted`: the
    /// engine's run-scoped deadline, armed because the reference project
    /// declares `timeout_seconds = 60` on every entry. This is
    /// `flagship/valid/awl_hello`'s history at seq 2, verbatim in shape.
    fn deadline_only(run_id: &RunId) -> Result<Vec<Event>, TestError> {
        let workflow_id = shared_workflow_id();
        Ok(vec![Event::TimerStarted {
            envelope: envelope(2, &workflow_id)?,
            timer_id: aion::time::deadline_timer_id(run_id)?,
            fire_at: "2026-08-24T00:01:00Z".parse::<chrono::DateTime<chrono::Utc>>()?,
        }])
    }

    /// The engine's run deadline is outstanding on every run in the corpus, so
    /// it can never be the evidence that one of them parked.
    #[test]
    fn the_engine_run_deadline_is_never_park_evidence() -> TestResult {
        let run_id = RunId::new(uuid::Uuid::from_u128(0x1111));
        let history = deadline_only(&run_id)?;

        assert_eq!(
            pending_timers(&history).len(),
            1,
            "the deadline IS an outstanding durable timer and stays in the record"
        );
        assert!(
            park_evidence(&history).is_empty(),
            "a run whose only outstanding timer is the engine's deadline has NOT \
             parked — this is the exact history `flagship/valid/awl_hello` \
             carries for the whole of its run, and reading it as a park is what \
             made a slow box look like a backend divergence"
        );
        Ok(())
    }

    /// A timer the workflow started is the evidence, and it survives beside the
    /// deadline — the shape every fixture in `EXPECTED_TIMER_PARKED` records.
    #[test]
    fn a_workflow_started_timer_is_park_evidence_beside_the_deadline() -> TestResult {
        let run_id = RunId::new(uuid::Uuid::from_u128(0x2222));
        let workflow_id = shared_workflow_id();
        let mut history = deadline_only(&run_id)?;
        history.push(Event::TimerStarted {
            envelope: envelope(3, &workflow_id)?,
            timer_id: TimerId::anonymous(0),
            fire_at: "2026-08-24T00:00:30Z".parse::<chrono::DateTime<chrono::Utc>>()?,
        });

        assert_eq!(
            park_evidence(&history),
            vec![TimerId::anonymous(0)],
            "the workflow's own timer is the park evidence"
        );
        assert_eq!(
            pending_timers(&history).len(),
            2,
            "the RECORD still carries both timers, so the two backends compare \
             their full timer sets"
        );
        Ok(())
    }

    /// A cancelled timer is not outstanding. The engine retires a run's
    /// deadline by CANCELLING it at the terminal, so a `Completed` trail read
    /// through a fired-only rule reported a live deadline forever.
    #[test]
    fn a_cancelled_timer_is_not_outstanding() -> TestResult {
        let run_id = RunId::new(uuid::Uuid::from_u128(0x3333));
        let workflow_id = shared_workflow_id();
        let deadline = aion::time::deadline_timer_id(&run_id)?;
        let mut history = deadline_only(&run_id)?;
        history.push(Event::TimerCancelled {
            envelope: envelope(10, &workflow_id)?,
            timer_id: deadline,
            cause: TimerCancelCause::WorkflowIntent,
        });

        assert!(
            pending_timers(&history).is_empty(),
            "a terminal run's cancelled deadline is retired, not pending"
        );
        Ok(())
    }

    /// A fired timer is not outstanding — the rule that was already right, kept
    /// under test so the retirement rewrite cannot lose it.
    #[test]
    fn a_fired_timer_is_not_outstanding() -> TestResult {
        let workflow_id = shared_workflow_id();
        let history = vec![
            Event::TimerStarted {
                envelope: envelope(2, &workflow_id)?,
                timer_id: TimerId::anonymous(0),
                fire_at: "2026-08-24T00:00:30Z".parse::<chrono::DateTime<chrono::Utc>>()?,
            },
            Event::TimerFired {
                envelope: envelope(3, &workflow_id)?,
                timer_id: TimerId::anonymous(0),
            },
        ];

        assert!(
            pending_timers(&history).is_empty(),
            "a fired timer is retired"
        );
        assert!(
            park_evidence(&history).is_empty(),
            "and is not park evidence"
        );
        Ok(())
    }

    /// A timer restarted after being retired is outstanding again — the
    /// last-event-wins law, which a set-difference rule would get wrong in
    /// exactly one direction.
    #[test]
    fn a_timer_restarted_after_retirement_is_outstanding_again() -> TestResult {
        let workflow_id = shared_workflow_id();
        let history = vec![
            Event::TimerStarted {
                envelope: envelope(2, &workflow_id)?,
                timer_id: TimerId::anonymous(0),
                fire_at: "2026-08-24T00:00:30Z".parse::<chrono::DateTime<chrono::Utc>>()?,
            },
            Event::TimerCancelled {
                envelope: envelope(3, &workflow_id)?,
                timer_id: TimerId::anonymous(0),
                cause: TimerCancelCause::CancelTeardown,
            },
            Event::TimerStarted {
                envelope: envelope(4, &workflow_id)?,
                timer_id: TimerId::anonymous(0),
                fire_at: "2026-08-24T00:00:45Z".parse::<chrono::DateTime<chrono::Utc>>()?,
            },
        ];

        assert_eq!(
            park_evidence(&history),
            vec![TimerId::anonymous(0)],
            "a re-armed timer is live again (reopen re-arms exactly this way)"
        );
        Ok(())
    }
}
