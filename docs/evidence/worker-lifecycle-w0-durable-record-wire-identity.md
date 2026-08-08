# W-0 evidence — durable deployment record + wire identity

Date: 2026-08-08  
Base: `573de3878` (the 0.12.0 release commit)  
Authoritative brief: `docs/design/WORKER-LIFECYCLE-W0-BRIEF-2026-08-08.md`

## Scope built

W-0 records deployments and associates registrations; it does not spawn,
reconcile, scale, drain, or stop processes. A regression source fence scans the
W-0 store, HTTP, wire, and registry sites outside comments/string literals for
`std::process`, `tokio::process`, and `Command::new`.
`last_spawn_binary` is initialized to `None` and the shared backend suite
asserts that it stays empty.

### Durable entity and backends

`aion-store` now owns the strict `WorkerDeployment` contract:

- one extensible `WorkerArtifactRef::Builtin { verb }` artifact tag;
- deploy-time binary version, commit, dirty stamp, and executable SHA-256;
- the reserved `last_spawn_binary` slot;
- namespace/task-queue/node routing axes and desired state;
- append-only `created`, `replaced`, and `desired-state-changed` history;
- RFC-3339 instants and unknown-artifact-tag refusal;
- create/replace, read, poison-reporting ordered list, desired-state update, and
  atomic key-delete methods.

The same shared conformance scenarios run against in-memory, libSQL, and
Haematite stores. Durable backends reopen a second instance over the same file
or directory. The in-memory restart exemption prints a visible skip line. Every
backend's raw-write hook plants an undecodable value and proves that listing
returns good records plus the bad key/error, delete removes the poisoned row,
and PUT over the poisoned key reports `replaced` while installing a fresh
record. Replacing a decodable record preserves `created_at`, prior history, and
`last_spawn_binary`, appends `replaced`, and advances `updated_at`.
Haematite uses a disjoint keyspace and runs blocking replication paths under
`spawn_blocking`; libSQL desired-state mutation is transactional.

### Server and stream

All five deploy-granted HTTP routes are mounted only while `[deploy].enabled`
is true; while false, the bare family and descendants return plain empty 404s:

- `PUT /worker-deployments/{name}`
- `GET /worker-deployments`
- `GET /worker-deployments/{name}`
- `POST /worker-deployments/{name}/desired-state`
- `DELETE /worker-deployments/{name}`

Authorization precedes binary capture and durable mutation. A focused denial
pin verifies that a caller without the deploy grant leaves no row. PUT captures
`BuildIdentity::current()` plus SHA-256 of `current_exe()` bytes at request time.
`GET /worker-deployments` returns `{ deployments, undecodable }`, so poisoned
keys and decode errors remain operator-visible. DELETE makes one atomic store
call, removes by key even when decoding fails, and returns whether a row existed
plus the decoded record when available.

The cluster channel carries deployment PUT, desired-state, and deletion deltas.
Its priming snapshot lists every decodable deployment and warns for each
undecodable key without terminating the channel. Worker entries carry a
nullable `deployment`/`deployment_association` pair: `known`, `absent`, and
`unchecked` are explicit states, while two nulls mean no instance identity.
All four states are pinned. Desired state and PUT outcome are typed kebab-case
wire enums. Rust-owned TypeScript bindings were regenerated; the strict console
decoder, deployment reducer, and typed fixtures consume the fields and variants.

### Registration wire

The hand-written and generated protobuf contracts carry optional tag 7
`WorkerInstanceIdentity { deployment, instance_id }`, distinct from tag-6 build
identity. An exact bytes test pins key `0x3a` (tag 7), and the gRPC decoder maps
it into the shared registry core. Absence still encodes to no bytes and remains
a first-class registration.

## Acceptance evidence

- **AC1:** backend conformance reopens libSQL and Haematite; the HTTP test writes
  through a state whose namespace/deployment dimensions derive from the same
  libSQL leaf, drops it, reopens that leaf, and reads the identical record.
- **AC2:** executable capture is compared with `BuildIdentity::current()` and an
  independently computed SHA-256; lowercase hex is pinned against the SHA-256
  empty-input known answer; the record round trip includes `last_spawn_binary`.
  The amended brief defers cross-binary comparison to W-2.
- **AC3 (gRPC and shared core):** generated tag 7 maps through `decode_register`;
  known/missing/no-store/absent identities produce `known`/`absent`/`unchecked`/
  null association in WorkerConnected and snapshots.
- **AC4:** no process creation was added; `last_spawn_binary` is `None` on every
  W-0 creation and is asserted in HTTP and backend tests.
- **AC5:** an unknown artifact tag produces `StoreError::Serialization` and is
  asserted in the entity tests and shared suite.
- **AC6:** subscribed PUT, desired-state, and delete deltas are asserted. A
  post-delete snapshot omits the record, and a poisoned row cannot kill snapshot
  construction. The console decoder accepts and reducer consumes the same data.

## Mutation probes

Each mutation was applied, the focused test was required to fail, and the
correct implementation was restored before final gates.

1. Codec writes `last_spawn_binary: None` instead of the record value:
   `codec_round_trips_every_field_and_spawn_slot` failed with the missing spawn
   identity. Log:
   `gate-logs/w0-worker-deployment/w0-mutation-codec.log`.
2. Executable capture returns `content_hash = "unknown"`:
   `capture_uses_build_identity_and_running_executable_sha256` failed against
   the independently hashed executable. Log:
   `gate-logs/w0-worker-deployment/w0-mutation-binary-identity.log`.
3. gRPC `decode_register` drops tag 7:
   `decode_register_maps_tag_seven_instance_without_changing_absent_registration`
   failed (`None` versus `Some("shells")`). Log:
   `gate-logs/w0-worker-deployment/w0-mutation-grpc-tag7.log`.

All three focused tests passed again after restoration.

## Final gates

Passing commands on the final tree:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- per-crate strict clippy for `aion-core`, `aion-proto-generated`, `aion-proto`,
  `aion-store`, `aion-store-libsql`, `aion-store-haematite`, `aion-worker`, and
  `aion-server`
- complete test suites for all eight touched Rust crates at the final tree,
  with each crate's authoritative log named so the mapping cannot drift:
  `fix2-gates/rust-tests-six-crates-final.log` covers `aion-store`,
  `aion-store-libsql`, `aion-store-haematite`, `aion-server`, `aion-worker`,
  and `aion-proto-generated`; `fix2-gates/round2-aion-worker-tests-final.log`
  and `fix2-gates/round2-aion-proto-generated-tests-final.log` are the
  dedicated runs of those two; `fix2-gates/rust-tests-core-proto-final.log`
  covers `aion-core` and `aion-proto` (4 verdict lines, 0 failed) — including
  `export_ops_console_wire_types` (the bindings staleness gate, green, which
  is what makes the round-3 console-gate skip a measured fact rather than an
  assumption) and `worker_instance_identity_uses_wire_tag_seven` (the tag-7
  exact-bytes pin at the final tree). The earlier `fix-gates/rust-tests.log`
  is the round-2-era record and is superseded by the logs above.
- `AION_UPDATE_BINDINGS=1 cargo test -p aion-core export_ops_console_wire_types`
- ops console: `bun test` (609 passed, `fix-gates/bun-test.log`),
  `bun run typecheck`, `bun run check`, and `bun run build` — run in the
  round-2 pass; round 3 changed no TypeScript, so the console gates were
  deliberately not re-run, and the bindings staleness gate above measured
  that the committed bindings still match the exporter at the final tree
- `git diff --check`

The `fix-gates/` logs and round-2 `w0-fix2-gates/` logs are authoritative at the
landing tree; the older `w0-*.log` files beside them are the R1-era record kept
for the refusal-chain history.

## Unresolved external transport blocker

AC3 is **not complete for Liminal**, so this evidence must not be read as a
landing approval. The active published dependency is `liminal-rs v0.5.0`.
Its `liminal::protocol::WorkerRegistration` has only `namespaces`,
`task_queue`, `node`, `activity_types`, `identity`, and `activities`; it has no
optional instance identity field. Version 0.5.2 and upstream `main` have the
same shape. Aion therefore has no truthful Liminal field to map without either
landing and releasing the protocol change in the liminal repository or
vendoring/forking the entire transport crate. This lane deliberately does not
overload tag-6 build identity, smuggle metadata through activity descriptors,
or add a silent compatibility side channel.

The required completion is coordinated: add the optional instance identity to
Liminal's published registration wire and codec (with presence/absence tests),
upgrade the three Liminal dependencies here, then map it in
`worker/liminal_transport.rs` through the same durable existence lookup and
registry core used by gRPC. Until then W-0 remains partial.

The three pre-existing untracked `.aion-*` directories were left untouched.

## Verification leg (Vesper, 2026-08-08)

Added at the operator's seat during the pre-landing verification read, per the
brief's addendum:

- **R3 rationale at the association site:** the connect-inward-forever doc
  comment now lives on `WorkerRegistry::resolve_instance_identity`
  (`crates/aion-server/src/worker/registry.rs`) — the association is
  observational, never an admission gate.
- **Planted known-positive (AC1 restart survival, libSQL):** the conformance
  call site's reopen closure was planted to open a fresh empty database; the
  shared suite went red with "a second store instance over the same storage
  must see the complete record" (exit 101), pristine bytes were restored with
  SHA-256 equality verified (`446b4d26…`), and the rebuilt suite re-ran green
  (exit 0). This proves the restart pin reads durable storage rather than the
  live instance. Artifacts: `w0-planted-positive-restart-red.log`,
  `w0-planted-positive-restart-green.log` in the evidence pack.
- **Embedded-bundle freeze check:** `crates/aion-server/ops-console-embed/`
  is untouched by this lane (0 dirty entries; last change b20d2da17). The
  console `bun run build` gate wrote only the git-ignored
  `apps/aion-ops-console/dist/` — the frozen embedded bundle was not rebuilt.
- **The unattributed red is #85, attributed with a base rate (B2):** the
  original build-lane suite run (`w0-aion-server-tests.log`) carried one red —
  `namespace_restart` failing with libsql's "bad parameter or other API
  misuse" face — with the test binary hash (`92fd130beae0872c`) identical
  across that red run and the subsequent green run: the bytes did not change,
  the outcome did, which is nondeterminism, not a W-0 defect. The base rate
  is established independently of this lane (the treatment): issue #85
  (libsql 0.9.30 double-close) was re-reproduced 2026-08-06 on a pre-W-0
  tree — 2/2 contended `cargo test --workspace` runs red showing both faces
  (the API-misuse string and SIGSEGV), 5/5 isolated runs green, mechanism
  verified in the dependency source. Post-fix evidence at this lane's final
  tree: 10/10 isolated `cargo test -p aion-server --test namespace_restart`
  runs green (40/40 test rows; `fix-gates/b2-namespace-restart-run*.log`),
  the fix lane's own six-crate suite run green with no libsql face
  (`fix-gates/rust-tests.log`), and the 0.12.0 release battery ran the same
  suite green under heavier contention on a different box (248 targets,
  4,259/0 at 573de3878). The red is charged to #85 by its signature and its
  base rate, not waved through.
- **Fix-lane probe green legs re-proven (R2 prep):** norn's B1 and M3
  mutation probes recorded genuine reds (the poisoned-row snapshot pin and
  the Unchecked tri-state pin failing, exit 101) and byte-verified
  restorations, but their green re-runs filtered out the target test
  (`0 passed; 1 filtered out` — a vacuous green). Both pins were re-run at
  the operator's hand on the restored tree: 1 passed each
  (`fix-gates/probe-b1-green-vesper.log`,
  `fix-gates/probe-m3-green-vesper.log`). The M5 probe's green leg was
  genuine as recorded (1 passed).
- **Planted known-positive re-run at the round-2 call-site form (N2):** R2
  found the libSQL conformance call site had grown a live extra handle, and
  the fix (an explicit `drop(first)`, matching the haematite site) changed
  the code the earlier planted-positive artifact certified. The plant was
  re-run against the current form: the reopen closure was planted to open a
  fresh empty database, the suite went red with "a second store instance over
  the same storage must see the complete record" (exit 101), pristine bytes
  were restored with SHA-256 equality verified (`50cb309c…`), and the re-run
  was green (1 passed, exit 0). Artifacts:
  `fix2-gates/planted-positive-r2-red.log`,
  `fix2-gates/planted-positive-r2-green.log`. The earlier pair
  (`w0-planted-positive-restart-*.log`) remains as the R1-era record.
- **Accepted debt — the 500-line rule (recorded, not waved):** this lane's
  integration edits grew four files that were already past the 500-line
  ceiling before W-0: `crates/aion-server/src/state.rs` (2854→2927),
  `crates/aion-server/src/worker/registry.rs` (2483→2617),
  `crates/aion-store/src/memory.rs` (1166→1275), and
  `crates/aion-store-haematite/src/store.rs` (4235→4533) — roughly +610
  lines across the four. Every file W-0 created is under 500. Splitting
  those pre-existing monoliths mid-feature would have been a larger and
  riskier change than the feature; the debt is owned by task #27 (worker
  lifecycle) and falls due before the lifecycle units are complete, not
  after. Related: R2 noted the `cluster_self_node` method extraction in
  `state.rs` as fresh out-of-scope churn in the function m3 was charged
  against — it is semantics-preserving (verified against HEAD), the
  rationale survives in the new doc comment, and it is recorded here rather
  than silently absorbed.
