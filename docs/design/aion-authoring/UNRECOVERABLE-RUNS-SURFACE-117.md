# #117 — the operator-facing surface for degraded runs

**Status: BUILT.** `GET /workflows/unrecoverable`.

This closes the last piece of #117. The arc it completes:

| piece | commit | what it gave |
|---|---|---|
| the fact is retained | `7c9a2329` | the engine stops throwing away its own verdict |
| the run can be cancelled | `2eb515c0` | an operator who knows the id has a working lever |
| **the id can be obtained** | this lane | an operator who does not know the id can get it |

## The problem this closes

A run whose pinned package version no longer loads is skipped by startup
recovery. Every read surface still reports it as ordinary: it projects
`Running`, it appears in `list`, it describes with a full history. Nothing about
it looks wrong from the outside.

The one place its id was ever named as degraded was a single ERROR line at boot.
By the time anyone notices the run is not progressing, that line has scrolled
away — and it was never in a stream anyone could interrogate in the first place.

`POST /workflows/describe` already answers the degraded-residency question for a
run you can name. That is precisely the thing an operator cannot do here. A
lever you cannot aim is not a remedy.

## The shape, and why it copies `/queues/unserved`

`GET /queues/unserved` solved the identical problem one layer over: the per-run
half of "why is this stuck" was answerable, the fleet half was not. That
endpoint's shape is the precedent, deliberately followed rather than reinvented:

- a **GET** over live per-process state, no request body;
- **grant-filtered** on the same existence-leak boundary `GET /namespaces`
  enforces — a caller must never learn that a namespace it cannot access exists;
- an **empty list is the healthy answer**, never an error;
- ordering chosen **at the surface**, not baked into the storage layer.

## Three decisions that were not obvious

### The namespace read has to be unscoped, and that is a hazard

Every other namespace-aware read starts from a namespace the caller named and
asks a scoped question. A **sweep cannot**: it starts from a set of workflow ids
and does not yet know which namespace each belongs to. Scoping first would mean
guessing the answer it is trying to read.

So `NamespaceResolver::recorded_workflow_attribution` returns the attribution
raw, and the grant filter becomes the **caller's** obligation. That is a real
hazard — a scoped read fails safe by construction, an unscoped one does not —
so it is stated in the method's own docs as mandatory rather than left to the
call site to remember, and the boundary has its own test with a discriminating
control.

### An unattributed run is visible to the operator and to nobody else

A run whose history carries no owning namespace matches **no** enumerated grant.
Hiding it from everyone would recreate the original defect for exactly the runs
most likely to hit it — permanently invisible to the only person who can fix it.
Showing it to an enumerated caller would leak a run outside their tenancy.

The all-namespaces operator is the single caller for whom neither is true, and
that is where the line is drawn. `namespace: null` in the body is a fact about
the run, never a denial — the two are different and the surface does not merge
them.

### The remedy names redeploy FIRST

Both remedies are real and they are ordered. Redeploying the pinned version
makes the run recoverable on the next boot and loses nothing. Cancelling is
terminal. A surface that offered only `cancel` — the newer, more interesting
lever — would push operators toward destroying runs a redeploy would have saved.
The test asserts both words are present.

## It reports a live condition, not a scar list

Entries clear the moment the engine observes the run resident, so a non-empty
response is always about right now. This is inherited from
`UnrecoverableRuns::clear` rather than re-implemented, and it is checked end to
end through the endpoint with a non-vacuity read first: a degraded flag that
outlives its degradation sends an operator to redeploy a healthy run, which is
worse than no flag at all.

## Controls, run

| mutation | what died | what survived |
|---|---|---|
| `visible_to` returns `true` unconditionally | the tenant-boundary test and the unattributed test, and only those | the other four |
| the `sort_by` deleted | the ordering test, and only it | the other five |

sha256 re-verified byte-identical after each restore.

## Files

| file | what |
|---|---|
| `crates/aion-server/src/api/http/unrecoverable.rs` | the endpoint, the grant filter, the remedy sentence |
| `crates/aion-server/src/api/http/unrecoverable_tests.rs` | 6 tests |
| `crates/aion-server/src/api/http/router.rs` | `GET /workflows/unrecoverable` |
| `crates/aion-server/src/api/http/mod.rs` | module declaration |
| `crates/aion-server/src/state.rs` | `ServerState::unrecoverable_runs` |
| `crates/aion-server/src/namespace/resolver.rs` | `recorded_workflow_attribution` — unscoped, with the obligation stated |

## Stated limitation

**This is the HTTP surface only.** There is no gRPC method and no `aion` CLI
verb, because the CLI reaches the server over `aion_client` (gRPC) and adding
one means a proto change — a wider surface than this increment, and one that
should be decided alongside whatever else the next proto revision carries. The
ops console consumes HTTP, so the console path is open now; a terminal operator
reaches it with `curl` until the gRPC leg exists. That is named here rather than
left for someone to discover.
