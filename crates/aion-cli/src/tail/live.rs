//! Live transcript subscriptions plus workflow-event discovery of attempts and fanout.

use std::collections::{HashMap, HashSet};
use std::num::NonZeroU64;

use aion_client::{Client, TranscriptStreamItem, TranscriptTarget};
use aion_core::{ActivityId, Event};
use anyhow::{Context, Result};
use futures::StreamExt;
use serde_json::Value;
use tokio::sync::mpsc;

use super::{PendingSource, Source, Target, prefix, render_event, short_id};

pub(super) async fn follow(
    client: Client,
    sources: Vec<Source>,
    pending: Vec<PendingSource>,
    targets: Vec<Target>,
    heads: Vec<Option<u64>>,
) -> Result<Value> {
    let (sender, mut receiver) = mpsc::channel::<String>(256);
    for (target, after_seq) in targets
        .iter()
        .cloned()
        .zip(heads)
        .filter(|(target, _)| target.superseded_by.is_none())
    {
        spawn_transcript(client.clone(), target, after_seq, sender.clone());
    }
    for source in sources {
        let current = current_attempts(&source, &targets);
        spawn_source_monitor(client.clone(), source, current, sender.clone());
    }
    for pending_source in pending {
        spawn_pending_monitor(client.clone(), pending_source, sender.clone());
    }
    drop(sender);

    loop {
        tokio::select! {
            message = receiver.recv() => match message {
                Some(line) => println!("{line}"),
                None => return Ok(Value::Null),
            },
            signal = tokio::signal::ctrl_c() => {
                signal.context("failed to listen for Ctrl-C")?;
                return Ok(Value::Null);
            }
        }
    }
}

fn current_attempts(source: &Source, targets: &[Target]) -> HashMap<ActivityId, u32> {
    let mut current = HashMap::new();
    for target in targets
        .iter()
        .filter(|target| target.source.workflow_id == source.workflow_id)
    {
        current
            .entry(target.activity_id.clone())
            .and_modify(|attempt: &mut u32| *attempt = (*attempt).max(target.attempt))
            .or_insert(target.attempt);
    }
    current
}

fn spawn_source_monitor(
    client: Client,
    source: Source,
    current: HashMap<ActivityId, u32>,
    sender: mpsc::Sender<String>,
) {
    tokio::spawn(async move {
        monitor_source(client, source, current, sender).await;
    });
}

async fn monitor_source(
    client: Client,
    source: Source,
    mut current: HashMap<ActivityId, u32>,
    sender: mpsc::Sender<String>,
) {
    let mut discovered_children = HashSet::new();
    let mut events = client.subscribe_workflow(&source.workflow_id);
    while let Some(next) = events.next().await {
        let event = match next {
            Ok(event) => event,
            Err(error) => {
                send_line(
                    &sender,
                    format!(
                        "[{}] NOTICE: workflow event socket failed: {error}; new attempts will not be discovered",
                        source.label
                    ),
                )
                .await;
                return;
            }
        };
        if let Event::WorkflowStarted { run_id, .. } = &event
            && run_id != &source.run_id
        {
            send_line(
                &sender,
                format!(
                    "[{}] NOTICE: continued as new as run {run_id}; this generation is no longer followed",
                    source.label
                ),
            )
            .await;
            return;
        }
        if source.is_parent
            && let Event::ChildWorkflowStarted {
                child_workflow_id,
                workflow_type,
                ..
            } = &event
            && discovered_children.insert(child_workflow_id.clone())
        {
            let pending = PendingSource {
                workflow_id: child_workflow_id.clone(),
                label: format!("{workflow_type} {}", short_id(child_workflow_id)),
            };
            send_line(
                &sender,
                format!(
                    "[{}] child start recorded; run not started yet; following for its run",
                    pending.label
                ),
            )
            .await;
            spawn_pending_monitor(client.clone(), pending, sender.clone());
            continue;
        }
        observe_attempt(&client, &source, &mut current, event, &sender).await;
    }
    send_line(
        &sender,
        format!(
            "[{}] NOTICE: workflow event socket closed; new attempts will not be discovered",
            source.label
        ),
    )
    .await;
}

fn spawn_pending_monitor(client: Client, pending: PendingSource, sender: mpsc::Sender<String>) {
    tokio::spawn(async move {
        monitor_pending(client, pending, sender).await;
    });
}

async fn monitor_pending(client: Client, pending: PendingSource, sender: mpsc::Sender<String>) {
    let mut events = client.subscribe_workflow_from(&pending.workflow_id, NonZeroU64::MIN);
    let mut source = None;
    let mut current = HashMap::new();
    while let Some(next) = events.next().await {
        let event = match next {
            Ok(event) => event,
            Err(error) => {
                send_line(
                    &sender,
                    format!(
                        "[{}] NOTICE: pending child event socket failed: {error}; its run is not followed",
                        pending.label
                    ),
                )
                .await;
                return;
            }
        };
        if let Event::WorkflowStarted { run_id, .. } = &event {
            if let Some(active) = &source {
                let active: &Source = active;
                if run_id != &active.run_id {
                    send_line(
                        &sender,
                        format!(
                            "[{}] NOTICE: continued as new as run {run_id}; this generation is no longer followed",
                            active.label
                        ),
                    )
                    .await;
                    return;
                }
            } else {
                let started = Source {
                    workflow_id: pending.workflow_id.clone(),
                    run_id: run_id.clone(),
                    label: pending.label.clone(),
                    is_parent: false,
                };
                send_line(
                    &sender,
                    format!(
                        "[{}] child run {run_id} started; following live attempts",
                        started.label
                    ),
                )
                .await;
                source = Some(started);
            }
            continue;
        }
        if let Some(source) = &source {
            observe_attempt(&client, source, &mut current, event, &sender).await;
        }
    }
    send_line(
        &sender,
        format!(
            "[{}] NOTICE: pending child event socket closed before its run could be followed",
            pending.label
        ),
    )
    .await;
}

async fn observe_attempt(
    client: &Client,
    source: &Source,
    current: &mut HashMap<ActivityId, u32>,
    event: Event,
    sender: &mpsc::Sender<String>,
) {
    let Event::ActivityStarted {
        activity_id,
        attempt,
        ..
    } = event
    else {
        return;
    };
    let prior = current.get(&activity_id).copied();
    if prior.is_some_and(|prior| prior >= attempt) {
        return;
    }
    if let Some(prior) = prior {
        let old = Target {
            source: source.clone(),
            activity_id: activity_id.clone(),
            attempt: prior,
            superseded_by: Some(attempt),
        };
        send_line(
            sender,
            format!(
                "{} superseded; this stream is no longer working",
                prefix(&old)
            ),
        )
        .await;
    }
    current.insert(activity_id.clone(), attempt);
    spawn_transcript(
        client.clone(),
        Target {
            source: source.clone(),
            activity_id,
            attempt,
            superseded_by: None,
        },
        None,
        sender.clone(),
    );
}

fn spawn_transcript(
    client: Client,
    target: Target,
    after_seq: Option<u64>,
    sender: mpsc::Sender<String>,
) {
    tokio::spawn(async move {
        follow_transcript(client, target, after_seq, sender).await;
    });
}

async fn follow_transcript(
    client: Client,
    target: Target,
    mut after_seq: Option<u64>,
    sender: mpsc::Sender<String>,
) {
    loop {
        let subscription = client
            .subscribe_transcript(TranscriptTarget {
                workflow_id: target.source.workflow_id.clone(),
                run_id: target.source.run_id.clone(),
                activity_id: target.activity_id.clone(),
                attempt: target.attempt,
                after_seq,
            })
            .await;
        let mut stream = match subscription {
            Ok(stream) => stream,
            Err(error) => {
                send_line(
                    &sender,
                    format!(
                        "{} NOTICE: transcript socket could not attach: {error}; this leg is no longer followed",
                        prefix(&target)
                    ),
                )
                .await;
                return;
            }
        };
        let mut reconnect = false;
        while let Some(item) = stream.next().await {
            match item {
                Ok(TranscriptStreamItem::Event(event)) => {
                    if let Some(store_seq) = event.store_seq {
                        after_seq = Some(after_seq.map_or(store_seq, |seen| seen.max(store_seq)));
                    }
                    send_line(
                        &sender,
                        format!("{} {}", prefix(&target), render_event(&event)),
                    )
                    .await;
                }
                Ok(TranscriptStreamItem::Lagged { skipped }) => {
                    send_line(
                        &sender,
                        format!(
                            "{} NOTICE: transcript lagged by {skipped} live events; recovering this leg from its last durable sequence",
                            prefix(&target)
                        ),
                    )
                    .await;
                    reconnect = true;
                    break;
                }
                Err(error) => {
                    send_line(
                        &sender,
                        format!(
                            "{} NOTICE: transcript frame/socket failed: {error}; this leg is no longer followed",
                            prefix(&target)
                        ),
                    )
                    .await;
                    return;
                }
            }
        }
        if !reconnect {
            send_line(
                &sender,
                format!(
                    "{} NOTICE: transcript socket closed; this leg is no longer followed",
                    prefix(&target)
                ),
            )
            .await;
            return;
        }
    }
}

async fn send_line(sender: &mpsc::Sender<String>, line: String) {
    if let Err(error) = sender.send(line).await {
        eprintln!(
            "tail output channel closed before this operator notice could be delivered: {}",
            error.0
        );
    }
}

#[cfg(test)]
mod tests {
    use aion_core::{RunId, WorkflowId};

    use super::*;

    #[test]
    fn current_attempts_uses_the_highest_attempt_per_activity() {
        let source = Source {
            workflow_id: WorkflowId::new(uuid::Uuid::from_u128(1)),
            run_id: RunId::new(uuid::Uuid::from_u128(2)),
            label: "leg".to_owned(),
            is_parent: false,
        };
        let activity_id = ActivityId::from_sequence_position(7);
        let targets = [1, 2].map(|attempt| Target {
            source: source.clone(),
            activity_id: activity_id.clone(),
            attempt,
            superseded_by: None,
        });
        assert_eq!(
            current_attempts(&source, &targets).get(&activity_id),
            Some(&2)
        );
    }
}
