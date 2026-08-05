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
working directory (then `<home>/config.toml`); with neither, it runs on the
built-in defaults. **An empty config boots**: every always-mounted surface
has a default, and keys only become required when the operator commissions
the surface they belong to.

### `aion server` flags

| Flag | Effect |
|---|---|
| `--config <path>` | Explicit TOML config file; skips auto-discovery. |
| `--listen-address <addr>` | Override `server.listen_address` (HTTP/JSON + ops console). |
| `--grpc-address <addr>` | Override `server.grpc_address` — the server's **own bind address**. The global `--endpoint` is a *client* dial address; `aion server` refuses to start if it is passed rather than silently binding the default port. |
| `--store-url <path>` | Set `store.url` **and select the libSQL backend** (an implicit `haematite`/`memory` backend is coerced to `libsql` — haematite ignores `store.url`, so the flag is only meaningful as a libSQL choice). |
| `--scheduler-threads <n>` | Override `runtime.scheduler_threads`; must be non-zero. |
| `--drain-timeout <seconds>` | Override `drain.timeout_seconds`; must be non-zero. |
| `--workflow-package <path>` | Extra `.aion` archive loaded at startup; repeatable (see "Boot-time loads"). |
| `--gleam-path <path>` | Set `authoring.gleam_path`: commissions the server-side Gleam authoring loop and mounts `/authoring/*`; requires `--authoring-project-root`. |
| `--authoring-project-root <path>` | Set `authoring.project_root`: the built Gleam project submitted source is written into and packaged from. |
| `--open` | Open the ops console in the default browser once the HTTP listener accepts connections. Best-effort: failures are logged and never affect the server run. |

### Conditionally required keys (startup fails naming the missing key)

| Key | Required when |
|---|---|
| `store.url` | `store.backend = "libsql"`. |
| `auth.jwks_url` | `auth.enabled = true`. |
| `authoring.project_root` | `authoring.gleam_path` is set. |
| `store.cluster.node_id`, `store.cluster.bind_address` | a `[store.cluster]` section is present (and each `[[store.cluster.peers]]` entry needs `name` + `address`). |
| `outbox.reconcile_interval_ms` **and** `outbox.reconcile_stale_after_ms` | either one is set — the pair is both-or-neither; both absent leaves reconciliation dark. |
| `outbox.liminal_listen_address` | `outbox.enabled = true` with `transport = "liminal"` (surfaced when the dispatcher spawns). |

Tuning knobs (`runtime.query_timeout_ms`, the websocket capacities, the
deploy ceilings, the outbox knobs, the observability bounds) default when
**omitted** but reject an explicit `0`. Validation failures are loud and
name both the TOML key and its environment override, e.g.:

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
                                        # "libsql" = lightweight embedded backend;
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
                                        # haematite; ignored by memory/libsql
# url = "aion.db"                       # REQUIRED when backend = "libsql":
                                        # embedded libSQL file, created on start;
                                        # ignored by haematite

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
max_event_bytes = 262144                # default (256 KiB); ceiling on one
                                        # persisted transcript event's
                                        # serialized size (oversized events
                                        # are truncated before persisting)
max_stream_events = 20000               # default; retained events per
                                        # (workflow, activity, attempt)
                                        # transcript stream — past the cap one
                                        # marker record is kept and further
                                        # events stay live-only
```

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
| `AION_HOME` is set | that path (relative paths resolve against the current directory) |
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
- `AION_STORE_BACKEND`, `AION_STORE_URL` (selects libSQL, like
  `--store-url`), `AION_STORE_DATA_DIR`, `AION_STORE_SHARD_COUNT`
- `AION_RUNTIME_SCHEDULER_THREADS`, `AION_RUNTIME_QUERY_TIMEOUT_MS`
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
- `AION_OBSERVABILITY_MAX_EVENT_BYTES`, `AION_OBSERVABILITY_MAX_STREAM_EVENTS`

Structured values — `[store.cluster]`, `store.owned_shards`,
`[worker.queue_service]`, `worker.heartbeat_window`, `[ops_console]`,
`[namespace]`, `[tls]`, `namespaces.auto_create`,
`namespaces.max_in_flight_activities`, `outbox.transport`, and
`workflow_packages` — are config-file (or flag) only. `AION_HOME` is **not**
a config key — it is the home resolver above. Examples:

```sh
AION_SERVER_GRPC_ADDRESS=127.0.0.1:60051
AION_STORE_BACKEND=haematite            # default; "libsql" or "memory" to opt out
AION_STORE_DATA_DIR=aion-data           # haematite database directory
# AION_STORE_BACKEND=libsql AION_STORE_URL=aion.db   # lightweight embedded opt-out
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
| `store_backend` | `haematite`, `libsql`, or `memory` |
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
lose. With a durable store — the **haematite** event store (the default,
first-class ablative backend) or the lightweight embedded **libsql** store:

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

## Shutdown and drain

On SIGINT/SIGTERM the server drains: it stops accepting mutations
(in-flight deploy mutations are refused with a `backend`-class error and an
explicit message; the versions read model keeps serving), asks workers to
finish in-flight tasks (`DrainRequest`), and waits up to
`[drain] timeout_seconds`. `kill -9` skips all of that and is still safe
with a durable store (haematite or libsql) — that is the point of event
sourcing — but in-flight activities will re-dispatch on recovery.

## Operating runs

```sh
aion list [--status running|paused|completed|failed|cancelled|timed-out|continued-as-new]
aion describe <workflow-id> [--run-id <id>] [--raw] --pretty
aion start <workflow-type> --input '<json>'      # or --input-file <path>
aion signal <workflow-id> <signal-name> --payload '<json>'
aion query <workflow-id> <query-name>
aion cancel <workflow-id> [--reason '<text>']
aion pause <workflow-id> [--reason '<text>']
aion resume <workflow-id>
aion reopen <workflow-id>
aion inspect <workflow-id> [--run-id <id>] [--from <seq> --mock '<json>']
```

Every id-taking verb accepts `--run-id`; omitted, it targets the latest run.

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
