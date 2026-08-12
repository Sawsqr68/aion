# aion-client

Rust caller SDK for connecting to an Aion server deployment and operating Aion workflows. The crate exposes connect plus the seven workflow operations: `start`, `signal`, `query`, `cancel`, `list`, `describe`, and `subscribe`.

## Key public types

- `Client`, `ClientBuilder`, `ClientAuth`, and `TlsOptions` configure gRPC connections.
- `WorkflowHandle` scopes operations to a started workflow run.
- `StartOptions`, `StartOutcome`, `DisplayNameNotApplied`, `WorkflowDescription`, and `ListPage` model workflow operations.
- `EventStream`, `ResumingEventStream`, and `SubscribeTarget` stream events.
- `to_payload` and `from_payload` bridge typed values with `aion-core` payloads.

## Install

```toml
[dependencies]
aion-client = "0.4.0"
```

## Server prerequisite

Run an Aion server (`aion server --config <file>`) that implements the AW workflow API. The runnable example uses the AL-007 server fixture defaults:

```sh
export AION_SERVER_URL=http://127.0.0.1:50051
export AION_AUTH_TOKEN=dev-token # optional
cargo run -p aion-client --example seven_operations
```

See [`examples/seven_operations.rs`](examples/seven_operations.rs) for a complete program covering all seven operations.

## Connect

```rust
use aion_client::{ClientAuth, ClientBuilder};

let mut builder = ClientBuilder::new(std::env::var("AION_SERVER_URL")?)
    .with_namespace("conformance");
if let Ok(token) = std::env::var("AION_AUTH_TOKEN") {
    builder = builder.with_auth(ClientAuth::bearer(token));
}
let client = builder.build().await?;
```

## start

`start_typed` serializes a typed value to JSON and returns a `StartOutcome`, not a bare handle: `.handle` is the `WorkflowHandle` carrying the workflow and run IDs, and `.display_name_not_applied` reports a display name an idempotent replay could not apply (see below). `StartOptions::idempotency_key` makes caller retries safe: the same request returns the original handle and conflicting reuse returns `ClientError::AlreadyExists`.

```rust
use aion_client::StartOptions;
use serde::Serialize;

#[derive(Serialize)]
struct StartInput {
    message: &'static str,
    counter: u32,
}

let handle = client
    .start_typed(
        "conformance.echo",
        &StartInput { message: "hello", counter: 1 },
        StartOptions {
            idempotency_key: Some("readme-seven-operations".to_owned()),
            ..StartOptions::default()
        },
    )
    .await?
    .handle;
```

`routing_key` and `task_queue` are the two targeting dimensions. `routing_key` steers the start to the owner of that key's shard, forwarding there when the node you dialled is not the owner. `task_queue` sets the queue this workflow's activities default to, recorded durably on the start so it survives replay and failover. Both are part of the idempotency identity: reusing a key with a different routing key or task queue returns `ClientError::AlreadyExists` instead of handing back the first workflow.

`display_name` is the operator-facing name for the workflow this start creates, recorded durably on the start as the `aion.display_name` search attribute. It is a **label over the UUID identity, never an address**: no SDK call resolves a workflow by name, no SDK helper composes name-filter → take-first → act, and two workflows may wear the same name. Omit it to start unnamed. It is trimmed before it is sent, because the server trims it before recording it; a present-but-blank name is refused with `ClientError::InvalidArgument` before any round trip, because the server refuses one too rather than reading it as unnamed — absence is the only way to say "no name".

You name the run you are starting, but the label **reads back per workflow**: the recorded attribute carries no run id, so every reader folds it over the whole history, last write wins. A continue-as-new successor of this run inherits the name, and describing any run of the workflow returns whatever name was recorded most recently.

A display name is deliberately **not** part of the idempotency fingerprint. The key answers "is this the same act?", and a label carries no identity, so two starts differing only in name are the same act and dedupe to one run rather than returning `AlreadyExists`. The name that did not get applied is reported rather than dropped: the replay returns the existing run wearing its existing name, and `display_name_not_applied` carries both sides. The report fires only when this call **asked for** a name the standing run does not wear — whether that run wears a different name or none at all. A call that asked for no name dropped nothing, so it is told nothing even when the standing run has a name of its own.

```rust
let outcome = client
    .start_typed(
        "reports.monthly",
        &StartInput { message: "2026-07", counter: 1 },
        StartOptions {
            idempotency_key: Some("reports-2026-07".to_owned()),
            display_name: Some("July invoice run".to_owned()),
            routing_key: Some("tenant-42".to_owned()),
            task_queue: Some("reports".to_owned()),
            ..StartOptions::default()
        },
    )
    .await?;
if let Some(not_applied) = &outcome.display_name_not_applied {
    eprintln!(
        "{:?} was not applied; the run keeps {:?}",
        not_applied.requested, not_applied.standing
    );
}
let handle = outcome.handle;
```

Renaming an existing run is a console/HTTP action (`POST /workflows/rename`), not an SDK or CLI operation in this release.

## signal

```rust
#[derive(Serialize)]
struct SignalInput {
    value: &'static str,
}

handle
    .signal_typed("record", &SignalInput { value: "signal-observed" })
    .await?;
```

## query

Query results can be decoded into any `serde::Deserialize` type, and query arguments are carried to the workflow's registered handler. Pass the typed arguments the handler expects; `&()` serializes to the JSON `null` document, which is the canonical "no arguments" request a no-argument query answers.

```rust
use serde::Deserialize;
use std::time::Duration;

#[derive(Deserialize)]
struct EchoState {
    last_signal: Option<String>,
}

let state: EchoState = handle.query_typed("state", &(), Duration::from_secs(5)).await?;
```

## list

```rust
use aion_client::ListPage;
use aion_core::WorkflowFilter;

let summaries = client
    .list(
        &WorkflowFilter {
            workflow_type: Some("conformance.echo".to_owned()),
            ..WorkflowFilter::default()
        },
        ListPage::default(),
    )
    .await?;
```

## describe

`description.summary.display_name` is the workflow's current operator-facing name, or `None` when it is unnamed. It is projected off the recorded `aion.display_name` search attribute folded over the whole history, so it is the most recently recorded name for the workflow — not necessarily the one this run was started with.

```rust
let description = handle.describe().await?;
println!("history events: {}", description.history.len());
println!(
    "display name: {}",
    description.summary.display_name.as_deref().unwrap_or("(unnamed)")
);
```

## cancel

Cancellation is a cooperative request: success means the server accepted it, not that the workflow has already reached a terminal state.

```rust
handle.cancel("caller requested cancellation").await?;
```

## subscribe

`subscribe` returns a `Stream<Item = Result<Event, ClientError>>`. Transient disconnects are retried with the next per-workflow sequence number so delivered events are gap-free and duplicate-free; terminal failures are yielded as stream errors.

```rust
use futures::StreamExt;

let mut events = handle.subscribe();
while let Some(event) = events.next().await {
    let event = event?;
    println!("event seq={}", event.seq());
    break;
}
```

## Typed and raw payloads

Typed helpers (`start_typed`, `signal_typed`, `query_typed`, `to_payload`, `from_payload`) use JSON by default. For pre-serialized or non-JSON data, use the raw `aion_core::Payload` escape hatch with the raw operation variants:

```rust
use aion_core::{ContentType, Payload};

let raw = Payload::new(ContentType::Json, br#"{"value":"raw"}"#.to_vec());
handle.signal("record", raw).await?;
```

## Branching on errors

Every operation returns the shared branchable taxonomy via `ClientError`.
Each variant carries an `ErrorDetail` with the server's human detail message
and, when the wire supplied one, the structured `error_type` discriminator.

```rust
use aion_client::ClientError;

match handle.query_typed::<_, serde_json::Value>("state", &(), Duration::from_millis(10)).await {
    Ok(value) => println!("state: {value}"),
    Err(ClientError::QueryTimeout { detail }) => eprintln!("query timed out: {detail}"),
    Err(ClientError::UnknownQuery { detail }) => eprintln!("unknown query: {detail}"),
    Err(ClientError::Unavailable { detail }) => eprintln!("server is unavailable: {detail}"),
    Err(error) => return Err(error),
}
```
