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
   no control.
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
use aion_integration_acp::{AcpHarness, PermissionPolicy};

let harness = AcpHarness::new(
    "npx",
    ["@google/gemini-cli", "--acp"],
    "/srv/workspace",          // absolute: `session/new` requires it
    PermissionPolicy::Deny,    // required: an unattended run must always answer
)
.with_env("GEMINI_API_KEY", "…");
```

## Wire types

The ACP request/response/notification types come from the published
[`agent-client-protocol-schema`](https://docs.rs/agent-client-protocol-schema) crate, pinned exactly.
A hand-written mirror of a moving external protocol is a fossil waiting to happen; a protocol bump
should be a compile error, not a silent wire mismatch. Only the schema is taken — not the full
`agent-client-protocol` SDK, which carries its own async stack.

This crate requires Rust 1.88.0 (the schema crate's floor), above the rest of the workspace.

## License

AGPL-3.0-only.
