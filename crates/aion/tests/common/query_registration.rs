use std::time::{Duration, Instant};

use aion::{Engine, EngineError, QueryError};
use aion_core::{Payload, RunId, WorkflowId};

const RETRY_INTERVAL: Duration = Duration::from_millis(25);

pub async fn query_when_registered(
    engine: &Engine,
    workflow_id: &WorkflowId,
    run_id: &RunId,
    name: &str,
    registration_deadline: Duration,
) -> Result<Payload, EngineError> {
    let started = Instant::now();
    let deadline = started + registration_deadline;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            log_registration_failure(engine, workflow_id, run_id, name, started, "deadline").await;
            return Err(EngineError::Query(QueryError::UnknownQuery(
                name.to_owned(),
            )));
        }
        let outcome = tokio::time::timeout(
            remaining,
            engine.query(workflow_id, run_id, name, Payload::json_null()),
        )
        .await;
        match outcome {
            Ok(Err(EngineError::Query(QueryError::UnknownQuery(_)))) => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    log_registration_failure(
                        engine,
                        workflow_id,
                        run_id,
                        name,
                        started,
                        "deadline",
                    )
                    .await;
                    return Err(EngineError::Query(QueryError::UnknownQuery(
                        name.to_owned(),
                    )));
                }
                tokio::time::sleep(RETRY_INTERVAL.min(remaining)).await;
            }
            Ok(Err(EngineError::Query(QueryError::ReplyDropped))) => {
                log_registration_failure(
                    engine,
                    workflow_id,
                    run_id,
                    name,
                    started,
                    "ReplyDropped",
                )
                .await;
                return Err(EngineError::Query(QueryError::ReplyDropped));
            }
            Ok(other) => return other,
            Err(_) => {
                log_registration_failure(
                    engine,
                    workflow_id,
                    run_id,
                    name,
                    started,
                    "attempt_timeout",
                )
                .await;
                return Err(EngineError::Query(QueryError::Timeout));
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
