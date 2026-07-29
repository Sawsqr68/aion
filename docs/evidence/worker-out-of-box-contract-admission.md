# The scaffolded worker could not serve the queue it was scaffolded from

Working record for task #29. Written as I go so the findings survive a
compaction; the landed commits are the authority.

## Where this started

Task #29 is "ACP worker shipped out of the box". The banked scout finding said
`spawn_agent`/`spawn_dyn_agent` in `aion-worker` had **zero consumers** — the
same "helpers with no consumer" shape as the `action_bodies` defect.

**That finding is wrong**, and the memory note carrying it needs correcting.
`spawn_dyn_agent` is driven by the real worker execute path at
`crates/aion-worker/src/runtime/liminal.rs:974`, and
`crates/aion-cli/src/harness.rs` is a live composition root. The harness seam is
wired end to end and has e2e coverage
(`aion-server/tests/noi5b_noi6_live_agent_e2e.rs`).

The real gap is elsewhere, and it is bigger.

## Finding 1 — a declared body was demanded of every worker (FIXED, `23c9fdee`)

`contract_diffs` (`crates/aion-package/src/compatibility.rs`) required an
advertisement for **every** action in a deployed `WorkerContract`, with no
regard for whether the action carries a body.

An action whose body the declaration carries is executed by the **server**
in-process (`crates/aion-server/src/worker/declared_body.rs`). No registering
worker serves it and none can advertise it.

So any AWL `worker` block mixing a declared `run` action with a bodyless one
made its queue **unservable**: the worker serving the bodyless action was
refused with `WORKER_CONTRACT_MISMATCH` for omitting an action that was never
its job.

Live on both registration paths — gRPC (`api/worker_grpc.rs:641`) and liminal
(`worker/liminal_transport.rs:1523`) — and production wires the catalog
(`aion-server/src/run.rs:750`). Not theoretical.

Not a regression in the declared-command work (#60): those bodies are
server-executed and never register as a worker, so that live proof stands. The
defect bites the **combination**, which #30 (AWL-declared workers) would have
made routine.

Pinned by `a_declared_body_is_not_required_of_a_worker`, red before the fix.

## Finding 2 — the scaffolded shell worker advertises nothing

This is the out-of-box story broken at its last step.

The chain, each link read not assumed:

1. The studio scaffolds a shell worker from a checked AWL document, emitting a
   TOML manifest — `crates/aion-server/src/awl/scaffold.rs:117`.
2. The operator runs `aion worker shell --manifest <file>`.
3. `crates/aion-cli/src/worker_shell.rs` registers every action with
   `register_activity`, **not** `register_activity_with_contract`.
4. `ActivityRegistry::register_activity` inserts **no descriptor**
   (`crates/aion-worker/src/activity.rs`), so `activity_descriptors()` is empty.
5. `Worker::build` advertises exactly that empty list
   (`crates/aion-worker/src/worker.rs:100`).
6. AWL emits a `WorkerContract` for **every** worker block
   (`crates/aion-awl/src/compile.rs:337`), so the queue always carries a
   contract.
7. `contract_diffs` therefore emits one refusal diff per declared action.

The scaffolded worker is refused on the queue it was generated for, naming
every action. The design intent is sound — "Types, timeout, and retry remain
owned by the `.awl`" — but nothing carries those types into the advertisement.

### Why this was never noticed

The `aion worker shell` surface cannot report its own failure. From #28,
live-reproduced and handed to #58: a real server, a dead port, and an
unroutable address are **indistinguishable** — hang, zero bytes, exit only on
timeout. `aion-cli` installs no tracing subscriber, and `worker_shell.rs`
retries `usize::MAX`. A refusal is silent on both sides: the server also logs
nothing when it refuses a worker.

So the surface has never been able to tell anyone it does not work.

### The fix

The schemas live in the `.awl`, so they must come from the `.awl`.

- `ActivityRegistry::register_activity_with_descriptor` — register an untyped
  handler with an **explicit** wire descriptor. `register_activity_with_contract`
  cannot help here: it derives from `schemars` Rust types, and a
  declaration-driven worker has none. Refuses a descriptor naming a different
  activity than the handler it is registered under.
- The same passthrough on `WorkerBuilder`.
- `aion worker shell` takes the document, compiles it with the **same**
  `aion_awl::compile` path the deploy uses, and advertises the descriptors for
  exactly the actions it serves.

Rejected alternative: have the worker **fetch** the contract from the server at
registration. The gate would then compare the server's contract against itself —
one fact told twice, the exact antipattern that let the SDK fossils go green in
#28. Deriving from the document independently is what makes the check able to
catch a worker running a stale `.awl`.

All-or-nothing, matching the rule set in #28: a manifest action absent from the
document, or a bodyless document action absent from the manifest, is refused by
name. A partial advertisement gets the worker admitted on some actions and then
dispatched one it never declared.

## Finding 3 — hardcoded values in the shipped shell worker (flagged, not changed)

`worker_shell.rs` hardcodes `max_concurrency(4)`,
`reconnect_initial_backoff(100ms)`, `reconnect_max_backoff(5s)`, and
`reconnect_max_attempts(usize::MAX)`. CLAUDE.md forbids assumed defaults for
configurable values and says to discuss values before implementing.

Pre-existing, not introduced here, and changing them alters running behaviour —
so flagged rather than silently changed.

## Still open on #29 proper

Even after the above, **no agent worker can be served without writing a Rust
crate**. `aion worker` has exactly one subcommand, `Shell`. All eleven agent
workers in `examples/` are bespoke `main.rs` files hand-rolling
`composed_agent_harness()`. The `aion` binary's composition root builds
`default_agent_harness()` and immediately **drops** it
(`harness.rs:52` — `let _harness = ...`), which is a readiness announcement, not
a registration.

So even Norn, the shipped default harness, is not servable out of the box. ACP
having zero code is the smaller half of #29.

An agent activity's output is harness-owned, so it has no Rust type to derive
from either — which is why `AgentHarnessConfig::with_activity_descriptors`
exists. The descriptor seam built for Finding 2 is the same seam the agent path
needs. Build it once, use it for both.

## ACP: the facts, and the rulings taken from them

Established from primary sources on 2026-07-30 (the spec site, the schema
`meta.json`, and the crate sources — not from recollection), so this does not
need re-researching.

**The protocol.** Wire version is the integer **`1`** — a uint16, not a string,
not semver, and unrelated to the crate version (which is at 2.0.0). Transport is
newline-delimited JSON-RPC 2.0 over the agent subprocess's stdio, UTF-8, no
`Content-Length` framing; the **client spawns the agent**; the channel is
bidirectional. That is exactly what `aion-integrations`' existing
`JsonRpcConnection` already provides, so the adapter writes no new framing.

The project has moved out of `zed-industries` to `agentclientprotocol/`, and the
Rust SDK now lives in its own repo.

`session/prompt` stays open for the whole turn and its Response carries
`stopReason` (`end_turn`, `max_tokens`, `max_turn_requests`, `refusal`,
`cancelled`). That is structurally the same shape `NornSession` already handles —
hand the outstanding request id to the session and let its reader pump capture
that Response as the terminal result.

**Dependency ruling.** Depend on `agent-client-protocol-schema = "=1.5.0"` for
the wire types and drive them with our own transport. NOT the full
`agent-client-protocol` SDK: it is a builder-based rewrite (no `Agent`/`Client`
traits any more) carrying its own async stack — `async-io`, `async-process`,
`blocking` — and would drag a second runtime into a tokio codebase.

Taking the types from the crate rather than hand-writing them is the anti-fossil
choice, and that is the point. A hand-written mirror of a moving external
protocol is precisely the shape that went stale in the SDKs: a version bump
should be a compile error, not a silent wire mismatch.

⚠️ **MSRV.** That crate requires Rust **1.88.0** / edition 2024, and the
workspace declares `rust-version = "1.85"`. Handled with a `rust-version` on the
ACP crate alone. Raising the workspace value changes what every published crate
claims to support, which is a policy call and was not taken here.

**Capability ruling — we are the ACP client, but we are not an editor.** ACP's
`fs/read_text_file`, `fs/write_text_file`, and the five `terminal/*` methods
exist so an editor can serve unsaved buffers and embedded terminals to the agent.
An Aion worker has no buffers, and the agent is a subprocess with direct
filesystem access, so it reads and writes the disk itself. Advertising all three
capabilities as `false` is therefore correct for this consumer rather than a
degradation — and ACP v2 removes the entire surface in favour of MCP, which
confirms the direction. It also keeps a security-sensitive host-I/O surface out
of the first cut.

An inbound `fs/*` or `terminal/*` request must still be **answered** (`-32601`),
never ignored. An unanswered peer request blocks the agent forever.

**Permission ruling.** `session/request_permission` is a request, and leaving it
unanswered wedges the run — while an Aion activity is unattended by default. So
the policy is explicit and **required**, with no invented default, and every
permission request is emitted as an `ActivityEvent` *before* it is answered. A
silent auto-approval that an operator cannot audit afterwards would be worse than
a refusal.

**Launch commands are not standardised** — `--acp`, `acp`, `acp serve`,
`serve acp`, `--acp=true`, `agent stdio`,
`exec --output-format acp-daemon`, and no-arg-at-all all occur across the 38
agents in the registry. Command and args are configuration; nothing is inferred.
