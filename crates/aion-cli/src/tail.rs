//! Interleaved parent/child retained transcript backfill and live WebSocket tail.

use std::collections::HashMap;

use aion_client::{Client, ClientAuth, ClientBuilder};
use aion_core::{ActivityEvent, ActivityEventKind, ActivityId, Event, RunId, WorkflowId};
use anyhow::{Context, Result, anyhow};
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

mod live;

const DEFAULT_BACKFILL: u32 = 2_000;

pub(crate) struct TailConfig<'a> {
    pub(crate) namespace: &'a str,
    pub(crate) subject: &'a str,
    pub(crate) token: Option<&'a str>,
    pub(crate) grpc_endpoint: &'a str,
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
    for source in &sources {
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

    let socket_url = socket_url(config.http_endpoint)?;
    let mut builder = ClientBuilder::new(config.grpc_endpoint)
        .with_stream_endpoint(&socket_url)
        .with_namespace(config.namespace)
        .with_subject(config.subject)
        .with_authorized_namespaces([config.namespace]);
    if let Some(token) = config.token {
        builder = builder.with_auth(ClientAuth::bearer(token));
    }
    let stream_client = builder
        .build()
        .await
        .context("failed to connect tail workflow-event sockets")?;
    live::follow(
        stream_client,
        sources,
        targets,
        heads,
        config.namespace,
        &socket_url,
        http.headers.clone(),
    )
    .await
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
