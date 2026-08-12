import assert from "node:assert/strict";
import test from "node:test";
import {
  type ProtoStartWorkflowRequest,
  type ProtoStartWorkflowResponse,
  connect,
} from "./client.js";
import { InvalidArgumentError } from "./errors.js";

/**
 * The #211 display-name clause of the start contract, pinned at the SDK
 * boundary: an optional trimmed name on start, the blank refusal, and the start
 * outcome that reports a name an idempotent replay did not apply. The Rust and
 * Python SDKs pin the same cases; the shared conformance scenarios
 * (`conformance/aion-clients/scenarios.json`, scenario `display-name`) pin the
 * live-server observable.
 */

const startResponse: ProtoStartWorkflowResponse = {
  workflow_id: { uuid: "3f2a0d3e-0000-4000-8000-000000000001" },
  run_id: { uuid: "3f2a0d3e-0000-4000-8000-000000000002" },
};

class RecordingTransport {
  readonly starts: ProtoStartWorkflowRequest[] = [];

  async start(
    request: ProtoStartWorkflowRequest,
  ): Promise<ProtoStartWorkflowResponse> {
    this.starts.push(request);
    return startResponse;
  }

  signal(): Promise<void> {
    return Promise.reject(new Error("unused"));
  }

  query(): Promise<never> {
    return Promise.reject(new Error("unused"));
  }

  cancel(): Promise<void> {
    return Promise.reject(new Error("unused"));
  }

  list(): Promise<never> {
    return Promise.reject(new Error("unused"));
  }

  describe(): Promise<never> {
    return Promise.reject(new Error("unused"));
  }
}

/** A client under test never subscribes; iteration ends immediately. */
class NoopSubscribeTransport {
  subscribe(): AsyncIterable<unknown> {
    return {
      [Symbol.asyncIterator]: () => ({
        next: () => Promise.resolve({ done: true as const, value: undefined }),
      }),
    };
  }
}

async function clientWith(transport: RecordingTransport) {
  return connect({
    endpoint: "http://127.0.0.1:1",
    namespace: "testing",
    transport,
    streamTransport: new NoopSubscribeTransport(),
  });
}

test("a start with no display name leaves the field off the wire entirely", async () => {
  // Absent, not empty. The server REFUSES a present-but-blank name rather than
  // reading it as unnamed, so sending `""` for a nameless start would fail the
  // start outright. Absence is the only spelling of "unnamed".
  const transport = new RecordingTransport();
  const client = await clientWith(transport);

  const outcome = await client.start({
    workflowType: "testing.echo",
    input: { message: "hello" },
  });

  assert.equal(outcome.displayNameNotApplied, undefined);
  assert.equal(transport.starts.length, 1);
  assert.ok(
    !("display_name" in transport.starts[0]),
    "an unnamed start must not carry a display_name key",
  );
});

test("a start sends the display name it was given, and reports nothing unapplied", async () => {
  const transport = new RecordingTransport();
  const client = await clientWith(transport);

  const outcome = await client.start({
    workflowType: "testing.echo",
    input: { message: "hello" },
    displayName: "July invoice run",
  });

  assert.equal(transport.starts[0].display_name, "July invoice run");
  // A first start applies the name it asked for, so there is nothing to report.
  assert.equal(outcome.displayNameNotApplied, undefined);
});

test("a start trims the display name before sending it, because that is what the run will wear", async () => {
  // The server trims before recording, so an untrimmed name sent from here
  // would leave this SDK remembering a string the run does not have.
  const transport = new RecordingTransport();
  const client = await clientWith(transport);

  await client.start({
    workflowType: "testing.echo",
    input: null,
    displayName: "  Nightly settlement  ",
  });

  assert.equal(transport.starts[0].display_name, "Nightly settlement");
});

test("a present-but-blank display name is refused before any transport call", async () => {
  // `""`, `"   "` and `"\t"` are all a caller mistake, not a way to say
  // "unnamed": the server answers `invalid_input` for each, so forwarding one
  // would only buy a round trip to the same refusal. Omitting the option is how
  // a caller says "no name".
  for (const blank of ["", "   ", "\t", "\n "]) {
    const transport = new RecordingTransport();
    const client = await clientWith(transport);

    await assert.rejects(
      client.start({
        workflowType: "testing.echo",
        input: null,
        displayName: blank,
      }),
      (error: unknown) => {
        assert.ok(error instanceof InvalidArgumentError, String(error));
        assert.match(error.message, /displayName must not be blank/);
        return true;
      },
    );
    assert.deepEqual(
      transport.starts,
      [],
      "a refused start must not reach the transport",
    );
  }
});

test("a name difference does not split the idempotency key, and the dropped name is reported", async () => {
  // A label carries no identity, so two starts differing only in their display
  // name are the SAME act: the fingerprint deliberately excludes the name, so
  // the second dedupes to the first run instead of raising AlreadyExists. What
  // must never happen is the quiet version of that — handing back a run wearing
  // a name the caller did not ask for while saying nothing, or renaming the
  // standing run behind their back.
  const transport = new RecordingTransport();
  const client = await clientWith(transport);

  const first = await client.start({
    workflowType: "testing.echo",
    input: { n: 1 },
    idempotencyKey: "k",
    displayName: "July invoice run",
  });
  const second = await client.start({
    workflowType: "testing.echo",
    input: { n: 1 },
    idempotencyKey: "k",
    displayName: "August invoice run",
  });

  assert.equal(
    second.handle,
    first.handle,
    "a name difference must not start a second run",
  );
  assert.equal(
    transport.starts.length,
    1,
    "a name difference is the same act; it must not reach the server again",
  );
  assert.deepEqual(second.displayNameNotApplied, {
    requested: "August invoice run",
    standing: "July invoice run",
  });
  // The standing run keeps its own name: nothing was renamed behind the caller.
  assert.equal(transport.starts[0].display_name, "July invoice run");
});

test("whitespace around a replayed name is not a dropped name", async () => {
  // Both sides of the replay comparison are trimmed, so `"  X  "` and `"X"` are
  // the same name — the run wears `"X"` either way. Reporting that as a name
  // that could not be applied would be a false alarm about a drop that never
  // happened, and false alarms are how a real report gets ignored.
  for (const replayed of [
    "  Nightly settlement  ",
    "Nightly settlement",
    "\tNightly settlement\n",
  ]) {
    const transport = new RecordingTransport();
    const client = await clientWith(transport);

    await client.start({
      workflowType: "testing.echo",
      input: { n: 1 },
      idempotencyKey: "k",
      displayName: "Nightly settlement",
    });
    const outcome = await client.start({
      workflowType: "testing.echo",
      input: { n: 1 },
      idempotencyKey: "k",
      displayName: replayed,
    });

    assert.equal(outcome.displayNameNotApplied, undefined);
    assert.equal(transport.starts.length, 1);
  }
});

test("the unapplied-name report fires only for a replay that asked for a name", async () => {
  // The WHOLE rule, one case per combination of what the replay asked for and
  // what the standing run wears (#211). The report is raised only when the
  // caller ASKED for a name the standing run does not wear. A caller that asked
  // for nothing dropped nothing, so it is told nothing — the report's absence is
  // exactly the statement that no requested name went missing, and firing it for
  // an empty request would say the opposite of the truth. The Rust and Python
  // SDKs pin the same five cases.
  const cases: ReadonlyArray<{
    readonly standing: string | undefined;
    readonly requested: string | undefined;
    readonly expected: { requested: string; standing: string | undefined } | undefined;
  }> = [
    { standing: undefined, requested: undefined, expected: undefined },
    { standing: "Nightly", requested: undefined, expected: undefined },
    {
      standing: undefined,
      requested: "Nightly",
      expected: { requested: "Nightly", standing: undefined },
    },
    {
      standing: "Nightly",
      requested: "Weekly",
      expected: { requested: "Weekly", standing: "Nightly" },
    },
    { standing: "Nightly", requested: "Nightly", expected: undefined },
  ];

  for (const { standing, requested, expected } of cases) {
    const transport = new RecordingTransport();
    const client = await clientWith(transport);

    const first = await client.start({
      workflowType: "testing.echo",
      input: { n: 1 },
      idempotencyKey: "k",
      displayName: standing,
    });
    const replayed = await client.start({
      workflowType: "testing.echo",
      input: { n: 1 },
      idempotencyKey: "k",
      displayName: requested,
    });

    assert.equal(first.displayNameNotApplied, undefined);
    assert.deepEqual(
      replayed.displayNameNotApplied,
      expected,
      `standing=${String(standing)} requested=${String(requested)}`,
    );
    assert.equal(replayed.handle, first.handle);
    assert.equal(transport.starts.length, 1);
  }
});
