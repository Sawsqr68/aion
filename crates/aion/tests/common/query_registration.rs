use std::time::{Duration, Instant};

use aion::{Engine, EngineError, QueryError};
use aion_core::{Payload, RunId, WorkflowId};

const RETRY_INTERVAL: Duration = Duration::from_millis(25);

/// The outcome of [`query_when_registered`], split so the helper can never
/// manufacture a value an engine-outcome assertion accepts. `Engine` wraps
/// verdicts the engine itself produced; the other variants are the helper's
/// own harness failures, and no test arm asserting an engine behaviour can
/// match them — a hung engine turns the asserting test red, never green.
#[derive(Debug)]
pub enum QueryHelperError {
    /// A verdict the engine itself produced and the helper did not retry away.
    Engine(EngineError),
    /// The registration deadline elapsed while a query attempt was still in
    /// flight with no engine verdict at all. The in-flight future was dropped
    /// mid-attempt, so engine-side pending-reply state is indeterminate —
    /// treat as a harness failure. (The attempt is bounded by the WHOLE
    /// remaining registration budget, so the engine's own reply deadline
    /// always gets the first say.)
    DeadlineElapsedMidAttempt {
        /// Total time the helper waited across all attempts.
        waited: Duration,
    },
    /// The registration deadline was spent retrying `UnknownQuery`. Carries
    /// the engine's real last refusal, however the loop exits; `None` only
    /// when the deadline was already spent before a single attempt completed.
    RegistrationDeadline {
        /// The engine's last `UnknownQuery` refusal, verbatim.
        last_error: Option<EngineError>,
        /// How long the helper retried before giving up.
        waited: Duration,
    },
}

impl std::fmt::Display for QueryHelperError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Engine(error) => write!(f, "engine query error: {error}"),
            Self::DeadlineElapsedMidAttempt { waited } => write!(
                f,
                "query registration deadline elapsed mid-attempt after {waited:?} with no engine verdict"
            ),
            Self::RegistrationDeadline { last_error, waited } => match last_error {
                Some(error) => write!(
                    f,
                    "query registration deadline spent after {waited:?}; last engine refusal: {error}"
                ),
                None => write!(
                    f,
                    "query registration deadline spent after {waited:?} before any attempt completed"
                ),
            },
        }
    }
}

impl std::error::Error for QueryHelperError {}

pub async fn query_when_registered(
    engine: &Engine,
    workflow_id: &WorkflowId,
    run_id: &RunId,
    name: &str,
    registration_deadline: Duration,
) -> Result<Payload, QueryHelperError> {
    let started = Instant::now();
    let deadline = started + registration_deadline;
    // The engine's real refusals are carried, not discarded: the dominant
    // deadline exit is the loop-top check after a full-remainder sleep, and
    // its diagnostic must name the actual last refusal, not claim silence.
    let mut last_refusal: Option<EngineError> = None;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            let cause = match &last_refusal {
                Some(error) => format!("deadline; last refusal: {error}"),
                None => "deadline before any attempt completed".to_owned(),
            };
            log_registration_failure(engine, workflow_id, run_id, name, started, &cause).await;
            return Err(QueryHelperError::RegistrationDeadline {
                last_error: last_refusal,
                waited: started.elapsed(),
            });
        }
        let outcome = tokio::time::timeout(
            remaining,
            engine.query(workflow_id, run_id, name, Payload::json_null()),
        )
        .await;
        match outcome {
            Ok(Err(error @ EngineError::Query(QueryError::UnknownQuery(_)))) => {
                last_refusal = Some(error);
                // A zero-remaining sleep is a yield; the loop-top check owns
                // the deadline exit and carries the refusal just recorded.
                let remaining = deadline.saturating_duration_since(Instant::now());
                tokio::time::sleep(RETRY_INTERVAL.min(remaining)).await;
            }
            Ok(Ok(payload)) => return Ok(payload),
            Ok(Err(error)) => {
                // Every non-registration engine error passes through verbatim,
                // and every one is logged with history — a red must carry its
                // diagnosis, whatever the variant.
                log_registration_failure(
                    engine,
                    workflow_id,
                    run_id,
                    name,
                    started,
                    &format!("engine error: {error}"),
                )
                .await;
                return Err(QueryHelperError::Engine(error));
            }
            Err(_) => {
                log_registration_failure(
                    engine,
                    workflow_id,
                    run_id,
                    name,
                    started,
                    "deadline elapsed mid-attempt",
                )
                .await;
                return Err(QueryHelperError::DeadlineElapsedMidAttempt {
                    waited: started.elapsed(),
                });
            }
        }
    }
}

async fn log_registration_failure(
    engine: &Engine,
    workflow_id: &WorkflowId,
    run_id: &RunId,
    name: &str,
    started: Instant,
    cause: &str,
) {
    let history = engine.store().read_history(workflow_id).await;
    let residency = engine.registry().get(workflow_id, run_id).map(|handle| {
        handle.map(|handle| {
            format!(
                "residency={:?}, pid={:?}, live={}",
                handle.residency(),
                handle.pid(),
                engine.runtime().is_live(handle.pid())
            )
        })
    });
    eprintln!(
        "query_when_registered({name}) failed via {cause} after {:?}; workflow_id={workflow_id}, \
         run_id={run_id}, resolved={residency:#?}, history={history:#?}",
        started.elapsed()
    );
}
