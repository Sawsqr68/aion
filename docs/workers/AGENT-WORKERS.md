# Agent workers

How agent-backed activities get served in Aion. An **agent action** is an
activity whose work is done by an AI agent — its output is decided by the
agent at run time, so there is no Rust type to derive a schema from and the
AWL document is the only authority on what the action looks like. This guide
covers the two ways to serve one:

1. **`aion worker agent <file.awl>`** — the built-in agent worker. No crate
   to write: the binary you already installed serves the document's agent
   actions through a compiled-in harness (`norn` or `acp`).
2. **The assistant's example worker** (`examples/agent-dev/worker/`) — a
   hand-written Rust worker driving norn, kept as the reference for what a
   custom agent worker looks like and as the thing `scripts/setup.sh` runs.

Related reading: [DECLARED-COMMANDS.md](DECLARED-COMMANDS.md) (an action
WITH a body in the document is run by the *server* — no worker of any kind
serves it), and [WORKER-AUTHORING.md](WORKER-AUTHORING.md) (ordinary Rust
activity workers).

## `aion worker agent` — an agent worker without a crate

Shipped in aion 0.11.0. One command serves a document's agent actions:

```
aion worker agent examples/assistant/awl/assistant.awl \
    --harness norn \
    --liminal-address 127.0.0.1:50061
```

The rules it lives by, and why:

- **The document is the whole advertised surface.** The task queue is the
  `worker` block's name — derived when the document declares exactly one
  block, selected with `--task-queue` when it declares several. Every action
  on that queue whose body the document does NOT carry is served; an action
  WITH a body is the server's to run.
- **Serving is all-or-nothing.** A partial advertisement would be admitted
  on some actions and then dispatched one it never declared. The queue is
  served whole, or the verb refuses and names what is missing.
- **It dials liminal, not gRPC.** Workers connect on the liminal worker
  transport: `--liminal-address` (repeatable — extra addresses are ordered
  redial candidates for failover), env `AION_WORKER_LIMINAL_ADDRESSES`. The
  global `--endpoint` is a *client* gRPC target and is not what a worker
  uses. The address to dial is in the server log's
  `liminal server listener bound` line.
- **`--harness` picks the agent harness.** The default build compiles in
  both `norn` and `acp`, so the flag is required; a build carrying exactly
  one derives it. Env: `AION_AGENT_HARNESS`.
- **A flag for a harness you are not serving is refused, never ignored.** A
  flag that silently does nothing is how an operator comes to believe a
  policy is in force when it is not.
- **`--check` prints the surface without connecting**: the exact action
  descriptors the worker would advertise, the resolved harness, and its
  intervention controls. Those descriptors are the whole admission decision
  on a queue carrying a deployed contract — read them at the desk instead of
  decoding a remote refusal.

### The `norn` harness

One optional setting: `--norn-binary` (env `AION_NORN_BINARY`) if `norn` is
not on the worker's PATH. Everything else — account, model, session policy —
is norn's own configuration.

### The `acp` harness

ACP has no standard launch flag, so nothing is inferred from an agent's
name; every setting is explicit, and the required ones have **no defaults**
because they are security and correctness decisions:

| Flag | Required | What it decides |
|---|---|---|
| `--acp-command` | yes | the agent executable, passed verbatim |
| `--acp-cwd` | yes | the ABSOLUTE directory sessions are rooted at |
| `--acp-permission` | yes | `deny` or `allow-once`: how an *unattended* activity answers the agent's permission requests; recorded on the transcript every time |
| `--acp-exit-grace` | yes | seconds the agent may take to exit after the turn before it is killed (`0` = kill at once) |
| `--acp-arg` | repeatable | one launch argument (e.g. Gemini CLI needs `--acp-arg --acp`); hyphen-leading values accepted verbatim |
| `--acp-env` | repeatable | a `KEY=VALUE` set on the spawned agent only |

## The assistant's worker (`examples/agent-dev/worker/`)

The reference custom agent worker, and the one `scripts/setup.sh` builds and
starts. It serves two actions on queue `default` for the assistant workflow
(`examples/assistant/awl/assistant.awl`):

- `assistant_provision` — clones the run's `repo_path` into a per-run
  workspace and returns the opening prompt built from the embedded teaching
  resources.
- `assistant` — one norn round inside that clone.

### Flags

```
agent-dev-worker --address 127.0.0.1:50061 --norn-account default
```

| Flag | Required | Meaning |
|---|---|---|
| `--address` | yes (repeatable) | liminal address(es) to dial, in preference order |
| `--norn-account` | yes | norn OAuth account alias. No default on purpose: norn refuses to resume an existing session without an account, and guessing `default` would turn a clear startup error into three retried activity failures and a failed workflow |
| `--norn-model` | no | passed to norn as `--model`. **No default of this worker's**: omitted means norn chooses, exactly as if you ran norn by hand. Which model answers is the operator's call |
| `--norn-allow-degraded-session` | no (opt-in) | see the degraded-session section below |
| `--norn-bin` | no | the norn binary (default: `NORN_BIN` env, else `norn` on PATH) |
| `--identity` | no | worker identity announced in-band (default `agent-dev-worker`) |
| `--ready-file` | no | a file written once after the first registration, for scripts that wait on readiness |

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

The rest of the argv scopes the agent to the run: the per-run clone is
passed both as norn's `--workspace-root` (confining the file tools) and as
`-C` (the tool-execution cwd) — without the latter, the agent's shell
commands would run in the worker's own directory, outside the clone.

### Degraded sessions — the 28-day story

A durable workflow can park far longer than an agent provider's epoch: one
real run parked **28 days** and the engine resumed it perfectly. When the
provider has moved to a new epoch in the meantime, the exact provider
transcript can no longer be replayed, and norn refuses to resume the
session without explicit approval — so every round fails, on every retry.

The worker recognises that specific refusal and attaches the route out to
the activity failure: restart with `--norn-allow-degraded-session`. The
tradeoff, stated so the decision is informed: the session resumes from a
fresh provider epoch — the *conversation* is retained, provider-side
fidelity is reduced. It is an operator opt-in with no default because that
tradeoff is not the worker's to assume on anyone's behalf.

## One-command setup: `scripts/setup.sh`

For getting a machine from nothing to a running server with the assistant
deployed and its worker serving — including a machine that has never seen
this repository:

```
curl -fsSL https://raw.githubusercontent.com/ablative-io/aion/main/scripts/setup.sh | bash
```

Prerequisites: `git`, `cargo` (Rust), and a logged-in `norn`. The script
clones the repository if run standalone, installs `aion-cli` if missing
(a cold install compiles the server — allow ~25 minutes), prepares an
owner-only `AION_HOME`, starts the server, deploys
`examples/assistant/awl/assistant.awl`, builds this worker, and starts it
against the server's liminal listener. Every step detects work already done
and skips it, so re-running is safe. Overrides (`AION_HOME`,
`NORN_ACCOUNT`, `AION_NORN_MODEL`, `AION_ALLOW_DEGRADED_SESSION=1`, and
more) are documented in the script's header.
