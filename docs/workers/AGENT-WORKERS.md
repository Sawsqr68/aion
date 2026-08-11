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

Two things start an agent worker, and the split between them is deliberate:

- **The document declares the LAUNCH.** Which harness, how many activities run
  at once, how a dropped session is retried, what environment the agent
  subprocess is given, and the agent's own command and arguments — all of it in
  a `harness` section inside the `worker` block, reviewed and deployed with the
  rest of the document and read by `aion awl check`. **There is no flag for any
  of it.**
- **The command line declares the CONNECTION.** Where this process dials, who
  it says it is, and where it sits. Those are properties of one worker on one
  box rather than of the queue: two workers serving the same document
  legitimately differ in all three, which is exactly why they are not in the
  file.

So the whole invocation is short:

```
aion worker agent assistant.awl \
    --liminal-address 127.0.0.1:50061 \
    --identity assistant-agent-worker
```

The launch settings used to be a dozen flags on this verb — `--harness`,
`--concurrency`, the three `--reconnect-*`, every `--acp-*` and every
`--norn-*`. They are **deleted, not deprecated**: an invocation carrying one is
an argument-parse error, and their environment forms (`AION_AGENT_HARNESS`,
`AION_NORN_BINARY`, `AION_ACP_COMMAND`, `AION_ACP_CWD`, `AION_ACP_PERMISSION`,
`AION_ACP_EXIT_GRACE_SECONDS`, and — for this verb only — `AION_WORKER_CONCURRENCY`
and the three `AION_RECONNECT_*`) went with them. Keeping both would have been
a hand-maintained drift pair, and a launch setting that can come from two
places has already come from the wrong one.

The version string cannot tell you which side of that a binary is on. Check the
binary: `aion worker agent --help`. One that still accepts `--concurrency`
predates the move and reads no `harness` section at all.

None of the three documents this repository ships with an agent action —
`crates/aion-server/assistant-embed/assistant.awl`,
`workflows/norn-fleet/norn_fleet.awl` and `workflows/norn-dev/norn_dev.awl` —
carries a `harness` section yet, so serving any of them with this verb means
adding one first. (The census is every document with an `agent`-marked action:
`grep -rln 'agent,\|^    agent$' workflows/ crates/aion-server/assistant-embed/`.) Hand it a document
without one and it refuses by saying so: the queue "declares no `harness`
section", "There is no flag for any of it", and run `aion awl check` on it. The
section below is the whole of what to write, and the checker names anything you
leave out.

### A managed-worker record minted before this change

`aion worker agent` is also started by the SERVER, through the managed-worker
motion: a `builtin` worker-deployment record stores the operator's argv tail
verbatim and durably, and the supervisor replays exactly that on every restart
(`crates/aion-server/src/worker/supervisor/fleet.rs`,
`WorkerArtifactRef::Builtin`). So a record minted before the launch settings
moved carries the deleted flags **inside the record**, and:

- it fails on its next supervised restart, not on a deploy — the change lands
  quietly and the record breaks the next time something restarts it;
- it fails again on every retry, because the argv is fixed, until the crash-loop
  guard spends `max_restarts_per_window` and parks it as `Failed`;
- **editing the document does not fix it.** The document is not what the record
  replays.

There is no migration for these records and there is deliberately no tolerant
parse. There is nothing to migrate a stored argv *from* — the values a `harness`
section needs (this box's concurrency, its reconnect budget, the absolute path
to its agent, the variables that agent may see) were never in the record — and a
parse that accepted the old flags would be the compat shim this repository does
not build.

What there is instead is a refusal that says so. The verb recognises every one
of the fourteen deleted flags on its own argv and prints, beneath clap's
argument error, which flags were supplied, which document field replaced each,
and both remedies — including the record-shaped one, because an operator reading
a supervised failure did not type the command:

> If you did NOT type this command, a managed-worker deployment replayed it. […]
> The record is the thing to fix: add the `harness` section to the document,
> then re-mint the deployment (`PUT /worker-deployments/<name>`, or the ops
> console's deploy form) so its stored argv no longer carries these flags.

The supervisor forwards a managed worker's stderr into the server's own tracing
output, tagged with the worker name, so that sentence is what an operator finds
against the deployment that will not start. **Re-minting the record is the fix**:
write the `harness` section into the document, then `PUT` the deployment again
with a verb tail carrying only the document and the connection flags.

### Why the SERVER's own spawn environment is not cleared

`env_pass` governs the AGENT SUBPROCESS — the `claude-code-acp` or `norn` a
worker launches. It does not govern the worker PROCESS itself, and the managed
-worker supervisor (`crates/aion-server/src/worker/supervisor/`) still hands a
supervised worker the server's own environment, inherited. That is deliberate,
and it is not a gap left in the fence:

- The worker process's own settings are read from that environment BY DESIGN.
  The ops console's deploy form documents it in as many words — a setting left
  blank "must come from the server process's environment
  (`AION_WORKER_CONCURRENCY`, `AION_RECONNECT_*`)" — so clearing it would break
  every deployed `aion worker awl` worker that relies on it, silently, at the
  next supervised restart.
- Clearing it would not add safety where the incident happened. `CLAUDECODE`
  reaching a worker process harms nothing; it broke a live run by reaching the
  AGENT the worker spawned. That boundary is now closed at the spawn site,
  which is downstream of every launcher — a hand-run CLI, a console click, a
  supervised restart — so one mechanism covers all three and none of them can
  opt out.
- The two are independent by construction: whatever the worker process
  inherits, the agent child gets `env_pass` and nothing else.

Declaring the worker process's own environment is a separate decision on a
separate surface, and it needs the operator's word on what a supervised worker
is allowed to see. It is not this change.

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
- **The harness kind is the document's, and it is checked against the build.**
  `kind acp` or `kind norn` in the section; the default build compiles both
  adapters in. A document naming a kind this binary was not built with is
  refused, naming what the build does carry.
- **A setting written for the kind the section does not declare is refused,
  never ignored.** A setting that silently does nothing is how an operator
  comes to believe a policy is in force when it is not: `permission "deny"`
  accepted and ignored on a `norn` section reads exactly like a denial being
  enforced.
- **`--check` prints the surface without connecting**: the exact action
  descriptors the worker would advertise, the resolved harness, and its
  intervention controls. Those descriptors are the whole admission decision
  on a queue carrying a deployed contract — read them at the desk instead of
  decoding a remote refusal. `--check` needs the document and **nothing
  else** — not even a `harness` section, because a report that connects to
  nothing must not demand the configuration of a connection it will not make,
  and reading the surface is exactly what you do before writing the launch.

### Nothing is invented — the connection settings you must supply

`aion_worker::WorkerConfigBuilder` refuses to build without an identity,
because that is operator policy and the SDK has no business guessing it. This
verb does not guess it either. Every connection setting is supplied by a flag
or by its named environment variable, the flag wins, and a missing one is
reported BY NAME — with every missing setting listed at once, so a shell
invocation needs one correction rather than two
(`crates/aion-cli/src/worker_agent/args.rs`).

| Flag | Required | Env | What it decides |
|---|---|---|---|
| *(positional)* `<FILE.awl>` | yes | — | the document declaring the queue, the actions to serve, and the `harness` section that says how to launch the agent |
| `--liminal-address` | yes (repeatable) | `AION_WORKER_LIMINAL_ADDRESSES` | ordered liminal candidates; extras are redial targets for failover |
| `--identity` | yes | `AION_WORKER_IDENTITY` | stable worker-build identity, used to correlate compatibility decisions |
| `--task-queue` | no | `AION_TASK_QUEUE` | selects among several `worker` blocks; a document with one leaves nothing to choose |
| `--node` | no | — | locality affinity advertised at registration; omitted takes the worker SDK's own hostname default |
| `--check` | no | — | print the surface and exit without connecting |

The global `--namespace` (default `default`) belongs to the `aion` binary, not
to this verb, and goes before it.

Omit the required settings and you get this, before anything dials — every
one named at once, flag and variable:

```
error: these worker settings have no value; supply each as a flag or an environment variable:
  --identity or $AION_WORKER_IDENTITY
  --liminal-address or $AION_WORKER_LIMINAL_ADDRESSES
```

Name no document at all and the refusal is louder, because an operator meeting
it has almost certainly arrived with the deleted flags in their fingers: it
prints the shape the verb takes now, says the launch settings are "no longer a
flag", and points at `aion awl check`.

### The `harness` section — the whole launch, in the document

The section lives inside a `worker <queue>` block, before that block's `action`
declarations, one setting per line, indented one level under `harness`. Every
word in it — `harness` itself and each field name — is a plain identifier
rather than a reserved keyword: reserving twelve words in every other position
in the language to spell one section would be a poor trade.

```awl
worker default
  harness
    kind norn
    concurrency 4
    reconnect_initial_backoff 1s
    reconnect_max_backoff 30s
    reconnect_max_attempts 100
    env_pass "PATH", "HOME"
    args "--account", "default"
  action assistant(prompt: String) -> Reply
    agent
```

`aion awl fmt` prints the fields in one fixed order — the one above, then
`command`, `args`, `cwd`, `permission`, `exit_grace`, `binary` — whatever order
they were written in, so two operators who declared the same launch end up with
the same bytes and a review diff shows what actually changed.

The section is launch configuration and **not flow meaning**: the MIR ratchet
pins all three checked fixtures to the lowering the same document without a
section produces (`crates/aion-awl/src/mir/covered.rs`). Adding one changes how
a worker process starts an agent, and nothing about what the workflow does.

| Setting | Kind | Value | What it decides |
|---|---|---|---|
| `kind` | both, required | bare word `acp` or `norn` | which harness launches; it scopes every other required setting |
| `concurrency` | both, required | whole number > 0 | maximum activities served at once |
| `reconnect_initial_backoff` | both, required | duration (`5s`, `2m`, `1h`, `1d`), positive | wait before the first reconnect attempt |
| `reconnect_max_backoff` | both, required | duration, positive, not shorter than the initial | the cap the backoff climbs to |
| `reconnect_max_attempts` | both, required | whole number > 0 | cumulative session-drop budget |
| `env_pass` | both, required | quoted variable NAMES, comma-separated | the agent subprocess's WHOLE environment — see below |
| `args` | both, optional | quoted strings, comma-separated, order preserved | the agent's own launch arguments |
| `command` | `acp` only, required | quoted ABSOLUTE path | the agent executable, passed verbatim |
| `cwd` | `acp` only, required | quoted ABSOLUTE path | the directory ACP sessions are rooted at |
| `permission` | `acp` only, required | `"deny"` or `"allow-once"` | how an *unattended* activity answers the agent's permission requests |
| `exit_grace` | `acp` only, required | duration; `0s` is legitimate | how long the agent may take to exit after its stdin closes before it is killed |
| `binary` | `norn` only, optional | quoted path | the `norn` binary; omitted takes the adapter's own documented `norn`-on-`PATH` default |

**The required set is the DECLARED KIND's.** An `acp` section carrying no
`binary` is complete, and a `norn` section carrying no `command`, `cwd`,
`permission` or `exit_grace` is complete: demanding a value that configures
nothing is how a file comes to be full of settings nobody believes. In the
other direction, a setting belonging to the kind the section does *not* declare
is refused by name rather than ignored.

**`aion awl check <FILE.awl>` is where a missing setting is named**, before any
worker starts and before anything is deployed. `permission` and `cwd` in
particular have no defaults and now cannot acquire one: a defaulted permission
policy silently decides whether an unattended agent may write files and run
commands, and a defaulted working directory silently roots an agent somewhere
nobody chose. Moving them into the document did not soften either rule — it
moved the refusal earlier.

A `harness` section on a queue whose actions ALL carry declared `run` bodies is
refused too, naming them: the server runs those from the deployed contract, so
there is nothing on that queue for an agent to serve and the launch would start
a subprocess no work is ever sent to.

#### `env_pass` — the agent's whole environment, declared

A spawned process inherits its parent's environment unless something stops it,
and that inheritance is invisible: nobody wrote it down, nobody reviewed it,
and it changes with whatever shell started the worker. On 2026-08-11 that cost
a live run — an agent's own shell exports `CLAUDECODE`, the worker inherited
it, the spawned `claude-code-acp` saw it, refused to launch nested, and every
`session/new` came back `-32603`. Nothing in the worker's configuration
mentioned `CLAUDECODE`; nothing could have.

So a harness child's environment is **constructed, never inherited**. The
document names the variables; the machine running the worker supplies their
values; the child starts from empty and receives exactly those pairs. A
variable the document does not name is absent in the agent whatever the
worker's own environment carries — which is also how the old
`--norn-unset-env OPENAI_API_KEY` is spelled now: leave `OPENAI_API_KEY` out of
the list and it cannot reach norn.

Entries are **names only**. An entry containing `=` is refused: the value
belongs to the box, and writing one into a document puts a secret in a file
that gets committed, diffed and deployed. A named variable the machine does not
carry contributes nothing and is logged as absent, so a missing credential
surfaces at the worker rather than three layers down inside an agent's own
error message.

One refusal is worth knowing in advance. A `command` that is a bare program
name is looked up on `PATH`, so a launch whose `env_pass` omits `PATH` is
refused **before the spawn**, naming `PATH` — rather than arriving as the
kernel's bare `No such file or directory` against a command you can see is
spelled correctly.

### The `norn` harness

Norn's own setting is `binary`, and it is optional; `args` is optional too. So
a norn that sits on `PATH` and needs no extra arguments is already a complete
declaration — its whole section is the kind-independent six.

```awl
worker default
  harness
    kind norn
    concurrency 4
    reconnect_initial_backoff 1s
    reconnect_max_backoff 30s
    reconnect_max_attempts 100
    env_pass "PATH", "HOME"
    args "--workspace-root", "<workspace_root>/{workflow_id}/repo", "-C", "<workspace_root>/{workflow_id}/repo", "--session-id", "{workflow_id}-{activity_type}", "--resume-if-exists", "--account", "default"
    binary "/usr/local/bin/norn"
  action assistant(prompt: String) -> Reply
    agent
```

Each setting is one whole line, and a list does not wrap: a comma at the end of
a line is a parse error, so `args` and `env_pass` run long. That is the trade
for a refusal that can anchor on the field it is about.

The adapter expands two run-identity placeholders in each `args` entry at spawn
time — `{workflow_id}` and `{activity_type}`; any other `{...}` text passes
through literally (`crates/aion-integration-norn/src/harness.rs`, `expand_arg`).
That is what makes the example worker's session-pinned invocation (below)
reproducible without writing a crate.

`<workspace_root>` there is a value **you** substitute before checking the
document, not a placeholder the adapter expands. It must be the same directory
the server provisions the run's workspace under, or the agent works somewhere
other than where it was provisioned. Read it from the server's JSON startup
banner's `workspace_root` field — that is the server's own resolved value, and
re-deriving it is exactly how two components come to disagree about one path.
See
[DECLARED-COMMANDS.md](DECLARED-COMMANDS.md#workspace_root--how-a-body-learns-where-to-work).

Note what is *not* in that `env_pass`: no `OPENAI_API_KEY`. A stray ambient key
would otherwise take precedence over norn's OAuth login and fail every round,
and the way to keep it out is to not name it.

The composed argv is asserted in the harness tests, not just observed through
live runs — an argument only ever seen firing is untested in the case that
matters.

The verb also recognises norn's degraded-session refusal (see the 28-day story
below) and attaches the route out to the activity failure: add
`"--allow-degraded-session"` to the section's `args` and restart the worker.
The advice goes quiet exactly when that argument is already in the composed
argv.

### The `acp` harness

ACP has no standard launch flag, so nothing is inferred from an agent's name;
every setting is explicit, and the required ones have **no defaults** because
they are security and correctness decisions.

```awl
worker default
  harness
    kind acp
    concurrency 4
    reconnect_initial_backoff 1s
    reconnect_max_backoff 30s
    reconnect_max_attempts 100
    env_pass "PATH", "HOME"
    command "/usr/local/bin/claude-code-acp"
    cwd "/srv/agents/workspace"
    permission "deny"
    exit_grace 10s
  action assistant(prompt: String) -> Reply
    agent
```

`command` is the agent executable and must be an ABSOLUTE path: a worker is
started from whatever directory a supervisor or a shell happens to be in, so a
relative path names a different place on every launch. `cwd` is the directory
ACP's `session/new` roots sessions at, absolute for the same reason. `args`
carries the agent's own launch arguments in order — Gemini CLI needs `--acp`,
OpenCode needs `acp`, Claude Code's ACP adapter needs none. `permission` is
`"deny"` (refuse every gated action) or `"allow-once"` (a one-shot approval,
never a remember-this-choice grant), and it is recorded on the transcript every
time. `exit_grace` is how long the agent gets to flush its logs and release
what it holds once its stdin closes; `0s` means close stdin, then kill at once.

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
`aion worker agent` verb does the same, naming the document instead: add
`"--allow-degraded-session"` to the `harness` section's `args`. The tradeoff,
stated so the decision
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
