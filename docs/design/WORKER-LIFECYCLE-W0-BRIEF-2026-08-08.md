# W-0 build brief — durable deployment record + wire identity, 2026-08-08

**What this is.** The first build unit of the worker-lifecycle build, dispatched
under the ruling recorded in `WORKER-LIFECYCLE-DECISION-BRIEF-2026-08-08.md`
(Waffles, 2026-08-08, commit `dd7f6d182`). W-0 is defined in
`WORKER-LIFECYCLE-BUILD.md` ("Durable deployment record + wire identity");
this brief turns that definition plus the ruling's riders into numbered
requirements against verified tree facts (every file:line below checked
2026-08-08). Written by Vesper Lynd; norn builds, this seat reviews and gates.

**Scope fence.** W-0 records and associates; **nothing spawns**. The spawner
(W-2), the reconciler (W-3), and the stop/drain finish (W-4) are later units.
Rider 2 (timeout red-first proof) belongs to W-2; rider 5 (signal-0 stop gate)
belongs to W-4 — neither is in scope here, and nothing in W-0 may pre-build
them.

---

## R1 — The `WorkerDeployment` entity (aion-store)

A new store entity in `crates/aion-store/src/worker_deployment.rs`, modeled on
the `NamespaceRecord` pattern (`namespace.rs:34` — struct + private on-disk
serde form + RFC-3339 instants + unknown-tag rejection; codec tests in the
module, `namespace.rs:476-698` is the template). Fields:

- `name: String` — the operator-chosen deployment key (validated non-empty;
  the record's identity, as namespace records are keyed by name).
- `artifact: WorkerArtifactRef` — a tagged enum with **one variant in W-0**:
  `Builtin { verb: Vec<String> }`, the argv tail the server's own binary runs
  (e.g. `["worker", "shell", "--manifest", …]`; declared by the operator,
  never inferred). The codec's tag space is the extension point W-1 uses to
  add the `.aion` artifact kind; an unknown tag must **refuse to decode**
  (the `namespace.rs:692` pattern), never default.
- `binary: DeployedBinaryIdentity` — **rider 1, verbatim:** the identity of
  the binary the builtin deployment names, captured **at deploy time**:
  `version`, `commit`, `dirty` (from `BuildIdentity::current()`,
  `crates/aion-server/src/build_identity.rs:80`) **plus `content_hash`: the
  SHA-256 of the server's own executable bytes** (`std::env::current_exe()`
  read and hashed with the same `sha2` the package format mandates). The
  record schema also carries `last_spawn_binary: Option<DeployedBinaryIdentity>`
  — **empty in W-0** (nothing spawns); W-2 fills it at each spawn. A restart
  across an upgrade must be a visible, measurable fact, never a silent swap.
- `namespaces: BTreeSet<String>`, `task_queue: String`,
  `node: Option<String>` — the routing axes, same shapes the registry's
  `WorkerHandle` uses (`registry.rs:164`).
- `desired: DesiredState` — `Running` | `Stopped`. Recorded, not enforced
  (enforcement is W-3). No instance-count field in W-0: scale arrives with
  W-4's API and its value comes from the operator then — **no assumed
  default is minted now**.
- `status_history: Vec<StatusEntry>` — append-only within the record; each
  entry carries an instant + a status token + an optional detail string. W-0
  writes exactly two kinds: `created` and `desired-state-changed`. There is
  no append-only-history precedent in the store today (verified); this brief
  decides the history lives **inside the record**, unbounded (transitions are
  operator-rate; a cap would be an invented limit).
- `created_at`, `updated_at` — RFC-3339 via the `namespace.rs:351` helpers.

## R2 — The store contract and three backends

`pub trait WorkerDeploymentStore: Send + Sync + 'static` (`#[async_trait]`),
a sibling of `NamespaceStore` (`namespace.rs:371`) — **not** folded into
`EventStore`. Methods: `put_worker_deployment` (create-or-replace with a
`MintOutcome`-style created/replaced answer), `get_worker_deployment(name)`,
`list_worker_deployments()`, `set_desired_state(name, DesiredState)`
(read-modify-write appending the status entry; `Ok(None)` = no row, the
`set_namespace_placement:455` shape), `delete_worker_deployment(name)`
(records must be deletable by an operator; deletion appends nothing — the
record is gone, and W-3/W-4 own any richer retirement semantics later).
Re-export from `aion-store/src/lib.rs` beside the namespace exports (:60-63).

Implementations, each copying its sibling's known traps:

- **In-memory** (`aion-store/src/memory.rs`): `Mutex<BTreeMap<…>>` field +
  poison-tolerant lock helper (the `:103` pattern).
- **libSQL** (`aion-store-libsql`): delegating impl in `store.rs`, SQL in a
  new `worker_deployment.rs` (the `namespace.rs` sibling), DDL + index in
  `schema.rs` (:89-107 pattern) asserted in the schema test (:342-344
  pattern).
- **Haematite** (`aion-store-haematite`): new keyspace prefix in
  `keyspace.rs` (:122) **with the disjointness test extended** (:315,:321);
  the trait impl must wrap blocking replication in
  `tokio::task::spawn_blocking` — the `store.rs:3061-3070` trap, copied
  deliberately.

**Conformance decision (made here, explicitly):** unlike `NamespaceStore`
(whose cross-backend coverage is bespoke per backend — verified), the new
entity gets a **shared suite entry point**:
`pub async fn run_worker_deployment_suite` in
`aion-store/src/conformance.rs` (the `run_outbox_settlement_suite:76`
pattern), scenarios in `conformance/worker_deployment.rs`, called from all
three existing conformance call sites (`aion-store/tests/inmemory_conformance.rs`,
`aion-store-libsql/tests/conformance.rs`,
`aion-store-haematite/tests/conformance.rs`). One suite, three backends,
identical scenarios: round-trip, replace, desired-state flip appends history,
absent-row answers, delete, list ordering, unknown-artifact-tag refusal,
**and restart survival** — the scenario constructs a second store instance
over the same underlying storage where the backend supports it (libSQL:
same file; haematite: same data dir; in-memory: exempt and saying so in the
scenario, not silently).

## R3 — Wire identity on `RegisterWorker`

An optional **instance-identity** field, tag **7** (verified next-free on
both surfaces), on:

- `crates/aion-proto/src/worker.rs:65` `ProtoRegisterWorker` —
  `instance: Option<ProtoWorkerInstanceIdentity>` carrying
  `deployment: String` + `instance_id: String`. Distinct from the existing
  tag-6 `identity` (a *build* identity used by contract gating — verified it
  never reaches the registry; it stays untouched).
- `crates/aion-proto-generated/proto/worker.proto:103` `RegisterWorker` —
  the same field, regenerated stubs committed.

Server side, **both transports move together** (the drift trap is named in
the decision brief):

- gRPC: `decode_register` (`api/worker_grpc.rs:579`) maps the new field;
  the registration core `register_delivery` (`worker/registry.rs:603`)
  gains the optional instance identity and stores it on `WorkerHandle`
  (`registry.rs:164`).
- Liminal: the registration path through `worker/liminal_transport.rs`
  carries the same optional field to the same core.

**Association semantics (the whole point):** when a registration carries an
instance identity naming a deployment that exists in the store, the
registry's `WorkerConnected` cluster event (`registry.rs:699-708`) and the
cluster snapshot's per-worker entry (`ClusterWorker`,
`aion-core/src/cluster_event.rs:363`) carry the deployment name. A
registration naming a deployment that does **not** exist is not refused —
it registers first-class and the association is recorded as unknown in the
event detail (W-3 owns reconciling that state; refusing here would break
the connect-inward-forever contract). A registration with no instance
identity is today's worker and stays first-class forever.

## R4 — Announce on the cluster stream

New `ClusterEvent` variants (`aion-core/src/cluster_event.rs:109`):
`WorkerDeploymentPut` (created-or-replaced, carrying name + desired state +
binary version/hash) and `WorkerDeploymentDesiredStateChanged`. Emitted
through `ClusterEventPublisher::emit` (`cluster_publisher.rs:73` — the
closure form; no caller-fabricated seqs) from the server-side write path.
**The `:114` warning is load-bearing:** `ClusterSnapshot`
(`cluster_event.rs:383`) gains a `deployments` list built in
`build_snapshot` (`stream/cluster_stream.rs:144`) from
`list_worker_deployments`, so a console that connects after the write still
sees the record. The store write itself follows the `NamespaceMinter`
constraint (`namespace/minter.rs:1-16`): durable write strictly after the
caller's authorization.

## R5 — The server write path (minimal, W-0-shaped)

W-0 needs a way to create the record so restart-survival and association
are provable end-to-end. Scope: the store trait wired onto `ServerState`
(the `state.rs:71`/`:662` publisher pattern), plus HTTP routes under the
deploy grant (the same authority the cluster stream requires,
`cluster_stream.rs:9-21`): `PUT /worker-deployments/{name}`,
`GET /worker-deployments`, `GET /worker-deployments/{name}`,
`POST /worker-deployments/{name}/desired-state`,
`DELETE /worker-deployments/{name}`. Console surface is W-6; CLI verbs are
not in W-0. The existing `/workers/{id}/drain|stop` routes
(`router.rs:271-273`) are **untouched** — their finish is W-4.

## Acceptance criteria (each pinned by a test that reddens when flipped)

- **AC1 (restart survival, the unit's exit):** deployment records survive a
  server restart — proven in the conformance suite against real storage,
  and end-to-end: write via the HTTP route, stop the server process, start
  a new one on the same store, `GET` returns the record. Mutation probe:
  break the codec round-trip (drop a field) → the pin reddens.
- **AC2 (rider 1, verbatim):** the record captures the named binary's
  version and content hash at deploy time, and the schema carries the
  at-each-spawn slot; a deploy recorded under one binary and read under
  another surfaces the difference (test: record with identity A, compare
  against a current identity B, the comparison is inequality the caller can
  see — never a silent swap). Mutation probe: make capture return
  `UNKNOWN` constants → the pin reddens.
- **AC3 (association):** a hand-started worker registering with an instance
  identity is associated — `WorkerConnected` and the snapshot carry the
  deployment name; the same worker registering without the field registers
  exactly as today (both transports covered). Mutation probe: drop the
  field mapping in `decode_register` → the association pin reddens while
  the no-field pin stays green.
- **AC4 (nothing spawns):** no code path in W-0 creates an OS process.
  `last_spawn_binary` is `None` everywhere and asserted so.
- **AC5 (unknown-tag refusal):** an artifact ref with an unrecognized tag
  refuses to decode with a typed error; the refusal is pinned.
- **AC6 (stream honesty):** the put/desired-state events arrive on
  `/events/stream` and the snapshot reflects existing records on connect
  (both sides of the `:114` warning tested).

## Gates

The standard battery at the final tree: `cargo clippy --workspace
--all-targets -- -D warnings` **and** per-crate `-p` clippy for every
touched crate (both required — different feature resolution), `cargo fmt`
(autoformat, whole tree), the touched crates' test suites plus all three
conformance call sites, suite output to files. No `#[allow]`/`#[ignore]`/
`_var` bypasses; no `unwrap`/`expect` in library code; every file under
500 LOC; `mod.rs` re-export-only. Evidence pack per the house pattern.
Opus review before landing; mutation probes run and recorded per the
standing instruction.
