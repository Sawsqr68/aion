# Agent workers

How agent-backed activities get served in Aion. An **agent action** is an
activity whose work is done by an AI agent — its output is decided by the
agent at run time, so there is no Rust type to derive a schema from and the
AWL document is the only authority on what the action looks like. Its result
is always the **canonical agent-outcome record** — a declared record of
exactly `text: String` (the agent's terminal reply) and `stop_reason: String`
(the canonical snake_case reason: `end_turn | tool_use | limit_reached |
cancelled | error | other`) — which every harness emits from its own
integration boundary, so a flow reads `reply.text` and never parses
harness-specific JSON. This guide covers the two ways to serve one:

1. **`aion worker agent <file.awl>`** — the built-in agent worker. No crate
   to write: the binary you already installed serves the document's agent
   actions through a compiled-in harness (`norn` or `acp`).
2. **The assistant's example worker** (`examples/agent-dev/worker/`) — a
   hand-written Rust worker driving norn, kept as the reference for what a
   custom agent worker looks like and as the thing `scripts/setup.sh` runs.

Related reading: [DECLARED-COMMANDS.md](DECLARED-COMMANDS.md) (an action
WITH a `run` body in the document is executed by the *server* — no worker of
any kind serves it, there is no sandbox, and that document is the whole
account of what is and is not guaranteed), and
[WORKER-AUTHORING.md](WORKER-AUTHORING.md) (ordinary Rust activity workers).

## `aion worker agent` — an agent worker without a crate

**Without a repository checkout**, get the assistant document out of the binary
first — `aion` embeds it (#139 leg C), and the file it writes is byte-identical
to the tracked one every path below names:

```
aion assistant document --output assistant.awl
```

One command serves a document's agent actions. The surface below is the
current tree's — and the version string cannot tell you whether a binary has
it, because `--norn-arg` and `--norn-unset-env` landed in `e7c748e62` without
a version bump: a binary built before that commit says 0.11.0 and refuses
both flags, and one built after says 0.11.0 and accepts them. Check the
binary itself: `aion worker agent --help | grep norn-arg`. On a pre-that
binary, spawned norn processes run with bare norn defaults (default account,
no session pinning, **no `--workspace-root` confinement**):

```
aion worker agent crates/aion-server/assistant-embed/assistant.awl \
    --harness norn \
    --liminal-address 127.0.0.1:50061 \
    --identity assistant-agent-worker \
    --concurrency 4 \
    --reconnect-initial-backoff 1 \
    --reconnect-max-backoff 30 \
    --reconnect-max-attempts 100
```

That is the whole required set, and it is longer than it looks like it
should be on purpose — see "Nothing is invented" below.

The rules it lives by, and why:

- **The document is the whole advertised surface.** The task queue is the
  `worker` block's name — derived when the document declares exactly one
  block, selected with `--task-queue` when it declares several. Every action
  on that queue whose body the document does NOT carry is served
  (`crates/aion-cli/src/worker_surface.rs`, `serviceable_action_names`); an
  action WITH a body is the server's to run.
- **Serving is all-or-nothing.** A partial advertisement would be admitted
  on some actions and then dispatched one it never declared. The queue is
  served whole, or the verb refuses and names what is missing. A queue whose
  actions ALL carry bodies is refused too, naming them: there is nothing on
  it for an agent worker to do.
- **It dials liminal, not gRPC.** Workers connect on the liminal worker
  transport: `--liminal-address` (repeatable — extra addresses are ordered
  redial candidates for failover), env `AION_WORKER_LIMINAL_ADDRESSES`
  (comma-separated, same order). The global `--endpoint` is a *client* gRPC
  target and is not what a worker uses. The address to dial is in the server
  log's `liminal server listener bound` line — and that line only exists when
  the server's outbox is enabled; see
  [WORKER-AUTHORING.md](WORKER-AUTHORING.md#what-a-worker-is).
- **The namespace comes from the global `--namespace`** (default `default`),
  not from the document. Namespace and task queue are independent — see
  [activities-and-workers.md](activities-and-workers.md#the-routing-model-namespace--task-queue--node).
- **`--harness` picks the agent harness.** The default build compiles in
  both `norn` and `acp`, so the flag is required; a build carrying exactly
  one derives it. Env: `AION_AGENT_HARNESS`.
- **A flag for a harness you are not serving is refused, never ignored.** A
  flag that silently does nothing is how an operator comes to believe a
  policy is in force when it is not. An ambient environment *variable* for
  another harness is not refused: a box may legitimately export both.
- **`--check` prints the surface without connecting**: the exact action
  descriptors the worker would advertise, the resolved harness, and its
  intervention controls. Those descriptors are the whole admission decision
  on a queue carrying a deployed contract — read them at the desk instead of
  decoding a remote refusal. `--check` needs the document and the harness
  selection and **nothing else**: a report that connects to nothing must not
  demand the configuration of a connection it will not make.

### Nothing is invented — the settings you must supply

`aion_worker::WorkerConfigBuilder` refuses to build without an identity, a
concurrency, and all three reconnect settings, because those are operator
policy and the SDK has no business guessing them. This verb does not guess
them either. Every one is supplied by a flag or by its named
environment variable, the flag wins, and a missing one is reported BY NAME —
with every missing setting listed at once, so a shell invocation needs one
correction rather than seven
(`crates/aion-cli/src/worker_agent/args.rs`).

| Flag | Required | Env | What it decides |
|---|---|---|---|
| *(positional)* `<file.awl>` | yes | — | the document declaring the queue and the actions to serve |
| `--liminal-address` | yes (repeatable) | `AION_WORKER_LIMINAL_ADDRESSES` | ordered liminal candidates; extras are redial targets for failover |
| `--identity` | yes | `AION_WORKER_IDENTITY` | stable worker-build identity, used to correlate compatibility decisions |
| `--concurrency` | yes | `AION_WORKER_CONCURRENCY` | maximum activities served at once; must be > 0 |
| `--reconnect-initial-backoff` | yes | `AION_RECONNECT_INITIAL_BACKOFF_SECONDS` | seconds before the first reconnect attempt; fractions accepted |
| `--reconnect-max-backoff` | yes | `AION_RECONNECT_MAX_BACKOFF_SECONDS` | reconnect backoff cap, in seconds |
| `--reconnect-max-attempts` | yes | `AION_RECONNECT_MAX_ATTEMPTS` | cumulative session-drop budget |
| `--task-queue` | no | `AION_TASK_QUEUE` | selects among several `worker` blocks; a document with one leaves nothing to choose |
| `--node` | no | — | locality affinity advertised at registration; omitted takes the worker SDK's own hostname default |
| `--check` | no | — | print the surface and exit without connecting |

Omit the required settings and you get this, before anything dials — every
one named at once, flag and variable:

```
error: these worker settings have no value; supply each as a flag or an environment variable:
  --concurrency or $AION_WORKER_CONCURRENCY
  --identity or $AION_WORKER_IDENTITY
  --liminal-address or $AION_WORKER_LIMINAL_ADDRESSES
  --reconnect-initial-backoff or $AION_RECONNECT_INITIAL_BACKOFF_SECONDS
  --reconnect-max-attempts or $AION_RECONNECT_MAX_ATTEMPTS
  --reconnect-max-backoff or $AION_RECONNECT_MAX_BACKOFF_SECONDS
```

### The `norn` harness

Every setting is optional — a norn that needs no extra arguments and no
environment stripping is a complete configuration:

| Flag | Required | What it decides |
|---|---|---|
| `--norn-binary` | no | the `norn` binary (env `AION_NORN_BINARY`) if `norn` is not on the worker's PATH; omitted means the adapter's own `norn`-on-PATH default |
| `--norn-arg` | repeatable | one launch argument for the spawned norn, order preserved; hyphen-leading values accepted verbatim, because the arguments you most need to pass ARE flags (`--session-id`, `--resume-if-exists`, `--account`) |
| `--norn-unset-env` | repeatable | a variable REMOVED from the spawned norn's environment only — e.g. `--norn-unset-env OPENAI_API_KEY` forces norn onto its OAuth login when a stray ambient key would otherwise take precedence and fail |

Both repeatable flags are flag-only (no environment variable): an ordered
list of arbitrary strings has no safe single-variable encoding, and even a
variable *name* may contain a would-be separator.

The adapter expands two run-identity placeholders in each `--norn-arg` at
spawn time — `{workflow_id}` and `{activity_type}`; any other `{...}` text
passes through literally
(`crates/aion-integration-norn/src/harness.rs`, `expand_arg`). That makes the
example worker's session-pinned invocation (below) reproducible without a
crate:

```
aion worker agent crates/aion-server/assistant-embed/assistant.awl \
    --harness norn --liminal-address 127.0.0.1:50061 \
    --identity assistant-agent-worker --concurrency 4 \
    --reconnect-initial-backoff 1 --reconnect-max-backoff 30 \
    --reconnect-max-attempts 100 \
    --norn-arg --workspace-root --norn-arg "<workspace_root>/{workflow_id}/repo" \
    --norn-arg -C --norn-arg "<workspace_root>/{workflow_id}/repo" \
    --norn-arg --session-id --norn-arg "{workflow_id}-{activity_type}" \
    --norn-arg --resume-if-exists \
    --norn-arg --account --norn-arg default \
    --norn-unset-env OPENAI_API_KEY
```

`<workspace_root>` there is a value **you** substitute, not a placeholder the
adapter expands. It must be the same directory the server provisions the run's
workspace under, or the agent works somewhere other than where it was
provisioned. Read it from the server's JSON startup banner's `workspace_root`
field — that is the server's own resolved value, and re-deriving it is exactly
how two components come to disagree about one path. See
[DECLARED-COMMANDS.md](DECLARED-COMMANDS.md#workspace_root--how-a-body-learns-where-to-work).

The composed argv is asserted in the harness tests, not just observed
through live runs — a flag only ever seen firing is untested in the case
that matters.

The verb also recognises norn's degraded-session refusal (see the 28-day
story below) and attaches the route out to the activity failure: restart
with `--norn-arg --allow-degraded-session`. The advice goes quiet exactly
when that flag is already in the composed argv.

### The `acp` harness

ACP has no standard launch flag, so nothing is inferred from an agent's
name; every setting is explicit, and the required ones have **no defaults**
because they are security and correctness decisions:

| Flag | Required | Env | What it decides |
|---|---|---|---|
| `--acp-command` | yes | `AION_ACP_COMMAND` | the agent executable, passed verbatim |
| `--acp-cwd` | yes | `AION_ACP_CWD` | the ABSOLUTE directory sessions are rooted at; a relative path is refused rather than resolved against an unstated base |
| `--acp-permission` | yes | `AION_ACP_PERMISSION` | `deny` or `allow-once`: how an *unattended* activity answers the agent's permission requests; recorded on the transcript every time |
| `--acp-exit-grace` | yes | `AION_ACP_EXIT_GRACE_SECONDS` | seconds the agent may take to exit after the turn before it is killed (`0` = kill at once) |
| `--acp-arg` | repeatable | — | one launch argument (e.g. Gemini CLI needs `--acp-arg --acp`); hyphen-leading values accepted verbatim |
| `--acp-env` | repeatable | — | a `KEY=VALUE` set on the spawned agent only |

## The assistant's worker (`examples/agent-dev/worker/`)

The reference custom agent worker, and the one `scripts/setup.sh` builds and
starts.

### What it serves, and what the server runs instead

The assistant workflow (`crates/aion-server/assistant-embed/assistant.awl`) declares two
actions on queue `default`, and they are served by two different things:

- **`assistant_provision` — SERVER-RUN.** It carries a `run "…"` body, so the
  server executes it from the deployed contract; no worker serves it, and a
  worker claiming it would shadow the declared implementation. It materialises
  the session workspace at `<workspace_root>/<run_id>/repo` and prints that
  resolved path as its sole stdout, so the recorded activity result is where
  the workflow reads the workspace path from. The server states the root:
  `{workspace_root}` in the body is expanded at dispatch. Read
  [DECLARED-COMMANDS.md](DECLARED-COMMANDS.md) before changing it — including
  the parts about there being no sandbox and cancellation not being wired.
- **`assistant` — this worker.** One norn round inside that workspace: one
  prompt string in, the canonical agent-outcome record out (`text` — the
  agent's terminal reply — plus the canonical snake_case `stop_reason`; the
  document declares the record type and the harness boundary emits it). It is
  the worker's ONLY advertised action.

That split was not always so. Provisioning used to be a worker handler, and
moved into a server-run declared body in `fe702945`; the worker's own
`main.rs` says so in its module documentation.

**The advertisement is derived from the document, never hand-written.**
`descriptors.rs` embeds `assistant.awl` with `include_str!`, compiles it with
the same `aion_awl::compile` the `aion worker agent` verb uses, and advertises
the compiled contract's own schemas for every action whose body is absent.
That is not fastidiousness: a worker advertising activity-type NAMES and no
typed CONTRACTS is refused by admission on every contract-bearing queue, and
this exact worker shipped that bug. Read
[activities-and-workers.md](activities-and-workers.md#registration-admission-the-first-thing-that-goes-wrong)
for what the refusal looks like.

The binary also carries handlers nothing advertises — plain registry
activities `provision`, `gate`, `land` and agent rounds `scout`, `dev`,
`review` — for the Gleam-authored `agent_dev` package. No deployable contract
declares them (a Gleam package cannot start at all under queue-routed
serving), so advertising a schema for them would mean inventing one. They stay
registered and dispatchable; they are simply never announced.

### Flags

```
agent-dev-worker --norn-account default
```

| Flag | Required | Meaning |
|---|---|---|
| `--norn-account` | **yes** | norn OAuth account alias. No default on purpose: norn refuses to resume an existing session without an account, and guessing `default` would turn a clear startup error into three retried activity failures and a failed workflow |
| `--address` | no (repeatable) | liminal address(es) to dial, in preference order. **Defaults to `127.0.0.1:50061`** when none is given — that is this worker's own convention, not a value the server publishes (the server has no default listen address), so pass it explicitly whenever the server bound elsewhere |
| `--norn-model` | no | passed to norn as `--model`. **No default of this worker's**: omitted means norn chooses, exactly as if you ran norn by hand. Which model answers is the operator's call |
| `--norn-allow-degraded-session` | no (opt-in) | see the degraded-session section below |
| `--norn-bin` | no | the norn binary (default: `NORN_BIN` env, else `norn` on PATH) |
| `--identity` | no | worker identity announced in-band (default `agent-dev-worker`) |
| `--ready-file` | no | a file written once after the first registration, for scripts that wait on readiness |

`--norn-account` is the only one with no answer of its own, and it is enforced
at parse time: the worker refuses to start rather than start healthy-looking
and fail every agent round with `norn: auth error`. Every value-taking flag
bails when its value is missing, and an unknown argument is a loud error.

**Where it works.** The worker resolves one workspace root at startup and
threads it to both consumers — the `provision` handler and the agent harness's
`--workspace-root` template. The root is `AION_WORKSPACE_ROOT` when set,
otherwise `$HOME/.aion/clones`; it must be absolute, and a resolution failure
is a startup error rather than a silent temp-directory fallback, because the
workspace path is recorded in durable history and must survive a reboot.
`scripts/setup.sh` sets `AION_WORKSPACE_ROOT` from the server's banner, which
is what makes the worker's root the server's root by construction rather than
by coincidence.

### Session pinning — why one workflow is one conversation

Every norn round is invoked with:

```
--session-id {workflow_id}-{activity_type} --resume-if-exists
```

The session name is derived from the workflow, so every round of one
workflow lands in one continuous norn conversation — across worker
restarts, across server restarts, across weeks of the workflow sleeping.
The workflow's durability and the agent session's persistence line up
because they share a key. This is also why `--norn-account` is mandatory:
resuming an existing OAuth session is exactly the operation norn refuses to
do without an account alias.

The rest of the argv scopes the agent to the run: the per-run workspace is
passed both as norn's `--workspace-root` (confining the file tools) and as
`-C` (the tool-execution cwd) — without the latter, the agent's shell
commands would run in the worker's own directory, outside the workspace.

### Degraded sessions — the 28-day story

A durable workflow can park far longer than an agent provider's epoch: one
real run parked **28 days** and the engine resumed it perfectly. When the
provider has moved to a new epoch in the meantime, the exact provider
transcript can no longer be replayed, and norn refuses to resume the
session without explicit approval — so every round fails, on every retry.

The recognition and advice live in `aion-integration-norn` now, so both
paths teach the same way, each naming its own restart spelling: this worker
recognises that specific refusal and attaches the route out to the activity
failure — restart with `--norn-allow-degraded-session` — and the built-in
`aion worker agent` verb does the same, naming
`--norn-arg --allow-degraded-session`. The tradeoff, stated so the decision
is informed: the session resumes from a fresh provider epoch — the
*conversation* is retained, provider-side fidelity is reduced. It is an
operator opt-in with no default because that tradeoff is not the worker's
to assume on anyone's behalf.

## One-command setup: `scripts/setup.sh`

For getting a machine from nothing to a running server with the assistant
deployed and its worker serving — including a machine that has never seen
this repository:

```
curl -fsSL https://raw.githubusercontent.com/ablative-io/aion/main/scripts/setup.sh | bash
```

Prerequisites: `git`, `cargo` (Rust), and a logged-in `norn`. All three are
checked up front and a missing one fails immediately, by name.

What it actually does, in order — every step detects work already done and
skips it, so re-running is safe:

1. **Finds or clones the repository.** Run from a clone it uses that clone;
   run standalone it clones into `./aion` (or `AION_CHECKOUT_DIR`). The clone
   is `--depth 1` on purpose: the repository's history carries ~2 GiB of build
   artifacts committed by an old gate pipeline, and the build needs only the
   working tree.
2. **Installs `aion-cli` if `aion` is not on PATH.** A cold install compiles
   the whole server — allow ~25 minutes.
3. **Prepares `AION_HOME`** (default `~/.aion`) and `chmod 700`s it. The
   server's path-safety gate refuses a store with a group- or world-writable
   ancestor; owner-only up front means it never fires.
4. **Starts `aion server` from `AION_HOME`** and waits up to 120 s for it.
   The script writes no config: a first boot that finds no config at any
   discovery layer scaffolds `<AION_HOME>/config.toml` itself (mode `600`)
   from the server's embedded template, and that template ships with the two
   surfaces this flow needs already on. Both are **dark** on built-in
   defaults: the deploy gRPC service is mounted only when `deploy.enabled`,
   and the liminal listener that remote workers connect IN to exists only
   when the outbox is commissioned — without the scaffolded file the deploy
   step has nothing to talk to and the worker has nothing to dial. An
   existing config is never touched, and the file says out loud what
   enabling deploy means — deploying is running code on this server, which
   is why it ships off by default. The wait itself
   does not assume a port: readiness is the server's own log banner naming the
   address it bound, plus a `/build` response body carrying a `version` (the
   console's SPA fallback answers any path with 200 text/html, so a status code
   alone proves nothing).
5. **Reads the addresses and the workspace root from the JSON startup
   banner.** `http_address`, `grpc_address`, and `workspace_root` all come from
   the banner rather than from an assumption. A `workspace_root` reported as
   `unresolvable: <reason>` fails the script here, because declared
   provisioning cannot dispatch without one — see
   [DECLARED-COMMANDS.md](DECLARED-COMMANDS.md#workspace_root--how-a-body-learns-where-to-work).
6. **Deploys `crates/aion-server/assistant-embed/assistant.awl`** against the banner's
   `grpc_address`.
7. **Builds the worker** (release), skipping the build when the binary is
   newer than every `.rs` under `src/`.
8. **Starts the worker** against the liminal address from the server log's
   `liminal server listener bound` line — and fails with a pointer at the
   `[outbox]` config if that line is absent. It exports
   `AION_WORKSPACE_ROOT` from the banner value first, so the worker's
   workspace root IS the server's, threaded from the server that will use it
   rather than re-derived. It then waits up to 30 s for a registration to
   appear in the server log, and fails if the worker exits during startup.

Overrides (`AION_HOME`, `AION_CHECKOUT_DIR`, `AION_REPO_URL`, `NORN_ACCOUNT`,
`AION_NORN_MODEL`, `AION_ALLOW_DEGRADED_SESSION=1`) are documented in the
script's header.
