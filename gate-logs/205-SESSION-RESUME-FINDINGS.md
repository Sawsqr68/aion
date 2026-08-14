# #205 — session resume: what is already built, what is missing, and one corrected premise

Measured 2026-08-14 at Vesper's hand. Every claim below names the file and line
it was read from, or the command that produced it.

## 1. Tom's stated constraint is stricter than the tool's actual one

Tom's design (TOM-RULINGS-20260814.md §6) says *"Claude Code sessions need to be
a UUID version 4, but we can generate that at the start of the workflow as the
first step, and then just use that throughout."*

Measured directly against the installed `claude` binary:

```
$ claude --session-id not-a-uuid -p "x"
Error: Invalid session ID. Must be a valid UUID.

$ claude --session-id f81d4fae-7dec-11d0-a765-00a0c91e6bf6 -p "Reply with the single word: ok"
ok
```

`f81d4fae-7dec-11d0-a765-00a0c91e6bf6` is the RFC 4122 **version 1** example
UUID — its version nibble is `1`, not `4`. It was accepted and the session ran.

**So the constraint is "a valid UUID", not "a version 4 UUID".** That is not a
nit: it is what makes the better design below possible. A v4 is *random*, so it
must be minted once and then recorded to survive replay. Any-UUID admits a
**derived** id, which needs no recording at all.

## 2. Every production mint of a WorkflowId is v4 — but the type does not guarantee it

Four mint sites, all `WorkflowId::new_v4()`:

| Site | What it mints |
|---|---|
| `crates/aion/src/lifecycle/start_admission.rs:120` | the engine default, when the caller supplies no id |
| `crates/aion-store-haematite/src/store.rs:600` | unsteered-start remint candidates (loop until one lands on an owned shard) |
| `crates/aion-store-haematite/src/store.rs:632` | steered-start mint candidates for a target shard |
| `crates/aion-server/src/routing/edge.rs:183` | the fallback when the bounded mint loop is exhausted |

⚠️ `WorkflowId::new(uuid)` (`crates/aion-core/src/ids.rs:15`) accepts **any**
UUID, nil included, and the start path takes a caller-supplied
`workflow_id: Option<WorkflowId>` off the wire
(`crates/aion/src/engine/api_workflow_ops.rs:86`,
`crates/aion-server/src/api/grpc/routing_resolve.rs:38`). So "always v4" is a
property of the **mint sites**, not of the type. Any design that depends on
`workflow.id` being v4 would be resting on a convention the boundary does not
enforce. Since the real requirement is only "a valid UUID", this does not bite —
but it must not be forgotten if the requirement ever tightens.

## 3. Half of the design is already built — for norn only

`docs/workers/AGENT-WORKERS.md` §"Session pinning — why one workflow is one
conversation": every norn round is invoked with
`--session-id {workflow_id}-{activity_type} --resume-if-exists`, declared in the
**worker document's `harness args`** line, not in the workflow document. That
split is load-bearing — `norn_dev.awl` carries routing (`route`, `outcome`) and
no session vocabulary at all.

`--norn-account` is mandatory *because* resuming an OAuth session is what norn
refuses without an account alias.

## 4. Which harness would run Claude Code — I got this wrong first, then measured it

My first pass named `aion-integration-cli` as "the harness that would run
`claude`". **That is wrong, and the error is worth recording** because it is the
"incidental claim rides on the main one" shape: I asserted the routing in passing
while checking the id shape, and it escaped scrutiny for two tool calls.

`crates/aion-cli/src/harness/catalogue.rs` — the module that decides which
harness a worker document's `kind` may name — carries exactly **two** variants,
`Norn` and `Acp`. There is no `Cli` variant. `CliHarness` appears nowhere outside
its own crate and its own tests (`grep -rn CliHarness crates/` → only
`aion-integration-cli/src` and `aion-integration-cli/tests`). **The plain-CLI
harness is not selectable from a worker document at all**, so its missing
placeholder expansion is not the blocker — it is not on the path.

The route for Claude Code is **ACP**, stated in the adapter's own module table
(`crates/aion-integration-acp/src/harness.rs:16`):

| agent | invocation |
|---|---|
| Claude Code | `npx @agentclientprotocol/claude-agent-acp` (no args) |

## 5. The real gap: the ACP adapter always mints a new session and can never load one

ACP has a first-class resume mechanism — `session/load`, gated on the agent's
advertised `agentCapabilities.loadSession`. Our adapter does not use it:

- `crates/aion-integration-acp/src/protocol.rs:62` defines `METHOD_SESSION_NEW`.
  There is **no** `METHOD_SESSION_LOAD` constant anywhere in the crate.
- The lifecycle is stated three times in the crate's own docs
  (`lib.rs:20`, `harness.rs:6`, `harness.rs:524`): initialize → **`session/new`**
  → one long-running `session/prompt`. Every attempt, unconditionally.
- The wire-test mock advertises `"agentCapabilities": {"loadSession": false}`
  (`wire_tests.rs:142`) — the capability is not even exercised.
- The minted `SessionId` is held for the attempt (`session.rs:73`, logged at
  `session.rs:286` and `harness.rs:576`) and **never surfaced to the engine**. It
  dies with the attempt; nothing durable can name it later.

**So resuming a Claude Code conversation is not an argv problem.** No
`--session-id` flag is involved on this path at all: in ACP the *agent* mints the
session id and returns it from `session/new`. Reusing one means (a) keeping the
id the agent gave us, and (b) calling `session/load` with it next time.

That reverses the design conclusion I reached in §1. The derived-UUID idea is
right for **norn**, where *we* name the session; it is wrong for **ACP/Claude
Code**, where the agent names it and our only job is to remember it. A derived id
cannot be imposed on an ACP agent.

### What that means for the two harnesses

| | who names the session | how resume works | what is missing |
|---|---|---|---|
| **norn** | we do, via `--session-id` | `--resume-if-exists` | already built and documented |
| **ACP / Claude Code** | the agent does, at `session/new` | `session/load` with the remembered id | the whole path: no load method, and the id is never kept |

## 6. The design this now points at

For ACP the session id must become a **durable value of the run**, not a local
variable of one attempt:

1. The adapter reads `agentCapabilities.loadSession` from the `initialize`
   handshake — it is already on the wire and already ignored.
2. When a prior session id is supplied for this attempt AND the agent advertises
   `loadSession`, issue `session/load` instead of `session/new`.
3. When it is not supplied, or the agent does not advertise the capability,
   `session/new` as today — and the id it returns is carried back out so the run
   can keep it.
4. A refusal that names the reason when a resume is asked for and the agent
   cannot honour it. "Asked to resume and silently started fresh" is exactly the
   failure this whole task exists to end.

`{workflow_id}` / `{activity_type}` placeholder expansion in the ACP adapter's
`args` is a **separate, smaller** want (its args are passed verbatim —
`harness.rs:270`, pinned by `the_configured_command_and_args_are_passed_through_verbatim`
at `harness.rs:777`). It is worth having for parity with norn, but it does not
deliver session resume for Claude Code and must not be mistaken for it. If it is
added, copy norn's `Template` / `Literal` split (`harness.rs:194`/`:201`) rather
than expanding every argument, or that verbatim pin becomes a lie.

Nothing here needs the `uuid` `v5` feature. That cost belonged to the derived-id
design, which §5 retires for this path.

## 7. What must move, in order

1. **ACP `session/load`** — the capability read, the load call, the id carried
   out, and the honest refusal. This is #205's actual content for Claude Code.
2. **The coordinator's own session**, which is where #205 started and which
   neither harness touches.
3. *(smaller, parity only)* placeholder expansion in the ACP adapter's args,
   Template/Literal split preserved.

Related: **#210** ({workspace_root} expansion + per-box harness-arg binding) sits
on item 3's seam, not item 1's.

## 8. Two claims of mine that did not survive, kept on purpose

- "The plain-CLI harness is the blocker." Refuted in §4 — it is not reachable
  from a worker document.
- "Derive the session id as a v5 UUID." Right for norn, refuted for ACP in §5 —
  the agent owns the id there.

Both were stated confidently before the routing was checked. The measurement that
caught them was reading the *catalogue*, which is the file that decides what an
operator can actually write.

## 9. Build design for item 1 (written 2026-08-14, pre-build)

Measured seams, in dependency order:

**(a) Input side — the session id arrives like the directory does.**
`HarnessWorkspace::prompt_for_attempt`'s object arm demands the input carry
EXACTLY ONE field besides the workspace's directory parameter
(workspace.rs `prompt_field`). A `session_id` field in the job input would
make prompt extraction refuse with "carries 2 fields". So the session
parameter must be DECLARED (worker document `harness` section, like
`cwd from <parameter>` — #204/#228 precedent) and subtracted in
`prompt_field` exactly as the directory is. Plumbing: a session source
beside the workspace on the adapter config; absent = a worker whose jobs
never resume (real absence, not a default). Absent parameter in a job that
declared the source = fresh session (session/new) — the id is OPTIONAL per
job; a job that carries it is asking to resume.

**(b) Adapter — capability read + load call + honest refusal.**
- `protocol.rs`: add `METHOD_SESSION_LOAD = "session/load"` (schema:
  `SESSION_LOAD_METHOD_NAME`, types `LoadSessionRequest::new(session_id,
  cwd)` + `.additional_directories(...)`, `LoadSessionResponse` — all in
  agent-client-protocol-schema 1.5.0, verified).
- `connect` currently DROPS the InitializeResponse after logging;
  it must thread `agent_capabilities.load_session` into `open_turn`.
- `open_turn`: prior id supplied AND `load_session` advertised →
  `session/load` (replayed `session/update`s during the handshake land in
  `deferred`, which the pump already replays — machinery exists); no prior
  id → `session/new` exactly as today.
- Refusal: prior id supplied, capability NOT advertised →
  `HarnessError::configuration` (deterministic: same document → same agent
  → same answer; the require_absolute precedent). "Asked to resume and
  silently started fresh" must be impossible.

**(c) Out seam — BLOCKED ON A RULING.** The workflow can only keep the id
if the result carries it, and the result is the #181-ratified
`AgentOutcome { text, stop_reason }` — checker-enforced exact shape,
serialization pinned as exactly two keys. Carrying `session_id` means
widening THAT record across checker + both harnesses + docs (norn fills it
with its own derived id — uniform, no optionality; house rule = replace,
no compat shim, existing two-field declarations get checker errors and are
updated). This is Waffles' ruling to make: #181 was his chain. Question
sent 2026-08-14. Until it lands, (a)+(b) are still correct and complete
for "the dispatcher already knows the id" cases (e.g. an operator or
workflow that recorded it out-of-band), but the self-contained
resume-across-attempts story waits on (c).

**As built (2026-08-14, landed bc6703ef4; deviation RATIFIED by Waffles).**
The ruling landed and (c) is closed, with one classification change from
the text above. Norn does NOT fill the field with a derived id: the
envelope carries norn's own `session_id`, and the adapter reads it as a
FACT — an id this adapter derived would name a session norn never
persisted. That fact-read forces the null arm the design above never
priced: a COMPLETED envelope whose `session_id` is null/absent (a
`--no-session` invocation, the one case the stop-envelope contract names)
is `HarnessError::Contract` naming `--no-session` — NOT Protocol — because
a worker configured to discard its session is a deterministic
misconfiguration, not malformed data: the same configuration answers the
same way every time, and a retryable classification would burn a retry
budget of live agent runs against a config that cannot heal.
Empty-string and non-string `session_id` stay Protocol — off the
stop-envelope contract entirely, the malformed-data lane, kept distinct.
