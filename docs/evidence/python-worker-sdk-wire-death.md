# The Python and TypeScript worker SDKs were wire-dead, and their tests said otherwise

**Date:** 2026-07-29
**Found while:** starting task #28 (Python worker story), immediately after landing
declared commands (task #60).

## Summary

Neither the Python nor the TypeScript worker SDK could register with the current
Aion server. Both had been broken for some time. Both had passing test suites.

The suites passed because each SDK ships **committed protobuf stubs** that were
never regenerated after the wire contract changed. The tests therefore validated
the SDKs against a fossil of the protocol, and agreed with them.

This is worth stating plainly because it is the failure mode that matters: not a
bug that tests missed, but a bug that tests actively concealed. Every green run
was evidence for a claim that was false.

## What was actually broken

### 0. The Python SDK would not install at all

`pip install -e sdks/python/aion-worker` failed:

```
RuntimeError: could not locate crates/aion-proto/proto/worker.proto
```

Commit `0fe5961b` ("aion-proto: isolate generated stubs into own crate") moved
the `.proto` files to `crates/aion-proto-generated/proto/`. The Python build
hook still pointed at the old path. Nothing in the Rust build references that
hook, so nothing went red — the SDK simply stopped installing, silently, for
everyone.

The same stale path was in `sdks/python/aion-client/aion_client/proto/__init__.py`
and in the worker SDK's README.

### 1. Registration sends a field that no longer exists

`aion_worker/session.py:361-365`:

```python
register = worker_pb2.RegisterWorker(
    namespace=self._config.task_queue,   # <- no such field; and the WRONG VALUE
    activity_types=activity_types,
)
```

The current `RegisterWorker` has six fields — `namespaces` (repeated),
`activity_types`, `task_queue`, `node`, `activities`, `identity` — and none is
`namespace`. The SDK also put the **task-queue value into the namespace field**,
which was semantically wrong even before the rename: namespace and task_queue
are disjoint routing dimensions.

With the true stubs in place this fails loudly:
`ValueError: Protocol message RegisterWorker has no "namespace" field`.

TypeScript is identical — `sdks/typescript/aion-worker/src/session.ts:244` sends
`namespace: registration.taskQueue`.

### 2. Every result would be refused

`aion_worker/session.py:306-326` sends `ActivityResult` with no `run_id` and no
`completion_token`. The server rejects both unconditionally:

- `crates/aion-server/src/worker/dispatch.rs:507-513` — `"activity result run id is missing"`
- `crates/aion-server/src/worker/envelope.rs:29-35` — `MissingCompletionToken`

And `session.py:141-152` never reads either field off the inbound task, so the
SDK could not echo them even with correct stubs. So even a registration fix
alone would have produced a worker that received work and could never return it.

### 2b. TypeScript was worse: the stubs were never generated at all

The Python diagnosis does not transfer verbatim, and checking rather than
assuming is what found the difference.

`sdks/typescript/aion-worker/src/proto/` was **hand-written** — type-only
interfaces with no encode/decode, no service definition, no client. Nothing in
it was machine-produced, so there was nothing to "regenerate and diff".

Three compounding facts:

1. `generate:proto` read `crates/aion-proto/proto`, the same directory that
   moved in `0fe5961b`. Since `build` runs `generate:proto` first, **`npm run
   build` had been failing outright** — the package could not be built or
   published at all. The last commit touching `src/proto/` was `64088f31`;
   `worker.proto` has had four contract-changing commits since.
2. **There was no wire implementation.** `WorkerProtocolClient` and
   `GrpcClientFactory` were interfaces with no implementation, and
   `Worker.run()` threw without an injected factory. The SDK could not connect
   even in principle; a user had to hand-write a gRPC client.
3. A second build break (TS5069: `declaration: false` inheriting
   `declarationDir`) sat behind the first, invisible because the build died
   earlier. The build was also compiling `*.test.ts` into the published `dist/`.

It now generates from `crates/aion-proto-generated/proto/` with ts-proto, which
gives the SDK a working gRPC client for the first time, and carries the same
regenerate-and-byte-compare gate — demonstrated red twice, on a one-character
change and on a re-introduced hand-written file among generated neighbours.

**TypeScript is live-proven too** — see below. That was not true when this
section was first written, and the gap was recorded rather than glossed until
the run existed.

### 3. No typed action surface

`RegisterWorker.activities` — the `ActivityDescriptor` list that proves a worker
serves the contract a deployed package requires — was never populated. Empty
means "predates contract commitment" and is refused whenever a deployed contract
requires the queue.

That was survivable when nothing carried contracts. As of the declared-commands
work landed the same night, **every AWL deployment carries a worker contract**,
so this went from dormant to fatal.

## The schema rule that governs the fix

How the server matches a worker's descriptors against a deployed contract turns
out to be asymmetric, and it decides what an SDK may honestly advertise.

`crates/aion-package/src/compatibility.rs:52-103` calls `schema_is_subset(sub, sup)`
(`compatibility/schema.rs:80`), which returns true whenever `sup` is the
permissive schema. The two calls have **opposite variance**:

| | call | worker's schema is | permissive is |
|---|---|---|---|
| input (`:78`) | `schema_is_subset(expected, advertised)` | `sup` | **sound — always passes** |
| output (`:91`) | `schema_is_subset(advertised, expected)` | `sub` | **unsound — refused** |

Both are correct. A worker that accepts anything can serve any contract's input.
A worker must not claim to produce more than the contract declares.

The consequence for a dynamically-typed SDK is precise, and it is not a matter
of taste:

- an unknown **input** shape may be advertised permissively, honestly;
- an unknown **output** shape may not be advertised at all, and the SDK should
  refuse locally — naming the activity — rather than emit something the server
  will reject with an opaque schema diff;
- `false` is not an escape hatch. It passes the subset check (`schema.rs:84-86`)
  because it is a subset of everything, but it means "produces nothing", which
  is a lie in the other direction.

This means **a Python or TypeScript handler returning a bare `dict`/`object`
cannot serve an AWL-declared queue at all.** Schema-bearing return types are not
an ergonomic preference; they are the only admissible shape.

## What now prevents a recurrence

`sdks/python/aion-worker/tests/test_proto_contract_location.py`.

The location checks alone were not enough — they prove the contract is still
*where* the hook looks, and would have passed happily throughout the entire
period the contract's *content* was drifting. The gate that matters regenerates
the stubs into a scratch directory and **byte-compares them against what is
committed**.

Proven by breaking it rather than by reading it. With one comment appended to a
committed stub:

```
AssertionError: the committed protobuf stubs are stale: ['worker_pb2.pyi'] differ
from what crates/aion-proto-generated/proto generates today. The SDK is speaking
an older wire contract than the server.
```

Restored, it passes, and the restored file is byte-identical to the original. A
companion test asserts the generator is reproducible, because a flaky gate gets
disabled and a disabled gate is how this happened.

The gate found one more thing on its first run: `worker_pb2_grpc.pyi` and
`common_pb2_grpc.pyi` are **not generated** — grpcio-tools emits no `.pyi` for
gRPC service classes, so `WorkerProtocolStub`'s type is hand-written while its
implementation is generated, with nothing forcing them to agree. They are now
named explicitly in `HAND_WRITTEN_STUBS` and checked against the generated
module by constructing the real stub with a recording channel, rather than
excluded and forgotten.

Two latent defects were fixed in the build hook along the way: the import
patcher silently no-opped on a miss (shipping a package that fails at
`import aion_worker` on the user's machine), and — found by my own test —
`old` is a strict substring of `new`, so a second regeneration pass over
already-patched output produced `from . from . import ...` and corrupted the
file. The README documents a regenerate-and-commit cycle, so that second pass
is the documented workflow.

## Live measurements taken

Server built from this tree, `examples/python-worker/image_report.awl` deployed
to it, one run started with no worker present.

Deploy:
```json
{"content_hash":"c718c3a6f9f098a5f8bc0a3a673318ca326d88611885f8b5ecee9c62aff648c1",
 "workflow_type":"image_report","freshly_loaded":true}
```

Recorded history (`aion describe`), which confirms from the event store rather
than from reading the compiler:
```json
{"type":"ActivityScheduled","data":{
  "activity_id":0,
  "activity_type":"inspect_image",
  "task_queue":"python_box",
  "input":{"path":"/.../red.png"},
  "node":null}}
```

- the activity type is the bare AWL action name;
- the task queue is the AWL `worker` block's name;
- **activity input is a JSON object keyed by parameter name** — `{"path": ...}`,
  not a bare value;
- status stayed `Running` with `ActivityStarted` recorded and nothing serving it,
  which also proves the bodyless action genuinely requires a worker and does not
  fall through to any in-server path.

That run was left parked deliberately, as the oracle.

## THE LIVE PROOF

The run parked at **21:08:31** completed at **21:24:17** — sixteen minutes
waiting for a worker that did not exist and, in that state of the code, could
not have existed. It completed the instant a correct one connected.

Worker, started with no server-side change of any kind:

```
INFO aion_worker.host  Serving inspect_image on task queue 'python_box' in namespaces default as 'live-proof-py'
INFO aion_worker.loop  Connected
INFO aion_worker.loop  Registered activities: inspect_image
INFO aion_worker.loop  Received task inspect_image for workflow 60f432fe-… (attempt 1)
INFO aion_worker.loop  Completed inspect_image in 3ms
```

Recorded history:

```json
{"type":"ActivityCompleted","data":{"result":{
  "bytes":73,"format":"png","height":2,"width":3,"path":"/.../red.png"}}}
{"type":"WorkflowCompleted","data":{"result":{
  "outcome":"inspected","payload":{"bytes":73,"format":"png","height":2,"width":3,…}}}}
```

`status: "Completed"`.

The numbers are the proof that the worker did real work: the fixture is a PNG I
constructed byte by byte — 3×2 pixels, 73 bytes total — and the worker reported
exactly that, having parsed the IHDR chunk itself. A stub returning zeroes would
have looked just as green.

This single run exercises every one of the three defects at once. Registration
had to send all six fields correctly, or there would have been no connection.
The typed `activities` descriptors had to match the deployed contract, or
admission would have refused. And `run_id` and `completion_token` had to be
echoed verbatim, or the completion would have been rejected and the run would
still be parked.

### The admission gate proven real, not vacuous

A green run does not prove the gate that admitted it does anything. So I broke
it. A worker whose output type drops two contract-declared fields and mistypes a
third is refused, field by field:

```
FAILED_PRECONDITION  WORKER_CONTRACT_MISMATCH: worker build `missing-fields` queue `python_box` refused:
  action `inspect_image` field `output_schema.properties.bytes.type`
      expected "integer" but worker advertised "string";
  field `output_schema.properties.height` expected {"type":"integer"} but worker advertised <missing>;
  field `output_schema.properties.width`  expected {"type":"integer"} but worker advertised <missing>;
  field `output_schema.required` expected ["path","format","width","height","bytes"]
      but worker advertised ["path","format","bytes"]
```

The Python SDK surfaced that in full, which is the other half of the fix: the
old redial loop would have swallowed it and retried forever.

### The same proof, taken for TypeScript

Run started at a moment when no worker of any kind existed — the Python workers
had been stopped and no TypeScript worker had ever connected. It sat `Running`.
A Node worker built on the SDK then registered against the same queue and it
completed:

```json
{"type":"ActivityCompleted","data":{"result":{
  "bytes":73,"format":"png","height":2,"width":3,"path":"/.../red.png"}}}
{"type":"WorkflowCompleted","data":{"result":{"outcome":"inspected",…}}}
```

The worker read the IHDR chunk itself in Node (`readUInt32BE` at offsets 16 and
20), so the dimensions are measured, not echoed.

**Attributed by control, not by elimination.** No Python worker process existed
(`pgrep` confirmed zero), but "nothing else was running" is an argument, not a
demonstration — and the server does not log which worker registered, so the log
cannot settle it either. So the TypeScript worker was killed and an identical
run started: it stayed `Running`. The queue is served when the TypeScript worker
is up and unserved when it is down, which is the demonstration.

One observability note found in passing: the TypeScript worker printed nothing
at all across the whole successful run. The Python host logs connection,
registration, each task, and each completion. That asymmetry is not a defect in
the wire work, but an operator watching a TypeScript worker has nothing to watch.

### A second badly-designed test of my own, recorded

My first adversarial worker added an **extra** output field (`exfiltrated_secret`)
and expected a refusal. It was admitted, and that is correct — I had the variance
backwards. A schema with more required properties is *more* constrained, so
`WrongFacts ⊆ ImageFacts` genuinely holds: every value the worker produces
satisfies everything the contract promises, plus a field the consumer ignores.
Producing more than you promised is sound covariance. Only a *missing* or
*wrong-typed* field breaks the contract, which is what the test above had to
show. The system was right and my test was wrong.

## The sweep for other instances

Two other places in this repo hold committed generated code.

**The ops console's ts-rs wire types already comply.** `.github/workflows/ops-console-embed.yml` regenerates them and runs `git diff --exit-code`, unconditionally on every push and pull request. So the house pattern existed and was correct, and these SDKs were simply outside it with no CI lane at all. That is a different finding from "we have no gate": one calls for design, the other for enrolment.

**The `aion-client` Python SDK is fossilized too, but differently.** Regenerating and comparing (normalizing the known import patch) shows semantic drift, not banner noise: `StartWorkflowRequest` is missing `routing_key` and `task_queue`, and the entire namespace-minting surface — `NamespaceMintOrigin`, `MintNamespaceRequest`, `MintNamespaceResponse` — is absent.

The distinction is worth holding. The client is **feature-dead, not wire-dead**: proto3 is additive-tolerant, so everything it does send is still valid and existing calls still work. Nothing is broken today. What is broken is *reach* — every capability added since the stubs froze is invisible to Python callers, silently, with nothing to notice. The worker SDKs failed loudly the moment the fossil was removed; this one will keep working and keep being incomplete, which is the harder failure to find. Recorded as task #63 with the measurement, not fixed here.

## Handed off, not fixed here

Recorded in full against task #58, which owns the invisible-refusal lane and was
already in flight. Not fixed here, to avoid colliding with it.

### The server is silent when it refuses a worker

`crates/aion-server/src/api/worker_grpc.rs:66` propagates a failed contract check
and `:649` maps it to `FailedPrecondition`, neither with a `tracing` call; the
only nearby warn (`:633`) fires on the path where the check is SKIPPED. So the
server logs when it does not check, and is silent when it checks and refuses.

`ContractAdmissionError` already carries the identity, the task queue, and a
field-level diff (`crates/aion-server/src/worker/contracts.rs:61-65`) — the
diagnostic exists and is discarded at the boundary.

### `aion worker shell` cannot report any failure, ever

While looking for a known-good control to run against this queue, I found the
distribution's own worker command is undiagnosable. Same manifest, same binary,
three endpoints:

| endpoint | result |
|---|---|
| real server, real deployed queue | hangs, 0 bytes out, exit only on timeout |
| `127.0.0.1:59999`, nothing listening | **identical** |
| `192.0.2.1:1`, unroutable | **identical** |

`RUST_LOG=debug` changes nothing, because there is nothing to configure:
`aion-cli` has no `tracing-subscriber` dependency and installs no subscriber, so
every event the worker SDK emits is discarded. Compounding it,
`crates/aion-cli/src/worker_shell.rs:73` sets `.reconnect_max_attempts(usize::MAX)`,
so the redial loop retries forever and never surfaces anything to the anyhow top
level. An operator cannot distinguish "working" from "the server does not exist".

Server-side confirmation it never registered: `POST /awl/workers/availability`
returned `{"available":false,"connected_workers":0}` while the worker process was
running, and the parked-dispatch warning reported `workers_in_pool: 0`.

### A hypothesis my own control overturned

I first believed the shell worker was being refused by the contract check, and
the reasoning was sound: `worker_shell.rs` registers with `register_activity`
rather than `register_activity_with_contract`, so it advertises an **empty**
`activities` list, which by the rule above is refused by any contract-bearing
queue.

So I ran the control — the same worker against `no_contract_queue`, a queue no
deployment declares and against which no contract can be violated. It also
showed `connected_workers: 0`. The registration failure is therefore more basic
than the contract check, and my explanation was wrong.

Worth writing down for two reasons. The obvious one: a plausible mechanism that
fits the symptom is not the same as the cause, and the control cost about a
minute. The less obvious one: the contract-descriptor gap is **still real** — it
is simply latent behind an earlier failure, and will surface the moment the
silence is fixed. Had I not run the control I would have "fixed" the descriptors,
seen no change, and had no idea why.
