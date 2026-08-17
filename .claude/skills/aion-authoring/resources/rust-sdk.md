# Rust worker SDK (`crates/aion-worker`)

A **library crate** — it ships no binaries; a worker is your own crate. Two
starting points:

- **Generated**: `aion awl scaffold FILE.awl --out DIR` emits a compiling
  worker crate per `worker` block, schemas taken from the compiled contract.
  `src/handlers.rs` is yours; re-running never rewrites it.
- **By hand**: model the package on `examples/dev-brief/worker/` — the
  proven reference implementation — with `docs/workers/WORKER-AUTHORING.md`
  beside it. When that guide and the code disagree, the code wins.

## Minimal worker

```rust
use std::time::Duration;
use aion_worker::{ActivityContext, HandlerFuture, Worker, WorkerConfig};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct GreetInput { name: String }

#[derive(Serialize)]
struct GreetOutput { message: String }

fn greet(input: GreetInput, _context: &ActivityContext) -> HandlerFuture<'_, GreetOutput> {
    Box::pin(async move {
        Ok(GreetOutput { message: format!("hello, {}", input.name) })
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = WorkerConfig::builder()
        .endpoint("http://127.0.0.1:50051")
        .task_queue("default")
        .identity("rust-worker-1")
        .max_concurrency(8)
        .reconnect_initial_backoff(Duration::from_millis(100))
        .reconnect_max_backoff(Duration::from_secs(5))
        .reconnect_max_attempts(10)
        .build()?;

    Worker::builder(config)
        .register_activity("examples.greet", greet)?
        .build()?
        .run()
        .await?;
    Ok(())
}
```

Trait bounds before fighting the compiler: inputs need
`Serialize + DeserializeOwned + Send + Sync + 'static` (derive **both**);
outputs need `Serialize + Send + Sync + 'static`; handlers return
`HandlerFuture<'context, Output>` — wrap an `async move` block in
`Box::pin`. Every `WorkerConfig` field shown is required; there are no
hidden defaults.

## Failure classification

`ActivityFailure::retryable(msg)` / `ActivityFailure::terminal(msg)`
(attach structure with `.with_detail(payload)`). The worker never retries on
its own — the classification rides back and the workflow's declared retry
policy drives retries. `ActivityContext` exposes `heartbeat(detail)`,
`attempt()`, and cooperative cancellation via `is_cancelled()`.

Apply the taxonomy from [workers.md](workers.md): can't-run-at-all is
terminal; a command that ran and exited non-zero is **data returned as
success** for the workflow to branch on.

## Serving against a real server (liminal push transport)

Long-lived workers in this estate use the `liminal-transport` feature and
`serve_with_redial(candidates, &config, &registry, RedialTiming::new(..),
&stop, agent_harness_or_none, on_registered)` — the server pushes dispatches
over the connection the worker dialed. Key habits from the reference
implementation:

- One connection registers **one node** and must serve every activity type
  that node owns (routing is namespace × queue × node, never by type).
- Reconnect with effectively unlimited attempts — a long-lived worker must
  outwait server restarts.
- Write a readiness file in `on_registered` so supervisors can await
  "connected".
- The dial address is the server's `[outbox] liminal_listen_address` — read
  the address the server actually bound from its log line
  (`liminal server listener bound`), don't assume.
- Standalone crate (empty `[workspace]` table) consuming the aion crates by
  path or from the registry; builds go to the package's own `target/`,
  never `/tmp`.

## Driven-agent activities

For activities that run a coding agent, compose a harness from
`aion-integrations` + `aion-integration-norn`: the inner harness carries
static arguments; a thin wrapper intercepts one seam to assemble the
per-turn prompt and per-run values; erase to `Arc<dyn DynAgentHarness>` and
serve via `AgentHarnessConfig`. Session ids templated on `{workflow_id}`
with resume-if-exists give an activity called twice one continuous
conversation. Full treatment: `docs/workers/WORKER-AUTHORING.md` §"Driven
agent activities" — and consider `aion worker agent` (no crate at all)
before writing this by hand.

## Testing (the hermetic pattern)

Construct the shell boundary against a directory of **fake CLI shims**
(tiny scripts named `git`, `cargo`, …): handlers really shell out, shims
intercept at the process boundary — realistic, no network, no real repos.
Test argv assembly exactly, and every failure-taxonomy branch. See
`examples/dev-brief/worker/tests/`.

## Caller side

`crates/aion-client` is the Rust caller SDK (start/signal/query/await); its
README is the reference.
