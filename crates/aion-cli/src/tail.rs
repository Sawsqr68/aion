//! Interleaved parent/child retained transcript backfill and live WebSocket tail.

use std::collections::HashMap;

use aion_client::{Client, ClientAuth, ClientBuilder};
use aion_core::{ActivityEvent, ActivityEventKind, ActivityId, RunId, WorkflowId};
use aion_server::config::{DEFAULT_HTTP_ADDRESS, MAX_TRANSCRIPT_WINDOW_LIMIT};
use anyhow::{Context, Result, anyhow};
use clap::Args;
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

mod live;

/// Arguments owned by the `tail` verb rather than the binary entry point.
#[derive(Debug, Args)]
pub(crate) struct TailArgs {
    /// Parent workflow identifier. Child identifiers are derived; do not supply them.
    pub(crate) workflow_id: String,
    /// Specific parent run. Defaults to the latest recorded run.
    #[arg(long)]
    pub(crate) run_id: Option<String>,
    /// HTTP/WebSocket listener used by child and transcript APIs.
    /// May also be set with `AION_HTTP_ENDPOINT`.
    #[arg(long, env = "AION_HTTP_ENDPOINT", default_value_t = default_http_endpoint())]
    pub(crate) http_endpoint: String,
    /// Print the retained transcript snapshot and exit instead of following live sockets.
    #[arg(long)]
    pub(crate) no_follow: bool,
}

pub(crate) async fn dispatch(client: &Client, args: &TailArgs, cli: &crate::Cli) -> Result<Value> {
    let token = crate::deploy::resolve_token(cli.token.clone());
    let grpc_endpoint = cli.endpoint();
    run(
        client,
        &args.workflow_id,
        args.run_id.as_deref(),
        TailConfig {
            namespace: &cli.namespace,
            subject: &cli.subject,
            token: token.as_deref(),
            grpc_endpoint: &grpc_endpoint,
            http_endpoint: &args.http_endpoint,
            follow: !args.no_follow,
        },
    )
    .await
}

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
    is_parent: bool,
}

#[derive(Clone, Debug)]
struct PendingSource {
    workflow_id: WorkflowId,
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
    let http = HttpTailClient::new(&config)?;
    let (sources, pending) =
        discover_sources(client, &http, &workflow_id, requested_run, &config).await?;
    let mut targets = discover_targets(&http, &sources, config.namespace).await?;
    mark_superseded(&mut targets);
    let heads = backfill_and_print(&http, &targets, config.namespace).await?;
    if !config.follow {
        return Ok(Value::Null);
    }

    let mut builder = ClientBuilder::new(config.grpc_endpoint)
        .with_stream_endpoint(transcript_socket_url(config.http_endpoint)?)
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
    live::follow(stream_client, sources, pending, targets, heads).await
}

async fn discover_sources(
    client: &Client,
    http: &HttpTailClient,
    workflow_id: &WorkflowId,
    requested_run: Option<RunId>,
    config: &TailConfig<'_>,
) -> Result<(Vec<Source>, Vec<PendingSource>)> {
    let description = client
        .describe(workflow_id, requested_run.as_ref())
        .await
        .context("failed to resolve workflow run for tail")?;
    let parent_run = requested_run.unwrap_or(description.run_id);
    let mut sources = vec![Source {
        workflow_id: workflow_id.clone(),
        run_id: parent_run.clone(),
        label: format!("parent {}", short_id(workflow_id)),
        is_parent: true,
    }];
    let children: ChildrenResponse = http
        .post(
            "/workflows/children",
            &ChildrenRequest {
                namespace: config.namespace,
                workflow_id,
                run_id: &parent_run,
            },
        )
        .await?;
    let mut pending = Vec::new();
    for child in children.children {
        let name = child_label(child.display_name.as_deref(), &child.workflow_type);
        let label = format!("{name} {}", short_id(&child.workflow_id));
        if let Some(child_run) = child.run_id {
            sources.push(Source {
                label,
                workflow_id: child.workflow_id,
                run_id: child_run,
                is_parent: false,
            });
        } else {
            let action = if config.follow {
                "following for its run"
            } else {
                "no transcript exists in this snapshot"
            };
            println!("[{label}] child start recorded; run not started yet; {action}");
            pending.push(PendingSource {
                label,
                workflow_id: child.workflow_id,
            });
        }
    }
    Ok((sources, pending))
}

async fn discover_targets(
    http: &HttpTailClient,
    sources: &[Source],
    namespace: &str,
) -> Result<Vec<Target>> {
    let mut targets = Vec::new();
    for source in sources {
        let streams: StreamsResponse = http
            .post(
                "/workflows/transcripts",
                &json!({
                    "namespace": namespace,
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
    Ok(targets)
}

async fn backfill_and_print(
    http: &HttpTailClient,
    targets: &[Target],
    namespace: &str,
) -> Result<Vec<Option<u64>>> {
    let mut backfill = Vec::new();
    let mut heads = Vec::new();
    for target in targets {
        let transcript: TranscriptResponse = http
            .post(
                "/workflows/transcript",
                &json!({
                    "namespace": namespace,
                    "workflow_id": target.source.workflow_id,
                    "run_id": target.source.run_id,
                    "activity_id": target.activity_id,
                    "attempt": target.attempt,
                    "last": MAX_TRANSCRIPT_WINDOW_LIMIT,
                }),
            )
            .await
            .with_context(|| format!("failed to backfill {}", prefix(target)))?;
        let first_store_seq = transcript.events.first().and_then(|event| event.store_seq);
        if let Some(notice) = backfill_notice(
            transcript.events.len(),
            transcript.head_seq,
            first_store_seq,
        ) {
            println!("{} NOTICE: {notice}", prefix(target));
        }
        backfill.extend(
            transcript
                .events
                .into_iter()
                .map(|event| (event.emitted_at, (prefix(target), event))),
        );
        heads.push(transcript.head_seq.checked_sub(1));
    }
    order_backfill(&mut backfill);
    for (_, (prefix, event)) in backfill {
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
    Ok(heads)
}

struct HttpTailClient {
    base_url: String,
    client: reqwest::Client,
    headers: HeaderMap,
}

impl HttpTailClient {
    fn new(config: &TailConfig<'_>) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-aion-subject",
            HeaderValue::from_str(config.subject)
                .with_context(|| format!("invalid x-aion-subject value {:?}", config.subject))?,
        );
        headers.insert(
            "x-aion-namespaces",
            HeaderValue::from_str(config.namespace).with_context(|| {
                format!("invalid x-aion-namespaces value {:?}", config.namespace)
            })?,
        );
        if let Some(token) = config.token {
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {token}"))
                    .context("invalid authorization bearer token header value")?,
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

fn order_backfill<K: Ord, T>(backfill: &mut [(K, T)]) {
    backfill.sort_by(|left, right| left.0.cmp(&right.0));
}

fn backfill_notice(
    event_count: usize,
    head_seq: u64,
    first_store_seq: Option<u64>,
) -> Option<String> {
    let first_store_seq = match first_store_seq {
        Some(first_store_seq) => first_store_seq,
        None if event_count == 0 && head_seq > 0 => {
            return Some(format!(
                "no retained events; head at durable seq {head_seq}; earlier records were trimmed by retention"
            ));
        }
        None => return None,
    };
    if first_store_seq <= 1 {
        return None;
    }
    let last_store_seq = head_seq.saturating_sub(1);
    Some(format!(
        "retained backfill begins at durable seq {first_store_seq}; showing {event_count} events through durable seq {last_store_seq} (earlier records may be unavailable because of retention or the CLI's {MAX_TRANSCRIPT_WINDOW_LIMIT}-event request bound; durable seq {first_store_seq}..{last_store_seq})"
    ))
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

fn child_label(display_name: Option<&str>, workflow_type: &str) -> String {
    display_name
        .filter(|name| !name.is_empty())
        .unwrap_or(workflow_type)
        .to_owned()
}

fn short_id(workflow_id: &WorkflowId) -> String {
    workflow_id.to_string().chars().take(8).collect()
}

pub(crate) fn default_http_endpoint() -> String {
    format!("http://{DEFAULT_HTTP_ADDRESS}")
}

fn transcript_socket_url(endpoint: &str) -> Result<String> {
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

    /// The three stream shapes a transcript reader must handle, forever.
    ///
    /// `Old` is what every stream written before envelope compaction contains. `Delta` is what a
    /// stream written after it contains. `Mixed` is a single stream that spans the change — the
    /// shape that never goes away, because history is not rewritten.
    const OLD: &str = include_str!("../tests/fixtures/norn_two_turns_old.json");
    const DELTA: &str = include_str!("../tests/fixtures/norn_two_turns_delta.json");
    const MIXED: &str = include_str!("../tests/fixtures/norn_two_turns_mixed.json");

    /// The one rendering every shape must produce. Generated from the OLD fixture by the
    /// unmodified renderer, so it is the reader's real behaviour and not a description of it.
    const GOLDEN: &str = include_str!("../tests/fixtures/norn_two_turns_old.rendered.txt");

    /// Everything the CLI receives has already crossed the server's transcript serving boundary,
    /// which reconstructs compacted envelopes. This is that boundary's call, so a fixture is put
    /// through exactly what a served stream is put through.
    fn as_served(events: &[ActivityEvent]) -> (Vec<ActivityEvent>, usize) {
        let mut decoder = aion_integrations::envelope_delta::EnvelopeDeltaDecoder::new();
        let mut unresolved = 0;
        let served = events
            .iter()
            .map(|event| {
                let (resolved, report) = decoder.resolve_event(event);
                if report.is_some() {
                    unresolved += 1;
                }
                resolved
            })
            .collect();
        (served, unresolved)
    }

    fn render_all(events: &[ActivityEvent]) -> String {
        events
            .iter()
            .map(render_event)
            .collect::<Vec<_>>()
            .join("\n")
            + "\n"
    }

    /// The control arm: the old format renders to the golden, by construction.
    #[test]
    fn the_old_format_two_turn_stream_is_the_control_rendering() -> Result<()> {
        let old: Vec<ActivityEvent> = serde_json::from_str(OLD)?;
        assert_eq!(old.len(), 6, "two turns of three lifecycle frames");
        assert_eq!(render_all(&old), GOLDEN);
        Ok(())
    }

    /// Shape two: every frame after each turn's first is a delta.
    #[test]
    fn a_pure_delta_stream_renders_byte_for_byte_as_the_old_format_did() -> Result<()> {
        let delta: Vec<ActivityEvent> = serde_json::from_str(DELTA)?;
        let compacted = delta
            .iter()
            .filter(|event| {
                matches!(&event.kind,
                ActivityEventKind::Raw { value, .. }
                    if value.get(aion_integrations::envelope_delta::ENVELOPE_DELTA_KEY).is_some())
            })
            .count();
        assert_eq!(compacted, 4, "both turns keep a base and compact the rest");

        let (served, unresolved) = as_served(&delta);
        assert_eq!(unresolved, 0, "every delta has its base in this window");

        // The stronger claim first: the served EVENTS are byte-identical to the old-format ones,
        // so equivalence does not depend on the renderer being lossy in a convenient way.
        let old: Vec<ActivityEvent> = serde_json::from_str(OLD)?;
        assert_eq!(serde_json::to_vec(&served)?, serde_json::to_vec(&old)?);
        assert_eq!(render_all(&served), GOLDEN);
        Ok(())
    }

    /// Shape three: one stream carrying old-format frames and then compacted ones. This is the
    /// shape that exists forever once the emitter changes, and the reason the reader landed first.
    #[test]
    fn a_mixed_old_then_delta_stream_renders_byte_for_byte_as_the_old_format_did() -> Result<()> {
        let mixed: Vec<ActivityEvent> = serde_json::from_str(MIXED)?;
        let compacted = mixed
            .iter()
            .filter(|event| {
                matches!(&event.kind,
                ActivityEventKind::Raw { value, .. }
                    if value.get(aion_integrations::envelope_delta::ENVELOPE_DELTA_KEY).is_some())
            })
            .count();
        assert_eq!(
            compacted, 2,
            "the first turn is old-format, the second compacted"
        );

        let (served, unresolved) = as_served(&mixed);
        assert_eq!(unresolved, 0);
        let old: Vec<ActivityEvent> = serde_json::from_str(OLD)?;
        assert_eq!(serde_json::to_vec(&served)?, serde_json::to_vec(&old)?);
        assert_eq!(render_all(&served), GOLDEN);
        Ok(())
    }

    /// A window that opens inside a turn cannot reconstruct that turn, and says so rather than
    /// inventing an envelope. The frames whose base IS in the window still resolve.
    #[test]
    fn a_window_opening_after_a_base_reports_it_and_invents_nothing() -> Result<()> {
        let delta: Vec<ActivityEvent> = serde_json::from_str(DELTA)?;
        // Drop the first turn's base — the retention/resume case.
        let (served, unresolved) = as_served(&delta[1..]);
        assert_eq!(unresolved, 2, "the first turn's two deltas lose their base");

        let rendered = render_all(&served);
        assert!(
            rendered.contains("aion_envelope_delta"),
            "an unresolvable frame is shown as the delta it is: {rendered}"
        );
        assert!(
            !rendered.contains("resp_fixture_turn_01\",\"object\""),
            "the first turn's envelope must NOT be fabricated"
        );
        // The second turn is untouched by the first turn's loss. The window dropped OLD[0], so
        // the second turn's three frames are SERVED[2..5] against OLD[3..6].
        let old: Vec<ActivityEvent> = serde_json::from_str(OLD)?;
        assert_eq!(
            serde_json::to_vec(&served[2..5])?,
            serde_json::to_vec(&old[3..6])?,
            "a turn whose base is in the window resolves regardless of an earlier turn's loss"
        );
        Ok(())
    }

    /// What the change is for. The mechanism is the claim: the turn-invariant instructions block
    /// stops being persisted once per lifecycle frame and is persisted once per turn.
    ///
    /// The byte ratio this fixture shows is real but understates the deployed one — these frames
    /// carry a ~430 B instructions block inside a ~1 KB envelope, where the measured production
    /// frames are ~82 KB dominated by that block. The saving is the size of what stops repeating,
    /// so the assertion here is on the repetition count, and the ratio is asserted only as the
    /// strict reduction this fixture genuinely demonstrates.
    #[test]
    fn the_instructions_block_stops_repeating_and_the_turn_costs_strictly_less() -> Result<()> {
        const INVARIANT: &str = "Never use the live estate, a sibling worktree";
        let old: Vec<ActivityEvent> = serde_json::from_str(OLD)?;
        let delta: Vec<ActivityEvent> = serde_json::from_str(DELTA)?;

        let carrying = |events: &[ActivityEvent]| -> Result<usize> {
            let mut count = 0;
            for event in events {
                if String::from_utf8_lossy(&serde_json::to_vec(&event.kind)?).contains(INVARIANT) {
                    count += 1;
                }
            }
            Ok(count)
        };
        assert_eq!(
            carrying(&old)?,
            6,
            "the old format repeats it in every frame"
        );
        assert_eq!(carrying(&delta)?, 2, "compaction leaves one copy per turn");

        let bytes = |events: &[ActivityEvent]| -> Result<usize> {
            let mut total = 0;
            for event in events {
                total += serde_json::to_vec(&event.kind)?.len();
            }
            Ok(total)
        };
        let (old_bytes, delta_bytes) = (bytes(&old)?, bytes(&delta)?);
        assert!(
            delta_bytes < old_bytes,
            "compacted {delta_bytes} B must cost less than {old_bytes} B"
        );
        Ok(())
    }

    #[test]
    fn old_norn_response_turn_render_is_pinned_before_format_change() -> Result<()> {
        let events: Vec<ActivityEvent> = serde_json::from_str(include_str!(
            "../tests/fixtures/norn_response_turn_old.json"
        ))?;
        let rendered = events
            .iter()
            .map(render_event)
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";

        assert_eq!(
            rendered,
            include_str!("../tests/fixtures/norn_response_turn_old.rendered.txt")
        );
        Ok(())
    }

    #[test]
    fn prefix_keeps_attempt_and_supersession_visible() {
        let mut targets = vec![target(1), target(2)];
        mark_superseded(&mut targets);
        assert_eq!(targets[0].superseded_by, Some(2));
        assert!(prefix(&targets[0]).contains("attempt:1 SUPERSEDED→2"));
        assert!(prefix(&targets[1]).contains("Builder L1 24410000"));
    }

    #[test]
    fn retained_events_interleave_by_emitted_timestamp() {
        let mut backfill = [(2_u64, "child"), (1_u64, "parent")];
        order_backfill(&mut backfill);
        assert_eq!(backfill.map(|(_, label)| label), ["parent", "child"]);
    }

    #[test]
    fn child_label_falls_back_for_absent_or_empty_display_names() {
        assert_eq!(child_label(None, "builder_leg"), "builder_leg");
        assert_eq!(child_label(Some(""), "builder_leg"), "builder_leg");
        assert_eq!(child_label(Some("Builder"), "builder_leg"), "Builder");
    }

    #[test]
    fn backfill_notice_does_not_treat_head_sequence_as_an_event_count() -> Result<(), &'static str>
    {
        let notice =
            backfill_notice(37, 1_042, Some(1_005)).ok_or("a sequence gap needs notice")?;
        assert!(notice.contains("durable seq 1005..1041"));
        assert!(notice.contains("retention or the CLI's 2000-event request bound"));
        assert!(!notice.contains("37 of 1042 events"));
        assert!(!notice.contains("server's published transcript window limit"));
        Ok(())
    }

    #[test]
    fn empty_retained_backfill_reports_its_nonzero_durable_head() -> Result<(), &'static str> {
        let notice = backfill_notice(0, 42, None).ok_or("an empty retained range needs notice")?;
        assert!(notice.contains("no retained events"));
        assert!(notice.contains("head at durable seq 42"));
        assert!(notice.contains("earlier records were trimmed by retention"));
        Ok(())
    }

    fn target(attempt: u32) -> Target {
        Target {
            source: Source {
                workflow_id: WorkflowId::new(uuid::Uuid::from_u128(0x2441)),
                run_id: RunId::new(uuid::Uuid::from_u128(0x2442)),
                label: "Builder L1 24410000".to_owned(),
                is_parent: false,
            },
            activity_id: ActivityId::from_sequence_position(7),
            attempt,
            superseded_by: None,
        }
    }
}
