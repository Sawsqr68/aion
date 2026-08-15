//! Live transcript sockets plus workflow-event discovery of successor attempts.

use std::collections::HashMap;

use aion_client::Client;
use aion_core::{ActivityEvent, ActivityId, Event};
use anyhow::{Context, Result, anyhow};
use futures::{SinkExt, StreamExt};
use reqwest::header::HeaderMap;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::client::IntoClientRequest};

use super::{Source, Target, prefix, render_event};

#[derive(Debug, Deserialize)]
struct ActivityFrame {
    activity_event: Option<ActivityEvent>,
    error: Option<Value>,
}

pub(super) async fn follow(
    stream_client: Client,
    sources: Vec<Source>,
    targets: Vec<Target>,
    heads: Vec<Option<u64>>,
    namespace: &str,
    socket_url: &str,
    headers: HeaderMap,
) -> Result<Value> {
    let (sender, mut receiver) = mpsc::channel::<Result<String>>(256);
    for (target, after_seq) in targets
        .iter()
        .cloned()
        .zip(heads)
        .filter(|(target, _)| target.superseded_by.is_none())
    {
        spawn_socket(
            target,
            namespace.to_owned(),
            socket_url.to_owned(),
            headers.clone(),
            after_seq,
            sender.clone(),
        );
    }
    for source in sources {
        let current = current_attempts(&source, &targets);
        tokio::spawn(monitor_attempts(
            stream_client.clone(),
            source,
            current,
            namespace.to_owned(),
            socket_url.to_owned(),
            headers.clone(),
            sender.clone(),
        ));
    }
    drop(sender);

    loop {
        tokio::select! {
            message = receiver.recv() => match message {
                Some(Ok(line)) => println!("{line}"),
                Some(Err(error)) => return Err(error),
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

async fn monitor_attempts(
    client: Client,
    source: Source,
    mut current: HashMap<ActivityId, u32>,
    namespace: String,
    socket_url: String,
    headers: HeaderMap,
    sender: mpsc::Sender<Result<String>>,
) {
    let mut events = client.subscribe_workflow(&source.workflow_id);
    while let Some(next) = events.next().await {
        let event = match next {
            Ok(event) => event,
            Err(error) => {
                let _ = sender
                    .send(Err(anyhow!("child workflow event socket failed: {error}")))
                    .await;
                return;
            }
        };
        if matches!(
            &event,
            Event::WorkflowStarted { run_id, .. } if run_id != &source.run_id
        ) {
            return;
        }
        let Event::ActivityStarted {
            activity_id,
            attempt,
            ..
        } = event
        else {
            continue;
        };
        let prior = current.get(&activity_id).copied();
        if prior.is_some_and(|prior| prior >= attempt) {
            continue;
        }
        if let Some(prior) = prior {
            let old = Target {
                source: source.clone(),
                activity_id: activity_id.clone(),
                attempt: prior,
                superseded_by: Some(attempt),
            };
            let _ = sender
                .send(Ok(format!(
                    "{} superseded; this stream is no longer working",
                    prefix(&old)
                )))
                .await;
        }
        current.insert(activity_id.clone(), attempt);
        spawn_socket(
            Target {
                source: source.clone(),
                activity_id,
                attempt,
                superseded_by: None,
            },
            namespace.clone(),
            socket_url.clone(),
            headers.clone(),
            None,
            sender.clone(),
        );
    }
}

fn spawn_socket(
    target: Target,
    namespace: String,
    socket_url: String,
    headers: HeaderMap,
    after_seq: Option<u64>,
    sender: mpsc::Sender<Result<String>>,
) {
    tokio::spawn(async move {
        let result = tail_socket(
            target,
            &namespace,
            &socket_url,
            headers,
            after_seq,
            sender.clone(),
        )
        .await;
        if let Err(error) = result {
            let _ = sender.send(Err(error)).await;
        }
    });
}

async fn tail_socket(
    target: Target,
    namespace: &str,
    socket_url: &str,
    headers: HeaderMap,
    after_seq: Option<u64>,
    sender: mpsc::Sender<Result<String>>,
) -> Result<()> {
    let mut request = socket_url.into_client_request()?;
    request.headers_mut().extend(headers);
    let (mut socket, _) = connect_async(request).await.with_context(|| {
        format!(
            "failed to connect transcript socket for {}",
            prefix(&target)
        )
    })?;
    let mut transcript = json!({
        "namespace": namespace,
        "workflow_id": target.source.workflow_id,
        "run_id": target.source.run_id,
        "activity_id": target.activity_id,
        "attempt": target.attempt,
    });
    if let Some(after_seq) = after_seq {
        transcript["after_seq"] = json!(after_seq);
    }
    socket
        .send(tokio_tungstenite::tungstenite::Message::Text(
            json!({"type":"subscribe", "subscription":{"transcript":transcript}})
                .to_string()
                .into(),
        ))
        .await?;
    while let Some(frame) = socket.next().await {
        let frame = frame?;
        if !frame.is_text() && !frame.is_binary() {
            continue;
        }
        let decoded: ActivityFrame = serde_json::from_slice(&frame.into_data())
            .context("invalid transcript WebSocket frame")?;
        if let Some(error) = decoded.error {
            return Err(anyhow!("transcript socket error: {error}"));
        }
        if let Some(event) = decoded.activity_event {
            sender
                .send(Ok(format!("{} {}", prefix(&target), render_event(&event))))
                .await
                .map_err(|_| anyhow!("tail output closed"))?;
        }
    }
    Ok(())
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
