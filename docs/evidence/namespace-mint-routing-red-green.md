# Red-then-green evidence — build/namespace-mint-routing

Tom's ruling, 2026-07-28 (verbatim: "one"): **the server routes the namespace
mint internally to its owning node.** This lane discharges it. It is the
production lane the failover start-path investigation's verdict called for
(`docs/evidence/failover-start-path-investigation.md`, landed `1d0bdddc`).

Base: `main` **bbfe7065**. Lane `build/namespace-mint-routing`, worktree
`.worktrees/namespace-mint-routing`, default target dirs, no
`rust-toolchain.toml` edit.

Toolchain stamped in `gate-logs/namespace-mint-routing/stamp.log` and here:

```text
rustc 1.97.1 (8bab26f4f 2026-07-14)
binary: rustc
commit-hash: 8bab26f4f68e0e26f0bb7960be334d5b520ea452
commit-date: 2026-07-14
host: aarch64-apple-darwin
release: 1.97.1
LLVM version: 22.1.6

cargo 1.97.1 (c980f4866 2026-06-30)
jq-1.8.1
df /Users/tom volume: 172Gi free at the stamp (81% used)
```

---

## The defect, restated at this tree's bytes

An `aion start` mints its namespace on use. A namespace's registry record is
keyed `n: || name` (`keyspace.rs:125`), so it hashes to a shard of its OWN —
unrelated to the shard the workflow lands on. `namespace_key("default")` is
shard **1** at `shard_count = 3`. Node 0 of the kill-9 cluster owns shard 0.
Its quorum CAS for the namespace record was therefore fenced by shard 1's
serving owner, on every attempt, forever: `NotOwner { shard: 1 }`, 262 out of
262 on the investigation's record.

The workflow half was never the problem: the R-1 unsteered remint already puts
the execution on a locally-owned shard. **Two keys, two routings; only the
namespace half was unrouted.**

`shard_for_namespace("default") == 1` at `shard_count = 3` is now a unit test
(`crates/aion-store-haematite/src/namespace_shard_tests.rs`), so the constant
the investigation measured is pinned in the tree rather than remembered.
`orders` lands on 2 for contrast — no node can own the registry for every
namespace, which is why routing is needed rather than a configuration knob.

## The ruled design, as built

**Route the MINT, never the start.** Routing the whole start was rejected on
the brief's grounds and is not built: workflow-shard steering and the namespace
shard hash different keys and routinely disagree, so start-forwarding would
re-couple two independent placement decisions and could bounce off the forward
hop cap (`MAX_FORWARD_HOPS = 2`).

**The seam is the minter.** `crates/aion-server/src/namespace/route.rs` —
`NamespaceRouting`, the one optional dependency `NamespaceMinter` gains, over
three narrow traits:

| trait | production impl | what it answers |
| --- | --- | --- |
| `NamespaceShardResolver` | `HaematiteStore::shard_for_namespace` | which registry shard a name lives on |
| `MintShardOwners` | `routing::StaticShardDirectory` | who owns that shard right now |
| `MintForwarder` | `namespace::GrpcMintForwarder` over the R-3 forwarder | how to ship an authorized mint there |

The directory implementation is the SAME `StaticShardDirectory` the request
edge consults (with its SS-3 quorum-replicated shard-owner overlay and its
peer-liveness gate), so a mint and a signal can never disagree about who owns a
shard.

Because the seam is the minter, every `mint_or_gate` choke-point is routed with
no per-transport work: the gRPC start, the HTTP start, and worker
registration. **Both construction sites thread it** —
`ServerState::namespace_minter()` (per request) and the boot's
`ConnectedWorkerRegistry` via `RoutingState.mint_routing`, assembled once
alongside the directory and forwarder in `build_routing_state`.

**Decision order.** Resolve `shard_for_namespace(name)`; ask the directory.
`Local` writes here. `Remote` with a dialable address forwards. `Unknown` — no
record, owner believed down, or a known owner with no forward address — writes
HERE *deliberately*, so the receiver fence answers. **The directory is a
liveness-gated hint; it is never trusted over the fence** (haematite
`receiver.rs:105-116` — "the receiver fence is the enforcement" — is unchanged
and untouched).

`None` on every single-node / non-clustered boot, where the minter is
byte-for-byte what it was.

## Consistency at the final bytes

The brief's consistency claims, checked against what shipped rather than
restated:

- **`register_namespace_record` is unchanged.** Not one line of
  `crates/aion-store-haematite/src/store.rs` was edited in this lane
  (`git diff --stat` over the lane touches only the new `namespace_shard.rs`
  sibling and `lib.rs`'s `mod` line). It has no notion of caller or
  forwarded-ness: identical branches, identical `expected` CAS hash, identical
  membership and timeout whichever node executes it.
- **The ENTIRE read-modify-write travels.** The forward carries a namespace
  NAME and a provenance, nothing more; the `database.get`, the `Hash::of`
  expected value and the `replicate_write` proposal all execute on the node
  that runs the mint. The banned split — read locally, propose remotely, a
  cross-node TOCTOU — is not representable through this seam: the seam's only
  outbound payload is `(namespaces, origin)`.
- **The epoch fence is receiver-side and unchanged**, and forwarding to the
  owner is precisely what supplies the stamp receivers accept. It STRENGTHENS
  the single-proposer story: the proposer is now the fenced owner.
- **Idempotence converges.** `CasConflict → AlreadyExisted` on both branches is
  untouched, so racing local and forwarded mints, double forwards, and
  retry-after-forward land on exactly one record. The cluster test re-mints
  through the route and asserts `created_at` is unchanged — no second create.
  `crates/aion-store-haematite/tests/namespace_quorum_cas.rs` is untouched and
  green (see the battery).
- **One observable delta, stated not assumed.** `MintOutcome::Created` is
  decided where the write executes, so a forwarded mint's
  `ClusterEvent::NamespaceCreated` fires on the OWNER's publisher. That
  publisher is the deployment-global cluster channel the ops console already
  subscribes to, so the console still sees each namespace exactly once. This is
  accepted behaviour, recorded here.
- **Neither existing `NotOwner` message string changed.** The store fence's
  (`aion-store-haematite/src/error.rs`) and the routing edge's
  (`grpc/routing_resolve.rs`: "workflow shard {shard} is owned by another
  cluster node") are byte-identical. The mint's unreachable-owner refusal is a
  THIRD, distinct string naming the namespace — the investigation's diagnosis
  method (telling the two apart by their text) still works, and now has a third
  distinguishable case.

## The RPC vehicle, and why it is not a bypass

`MintNamespace` is new public gRPC surface on `WorkflowService`, carrying the
namespace set and a typed `NamespaceMintOrigin`. `ForwardRequest`/`ForwardReply`
gained matching variants, so the mint rides the SAME forwarder that already
relays signals, queries, cancels and steered starts — same verbatim
caller-metadata copy, same hop stamp, no second transport.

The owner-side handler is not privileged:

1. the caller is resolved from the forwarded metadata by `caller_from_metadata`,
   exactly as on every other RPC, so a forwarded mint authenticates as the
   ORIGINAL caller and never as "a peer node";
2. every namespace in the set is authorized against that caller through the
   same guard the worker-registration seam uses;
3. **RIDER 1:** the mint runs through this node's ordinary
   `NamespaceMinter::mint_or_gate` — never a raw store write.

Rider 1 is pinned by test, not by assertion:
`a_closed_policy_refuses_a_forwarded_mint_exactly_as_a_local_one` builds an
`auto_create = closed` owner, takes the LOCAL refusal from the very minter the
handler will use, then drives a forwarded mint over the real hop and asserts
the returned wire error carries the **same code and the same message**, and
that the registry still holds nothing. (Brief drift: the brief says
`AutoCreate::Disabled`; the enum in the tree is `AutoCreate::{Open, Closed}`.
`Closed` is the variant meant and the variant tested.)

Caller credentials travel as `MintCredentials`, a verbatim copy of the inbound
gRPC metadata, whose hand-written `Debug` prints an entry COUNT and never a
value — a bearer token cannot reach a log through a derive. Pinned by
`credentials_debug_redacts_values`.

Honestly stated limit: the HTTP start seam and the worker-registration seam
have no inbound gRPC metadata to copy, so a mint forwarded from them carries
empty credentials and authorizes on the owner exactly as an unauthenticated
request would — the operator identity with auth off, a refusal with auth on.
That is a truthful refusal, never a silent bypass, and it is no worse than
today's behaviour on those seams (a hard `NotOwner`). Threading a credential
carrier through those two seams is named as a follow-up below.

## Failure surface

Exactly one truthful typed refusal per failed mint; no internal retry loop, the
client owns retry policy.

- The owner answered and refused → its typed refusal is relayed **verbatim**
  (`ServerError::Wire`), so a closed-policy denial on the owner is a namespace
  denial to the original caller.
- The forward reached no decision (dial failure, transport break, mismatched
  reply) → a `not_owner` naming the namespace AND its shard.

Refused-vs-unreachable is decided **structurally**: does the tonic status carry
the server's encoded `ProtoWireError` in its details? A bare transport status
does not. Message text is never matched. (proto3 decodes an empty buffer into
an all-default message, so emptiness is checked BEFORE decoding — otherwise a
transport status would masquerade as an authored refusal.)

**No re-forward chains.** A mint that already travelled does not travel again:
the owner-side handler mints with routing dropped (`without_routing()`), so a
shard that moved mid-flight yields the local fence's typed refusal, which
returns to the initiator and is surfaced.

## The CLI hint fix — the ledgered board item is discharged

`not_owner` collapsed into `ClientError::Unavailable` at
`aion-client/src/error.rs` BEFORE the CLI ever saw it, so `render.rs` could
only print *"cannot reach the server; check --endpoint and that aion-server is
running"*. The server had answered. That hint sent the investigation hunting a
network fault that did not exist.

Fixed at the **taxonomy**, not at the rendering: `ClientError::NotOwner` is its
own class (`not_owner`). Every surface reads the distinction from the type;
nothing string-matches a detail to recover a class the type threw away.
`Lagged` keeps its `unavailable` mapping; the bare-gRPC-code fallback is
untouched (`ABORTED` still falls to `Server`, because it also carries
`sequence_conflict` and the server always attaches typed details).

`crates/aion-cli/tests/error_rendering.rs` is **extended, not weakened**: the
existing `unavailable` test is unchanged and still asserted, and a new
end-to-end test drives the mock's fence through the real CLI and pins the
`error[not_owner]` line, the `NotOwner` error type, the routing hint, and the
ABSENCE of the connectivity hint — so the two classes are proved distinct
rather than swapped.

**This discharges the ledgered board item for the wrong CLI hint.**

## Red

`gate-logs/namespace-mint-routing/red-focused.{log,exit}`, committed at
`2ab1d058`. The focused test stands up a REAL 3-node haematite cluster (the
`ss3_directory_failover_e2e` wiring: real beamr loopback, real distribution,
plain `#[test]` for the coordinator's no-entered-runtime rule) and makes three
ordered claims on ONE cluster:

1. **non-vacuity** — `default`'s registry shard is 1 and the minting node C
   owns only shard 0, so the fence is genuinely reachable;
2. **the unrouted path** — a minter with no routing context refuses
   `NotOwner { shard: 1 }` and creates nothing;
3. **the routed path** — the same mint from the same node must land the record
   on the OWNER and become readable on C's own replica, and re-mint
   idempotently.

Claims 1 and 2 passed at the red commit. Claim 3 failed, with exactly the
refusal claim 2 pins:

```text
running 1 test
Error: StoreBackend { source: NotOwner { shard: 1 } }
test unsteered_namespace_mint_reaches_the_registry_shards_owner ... FAILED

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
```

`.exit` = `101`.

Claim 2 is not scaffolding that gets deleted: it stays green forever and is the
pin that **off-cluster boots did not change behaviour**.

## Green

`d185c8c3` flips claim 3 without weakening a line of the test. The whole suite:

| suite | what it proves |
| --- | --- |
| `aion-store-haematite` `namespace_shard` (4 unit) | `default → 1` at 3 shards; `orders → 2`; every shard inside the space; 1-shard store routes everything to 0 |
| `aion-server` `namespace::route` (9 unit) | the decision table (Local / Remote / Unknown), independent routing per namespace, verbatim relay of the owner's refusal, one truthful `not_owner` on an unreachable owner, credential redaction |
| `aion-server` `namespace_mint_routing_e2e` (real 3-node cluster) | the three ordered claims above, green |
| `aion-server` `namespace_mint_forward_e2e` (real tonic hop, 5) | metadata + hop stamp observed AT the owner by an interceptor; durable mint with forwarded provenance; the closed-policy parity pin; refusals for an empty set and an unspecified provenance; the cost measurement |
| `aion-client` `error_tests` | the taxonomy's all-variants class pin gains `not_owner`; the fence never collapses into `unavailable` while `lagged` still does |
| `aion-cli` `error_rendering` (5, extended) | the honest routing hint end to end, and the absence of the connectivity hint |

The two e2e halves are complementary by design, and the division is stated in
both files: the cluster test proves the ROUTING and the durable quorum write
(its forwarder is in-process, so no tonic server stands between the decision
and the write); the tonic test proves the WIRE (dial, verbatim metadata, hop
stamp, and the owner-side handler's policy). Neither claims the other's ground.

### One test edit rode a later commit, declared

`routed_minter` in `namespace_mint_routing_e2e.rs` — the lane's OWN new file
from the red commit — gains the routing wiring in `d185c8c3`, the commit that
creates that wiring. It is additive: no assertion changed, nothing was removed,
and claims 1 and 2 are byte-identical across the two commits. Two further test
edits ride `f9a9fda7` and are compile-mandatory rather than semantic: the
`mint_namespace` arm on `error_rendering.rs`'s mock service and the `NotOwner`
arm on `conformance.rs`'s variant-name map, both forced by the new RPC and the
new taxonomy class. `error_rendering.rs`'s mock `cancel` — previously
off-scope and exercised by nothing — now returns the fence for the new test.

No other test in the tree was touched.

## Performance, measured

Every start on a node that does not own the namespace's registry shard now
forwards, and `GrpcRequestForwarder` is a unit struct with **no connection
pool** — so each forwarded mint pays a fresh TCP + HTTP/2 handshake.

Measured on this box, in one process, same number of mints down both paths
(`measure_the_cost_of_a_forwarded_mint_against_a_local_one`, which asserts only
that both paths mint — never a timing threshold, which would flake on a
contended box):

```text
MINT COST over 20 mints: local total 509.792µs (mean 25.489µs);
forwarded total 16.627708ms (mean 831.385µs), fresh dial per forward
```

**~806µs added per foreign-namespace mint over loopback, ~33x the local mint**,
dominated by the dial. A real network adds its RTT on top. This is
correctness-first and accepted: the alternative measured on the same box is a
mint that never succeeds at all.

Excluded follow-ups, named with the numbers beside them:

- **connection pooling in `GrpcRequestForwarder`** — would remove most of the
  ~806µs, and benefits every forwarded request class, not just mints;
- **a `last_seen` debounce** so an idempotent re-reference of an
  already-minted namespace does not forward at all (today every start pays the
  hop, even the 2nd through Nth for the same namespace). NOTE: this is *not*
  the ruling's rejected option two — that was relaxing the `last_seen` touch's
  quorum write. Debouncing the CLIENT-side re-reference is a different change
  and would need its own ruling.

## Scope wall, held in both directions

IN, and built: the `mint_or_gate` path (gRPC start, HTTP start, worker
registration); `shard_for_namespace`; the `MintNamespace` RPC and forward
variant; the CLI hint taxonomy fix; the tests above.

OUT, and NOT touched:

- **workflow placement** — the R-1 remint and the R-4 steered start are
  byte-identical; `routing/edge.rs` was not edited.
- **`create_explicit` (`POST /namespaces`) and `set_namespace_placement`** —
  both still fence on a non-owner node. They are operator actions with
  redialable clients, they sit on the SAME seam, and the machinery this lane
  builds is directly reusable for them: attach the same `NamespaceRouting` and
  give them their own RPC variants. **Follow-up lane / board item.**
- **board #157's post-kill quorum bug** — adjacent, cited, untouched.
- **generalized routing of arbitrary quorum writes** — the clean cut is
  namespace-mint-only.
- **any `last_seen`-touch relaxation** — the ruling's rejected option two. Not
  smuggled in; the touch's quorum write is unchanged.

## Laws

- No `unwrap` / `expect` / `panic` in any file this lane wrote, tests included.
- **No `#[allow]` anywhere in the lane** — and one was REMOVED: `minter.rs`'s
  test module carried `#![allow(clippy::expect_used)]`, which is gone along with
  the `expect` it existed for.
- ≤500 lines, and every new file small: `route.rs` 348, `mint_resolve.rs` 75,
  `mint_forward.rs` 89, `namespace_shard.rs` 44.
- God-files: **`store.rs` (4084 lines) did not grow by one line** —
  `shard_for_namespace` is a new sibling module, and
  `git diff --stat bbfe7065..HEAD -- crates/aion-store-haematite/src/store.rs`
  is empty. Two files were brought UNDER the law by moving their inline test
  modules to `#[path]` siblings rather than thinning them:
  `namespace/minter.rs` 512 → 417 and `aion-client/src/error.rs` 505 → 432.
  The `MintNamespace` handler body was peeled into
  `api/grpc/mint_resolve.rs` — the same AO-007 sibling-`impl` discipline
  `routing_resolve.rs` in that directory already follows — so `grpc/mod.rs`
  carries a one-line delegation and grew 901 → 922 rather than 901 → 966.
- Stated plainly rather than glossed: **`state.rs` did grow**, 2557 → 2652.
  The brief itself requires BOTH minter construction sites to thread the
  routing, and both live there, alongside every other `build_*` boot helper
  (`build_routing_state`, `build_worker_seams`, `build_real_time_publishers`).
  Peeling `build_namespace_routing` out would have broken that file's own
  local convention for the sake of ~25 lines, so it was not done. `registry.rs`
  grew by one 17-line builder for the same reason.
- No `#[ignore]` added or changed. `lsub5b` keeps the `#[ignore]` it has always
  had (it spawns four OS processes), and is run explicitly with `--ignored`.
- No `rust-toolchain.toml` edit. Default target dirs. No vendored cross-repo
  change: every edit is inside this repo, and every path dependency it touches
  was already tracked.

## Acceptance — `lsub5b_osproc_kill9_failover_e2e`, UNMODIFIED

Run exactly as the investigation ran it, single test, own log and `.exit`:

```text
cargo test -p aion-cli --features haematite-backend,liminal-transport \
  --test lsub5b_osproc_kill9_failover_e2e -- --ignored --nocapture
```

`gate-logs/namespace-mint-routing/acceptance-lsub5b.{log,exit}` — `.exit` = 101.
Node logs and configs preserved beside it in
`gate-logs/namespace-mint-routing/acceptance-lsub5b-nodes/`.

**THE GATE IS STILL RED — and the namespace-mint fence is GONE.** Both halves
of that sentence are load-bearing, so both are proved from the artifacts.

### THIS LANE'S CLAIM: discharged, live-fire

The investigation's record was **262 attempts, 1 distinct refusal**, every one
`NotOwner { shard: 1 }`. This run's record is **172 attempts, 1 distinct
refusal**, and it is a completely different one — the namespace fence does not
appear once:

```text
$ grep -c "NotOwner\|not_owner" gate-logs/namespace-mint-routing/acceptance-lsub5b-nodes/node*.log
node0.log:0
node1.log:0
node2.log:0
```

And the mint executed **on the owner**, not on the node the client contacted.
The client drove `http://127.0.0.1:52953`, which is node0 (`owned_shards =
[0]`). The mint's audit event fired on **node1** (`owned_shards = [1]`) — the
owner of `shard_for_namespace("default") == 1`:

```text
gate-logs/namespace-mint-routing/acceptance-lsub5b-nodes/node1.log
{"timestamp":"2026-07-28T19:41:20.135630Z","level":"INFO",
 "fields":{"message":"namespace created","namespace":"default",
           "origin":"start_mint"},
 "target":"aion_server::namespace::minter"}
```

Nothing in node0's or node2's log carries that event. That is the ruled design
happening for real: a real 3-OS-process cluster, real production wiring, a real
gRPC forward, no test double anywhere in the path. **Unsteered starts are no
longer refused by the namespace-registry shard fence.**

### THE NEW FINDING — reported, NOT chased in-lane

With the first blocker cleared, the start reaches the ENGINE, where a second,
independent, pre-existing gate refuses it — constant, 172 out of 172:

```text
error[invalid_state]: failed to start workflow: NO_QUEUE_DECLARATION: workflow
`aion_outbox_fixture` version `4ed167cb…` has unscoped activities
fixture_activity; re-deploy from a checked AWL contract
  server error type: NoQueueDeclaration
```

This is the queue-declaration contract, and it is **not this lane's**:

- `NO_QUEUE_DECLARATION` exists at the lane's base `bbfe7065`
  (`git grep -c NO_QUEUE_DECLARATION bbfe7065 -- crates/` →
  `crates/aion/src/error.rs`, `aion-server/src/worker/queue_service/taxonomy.rs`,
  `aion-cli/src/new/scaffold.rs`, `aion-cli/tests/new_agent_e2e.rs`);
- this lane touched **none** of those files (`git diff --stat bbfe7065..HEAD`
  — 21 files, not one of them in that set);
- it is the already-named out-of-box break: a legacy Gleam scaffold with
  unscoped activities, which `new_agent_e2e` already pins as a typed refusal
  pending the AWL rewrite. This gate's fixture, `aion_outbox_fixture`, is one
  of those legacy scaffolds.

The failover investigation could never have seen this: the namespace fence
masked it. Removing the first blocker exposed the second.

The `kill -9` this gate exists to exercise **still has not fired** — the run
dies in setup, as before, but now one seam later. So the post-kill path
(plausibly board #157's territory) remains unexercised and unproven, and this
lane makes no claim about it.

Per the brief's stop rule, that is where this lane stops. The fixture's
queue declaration is production/authoring work outside this lane's scope wall,
and re-deploying the gate's fixture from a checked AWL contract is a change to
the acceptance test's own assets — which this lane is required to leave
unmodified.

**Fleet gate 2 therefore stays unmet**, and is not claimed here. What is
claimed, and proved above, is that its FIRST blocker is gone.

### One unrelated tree change the gate produces, reverted and flagged

Running the acceptance gate rebuilds `spike/liminal-fan-worker`, which
refreshes its own `Cargo.lock` from a stale `aion-* 0.8.0` pin to `0.10.0` plus
the transitive additions that came with it. That drift is **pre-existing and
not this lane's** — nothing in this lane changed a dependency. It was reverted
(`git checkout -- spike/liminal-fan-worker/Cargo.lock`) so the lane's diff
carries only its own surface, and it is flagged here: anyone who runs this gate
will see the same dirty file, and someone should land the refresh on its own.

### Recommendation for the lead

Two candidate next lanes, neither taken here:

1. re-author `spike/liminal-fan-worker`'s `collect_four` /
   `aion_outbox_fixture` from a checked AWL contract so the fixture declares
   its activities' queue — the smallest change that lets the gate reach its
   kill;
2. or rule the queue-declaration gate's treatment of legacy fixtures, if the
   AWL rewrite lane is going to move them anyway.



### The now-stale comment in the acceptance test, NOT edited

`drive_failover`'s start loop carries a standing comment stating that its
premise "is FALSE, and the retry has never once succeeded", explaining the
constant `NotOwner { shard: 1 }`. After this lane that comment is **stale**.
The ruling for this lane is that the acceptance test runs UNMODIFIED, and
UNMODIFIED wins over comment freshness — so it is reported here for the lead
rather than edited. The same applies to the file's module-level "CURRENT
STATUS — RED, and it has NEVER been green anywhere" header. Both should be
rewritten by whoever lands this, in a commit that is honestly about the
comment.

## The battery

HARNESS R2 bridge shape: the clippy leg carries the JSON + jq verdict layer with
the denominator rule; the tests leg is the standing-law `cargo test`, not
nextest (aion's server-booting e2e suites depend on cargo's binary-level
serialization). `pipefail` on every leg; no `2>/dev/null`, no `|| true`; every
`.exit` is a real captured `$?`, never a summarized exit.

| leg | command | exit | verdict |
| --- | --- | --- | --- |
| df preflight | `df -h /Users/tom` | — | 171Gi free |
| fmt | `cargo fmt --all` | 0 | applied, tree clean |
| clippy | `cargo clippy --workspace --all-targets --all-features --message-format=json` | **0** | below |
| tests | `cargo test --workspace --no-fail-fast` | **0** | below |
| acceptance | `cargo test -p aion-cli … --test lsub5b_osproc_kill9_failover_e2e -- --ignored` | **101** | above — new finding |

The acceptance leg was run alone; the workspace tests leg was never run
concurrently with it.

`gate-logs/namespace-mint-routing/clippy.json` is the raw stream;
`…clippy.verdict.txt` is the jq extraction beside it:

```text
DENOMINATOR compiler-artifact count: 844
error   diagnostics: 0
warning diagnostics: 0
--- any non-empty extraction below this line is a finding ---
```

The empty diagnostic extraction is green against a **proven nonzero denominator
of 844 compiler artifacts from the same stream** — the stream was not empty, so
the absence of findings is a real absence and not a broken filter. (The first
clippy run of this tree was RED — one `similar_names` pedantic finding in the
new cost-measurement test — which is itself the proof the filter sees findings
when there are any. It was fixed by renaming the bindings, not by an `#[allow]`.)

`gate-logs/namespace-mint-routing/tests.{log,exit,verdict.txt}` carries the
workspace suite; its own `test result:` lines are the verdict, summed over all
222 of them:

```text
exit 0 — 222 suites, 3048 passed, 0 failed, 6 ignored
```

The 6 ignored are the pre-existing, individually-reasoned OS-process and
network suites (`osproc_*` including `lsub5b` itself, which this lane runs
explicitly with `--ignored` as its acceptance leg; the hex.pm-downloading
dev-pipeline template; the real-norn harness; `stacked_dev_lands_through_the_
real_worker_and_review_signal`). **This lane neither added nor changed an
`#[ignore]`.**

`crates/aion-store-haematite/tests/namespace_quorum_cas.rs` (the store's own
fence and idempotence gates) is untouched by this lane and is inside that green.

Battery bytes: the clippy and tests legs above were **re-run in full** after the
`MintNamespace` handler was peeled into `api/grpc/mint_resolve.rs`, on the exact
working-tree bytes that were then committed unchanged; only the git metadata
differs. The earlier run at the pre-peel tree is not cited — a battery is not
carried across trees.

The acceptance leg was NOT re-run after the peel: it is a pure code move (the
trait method delegates one line to the peeled `impl`), the workspace tests leg
covers it, and re-running a 95-second 4-process cluster gate to re-observe a
refusal that comes from a seam this lane does not touch would add no
information. That choice is stated here rather than hidden.


## Release note

**Deployed behaviour changes.** On a multi-shard haematite cluster:

- `aion start` (and worker registration) against a node that does not own the
  target namespace's registry shard **now succeeds** instead of failing
  `NotOwner`. Unsteered starts are a working contract on multi-shard clusters
  for the first time.
- A new public gRPC method, `WorkflowService.MintNamespace`, is served by every
  node. It applies the node's ordinary `auto_create` policy and authorizes its
  caller exactly as every other RPC does; it is not a privileged internal
  channel.
- `ClusterEvent::NamespaceCreated` deltas **may now originate from the
  namespace-owner node** rather than the node the client contacted. Both
  publish on the same deployment-global cluster channel, so console behaviour
  is unchanged.
- A wrong-shard-owner refusal now reaches CLI users as `error[not_owner]` with
  a routing hint, where it previously read as `error[unavailable]` with
  "check --endpoint". Any tooling that matched on the `unavailable` class to
  detect a fence must move to `not_owner`.
