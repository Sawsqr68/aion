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

### A limitation of this fix, stated rather than discovered later

`validate_worker_contracts` iterates **every** retained `.v4` contract for the
queue and requires the worker to satisfy all of them. A worker derived from one
document therefore advertises exactly one version's schemas, so if two package
versions with differing schemas are concurrently deployed on the same queue, that
worker satisfies at most one of them and is refused.

`contract_diffs`' own doc comment anticipates the multi-version case — "a worker
may advertise additional actions because one process can serve several
concurrently deployed package versions" — but *additional actions* is the case it
solves. Differing schemas for the *same* action name across live versions is not
something a single `--awl` document can express.

This is inherent to deriving from one document, not introduced here, and it is
the right trade for now: the alternative is advertising a union, which would mean
claiming to accept inputs the worker's actual command wiring cannot handle. Worth
knowing before someone relies on rolling two schema-incompatible versions of a
queue at once.

## Finding 3 — hardcoded values in the shipped shell worker (flagged, not changed)

`worker_shell.rs` hardcodes `max_concurrency(4)`,
`reconnect_initial_backoff(100ms)`, `reconnect_max_backoff(5s)`, and
`reconnect_max_attempts(usize::MAX)`. CLAUDE.md forbids assumed defaults for
configurable values and says to discuss values before implementing.

Pre-existing, not introduced here, and changing them alters running behaviour —
so flagged rather than silently changed.

## Closed: no agent worker could be served without writing a Rust crate

**Landed `945873a7`** — `aion worker agent <file.awl>`. Same document-derived
descriptor seam as Finding 2, shared through `worker_surface` rather than copied,
so `worker_shell` now calls it instead of carrying its own version of the rule.

Verified rather than assumed, because the transport constraint decides the whole
flag vocabulary: the harness seam is **liminal-only**. `AgentHarnessConfig`,
`serve_with_redial`, and `RedialTiming` are exported from `aion-worker` solely
under `liminal-transport`; the gRPC `WorkerBuilder` API has no harness parameter
anywhere in `worker.rs`, `runtime/loop_.rs`, or `protocol/*`; and the server's
only attempt-to-owner binding sits inside `send_liminal_activity_task`. So the
verb dials `--liminal-address` candidates, never the global gRPC `--endpoint`.
A latent packaging gap surfaced on the way: `aion-cli`'s `liminal-transport`
feature forwarded to `aion-server` but **not** to `aion-worker`, so a plain
`cargo build -p aion-cli` had no liminal worker client at all.

Live-proven against a scratch server, including the case that justifies deriving
from the document: a worker run against a **stale** document was refused at
registration naming the exact field —
`action 'review' field 'input_schema.properties.brief.type' expected "string"
but worker advertised "integer"`.

`aion_integration_norn::registration_capabilities()` publishes what the adapter
can honour, because a worker advertises capabilities at **registration**, before
any agent process exists to be asked. A registration set and a per-session
handshake set can disagree in **both** directions — an older binary advertising
fewer makes the session gate a control registration promised; one advertising
more passes the session gate and is then refused by the translation. Both surface
as an attempt-scoped `CapabilityNotSupported` naming the primitive, neither is
silent, and neither is resolvable at registration time. Advertising the adapter's
ceiling is therefore the honest choice. A test drives all five neutral primitives
and asserts published-set ⇔ translatable-set, so adding one to either side alone
fails.

### Historical: the gap as it stood

`aion worker` had exactly one subcommand, `Shell`. All eleven agent
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

## Findings routed, not fixed here

Each was found by reading code this work touched. None is a regression from it.

1. **`WorkerConfigBuilder` demands three values the liminal transport ignores.**
   `endpoint` is read only by `protocol/session.rs` (`GeneratedClient::connect`)
   and one gRPC tracing field — verified, nothing on the liminal path reads it;
   `max_concurrency` is read only by the gRPC `loop_.rs` semaphore; and the redial
   loop takes no attempt bound. A builder that refuses to build without three
   settings one of its two transports cannot use is a real wart. Named in the
   value itself (`UNUSED_GRPC_ENDPOINT`) rather than accepting a flag for
   something that provably cannot matter.

2. **`aion worker shell` still installs no tracing subscriber**, so the redial
   driver's refused-dial warning — the one signal distinguishing "being turned
   away" from "idle" — is dropped on the floor. The agent verb installs the
   product's own subscriber; shell was out of scope to change. Belongs with #58.

3. **A deploy refused because the `[deploy]` surface is disabled renders as
   `error[backend]: failed to deploy package: wire error code is missing`** — an
   unrenderable wire error in place of the actual reason.

4. **The liminal redial loop is unbounded**: a worker dialing a wrong address
   retries forever (loudly, with an escalating counter). Existing semantics, worth
   a ruling alongside item 1.

5. **The TypeScript caller SDK has the Python SDK's disease** via a hand-written
   mirror: no `src/proto/`, no `routing_key`/`task_queue`, no reopen/pause/resume,
   and `src/errors.ts:250 numericWireCode()` is the identical fossil stopping at
   case 10 — **and there is no TypeScript CI lane at all**, so nothing there is
   gated by anything.

6. **The client conformance harness would not have caught any of it**, on two
   counts: nothing sets `AION_SERVER_URL`, and `conformance/aion-clients/scenarios.json`
   has no `taskQueue`/`routingKey`/reopen/pause/resume steps. New scenarios are
   needed, and adding them will red both the Rust and TS harnesses — which is the
   point, but it is a lane, not a drive-by.

7. **`sdks/python/aion-worker` has the same install-then-explode floor mismatch**
   Python's caller SDK just had: `grpcio>=1.68` against stubs demanding 1.83.0.

8. **`aion-integration-norn` carried the bypasses CLAUDE.md says void the work** —
   `#[allow(clippy::unwrap_used, expect_used, panic)]` on two test modules and as a
   crate-level attribute in `tests/norn_e2e.rs`, plus `#[ignore]` at
   `norn_e2e.rs:116`. Also `RUSTDOCFLAGS="-D warnings" cargo doc -p
   aion-integration-norn` fails (~10 unresolved intra-doc links, and public
   documentation linking the private `PARTIAL_OUTPUT_MESSAGE_BYTES`). Being fixed,
   not deferred.

## `preserve_order`: investigated, benign

`agent-client-protocol-schema` enables `serde_json/preserve_order`, and Cargo
feature unification turns that on **workspace-wide** — `serde_json::Map` becomes
insertion-ordered instead of sorted. Confirmed on: `indexmap` appears under
`serde_json` in the resolved feature tree, which only happens with that feature.

It does **not** move package identity. `content_hash_with_contract` — the `.v4`/`.v5`
digest behind Load-Bearing Invariant 5's `logical_name$hash` — encodes the contract
through `contract.rs::encode_json`, which explicitly sorts object entries by key
(`entries.sort_by_key(|(left, _)| *left)`) and sorts `required`/`enum` arrays. It is
order-independent **by construction**, which is exactly why it was written that way.

The digest that *can* shift is `Manifest::canonical_digest` (SHA-256 over
`serde_json::to_vec(self)`, whose `input_schema`/`output_schema` are raw `Value`s).
That one is never persisted: its only comparison is resident-vs-incoming inside a
single running process (`aion/src/loader/catalog_load.rs:252`), so both sides shift
together and nothing compares an old digest to a new one. No archive is invalidated.

Worth knowing rather than acting on — but a digest named "canonical" that depends on
a transitive dependency's feature unification is worth making order-independent by
construction on its own merits, the way the contract encoder already is.
