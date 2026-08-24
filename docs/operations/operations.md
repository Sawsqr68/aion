# Operations guide

Running, configuring, deploying to, and recovering an Aion server. The
server is started by the one user binary:

```sh
aion server --config aion.toml
```

There is no separate server binary — `aion-server` on crates.io is the
server **library** that `aion server` embeds. Aion runs **single-node by
default**: one server process owns the store. The haematite backend can
additionally run distributed across nodes via the optional `[store.cluster]`
section (see the key reference below); with the section absent the store is
byte-identical to the single-node path.

## Configuration

The server starts from built-in defaults, then applies config-file values,
then `AION_*` environment overrides, then `aion server` flags — last layer
wins. When `--config` is omitted it auto-discovers `aion.toml` in the
working directory (then `<home>/config.toml`). A **hand-written** config
must rule on the three required-no-default keys —
`observability.max_batch_events` and `observability.max_batch_hold_ms` on
every backend, plus `store.node_cache_budget` when the store is haematite
(the default) — because each encodes a deployment trade the server refuses
to decide silently. Every other always-mounted surface has a default, and
the remaining keys only become required when the operator commissions the
surface they belong to. (The first-run scaffold below carries all three, so
a machine with no config at all still boots.)

A boot that discovers **no config at any layer** first scaffolds
`<home>/config.toml` (owner-only, `0600`) from the server's embedded
template — the same file `aion new` puts in a project — with the deploy
surface and the outbox worker channel enabled, then boots on it and logs the
written path. An existing file is never touched, and an explicit `--config`
naming a missing file still refuses rather than inventing one
(`crates/aion-server/src/config/scaffold.rs`).

### `aion server` flags

| Flag | Effect |
|---|---|
| `--config <path>` | Explicit TOML config file; skips auto-discovery. |
| `--listen-address <addr>` | Override `server.listen_address` (HTTP/JSON + ops console). |
| `--grpc-address <addr>` | Override `server.grpc_address` — the server's **own bind address**. The global `--endpoint` is a *client* dial address; `aion server` refuses to start if it is passed rather than silently binding the default port. |
| `--scheduler-threads <n>` | Override `runtime.scheduler_threads`; must be non-zero. |
| `--jit-threshold <n>` | Override `runtime.jit_threshold`; must be non-zero. Recorded calls to one `(module, function, arity)` before the embedded VM JIT-compiles it. Omitted, the VM applies its own default. **Moves the moment of compilation; it does not switch compilation off** — the call counter saturates rather than stopping, so a very large value defers compilation past any realistic workload but it still occurs beyond that horizon. |
| `--drain-timeout <seconds>` | Override `drain.timeout_seconds`; must be non-zero. |
| `--workflow-package <path>` | Extra `.aion` archive loaded at startup; repeatable (see "Boot-time loads"). |
| `--gleam-path <path>` | Set `authoring.gleam_path`: commissions the server-side Gleam authoring loop and mounts `/authoring/*`; requires `--authoring-project-root`. |
| `--authoring-project-root <path>` | Set `authoring.project_root`: the built Gleam project submitted source is written into and packaged from. |
| `--open` | Open the ops console in the default browser once the HTTP listener answers `/health/live`. Best-effort: failures are logged and never affect the server run. |

### The bare launcher: `aion` with no subcommand

Bare `aion` is the background form of `aion server`
(`crates/aion-cli/src/launch.rs`). It resolves the same effective config the
server would, probes the resolved HTTP address first — a live Aion server
(HTTP 200 on `/health/live`) is reported and the console opened, never
double-started; a port held by anything else is refused by name with exit 2 —
then spawns `aion server` in its own process group with stdout+stderr
appended to `<home>/server.log`, waits up to 60 seconds for liveness, and
reports in plain lines: the config (including whether the first boot
scaffolded it), the data directory, the console URL, the log path, and the
process id that stops it (`kill <pid>`). A spawned server that dies during
startup, or is still not live at the deadline, is reported loudly with the
tail of its log — a still-starting server is left running and named, never
killed. `--foreground` runs the server in the terminal instead (the ordinary
`aion server` path with `--open` semantics); `--no-open` skips the browser.
Both flags belong to the bare form only and are refused next to a subcommand.

### Conditionally required keys (startup fails naming the missing key)

| Key | Required when |
|---|---|
| `auth.jwks_url` | `auth.enabled = true`. |
| `authoring.project_root` | `authoring.gleam_path` is set. |
| `store.cluster.node_id`, `store.cluster.bind_address` | a `[store.cluster]` section is present (and each `[[store.cluster.peers]]` entry needs `name` + `address`). |
| `outbox.reconcile_interval_ms` **and** `outbox.reconcile_stale_after_ms` | either one is set — the pair is both-or-neither; both absent leaves reconciliation dark. |
| `outbox.liminal_listen_address` | `outbox.enabled = true` with `transport = "liminal"` (surfaced when the dispatcher spawns). |
| every `[worker_supervision]` key | ANY `[worker_supervision]` key is set — the section is all-or-nothing and has no defaults. Absent entirely = managed-worker supervision uncommissioned; partial = startup fails naming each missing key. |

### Retired store inputs (startup refuses, naming the remedy)

The libSQL backend is gone. Five inputs used to select it, and each one is now
a startup refusal rather than a silent ignore or a fallback:

| Retired input | Where |
|---|---|
| `backend = "libsql"` | `[store]` in the config file |
| `store.url` (`url = …`) | `[store]` in the config file |
| `AION_STORE_BACKEND=libsql` | environment |
| `AION_STORE_URL` | environment |
| `--store-url` | `aion server` command line |

Any one of them stops startup with a message naming the input it found and
prescribing `backend = "haematite"` plus `store.data_dir`. This build does not
read existing libSQL database files; there is no migration path and no
compatibility mode. **0.17.0 is the last release that can read them** — keep a
0.17.0 binary to reach an existing libSQL database before upgrading.

Tuning knobs (`runtime.query_timeout_ms`, the websocket capacities, the
deploy ceilings, the outbox knobs, `observability.max_event_bytes` and
`observability.max_stream_events`) default when **omitted** but reject an
explicit `0`. The transcript batch bounds are NOT tuning knobs:
`observability.max_batch_events` and `observability.max_batch_hold_ms` are
required and have no defaults, exactly like `store.node_cache_budget` on a
haematite boot. Validation failures are loud and name both the TOML key and
its environment override, e.g.:

```
runtime.query_timeout_ms must be a positive integer when set (the server
always mounts /workflows/query, so a zero deadline would fail every query
immediately); omit runtime.query_timeout_ms (or
AION_RUNTIME_QUERY_TIMEOUT_MS) to use the default, or set it to a positive
number of milliseconds
```

### Full key reference

```toml
# Workflow package archives loaded at startup (see "Boot-time loads" below).
workflow_packages = []                  # default: []; .aion files in the
                                        # working directory are auto-discovered

[server]
listen_address = "127.0.0.1:8080"       # default; HTTP/JSON + ops console
grpc_address = "127.0.0.1:50051"        # default; gRPC API + worker protocol
                                        # both need explicit non-zero ports
cors_allowed_origins = []               # default: empty — the SECURE default:
                                        # no cross-origin request permitted, no
                                        # CORS layer installed. Entries are
                                        # exact origins (scheme://host[:port],
                                        # no path, no wildcard) matched against
                                        # the browser's Origin header

[store]
backend = "haematite"                   # default; durable ablative event store.
                                        # "memory" = ephemeral, loses state on stop
data_dir = "…"                          # haematite database directory, created
                                        # on start; defaults to <home>/data (see
                                        # "The Aion home"). haematite only
shard_count = 64                        # default; virtual shards created on a
                                        # FRESH haematite db. Immutable at create
                                        # (routing is BLAKE3(key) % shard_count,
                                        # no reshard path); opening an existing
                                        # db, the on-disk count always wins
owned_shards = []                       # default: empty = this node owns ALL
                                        # shards (the single-node default). Set =
                                        # static shard assignment for multi-node
                                        # haematite; ignored by memory
node_cache_budget = "unlimited"         # REQUIRED, no default (haematite): byte
                                        # ceiling the node caches may hold —
                                        # `{ bytes = N }` or "unlimited" (the
                                        # pre-budget behaviour, stated out loud)

# Optional distributed-haematite membership. Absent (the default) = the
# single-node path, byte-identical to before the section existed. Only valid
# when backend = "haematite". A cluster of one — node_id + bind_address, no
# members/peers — is valid and self-quorums.
# [store.cluster]
# node_id = "node-0@127.0.0.1"          # REQUIRED: this node's unique
                                        # distribution name
# bind_address = "127.0.0.1:7000"       # REQUIRED: replication/quorum listener
# members = []                          # FULL membership by node id — the
                                        # quorum DENOMINATOR, never the
                                        # reachable subset; empty is treated
                                        # as [node_id]
# failover_poll_interval_ms = 500       # default; peer-liveness poll cadence
# failover_confirmations = 3            # default; consecutive disconnected
                                        # polls before a dead peer's shards
                                        # are adopted (must be >= 1)
# [[store.cluster.peers]]               # dialable peers; none for a cluster of one
# name = "node-1@127.0.0.1"             # REQUIRED: matches the peer's node_id
# address = "127.0.0.1:7001"            # REQUIRED: peer replication endpoint
# grpc_address = "127.0.0.1:60051"      # optional: peer client-API address for
                                        # request forwarding; absent = a typed
                                        # NotOwner error instead of forwarding
# owned_shards = [32, 33]               # shards the peer owns; must be declared
                                        # here for automatic failover to know
                                        # what to adopt when the peer dies

[runtime]
scheduler_threads = 1                   # default; must be > 0
# jit_threshold = 1000                  # ABSENT by default: the embedded VM's
                                        # own default governs. Set only to move
                                        # when hot code is compiled; a large
                                        # value defers compilation, it does not
                                        # disable it
query_timeout_ms = 10000                # default; reply deadline for
                                        # /workflows/query. Explicit 0 rejected

[drain]
timeout_seconds = 30                    # default; graceful shutdown bound

[auth]
enabled = false                         # default; dev mode trusts headers
jwks_url = ""                           # REQUIRED (non-empty) when enabled = true
jwks_refresh_seconds = 300              # default; must be > 0

[metrics]
enabled = true                          # default; serves GET /metrics

[namespaces]
default = "default"                     # default namespace name
auto_create = "open"                    # default; a worker registering for an
                                        # unseen namespace durably mints it.
                                        # "closed" rejects instead (the escape
                                        # hatch is an explicit POST /namespaces)
max_in_flight_activities = 1024         # default; cluster-wide per-namespace
                                        # concurrent-activity ceiling for
                                        # namespaces with no explicit override.
                                        # Stored-only today — nothing enforces
                                        # it yet

[ops_console]                           # canonical name; the legacy [dashboard]
                                        # spelling is accepted as a serde alias
source = "Embedded"                     # default: the compile-time embedded
                                        # console bundle
# source = { FileSystem = { asset_path = "/srv/console" } }
                                        # serve an operator-supplied built
                                        # bundle directory instead

[namespace]
mode = "SharedEngine"                   # default; namespace resolver mode
# mode = { SingleTenant = { namespace = "only" } }

[worker]
heartbeat_window = 30000                # default (ms); heartbeat cadence
                                        # advertised to workers in RegisterAck
                                        # (see "Workers and activity duration")

[worker.queue_service]                  # R1 dispatch policy; section absent =
                                        # every queue strict, no clocks
default_policy = "strict"               # default; a dispatch to a structurally
                                        # unserved queue refuses rather than
                                        # starting a lie. "durable_pending" =
                                        # mark the run unserved and park it
                                        # durably until a worker arrives
# service_availability_deadline = 45000 # (ms) how long a dispatch waits for a
                                        # compatible worker to EXIST before a
                                        # typed refusal; unset = unbounded.
                                        # Applies to strict-policy queues only
# schedule_to_start_timeout = 5000      # (ms) how long a dispatch tries to
                                        # hand its task to a live compatible
                                        # worker; unset = one attempt
# [[worker.queue_service.overrides]]    # written per-queue opt-ins; the first
                                        # entry matching a dispatch wins
# namespace = "lab"                     # optional; absent = every namespace
# task_queue = "general"                # REQUIRED
# policy = "durable_pending"            # REQUIRED

[websocket]
outbound_buffer_bound = 32              # default; per-connection buffer
event_broadcast_capacity = 256          # default; engine-global /events/stream
                                        # channel. Lag is filter-blind — size
                                        # for global event volume across all
                                        # namespaces. Explicit 0 rejected
cluster_broadcast_capacity = 64         # default; low-rate cluster-topology
                                        # channel backing the WS3 cluster
                                        # subscription. Explicit 0 rejected

[deploy]
enabled = false                         # default: surface dark — /deploy/*
                                        # not mounted, gRPC DeployService
                                        # answers Unimplemented
max_archive_bytes = 67108864            # default 64 MiB when enabled; upload
                                        # ceiling for .aion archives
max_inflated_bytes = 268435456          # default 256 MiB when enabled; must be
                                        # >= max_archive_bytes (a DEFLATE bomb
                                        # under the upload ceiling can inflate
                                        # ~1000:1)

[authoring]
# workspace_dir = "…"                   # AWL studio root, on by default;
                                        # defaults to <home>/authoring (see
                                        # "The Aion home")
# gleam_path = "/usr/local/bin/gleam"   # commissions the server-side Gleam
                                        # authoring loop and mounts
                                        # /authoring/*; no default — absent,
                                        # the server compiles no Gleam
# project_root = "/srv/aion/flows"      # REQUIRED when gleam_path is set; a
                                        # built Gleam project (gleam.toml, the
                                        # aion_flow dependency, workflow.toml,
                                        # schemas/) the operator provisions

[dev]
enabled = false                         # default: /dev/* not mounted, no
                                        # per-run activity-mock decorator. The
                                        # on/off gate is the only knob

[outbox]                                # durable-outbox fan-out dispatcher
enabled = false                         # default: dispatcher never spawned
poll_interval_ms = 20                   # default when enabled; claim-sweep cadence
batch_size = 16                         # default when enabled; rows claimed per sweep
max_attempts = 5                        # default when enabled; attempts before
                                        # a row is dead-lettered to failed
backoff_base_ms = 50                    # default when enabled; first-retry backoff
backoff_multiplier = 2                  # default when enabled; geometric growth
                                        # per prior attempt (must be >= 1)
backoff_max_ms = 1000                   # default when enabled; per-retry backoff
                                        # ceiling (must be >= backoff_base_ms)
# reconcile_interval_ms = 30000         # stale-claim reconciliation sweep — NO
# reconcile_stale_after_ms = 60000      # defaults: both absent = reconciliation
                                        # dark; setting either requires both
transport = "liminal"                   # default in the stock build (the
                                        # liminal-transport feature); a slim
                                        # build without it defaults to "grpc"
# liminal_listen_address = "0.0.0.0:7400"
                                        # REQUIRED when transport = "liminal":
                                        # the address the SERVER listens on for
                                        # inbound liminal worker connections
                                        # (workers connect in and self-register)

[observability]
max_batch_events = 64                   # REQUIRED, no default: most transcript
                                        # events one durable commit carries —
                                        # one commit re-persists the whole
                                        # containing storage leaf, so this is
                                        # a write-amplification decision
max_batch_hold_ms = 200                 # REQUIRED, no default: how long a
                                        # partial batch may be held open
                                        # waiting to fill — bounds how much
                                        # not-yet-durable transcript a crash
                                        # can cost (0 = never wait)
max_event_bytes = 262144                # default (256 KiB); ceiling on one
                                        # persisted transcript event's
                                        # serialized size (oversized events
                                        # are truncated before persisting)
max_stream_events = 20000               # default; retained events per
                                        # (workflow, activity, attempt)
                                        # transcript stream — past the cap one
                                        # marker record is kept and further
                                        # events stay live-only

[mcp]
enabled = false                         # default; DARK. With it false the
                                        # /mcp endpoint is not mounted and the
                                        # path is a plain 404. An MCP endpoint
                                        # lets a model start and cancel durable
                                        # executions and — behind the deploy
                                        # grant — save and deploy AWL
                                        # documents, so turning it on is an
                                        # explicit decision
allowed_origins = []                    # default; EMPTY means no browser
                                        # origin may reach /mcp. A programmatic
                                        # client sends no Origin header and is
                                        # unaffected; a browser always does, so
                                        # the empty list refuses every
                                        # browser-borne request (the
                                        # DNS-rebinding defence the transport
                                        # requires)
# discover_ttl_ms = 60000               # default; ttlMs on server/discover
# tools_list_ttl_ms = 300000            # default; ttlMs on tools/list
# task_poll_interval_ms = 1000          # default; pollIntervalMs advertised to
                                        # a task's client. A hint only —
                                        # nothing on the server depends on it
#
# There is NO task lifetime setting, and none is coming. An MCP task here is
# the run itself read through the task model: nothing is stored, nothing is
# held, so nothing expires. `ttlMs` is always null (the extension's spelling of
# unlimited), a task survives a server restart because the run does, and a task
# nobody ever polls costs the server nothing at all. If you are upgrading and
# your file still sets `task_ttl_ms` or `await_poll_interval_ms`, the server
# refuses to boot and names the key — silently ignoring it would leave you
# believing a lifetime was in force when nothing enforces one.

# Managed-worker supervision. NO DEFAULTS and no partial section: write every
# key below to commission it, or omit the section entirely and the server
# supervises no worker processes (and says so, with this remedy, on every
# status read and on any attempt to start one). A restart discipline has real
# operational consequences, so it is an operator decision, never a guess.
[worker_supervision]
restart_backoff_initial_ms = 500        # delay before the first restart in a
                                        # window
restart_backoff_max_ms = 30000          # ceiling the geometric backoff clamps
                                        # to (must be >= the initial delay)
restart_backoff_multiplier = 2          # growth per restart already made in
                                        # the window (1 = flat cadence)
restart_window_ms = 60000               # sliding window restarts are counted
                                        # over
max_restarts_per_window = 5             # restarts allowed in one window before
                                        # the instance is FAILED and left down
stop_grace_ms = 5000                    # grace given to EACH of SIGTERM and
                                        # SIGKILL when stopping a worker's
                                        # process group
```

The MCP surface is served on the **same HTTP listener** as the console and the
workflow API — one port, one process — and it inherits the server's existing
authentication exactly: the caller is resolved by the same extractor every
other route uses. Authorization then follows each tool family's own scope.
The eight run tools apply the same **namespace authorization** as the
workflow API — a namespace the caller does not hold answers as not-found.
The five authoring tools are **not namespace-scoped**: the authoring
workspace is one deployment-global directory, exactly as it is for the ops
console studio, so any authenticated caller can list, read, and check, and
the write pair is gated by the deployment-wide deploy grant instead. There is
no MCP-specific auth to configure and none to drift.

It publishes **thirteen tools**. Eight drive runs: `describe_run`,
`read_transcript`, `read_history`, `list_runs`, `query` (reads), and
`start_run`, `signal`, `cancel` (mutations; `cancel` is the only destructive
one). Five author AWL documents in the server's authoring workspace
(`authoring.workspace_dir`) through the same seams the ops console studio
uses: `list_documents`, `read_document`, and `check_document` need only an
authenticated caller, while `save_document` and `deploy_document` require the
**deploy grant** — the same grant `/awl/deploy` requires, carried by the JWT
`deploy` claim or the development `x-aion-deploy` header — and refuse
`deploy_denied`, naming the missing grant, without it. Deploy takes
`{path, content_hash}` and verifies the hash against the **saved** document
(the hash comes from `save_document` or `read_document`); a mismatch is
refused as `RevisionMismatch`, and there is deliberately no way to deploy
unsaved source. With no workspace configured, every authoring tool refuses
loudly and names `authoring.workspace_dir` — a state only an embedder
constructing the runtime config directly can reach: a served `aion server`
always fills the `<home>/authoring` default, so omitting the key does not
disable MCP authoring.

It implements the **2026-07-28 stateless revision** (`POST /mcp` only; `GET`
and `DELETE` are 405; no sessions, no SSE resumability, no `initialize`
handshake) plus the experimental `io.modelcontextprotocol/tasks` extension,
pinned to `ext-tasks` commit `2c1425d9a288b9b1f489430fe1e00bb392b47e48`. The
extension is advertised only to a client that declares it on the request.

(`[tls]` keys parse — `certificate_chain_path` and `private_key_path` — but
the server refuses to start with TLS material configured until transport
TLS is wired. See [`docs/API.md`](API.md) for the namespace/auth trust
model.)

### The Aion home (`~/.aion` and `AION_HOME`)

The server resolves one **home directory** — its user-level config and state
root — before it reads any config file
(`crates/aion-server/src/config/home.rs`):

| Situation | Resolved home |
|---|---|
| `AION_HOME` is set | that path (`~`/`~/…` expands against `$HOME`; relative paths resolve against the current directory; `~user` is refused) |
| `AION_HOME` is unset | `$HOME/.aion` |
| `AION_HOME` is set but empty | startup fails: `AION_HOME must not be empty` |
| neither `AION_HOME` nor `HOME` identifies a directory | startup fails: `cannot resolve Aion home: set AION_HOME or HOME` |

Config file discovery reads the first of these that exists
(`crates/aion-server/src/config/file.rs`):

1. `--config <path>`, when given;
2. `./aion.toml` in the working directory;
3. `<home>/config.toml`;
4. built-in defaults.

Three paths live *below* the home. Two are config keys with defaults
(`crates/aion-server/src/config/resolution.rs`, `defaults.rs`); the third is
derived and has no key at all:

| Path | Config key | What it holds |
|---|---|---|
| `<home>/data` | `store.data_dir` | the haematite database directory |
| `<home>/authoring` | `authoring.workspace_dir` | the authoring/AWL workspace root |
| `<home>/clones` | *none — derived* | session workspaces: the repositories and agent working trees a declared action body runs in |

`clones/` is the **workspace root** (`crates/aion-server/src/worker/workspace_root.rs`).
A declared action body runs with its environment cleared to `PATH` only, so it
cannot expand `~` or read a variable to learn where workspaces live; instead it
may carry the literal placeholder `{workspace_root}`, and the dispatch path
replaces every occurrence with this path before the command is parsed. There is
deliberately no configurable value — the home is already the operator's one
answer to where a server's state lives, and a second answer is how two
components come to disagree about one path. Durable history records workspace
paths, which is why they live under the home and never in the OS temp
directory.

Unlike the other two roots, `clones/` is **not** provisioned at boot: it is
created (recursively, `0700`) at the moment a placeholder-bearing body is first
dispatched, and an already-existing directory is left exactly as found. The
path is resolved once at state construction and reported on the startup banner
as `workspace_root`; a resolution failure is carried rather than raised, so
boot proceeds and the failure surfaces as a terminal refusal of the one
dispatch that needed it.

A legacy `./aion-data` or `./aion-authoring` directory in the working directory
is still adopted **only when the home was derived** from `$HOME`; setting
`AION_HOME` explicitly means the operator has named where state lives, so a
working-directory legacy is ignored rather than allowed to win silently. Either
way, the choice is logged.

Aion **provisions these roots itself**. It creates the home, the store data
root, and the authoring root owner-only (`0700`, applied by the `mkdir` itself
rather than a create-then-chmod window), and if one already exists, grants
group or world access, and is owned by the server's own user, it tightens the
mode in place and logs the change at INFO with the previous and applied modes.
You do not have to `chmod` anything by hand. A root Aion cannot make safe still
refuses startup, naming the path, the problem, and the remedy: a root owned by
another uid, a symlinked root or path component, a name occupied by a
non-directory, or a filesystem that will not carry Unix modes.

Setting `AION_HOME` per server is how you run more than one Aion on one box
without them sharing state.

### Environment overrides

Overrides follow `AION_<SECTION>_<KEY>` (uppercase, underscores), but they
are an **explicit allowlist**, not a generated mapping over every key. The
supported variables are:

- `AION_SERVER_LISTEN_ADDRESS`, `AION_SERVER_GRPC_ADDRESS`,
  `AION_SERVER_CORS_ALLOWED_ORIGINS` (comma-separated; empty clears the list)
- `AION_STORE_BACKEND`, `AION_STORE_DATA_DIR`, `AION_STORE_SHARD_COUNT`,
  `AION_STORE_NODE_CACHE_BUDGET` (`{ bytes = N }` or `unlimited` — the
  refusal for the absent key names this variable as its remedy)
- `AION_RUNTIME_SCHEDULER_THREADS`, `AION_RUNTIME_QUERY_TIMEOUT_MS`,
  `AION_RUNTIME_JIT_THRESHOLD`
- `AION_DRAIN_TIMEOUT_SECONDS`
- `AION_AUTH_ENABLED`, `AION_AUTH_JWKS_URL`, `AION_AUTH_JWKS_REFRESH_SECONDS`
- `AION_METRICS_ENABLED`
- `AION_NAMESPACES_DEFAULT`
- `AION_WEBSOCKET_OUTBOUND_BUFFER_BOUND`,
  `AION_WEBSOCKET_EVENT_BROADCAST_CAPACITY`,
  `AION_WEBSOCKET_CLUSTER_BROADCAST_CAPACITY`
- `AION_DEPLOY_ENABLED`, `AION_DEPLOY_MAX_ARCHIVE_BYTES`,
  `AION_DEPLOY_MAX_INFLATED_BYTES`
- `AION_DEV_ENABLED`
- `AION_AUTHORING_GLEAM_PATH`, `AION_AUTHORING_PROJECT_ROOT`,
  `AION_AUTHORING_WORKSPACE_DIR`
- `AION_OUTBOX_ENABLED`, `AION_OUTBOX_POLL_INTERVAL_MS`,
  `AION_OUTBOX_BATCH_SIZE`, `AION_OUTBOX_MAX_ATTEMPTS`,
  `AION_OUTBOX_BACKOFF_BASE_MS`, `AION_OUTBOX_BACKOFF_MULTIPLIER`,
  `AION_OUTBOX_BACKOFF_MAX_MS`, `AION_OUTBOX_RECONCILE_INTERVAL_MS`,
  `AION_OUTBOX_RECONCILE_STALE_AFTER_MS`,
  `AION_OUTBOX_LIMINAL_LISTEN_ADDRESS`
- `AION_OBSERVABILITY_MAX_EVENT_BYTES`, `AION_OBSERVABILITY_MAX_STREAM_EVENTS`,
  `AION_OBSERVABILITY_MAX_BATCH_EVENTS`, `AION_OBSERVABILITY_MAX_BATCH_HOLD_MS`
  (the two required batch bounds; their refusals name these variables as the
  remedy)

Structured values — `[store.cluster]`, `store.owned_shards`,
`[worker.queue_service]`, `worker.heartbeat_window`, `[ops_console]`,
`[namespace]`, `[tls]`, `namespaces.auto_create`,
`namespaces.max_in_flight_activities`, `outbox.transport`,
`[worker_supervision]`, and `workflow_packages` — are config-file (or flag)
only. `AION_HOME` is **not**
a config key — it is the home resolver above. Examples:

```sh
AION_SERVER_GRPC_ADDRESS=127.0.0.1:60051
AION_STORE_BACKEND=haematite            # default; "memory" to opt out
AION_STORE_DATA_DIR=aion-data           # haematite database directory
AION_RUNTIME_QUERY_TIMEOUT_MS=10000
AION_DEPLOY_ENABLED=true
```

Unknown `AION_*` variables are ignored. `AION_LOG` sets the tracing filter
(takes precedence over `RUST_LOG`); logs are JSON on stdout — pipe through
`jq` for interactive reading.

### The startup banner

Once config is resolved and server state is built — but before either
transport binds — the server emits exactly one `INFO` line whose message is
`aion-server startup banner` (`crates/aion-server/src/run.rs`). It is the
**supported way to read a running server's own resolved settings**, including
the addresses it is about to bind: read the value the server states rather than
re-deriving it from config, because config layering, flags, and env overrides
all land before this line is written.

Because logs are JSON on stdout, one `jq` filter pulls it out:

```sh
aion server --config aion.toml | jq -c 'select(.fields.message == "aion-server startup banner")'
```

Sixteen fields ride on it:

| Field | What it carries |
|---|---|
| `version` | the `aion-server` crate version (`CARGO_PKG_VERSION`) |
| `build` | the full build identity line — `<version>+<commit>[-dirty] built <rfc3339>` |
| `commit` | the source commit this binary was built from, or `unknown` for a build with no repository (a crates.io tarball, a vendored copy). A crate version cannot distinguish two builds from different commits of the same version; this can. The same identity is served at `GET /build` |
| `grpc_address` | the resolved gRPC + worker-protocol bind address |
| `http_address` | the resolved HTTP/JSON + ops-console bind address |
| `default_namespace` | `namespaces.default` as resolved |
| `namespace_mode` | `SharedEngine` or `SingleTenant` |
| `store_backend` | `haematite` or `memory` |
| `auth_enabled` | whether `[auth]` is commissioned |
| `deploy_enabled` | whether the deploy surface is mounted |
| `metrics_enabled` | whether `GET /metrics` serves |
| `workspace_root` | the resolved `<home>/clones` path declared bodies expand `{workspace_root}` with, or the literal text `unresolvable: <reason>` — never a fabricated path |
| `workflow_package_count` | how many boot-time `.aion` archives were resolved |
| `workflow_packages` | those archive paths, in load order |
| `owned_shards` | this node's statically assigned shard set from `[store] owned_shards` |
| `owns_all_shards` | `true` when that set is empty — the single-node default |

## Loading workflow code: two paths, one model

### Boot-time loads (config / flag / auto-discovery)

At startup the server loads, in order: the `workflow_packages` config array,
`.aion` files auto-discovered in the working directory (alphabetical), and
repeatable `--workflow-package <path>` flags — deduplicated by canonical
path. Last load wins the route per workflow type.

Boot-time loads are **not persisted**: they come from your config/filesystem
on every boot, and they win the route at boot. They are the right tool for
fixed, config-managed deployments.

### Runtime deploys (the deploy surface)

With `[deploy] enabled = true`, packages can be deployed to a **running**
server:

```sh
aion deploy my-flow.aion                # load + atomic route flip
aion versions [--workflow-type my_flow] # read model: every loaded version
aion route my_flow <content-hash>       # rollback / roll-forward
aion unload my_flow <content-hash>      # remove a non-routed, unpinned version
```

Runtime-deployed packages **persist in the event store** and are reloaded at
startup **before workflow recovery runs**, so recovered workflows always
find the exact code version they started on. (This is the key difference
from boot-time loads, which re-resolve from config each boot.)

Semantics worth knowing:

- **Deploy is idempotent.** Re-deploying a resident archive succeeds with
  `freshly_loaded: false`; `route_changed` reports whether routing moved.
  A deploy pipeline may retry blindly.
- **Versions are immutable and content-hashed.** Each package version
  deploys as modules named `<module>$<sha256>`; the hash covers compiled
  code only.
- **Running workflows are pinned.** A run executes the version it started
  on for its whole life, recorded in its history. New starts follow the
  route.
- **Unload is refused (`version_pinned`, HTTP 409) while anything pins the
  version**: it is route-active, a live instance runs it, a recoverable run
  recorded it, or an in-flight start holds it. Route away first; runs
  pinning an old version must finish before it can be unloaded.
- **A package load is engine-global.** Routing is per workflow *type*,
  startable from every namespace; the deploy surface has no namespace field.

**Authorization** is a deployment-wide `deploy` grant: a boolean `deploy`
claim in the bearer token when auth is enabled, or the development
`x-aion-deploy: true` header (the CLI sends it automatically; `--token`
overrides the `AION_TOKEN` environment variable). Denials are
`deploy_denied` (HTTP 403).

### Audit and metrics

Every deploy mutation emits one structured audit log line with `operation`
(`deploy.load` / `deploy.route` / `deploy.unload`), `subject`,
`grant_source`, `transport`, `workflow_type`, `content_hash`, `outcome`
(`loaded` / `idempotent` / `rerouted` / `unloaded`), and for loads
`freshly_loaded` / `route_changed`. Denials log at `warn`.

Metrics (Prometheus text at `GET /metrics` when `[metrics] enabled`):

- `aion_deploy_operations_total{operation, outcome}`
- `aion_deploy_denied_total{transport}`
- `aion_loaded_workflow_versions{workflow_type}` (gauge)

## Persistence and recovery

Status is a projection of event history; there is no mutable run state to
lose. With the durable **haematite** event store (the default):

1. Every workflow event (start, activity scheduled/completed, timer fired,
   signal, terminal) is appended durably as it happens.
2. On startup — including after `kill -9` — the server reloads
   runtime-deployed packages from the store, then replays each active run's
   history. Workflow code re-executes from the top; every recorded
   side-effect call returns its recorded result instead of acting again, so
   the run lands exactly where it was: same await, same pending timers, same
   registered query handlers.
3. Durable timers are re-armed from history; a workflow sleeping across the
   restart still wakes on schedule.

The **memory** store keeps the same semantics within a process lifetime but
loses everything on stop — development only.

Recovery caveats to plan around:

- An activity that was *executing on a worker* during the crash is
  re-dispatched (its completion was never recorded). Make activities
  idempotent.
- Boot-time-loaded packages must still be present at their configured paths;
  a recovered run pinned to a version the server cannot load cannot resume.

## Workers and activity duration

**The engine imposes no activity timeout.** An activity runs as long as its
worker is working on it — agent-style activities legitimately run for over
an hour. A running activity is bounded only by:

1. **The workflow's own `timeout_seconds`** — the deadline the workflow
   author declared for the whole run.
2. **Worker liveness**, detected two ways. A worker whose gRPC stream ends —
   process death, network disconnect, expired token — is declared lost
   immediately. A worker whose connection stays open but goes *silent* past
   `[worker] heartbeat_window` is expired by the heartbeat sweeper, which the
   server always commissions (`HeartbeatSweeper` in
   `crates/aion-server/src/worker/heartbeat.rs`, spawned unconditionally in
   `crates/aion-server/src/run.rs`; cadence is a quarter-window, clamped to
   `[1s, window]`). Either way the worker is deregistered and its in-flight
   activities are failed back to the engine. Nothing hangs waiting on a dead
   worker.

   Worker loss is a **transport-domain** failure, deliberately *not* the
   action's `retryable:` class
   (`crates/aion-server/src/worker/transport_loss.rs`): the activity never
   produced a result, so it consumes none of the authored retry budget and is
   re-dispatched at the **same attempt**. That neutrality is bounded by the
   transport's own ceiling — four `heartbeat_window`s of wall clock from an
   execution site's first loss, with no separate knob — after which the failure
   names itself as transport exhaustion rather than wearing the action's name.
   An operator reading a terminal event can always tell "your action is red"
   from "your infrastructure is flapping".

`[worker] heartbeat_window` is the heartbeat cadence the server advertises to
workers in `RegisterAck`, the silence threshold the sweeper expires against,
and the grace period an expired-token stream gets to re-authenticate. It does
**not** cap activity duration: what refreshes liveness is any frame from the
worker, and the worker runtime pumps automatic payload-free liveness beats at a
quarter-window cadence, so a healthy worker running an hour-long activity is
never expired. Explicit `heartbeat(details)` calls carry operator-visible
progress on top of that; a beat without a payload never erases the last
progress report. Author-declared per-activity timeouts are still unbuilt: the
Gleam SDK emits a `timeout_ms` field into each dispatch's `config` JSON and no
engine component reads it (tracked under Track F of
[`docs/vision/ROADMAP.md`](../vision/ROADMAP.md)).

## Managed workers: the server owns the process

A worker can be a process **the server starts, supervises, and stops** rather
than one an operator launches by hand. There is no init system in this story:
supervision is Aion-native (launchd/systemd as the worker supervisor is a
standing non-goal).

**Desired state is durable.** A `WorkerDeployment` record — created through
`PUT /worker-deployments/{name}` under the deploy grant — names the artifact
(today: `builtin`, meaning this server's own binary plus an operator-declared
argv tail such as `["worker", "shell", "--manifest", "…"]`), the namespaces and
task queue it serves, and whether it should be `running` or `stopped`. The argv
tail is the whole launch contract: nothing is appended or inferred, so what runs
is exactly what the record says.

**Actual state is a supervised process.** With `[worker_supervision]`
configured, the server converges actual toward desired: it spawns the process as
the leader of a fresh **process group**, restarts it on exit with the configured
backoff, and gives up loudly — status `failed`, naming
`max_restarts_per_window` — when a crash loop spends its budget. At boot the
whole fleet is reconciled from the durable records, so a server restart brings
the workers back without anyone typing anything.

**Verbs and reads.**

| Operation | CLI | Wire |
|---|---|---|
| Report the fleet | `aion worker status` | `GET /workers/managed`, `DeployService.ListManagedWorkers` |
| Start (and record the intent) | `aion worker start <name>` | `POST /workers/managed/{name}/start`, `DeployService.StartManagedWorker` |
| Stop (terminal) | `aion worker stop <name>` | `POST /workers/managed/{name}/stop`, `DeployService.StopManagedWorker` |
| Restart (intent stays running) | `aion worker restart <name>` | `POST /workers/managed/{name}/restart`, `DeployService.RestartManagedWorker` |

All of them carry the deployment-wide **deploy grant**, the same authority as
the deployment records themselves. The three lifecycle mutations are mounted
only when `[deploy] enabled = true` — the same switch that mounts the gRPC
`DeployService` and the `/worker-deployments/*` routes — so on a server that
is not a deploy target they are a plain 404 on every transport, while the
status read (`GET /workers/managed`) always answers. Each HTTP mutation
responds `{"worker": …}` with the same worker shape the status read lists, and
every durable desired-state change these verbs make is published on the
cluster event stream (`WorkerDeploymentDesiredStateChanged`), whichever
transport asked.

Every status line reports **desired and actual together**, because either alone
is the misleading half — `desired: running, state: stopped` is a real and
important condition. It also reports the deploy-time binary hash beside the hash
measured **at the most recent spawn**, so a restart that picked up an upgraded
binary is a stated fact rather than a silent swap. A server with no
`[worker_supervision]` section reports `commissioned: false` with the remedy,
and its deployments read `uncommissioned` — never `stopped`, which would look
like somebody meant it.

**Kill guarantees, precisely.** A stop signals the whole process group
(`SIGTERM`, the configured `stop_grace`, then `SIGKILL`) and returns success
only once the worker's output streams have reached **end-of-file** — which no
process still holding them open can produce, and which is therefore proof that
the whole tree is gone. Anything else is reported as a failure naming what was
observed, so "the record says stopped" can never stand in for "the process is
gone". A worker's own children (an agent it spawned, a command it ran) die with
it, because they are in that group. Server shutdown stops every managed worker
the same way, after the drain wait, and durable desired state is deliberately
left alone — the next boot brings the fleet back up.

Two details worth stating plainly. Signals are only ever sent while the worker's
leading process is still un-reaped: a process-group id is that process's pid,
and a pid is reusable the moment it is reaped, so signalling later would aim at
whatever inherited the number. That is why the proof is end-of-file rather than
a process-group probe. And `kill -9` of the **server** runs none of this, so a
hard-killed server can leave managed workers behind; the ruled stronger
mechanisms for that (cgroup v2 on Linux, a dead-man launcher on macOS) are not
built yet.

Worker stdout and stderr are captured and forwarded into the server's log, one
line at a time, tagged with the worker name and stream.

## Shutdown and drain

On SIGINT/SIGTERM the server drains: it stops accepting mutations
(in-flight deploy mutations are refused with a `backend`-class error and an
explicit message; the versions read model keeps serving), asks workers to
finish in-flight tasks (`DrainRequest`), and waits up to
`[drain] timeout_seconds`. `kill -9` skips all of that and is still safe
on the durable haematite store — that is the point of event
sourcing — but in-flight activities will re-dispatch on recovery.

## Operating runs

```sh
aion list [--status running|paused|completed|failed|cancelled|timed-out|continued-as-new]
aion describe <workflow-id> [--run-id <id>] [--raw] --pretty
aion start <workflow-type> --input '<json>' [--name '<label>']   # or --input-file <path>
aion signal <workflow-id> <signal-name> --payload '<json>'
aion query <workflow-id> <query-name>
aion cancel <workflow-id> [--reason '<text>']
aion pause <workflow-id> [--reason '<text>']
aion resume <workflow-id>
aion reopen <workflow-id>
aion inspect <workflow-id> [--run-id <id>] [--from <seq> --mock '<json>']
```

Every id-taking verb accepts `--run-id`; omitted, it targets the latest run.

- **`start --name`** — an optional operator-facing LABEL for the workflow you
  are starting, shown beside its id in the ops console and recorded durably on
  the start, so it survives replay and failover. It is only a label: **no verb
  takes a name as a target**, so you still address the workflow by its id. A
  whitespace-only `--name` is refused rather than quietly starting an unnamed
  run (the server refuses it too, so a raw HTTP or gRPC caller gets the same
  answer). Omit it and the workflow shows its bare id. Renaming is a console
  action (`POST /workflows/rename`), not a CLI verb.

  You name a run, but the name READS BACK per workflow. The recorded attribute
  carries no run id, so `aion list` and `aion describe` fold every recorded name
  over the whole history and show the last one — for every run of that workflow.
  Two consequences worth knowing at the terminal: a continue-as-new successor
  keeps showing the name its predecessor was given, and only a workflow's
  CURRENT run can be renamed, because a predecessor's new name would land on the
  successor that now owns the history head.

- **`pause` / `resume`** — `pause` durably holds **new** activity dispatch
  while preserving all durable state: the run projects the non-terminal
  `Paused` status, in-flight work keeps recording (timer fires, signal
  receipts, drained completions), and the hold survives `kill -9` — a
  paused run is not respawned at recovery, and its pending dispatch rows
  are never claimed while paused. `resume` releases the hold and the run
  proceeds to completion. Cancel of a paused run still works; the full
  semantics (dispatch-hold ruling, replay-invisible markers, the
  crashed-while-paused resume path) are in
  [`docs/contracts/PAUSE-RESUME-DESIGN.md`](../contracts/PAUSE-RESUME-DESIGN.md).
- **`reopen`** — reopens a terminal-reopenable (**Failed** or
  **Cancelled**) run and re-drives it from its recorded history.
- **`inspect`** — a time-travel debugger over a recorded run's event-store
  oplog, using the same `describe` read every other command uses (no
  debug-only log, no second store). For each recorded event it prints the
  workflow-visible state projection and the recorded `now()` for that step;
  `random()` is surfaced as a run-level draw-ordinal projection, not a
  per-step value. On a non-determinism fault it prints the exact divergent
  command (expected vs found at the sequence). With `--from <seq> --mock
  '<json>'` it runs a **what-if** re-run from the chosen event with a
  mocked outcome — entirely in-memory and read-only, never writing the
  event log. `--from` and `--mock` are required together. `--mock` shapes:

  ```json
  {"kind":"activity_completed","result":<json>}
  {"kind":"activity_failed","message":"..."}
  {"kind":"child_completed","result":<json>}
  {"kind":"child_failed","message":"..."}
  {"kind":"signal_delivered","payload":<json>}
  {"kind":"timer_fired"}
  ```

Global CLI flags: `--endpoint` (default `127.0.0.1:50051`), `--namespace`
(default `default`), `--subject` (default `cli-user`), `--token`,
`--pretty`. Errors print as `error[<code>]: <message>` with a `hint:` line
when there is an actionable next step — see the
[errors reference](errors.md).

Health endpoints: `GET /health/live`, `GET /health/ready`. Event streaming:
`GET /events/stream` (WebSocket; see [`docs/API.md`](API.md) for
subscription shapes and resumption cursors).

## Schedules

A schedule is a durable server-side resource that starts workflow
executions on a cadence. Its configuration (`aion_core::ScheduleConfig`)
carries:

- **`trigger`** — `{"Cron": {"expression": "0 2 * * *"}}` or
  `{"Interval": {"period": {"secs": 300, "nanos": 0}}}`.
- **`overlap_policy`** — what a tick does while an earlier triggered run is
  still active: `"Skip"` (default), `"BufferOne"` (keep at most one
  buffered tick), `"CancelPrevious"`, or `"AllowAll"`.
- **`catch_up_policy`** — what happens to ticks missed while the scheduler
  was unavailable: `"One"` (default; start one representative missed
  tick), `"All"`, or `"Skip"`.
- **`workflow_type`** — the workflow started on each fire.
- **`input`** — the opaque payload supplied to every triggered execution
  (`{"content_type": "Json", "bytes": [...]}` — UTF-8 JSON bytes).
- **`search_attributes`** — stamped on every triggered execution. The
  server force-stamps `aion.namespace` with the authorized namespace, so a
  schedule can never start runs in another tenant; the namespace recorded
  at creation is the schedule's immutable owner.

Schedule ids are server-generated UUIDs. `overlap_policy` and
`catch_up_policy` may be omitted and take their defaults.

**There is no `aion` CLI verb for schedules.** The surface is seven HTTP
routes and the seven matching gRPC RPCs (`CreateSchedule`,
`UpdateSchedule`, `PauseSchedule`, `ResumeSchedule`, `DeleteSchedule`,
`ListSchedules`, `DescribeSchedule`):

| Method | Route | Request |
|---|---|---|
| `POST` | `/schedules` | JSON body: `namespace` + `config` envelope (below). Returns `201` with `schedule_id` and a `state` envelope. |
| `GET` | `/schedules?namespace=<ns>` | List the namespace's schedules (state envelopes). |
| `GET` | `/schedules/{id}?namespace=<ns>` | Describe one schedule. |
| `PUT` | `/schedules/{id}` | JSON body: `namespace` + replacement `config` envelope; the id comes from the path. Cannot migrate a schedule between tenants. |
| `DELETE` | `/schedules/{id}?namespace=<ns>` | Delete the schedule. |
| `POST` | `/schedules/{id}/pause?namespace=<ns>` | Stop firing; state is preserved. |
| `POST` | `/schedules/{id}/resume?namespace=<ns>` | Resume firing. |

The `config` rides in the standard wire envelope: `{namespace, request_id,
payload}` where `payload.content_type` is `"application/json"` and
`payload.bytes` is the serialized `ScheduleConfig` as a **JSON byte array**
(the same envelope form the rest of the HTTP API uses — see
[`docs/API.md`](API.md)). Creating a schedule with `curl`, using `jq` to
build the byte arrays (`explode` yields code points, which equal UTF-8
bytes for ASCII config text; `[110,117,108,108]` is `null`):

```sh
config='{"trigger":{"Cron":{"expression":"0 2 * * *"}},"workflow_type":"nightly_report","input":{"content_type":"Json","bytes":[110,117,108,108]}}'
jq -n --arg ns default --arg cfg "$config" \
  '{namespace: $ns,
    config: {namespace: $ns, request_id: null,
             payload: {content_type: "application/json",
                       bytes: ($cfg | explode)}}}' |
curl -sS -X POST http://127.0.0.1:8080/schedules \
  -H 'content-type: application/json' --data-binary @-
```

Pause, resume, describe, and delete then take the returned id directly:

```sh
curl -sS -X POST "http://127.0.0.1:8080/schedules/<uuid>/pause?namespace=default"
curl -sS -X POST "http://127.0.0.1:8080/schedules/<uuid>/resume?namespace=default"
curl -sS "http://127.0.0.1:8080/schedules/<uuid>?namespace=default"
curl -sS -X DELETE "http://127.0.0.1:8080/schedules/<uuid>?namespace=default"
```

## The update check (slice one: know and show)

"Does a newer `aion-cli` exist on crates.io?" — the server can answer, and
the answer is always something an operator asked for.

### What it is

The server binary embeds a small AWL document,
`crates/aion-server/update-check-embed/update-check.awl`, installed into the
engine catalog at boot under the same only-the-empty-case rule as the
built-in assistant (a restart never re-points a route an operator chose).
Its one action declares a command body:

```
run "curl -fsS https://index.crates.io/ai/on/aion-cli"
```

so the fetch is **runtime `curl` through the declared-body spine** — the
server's default build carries zero outbound-HTTP dependencies, and every
line curl writes streams onto the run's transcript as it arrives, exactly
like any other declared command. When the run completes, the server parses
the fetched sparse-index lines, refuses loudly on anything malformed
(including a body whose lines name a different crate), and records the
greatest **installable** version: yanked releases and prereleases are both
excluded, because a plain `cargo install aion-cli` refuses both — announcing
`0.14.0-rc.1` against a stable `0.13.7` would send you through the upgrade
and hand you `0.13.7` back.

`GET /update-status` (authenticated, same posture as `/build`) serves the
result:

```json
{"installed": "0.13.7", "latest_known": "0.14.0", "checked_at": "2026-08-11T01:00:00Z"}
```

`latest_known` and `checked_at` are `null` until a check has ever completed
— a fresh server has honestly never checked, and nothing fabricates
otherwise. The recorded value is in-memory: a restart honestly forgets it,
because the registry may have moved while the server was down.

The ops console shows the result in the header, beside "What's new": an
"update available" pill appears only when `latest_known` is present and
semver-greater than the installed version (a malformed version on either
side is shown as its own loud state, never silently hidden), and the panel's
**Check now** button is the way to run a check — it starts one
`update_check` workflow in the selected namespace through the ordinary
start surface. From the CLI the exact command is:

```sh
aion start update_check --input '{}'
```

The `--input '{}'` is required: the document declares no inputs, so its
input schema is the empty object, and the CLI's default `--input null` is
refused by start-input validation (`null is not of type "object"` — a typed
refusal, pinned by `crates/aion-server/tests/update_check_start_binding.rs`).

### Manual-only, by design

There is **no cadence configuration key, no timer, and no check on
startup** — and the absent key is the design, not a missing default. The
server never phones home on its own; every check is one explicit operator
act. If a cadence is ever wanted, it gets designed then, with the
operator's number.

### A hung or failed check

The declared `curl` carries no timeout (a timeout would be an invented
number), so a check against an unreachable registry is an ordinary running
workflow that never completes — visible in the workflow list, cancellable
like any run:

```sh
aion cancel <workflow-id> --reason 'update check hung'
# or through the console's workflow view, or POST /workflows/cancel
```

A **failed** check (curl exit non-zero, e.g. no network) is a single
attempt — no retry policy is declared, so one click never becomes a retry
loop — and shows as a Failed `update_check` workflow; `/update-status`
keeps the previous answer and its `checked_at` visibly does not move.
Note also that, as with every activity, the command's whole output is
recorded into the run's durable event history: the fetched index body
(~a few KB today) is persisted per check, which is the durability contract
working as designed — worth knowing before pointing the check at anything
that answers with megabytes.

### What slice one will not do

Slice one performs **no install and no restart**. It knows and shows;
becoming the newer version is a separate act, and the estate law below
governs any future motion in that direction:

> **Ratified estate law:** any action that restarts Tom's live server gates
> on Tom's explicit act, always — his act must be the mechanism, not a
> confirmation dialog.
