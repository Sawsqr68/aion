//! Interleaved parent/child retained transcript backfill and live WebSocket tail.

use std::collections::HashMap;

use aion_client::Client;
use aion_core::{ActivityEvent, ActivityEventKind, ActivityId, Event, RunId, WorkflowId};
use anyhow::{Context, Result, anyhow};
use futures::{SinkExt, StreamExt};
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::client::IntoClientRequest};

const DEFAULT_BACKFILL: u32 = 2_000;

pub(crate) struct TailConfig<'a> {
    pub(crate) namespace: &'a str,
    pub(crate) subject: &'a str,
    pub(crate) token: Option<&'a str>,
    pub(crate) http_endpoint: &'a str,
    pub(crate) follow: bool,
}

#[derive(Clone, Debug)]
struct Source {
    workflow_id: WorkflowId,
    run_id: RunId,
    label: String,
}

#[derive(Clone, Debug)]
struct Target {
    source: Source,
    activity_id: ActivityId,
    attempt: u32,
    superseded_by: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct ChildrenResponse {
    children: Vec<ChildEntry>,
}

#[derive(Debug, Deserialize)]
struct ChildEntry {
    workflow_id: WorkflowId,
    run_id: Option<RunId>,
    workflow_type: String,
    display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StreamsResponse {
    streams: Vec<StreamEntry>,
}

#[derive(Debug, Deserialize)]
struct StreamEntry {
    activity_id: ActivityId,
    attempt: u32,
}

#[derive(Debug, Deserialize)]
struct TranscriptResponse {
    events: Vec<ActivityEvent>,
    head_seq: u64,
}

#[derive(Debug, Deserialize)]
struct ActivityFrame {
    activity_event: Option<ActivityEvent>,
    error: Option<Value>,
}

#[derive(Debug, Serialize)]
struct ChildrenRequest<'a> {
    namespace: &'a str,
    workflow_id: &'a WorkflowId,
    run_id: &'a RunId,
}

pub(crate) async fn run(
    client: &Client,
    workflow_id: &str,
    run_id: Option<&str>,
    config: TailConfig<'_>,
) -> Result<Value> {
    let workflow_id = parse_workflow_id(workflow_id)?;
    let requested_run = run_id.map(parse_run_id).transpose()?;
    let description = client
        .describe(&workflow_id, requested_run.as_ref())
        .await
        .context("failed to resolve workflow run for tail")?;
    let parent_run = requested_run
        .or_else(|| latest_run(&description.history))
        .ok_or_else(|| anyhow!("workflow history contains no WorkflowStarted run"))?;
    let http = HttpTailClient::new(&config)?;
    let mut sources = vec![Source {
        workflow_id: workflow_id.clone(),
        run_id: parent_run.clone(),
        label: format!("parent {}", short_id(&workflow_id)),
    }];
    let children: ChildrenResponse = http
        .post(
            "/workflows/children",
            &ChildrenRequest {
                namespace: config.namespace,
                workflow_id: &workflow_id,
                run_id: &parent_run,
            },
        )
        .await?;
    for child in children.children {
        if let Some(child_run) = child.run_id {
            let name = child.display_name.unwrap_or(child.workflow_type);
            sources.push(Source {
                label: format!("{name} {}", short_id(&child.workflow_id)),
                workflow_id: child.workflow_id,
                run_id: child_run,
            });
        }
    }

    let mut targets = Vec::new();
    for source in sources {
        let streams: StreamsResponse = http
            .post(
                "/workflows/transcripts",
                &json!({
                    "namespace": config.namespace,
                    "workflow_id": source.workflow_id,
                    "run_id": source.run_id,
                }),
            )
            .await
            .with_context(|| {
                format!(
                    "failed to enumerate transcript streams for {}",
                    source.label
                )
            })?;
        targets.extend(streams.streams.into_iter().map(|stream| Target {
            source: source.clone(),
            activity_id: stream.activity_id,
            attempt: stream.attempt,
            superseded_by: None,
        }));
    }
    mark_superseded(&mut targets);

    let mut backfill = Vec::new();
    let mut heads = Vec::new();
    for target in &targets {
        let transcript: TranscriptResponse = http
            .post(
                "/workflows/transcript",
                &json!({
                    "namespace": config.namespace,
                    "workflow_id": target.source.workflow_id,
                    "run_id": target.source.run_id,
                    "activity_id": target.activity_id,
                    "attempt": target.attempt,
                    "last": DEFAULT_BACKFILL,
                }),
            )
            .await
            .with_context(|| format!("failed to backfill {}", prefix(target)))?;
        backfill.extend(
            transcript
                .events
                .into_iter()
                .map(|event| (event.emitted_at, prefix(target), event)),
        );
        heads.push(transcript.head_seq.checked_sub(1));
    }
    backfill.sort_by_key(|(emitted_at, _, _)| *emitted_at);
    for (_, prefix, event) in backfill {
        println!("{prefix} {}", render_event(&event));
    }
    for target in targets
        .iter()
        .filter(|target| target.superseded_by.is_some())
    {
        println!(
            "{} superseded; this stream is no longer working",
            prefix(target)
        );
    }
    if !config.follow {
        return Ok(Value::Null);
    }

    let (sender, mut receiver) = mpsc::channel::<Result<String>>(256);
    for (target, after_seq) in targets
        .into_iter()
        .zip(heads)
        .filter(|(target, _)| target.superseded_by.is_none())
    {
        let sender = sender.clone();
        let socket_url = socket_url(config.http_endpoint)?;
        let headers = http.headers.clone();
        let namespace = config.namespace.to_owned();
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

struct HttpTailClient {
    base_url: String,
    client: reqwest::Client,
    headers: HeaderMap,
}

impl HttpTailClient {
    fn new(config: &TailConfig<'_>) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert("x-aion-subject", HeaderValue::from_str(config.subject)?);
        headers.insert(
            "x-aion-namespaces",
            HeaderValue::from_str(config.namespace)?,
        );
        if let Some(token) = config.token {
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {token}"))?,
            );
        }
        Ok(Self {
            base_url: config.http_endpoint.trim_end_matches('/').to_owned(),
            client: reqwest::Client::new(),
            headers,
        })
    }

    async fn post<T, B>(&self, path: &str, body: &B) -> Result<T>
    where
        T: for<'de> Deserialize<'de>,
        B: Serialize + ?Sized,
    {
        self.client
            .post(format!("{}{path}", self.base_url))
            .headers(self.headers.clone())
            .json(body)
            .send()
            .await
            .context("Aion HTTP request failed")?
            .error_for_status()
            .context("Aion HTTP endpoint refused tail request")?
            .json()
            .await
            .context("Aion HTTP response was invalid")
    }
}

fn latest_run(history: &[Event]) -> Option<RunId> {
    history.iter().rev().find_map(|event| match event {
        Event::WorkflowStarted { run_id, .. } => Some(run_id.clone()),
        _ => None,
    })
}

fn mark_superseded(targets: &mut [Target]) {
    let latest: HashMap<_, _> = targets.iter().fold(HashMap::new(), |mut latest, target| {
        let key = (
            target.source.workflow_id.clone(),
            target.activity_id.clone(),
        );
        latest
            .entry(key)
            .and_modify(|attempt: &mut u32| *attempt = (*attempt).max(target.attempt))
            .or_insert(target.attempt);
        latest
    });
    for target in targets {
        let key = (
            target.source.workflow_id.clone(),
            target.activity_id.clone(),
        );
        target.superseded_by = latest
            .get(&key)
            .copied()
            .filter(|latest| *latest > target.attempt);
    }
}

fn prefix(target: &Target) -> String {
    let superseded = target
        .superseded_by
        .map_or(String::new(), |attempt| format!(" SUPERSEDED→{attempt}"));
    format!(
        "[{} activity:{} attempt:{}{}]",
        target.source.label, target.activity_id, target.attempt, superseded
    )
}

fn render_event(event: &ActivityEvent) -> String {
    match &event.kind {
        ActivityEventKind::Message { role, text } => format!("{role:?}: {text}"),
        ActivityEventKind::Delta { text_fragment, .. } => text_fragment.clone(),
        ActivityEventKind::ToolCall { tool, input, .. } => format!("tool {tool} {input}"),
        ActivityEventKind::ToolResult {
            output, is_error, ..
        } => format!(
            "tool result{} {output}",
            if *is_error { " ERROR" } else { "" }
        ),
        ActivityEventKind::Progress { detail } => format!("progress {detail:?}"),
        ActivityEventKind::Stop { reason } => format!("stopped {reason:?}"),
        ActivityEventKind::Raw { source, value } => format!("{source}: {value}"),
    }
}

fn short_id(workflow_id: &WorkflowId) -> String {
    workflow_id.to_string().chars().take(8).collect()
}

fn socket_url(endpoint: &str) -> Result<String> {
    let base = endpoint.trim_end_matches('/');
    if let Some(rest) = base.strip_prefix("https://") {
        Ok(format!("wss://{rest}/events/stream"))
    } else if let Some(rest) = base.strip_prefix("http://") {
        Ok(format!("ws://{rest}/events/stream"))
    } else {
        Err(anyhow!(
            "--http-endpoint must begin with http:// or https://"
        ))
    }
}

fn parse_workflow_id(value: &str) -> Result<WorkflowId> {
    Ok(WorkflowId::new(
        uuid::Uuid::parse_str(value).context("workflow-id must be a UUID")?,
    ))
}

fn parse_run_id(value: &str) -> Result<RunId> {
    Ok(RunId::new(
        uuid::Uuid::parse_str(value).context("--run-id must be a UUID")?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefix_keeps_attempt_and_supersession_visible() {
        let mut targets = vec![target(1), target(2)];
        mark_superseded(&mut targets);
        assert_eq!(targets[0].superseded_by, Some(2));
        assert!(prefix(&targets[0]).contains("attempt:1 SUPERSEDED→2"));
        assert!(prefix(&targets[1]).contains("Builder L1 24410000"));
    }

    fn target(attempt: u32) -> Target {
        Target {
            source: Source {
                workflow_id: WorkflowId::new(uuid::Uuid::from_u128(0x2441)),
                run_id: RunId::new(uuid::Uuid::from_u128(0x2442)),
                label: "Builder L1 24410000".to_owned(),
            },
            activity_id: ActivityId::from_sequence_position(7),
            attempt,
            superseded_by: None,
        }
    }
}
