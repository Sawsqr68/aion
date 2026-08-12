# @aion/client

TypeScript caller SDK for connecting to an Aion server deployment and operating workflows from Node.js (>= 22.4, required by the built-in WebSocket subscription transport's upgrade-header support). Status: in progress/hardening; install from this checkout until a release is published for your target environment. It exposes connect plus the seven workflow operations: `start`, `signal`, `query`, `cancel`, `list`, `describe`, and `subscribe`.

## Install

```sh
npm install ./sdks/typescript/aion-client
```

## Server prerequisite

Run an Aion server (`aion server --config <file>`) that implements the AW workflow API. The runnable example uses the AL-007 fixture defaults:

```sh
export AION_SERVER_URL=http://127.0.0.1:8080
export AION_AUTH_TOKEN=dev-token # optional
npx tsx examples/seven-operations.ts
```

See [`examples/seven-operations.ts`](examples/seven-operations.ts) for a complete program covering all seven operations. Subscribe uses the built-in WebSocket transport against the server's `/events/stream` endpoint (the websocket URL is derived from the configured HTTP endpoint, auth headers are forwarded on the upgrade request, and reconnects resume from the last delivered sequence number); an injected `streamTransport` overrides it.

## Connect

```ts
import { connect } from "@aion/client";

const client = await connect({
  endpoint: process.env.AION_SERVER_URL ?? "http://127.0.0.1:8080",
  namespace: "conformance",
  auth: process.env.AION_AUTH_TOKEN
    ? { bearerToken: process.env.AION_AUTH_TOKEN, namespaces: ["conformance"] }
    : undefined,
  tls: { enabled: false },
});
```

## start

Generic JSON payloads are the typed path. `idempotencyKey` makes caller retries safe: identical retry returns the original handle, conflicting reuse raises `AlreadyExistsError`.

`start` returns a `StartOutcome`, not a bare handle: `{ handle, displayNameNotApplied }`.

```ts
type StartInput = { message: string; counter: number };

const { handle } = await client.start<StartInput>({
  workflowType: "conformance.echo",
  input: { message: "hello", counter: 1 },
  idempotencyKey: "readme-seven-operations",
  displayName: "Nightly settlement",
});
```

`displayName` is an optional operator-facing label recorded durably on the start. It is trimmed before it is sent (the server trims it too), and a present-but-blank one raises `InvalidArgumentError` before any round trip — omit the option to start the run unnamed. It is a LABEL over the UUID identity, never an address: no SDK call resolves a workflow by name, and this SDK deliberately offers no "find by name and act" helper — name-shaped filtering returns candidates and the caller chooses.

The name is not part of the idempotency fingerprint, because a label carries no identity: two starts differing only in name are the same act and dedupe to one run. When a replay matches a run that does not already wear the requested name, the drop is reported rather than swallowed:

```ts
const outcome = await client.start<StartInput>({
  workflowType: "conformance.echo",
  input: { message: "hello", counter: 1 },
  idempotencyKey: "readme-seven-operations",
  displayName: "A different name",
});
if (outcome.displayNameNotApplied !== undefined) {
  // The replayed run keeps its existing name; nothing is renamed behind you.
  console.log(
    outcome.displayNameNotApplied.requested,
    outcome.displayNameNotApplied.standing ?? "(unnamed)",
  );
}
```

Renaming an existing run is a console/HTTP action (`POST /workflows/rename`), not an SDK or CLI operation in this release.

## signal

```ts
await handle.signal({
  signalName: "record",
  payload: { value: "signal-observed" },
});
```

## query

The query result is decoded into the generic result type. AW's `QueryRequest` now carries an `arguments` payload, but this SDK does not forward it yet — an `input` passed here is validated and then dropped, so omit it until this SDK is wired to the new field.

```ts
type EchoState = { lastSignal?: string };

const state = await handle.query<undefined, EchoState>({
  queryName: "state",
  timeoutMs: 5_000,
});
```

## list

```ts
const { summaries } = await client.list({ namespace: "conformance" });
console.log(`listed ${summaries.length} workflow(s)`);
```

## describe

```ts
const description = await handle.describe({ includeHistory: true });
console.log(description.summary, description.history.length);
```

## cancel

Cancellation is a cooperative request: success means the server accepted the request.

```ts
await handle.cancel({ reason: "caller requested cancellation" });
```

## subscribe

`handle.subscribe()` returns an `AsyncIterable<WorkflowEvent>`. It reconnects after transient `Unavailable` failures using the last delivered per-workflow sequence number; terminal failures are thrown from iteration.

```ts
for await (const event of handle.subscribe({ maxReconnects: 3 })) {
  console.log(event.seq, event.envelope);
  break;
}
```

## Typed and raw payloads

Typed operations serialize JSON values by default. The raw escape hatch is always available through `rawPayload`, `startRaw`, `signalRaw`, and `queryRaw`:

```ts
import { rawPayload } from "aion-client-typescript";

await handle.signalRaw({
  signalName: "record",
  payload: rawPayload(new TextEncoder().encode('{"value":"raw"}'), "application/json"),
});
```

## Branching on errors

Errors are branchable subclasses and also carry a discriminating `kind`.

```ts
import {
  AlreadyExistsError,
  QueryTimeoutError,
  UnavailableError,
} from "aion-client-typescript";

try {
  await handle.query<undefined, EchoState>({ queryName: "state", timeoutMs: 10 });
} catch (error) {
  if (error instanceof QueryTimeoutError) {
    console.error("query timed out; use a longer timeout");
  } else if (error instanceof AlreadyExistsError) {
    console.error("idempotency key was reused for a different start");
  } else if (error instanceof UnavailableError) {
    console.error("server or stream transport is unavailable");
  } else {
    throw error;
  }
}
```
