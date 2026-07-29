# @aion/worker

TypeScript/Node remote-worker SDK for registering out-of-process Aion activities and serving them from an Aion server task queue. Status: in progress/hardening; install from this checkout until a release is published for your target environment.

## Install

```sh
npm install ./sdks/typescript/aion-worker
```

## Minimal worker

Define activities and run a worker. The gRPC transport is built in — a worker with an `endpoint` connects with no extra plumbing. (`clientFactory` and `sessionFactory` remain available for tests and alternative gRPC runtimes.)

```ts
import { Worker, defineActivity } from "@aion/worker";

const greet = defineActivity<{ name: string }, { message: string }>(
  "examples.greet",
  async (input) => ({ message: `hello, ${input.name}` }),
  {
    contract: {
      output: {
        type: "object",
        required: ["message"],
        properties: { message: { type: "string" } },
      },
    },
  },
);

const config = {
  endpoint: "127.0.0.1:50051",
  namespaces: ["default"],
  subject: "worker",
  taskQueue: "default",
  identity: "typescript-worker-1",
  maxConcurrency: 8,
  reconnect: {
    initialDelayMs: 100,
    maxDelayMs: 5_000,
    maxAttempts: 10,
  },
};

await new Worker(config, [greet]).run();
```

## Routing dimensions

`namespaces`, `taskQueue`, and `node` are **disjoint**. Conflating them is a correctness bug, not a tuning choice.

| Field | Meaning |
| --- | --- |
| `namespaces` | The **set** of correctness/isolation boundaries. A workflow's activities only ever reach workers whose set includes the workflow's namespace. Every entry must be authorized for the caller, or the whole registration is denied. Required. |
| `subject` | Auth subject. Sent with `namespaces` as gRPC metadata (`x-aion-subject`, `x-aion-namespaces`), which the server reads *before* the first stream frame. Required. |
| `taskQueue` | Pool/flavour selector **within** each namespace (`norn`, `claude`, `cpu`, `gpu`). A miss here is a liveness problem, never a correctness one. Required. |
| `node` | Optional locality affinity. Defaults to the machine hostname (falling back to `localhost`); pass `""` to advertise no locality, which makes the worker reachable only for unpinned dispatches. |

`namespaces` is required rather than defaulted. The Rust worker's *builder* defaults an unset value to `["default"]`, which is safe behind a builder whose fields are set one call at a time; a TypeScript config is a single object literal where every field is already spelled out, so defaulting would only hide a silent choice of correctness boundary.

## Typed action surface (activity contracts)

`RegisterWorker.activities` is what proves a worker serves the contract a deployed package requires. Since AWL emits a contract for every deployment, a worker that advertises nothing is **refused** on any AWL-declared queue.

The Rust worker derives both schemas from a handler's concrete types via schemars. TypeScript erases types at runtime, so this SDK has no such path: **the schemas are your declaration**, taken on trust. Declare them with the `contract` option on `defineActivity`.

```ts
defineActivity<Input, Output>("charge", handler, {
  contract: {
    // Optional. Omitted => the permissive schema `true` is advertised.
    input: { type: "object", required: ["amount"], properties: { amount: { type: "integer" } } },
    // Required. There is no honest default (see below).
    output: { type: "object", required: ["ok"], properties: { ok: { type: "boolean" } } },
  },
});
```

**Why `input` is optional but `output` is required.** The server's two schema checks have opposite variance:

- **input** — the contract's schema must be a subset of *yours*. A worker that accepts anything satisfies every contract, so omitting `input` and advertising the permissive schema is both accepted and true: `defineActivity` decodes with `JSON.parse` and an unchecked cast, so the handler really does accept any JSON.
- **output** — *your* schema must be a subset of the contract's. A permissive output claims to produce anything, which no real record type admits, so it is refused. (`false` slips past the matcher only by lying in the other direction: "produces nothing".)

There is therefore no value the SDK could invent for `output` on your behalf. Omitting it is a compile error, which beats an opaque `FAILED_PRECONDITION` schema diff from a remote server.

**All or nothing.** Declare a contract for *every* activity or for *none*:

- **none** → an empty `activities` list, the wire's defined "predates contract commitment" value. Admitted on an uncontracted queue, refused on a contract-bearing one.
- **all** → your full typed action surface.
- **some** → refused locally at registration, naming the activities that declared nothing. A partial list has no wire meaning, and the server would read the gap as "this worker does not serve that action" — a refusal pointing at the wrong problem.

## Completion fencing

Every `ActivityTask` carries a `runId` and an opaque `completionToken`, and the server **rejects any result missing either**. Both are echoed verbatim; the token is proof of an execution generation and must never be parsed, rebuilt, or inferred.

The SDK does this for you: `ActivityCompletionIdentity` bundles the pair so reporting an outcome without it is not expressible, and unacknowledged results replayed after a reconnect carry the *original* identity. A task arriving without a valid `runId`, `completionToken`, `idempotencyKey`, or with `attempt == 0`, is refused at decode rather than defaulted.

Handlers can read `ctx.idempotencyKey` — stable across retries and execution generations — to deduplicate an external side effect the engine cannot.

## Protobuf artifacts

`src/proto/` is **generated — do not edit**. It is produced from the authoritative contract in `crates/aion-proto-generated/proto/` (`common.proto`, `worker.proto`), which is the same source the Rust server and worker are built from.

```sh
npm run generate:proto   # regenerate src/proto/ in place
npm run check:proto      # verify src/proto/ matches the .proto sources; non-zero if stale
```

`npm run build` runs `check:proto` first, so a stale checkout fails the build with a diff instead of silently shipping a fossil.

**The gate is a test, not a ritual.** `src/proto-contract.test.ts` regenerates into a scratch directory on every `npm test` and byte-compares against what is committed. A documented regeneration command is not enough on its own: nobody runs a ritual to discover a problem they do not know they have, and that is precisely how these stubs went stale while the suite stayed green — the committed stubs and the tests shared one fossil, so a green was one fact told twice rather than two facts agreeing.

The gate checks four things:

- committed artifacts **byte-match** a fresh generation (not "do the fields we use still exist" — that is what let `activities` be added and ignored);
- `src/proto/` contains **only** generated files, so a hand-written stray cannot sit invisibly among generated neighbours;
- every artifact still carries its `DO NOT EDIT` banner;
- the generator is **reproducible** — two runs agree byte for byte. A flaky gate gets disabled, and a disabled gate is how the rot happened, so the gate must not be able to become the thing it replaced.

Verified deterministic for protoc 3.19.1 / protoc-gen-ts_proto 2.11.8. The generated banner pins both tool versions, so a toolchain bump legitimately changes the bytes: regenerate and commit — never normalize the comparison.

## Scripts

| Script | Purpose |
| --- | --- |
| `npm run build` | Verify protos, then emit ESM + CJS + types to `dist/` |
| `npm run typecheck` | Type-check everything, tests included |
| `npm test` | Run the vitest suite |
| `npm run lint` | ESLint over `src/` (generated protos excluded) |

See the main Aion repository at <https://github.com/ablative-io/aion>.
