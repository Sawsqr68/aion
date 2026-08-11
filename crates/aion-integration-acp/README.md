# aion-integration-acp

The first-party [Agent Client Protocol](https://agentclientprotocol.com) (ACP) adapter for Aion — an
[`aion-integrations`](https://docs.rs/aion-integrations) `AgentHarness` that drives **any** agent with
an ACP mode over newline-delimited JSON-RPC 2.0 on its stdio.

Where `aion-integration-norn` adapts one harness and `aion-integration-cli` adapts the
observability-only tier, this crate adapts a *protocol*: one implementation reaches Claude Code,
Codex, Gemini CLI, OpenCode, and whatever ships next.

- `AcpHarness` spawns the configured agent command with piped stdio (stderr inherited), performs the
  `initialize` handshake gating on ACP protocol version `1`, creates a session with `session/new`,
  and issues the one long-running `session/prompt` request carrying the activity input as a `text`
  content block.
- `AcpSession` streams live `ActivityEvent`s translated from `session/update`, advertises the
  intervention primitives ACP can actually deliver, answers every peer request the agent sends, and
  interprets the id-matched `session/prompt` Response as the terminal result.

## Three rulings worth knowing

1. **`Cancel` is the only intervention primitive advertised.** It is the only one ACP v1 can deliver.
   The other four have no protocol mechanism, and a control that silently does nothing is worse than
   no control. A live session gates on that set; a composition root advertising this adapter when a
   worker REGISTERS — before any agent process exists — publishes the same ceiling through
   `registration_capabilities()`, which is pinned against what the adapter can actually translate.
2. **The permission policy is required and has no default.** An unanswered
   `session/request_permission` blocks the agent until the activity times out, and an Aion activity
   is unattended, so the answer is decided when the harness is built. Every request and every
   decision is recorded on the transcript.
3. **No filesystem or terminal callbacks are offered** (`fs.readTextFile: false`,
   `fs.writeTextFile: false`, `terminal: false`). Those exist so an *editor* can serve unsaved
   buffers and host an embedded terminal; a worker has neither, and the agent reads and writes the
   disk itself. An inbound `fs/*` or `terminal/*` request is still **answered** (`-32601`) and
   recorded, because an unanswered peer request wedges the run.

## The launch command is configuration

There is no standard ACP launch flag. Verified against primary sources on 2026-07-30:

| Agent | Command | Args |
| --- | --- | --- |
| Claude Code | `npx` | `@agentclientprotocol/claude-agent-acp` (none of its own) |
| Gemini CLI | `npx` | `@google/gemini-cli --acp` |
| OpenCode | `opencode` | `acp` |

Others use `acp serve`, `--acp=true`, `agent stdio`, or `exec --output-format acp-daemon`. Two things
that will bite a reader working from older material: the Claude adapter's previous package
`@zed-industries/claude-agent-acp` is **deprecated and renamed** to the `@agentclientprotocol/` one
above, and Gemini's `--experimental-acp` is the older alias for `--acp`.

Because none of this is inferable, the command and its arguments are taken verbatim from the harness
configuration and never guessed. This table is documentation for whoever writes that configuration —
it is not consulted by any code path, so a future rename here is a stale doc rather than a broken run.

```rust,no_run
use std::time::Duration;

use aion_integration_acp::{AcpHarness, PermissionPolicy};

let harness = AcpHarness::new(
    "npx",
    ["@google/gemini-cli", "--acp"],
    "/srv/workspace",             // absolute: `session/new` requires it
    PermissionPolicy::Deny,       // required: an unattended run must always answer
    Duration::from_secs(5),       // required: the grace before a lingering agent is killed
)
.with_env("GEMINI_API_KEY", "…");
```

## Serving it without writing a crate

The `aion` binary composes this adapter in behind its default-on `acp` cargo feature, so an operator
serves an ACP agent on the queue an `.awl` document declares with no Rust at all:

The launch is declared in the document, in the `worker` block's `harness` section — there is no flag
for any of it:

```awl
worker reviewers
  harness
    kind acp
    concurrency 4
    reconnect_initial_backoff 1s
    reconnect_max_backoff 30s
    reconnect_max_attempts 10
    env_pass "PATH", "HOME", "GEMINI_API_KEY"
    command "/usr/local/bin/npx"
    args "@google/gemini-cli", "--acp"
    cwd "/srv/workspace"
    permission "deny"
    exit_grace 5s
  action review(prompt: String) -> Reply
    agent
```

```text
aion worker agent flows/review.awl \
  --liminal-address 127.0.0.1:50061 --identity reviewer-worker
```

`command` is an absolute path because a worker starts from whatever directory a supervisor happened
to be in — the table above spells the launchers the way a shell does, and the document wants that
program's full path with the rest of the line in `args`. `env_pass` is the agent's WHOLE environment: the document names the variables, the box
supplies their values, and a variable it does not name is absent in the agent whatever the worker
inherited. `aion awl check flows/review.awl` names any setting the section is missing, scoped to the
kind it declares. Add `--check` to the command to print the exact action surface, the resolved
harness, and the intervention controls it advertises without connecting to anything.
`aion worker agent --help` is the full contract.

## Wire types

The ACP request/response/notification types come from the published
[`agent-client-protocol-schema`](https://docs.rs/agent-client-protocol-schema) crate, pinned exactly.
A hand-written mirror of a moving external protocol is a fossil waiting to happen; a protocol bump
should be a compile error, not a silent wire mismatch. Only the schema is taken — not the full
`agent-client-protocol` SDK, which carries its own async stack.

This crate requires Rust 1.88.0 (the schema crate's floor), above the rest of the workspace.

## License

AGPL-3.0-only.
