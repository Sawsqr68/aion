import { expect, test } from 'bun:test';

import { ApiClient, ApiError } from '@/lib/api';

const namespace = 'default';
const workflowId = '00000000-0000-0000-0000-000000000001';
const runId = '00000000-0000-0000-0000-0000000000a1';

function jsonResponse(body: unknown, init?: ResponseInit): Response {
  return new Response(JSON.stringify(body), {
    headers: { 'content-type': 'application/json' },
    ...init,
  });
}

function clientReturning(body: unknown, calls: Request[], init?: ResponseInit): ApiClient {
  return new ApiClient({
    baseUrl: 'https://aion.example',
    credentials: { namespaces: [namespace] },
    fetchImpl: async (input, requestInit) => {
      calls.push(new Request(input, requestInit));
      return jsonResponse(body, init);
    },
  });
}

test('rename posts the id-addressed target + the new label and reads back the recorded name', async () => {
  const calls: Request[] = [];
  const client = clientReturning({ run_id: runId, display_name: 'nightly digest' }, calls);

  const result = await client.rename({ workflowId, displayName: 'nightly digest' }, { namespace });

  expect(calls[0]?.url).toBe('https://aion.example/workflows/rename');
  expect(calls[0]?.method).toBe('POST');
  // No run_id when omitted — the server resolves the latest run.
  expect(await calls[0]?.clone().json()).toEqual({
    namespace,
    workflow_id: workflowId,
    display_name: 'nightly digest',
  });
  expect(result).toEqual({ runId, displayName: 'nightly digest' });
});

test('rename forwards an explicit run_id when targeting one generation', async () => {
  const calls: Request[] = [];
  const client = clientReturning({ run_id: runId, display_name: 'x' }, calls);

  await client.rename({ workflowId, displayName: 'x', runId }, { namespace });

  expect(await calls[0]?.clone().json()).toEqual({
    namespace,
    workflow_id: workflowId,
    display_name: 'x',
    run_id: runId,
  });
});

// R3 (picker-not-parameter). Catches: a name leaking into the request as a way
// to ADDRESS the run. The only id-shaped keys the body may carry are
// workflow_id/run_id; display_name is the value being set, never a target.
test('the rename body addresses the run by id only — the name is never a lookup key', async () => {
  const calls: Request[] = [];
  const client = clientReturning({ run_id: runId, display_name: 'x' }, calls);

  await client.rename({ workflowId, displayName: 'x' }, { namespace });

  const body = (await calls[0]?.clone().json()) as Record<string, unknown>;
  expect(Object.keys(body).toSorted()).toEqual(['display_name', 'namespace', 'workflow_id']);
  expect(body.workflow_id).toBe(workflowId);
});

test('the response name is read back verbatim, not echoed from the request', async () => {
  const calls: Request[] = [];
  // The server trims what it records; the caller must display the RECORDED name.
  const client = clientReturning({ run_id: runId, display_name: 'trimmed name' }, calls);

  const result = await client.rename(
    { workflowId, displayName: '  trimmed name  ' },
    { namespace }
  );

  expect(result.displayName).toBe('trimmed name');
});

test('a malformed rename response is a typed error, never a silent success', async () => {
  const missingRun = clientReturning({ display_name: 'x' }, []);
  await expect(
    missingRun.rename({ workflowId, displayName: 'x' }, { namespace })
  ).rejects.toBeInstanceOf(ApiError);

  const missingName = clientReturning({ run_id: runId }, []);
  await expect(
    missingName.rename({ workflowId, displayName: 'x' }, { namespace })
  ).rejects.toBeInstanceOf(ApiError);

  const notAnObject = clientReturning(['nope'], []);
  await expect(
    notAnObject.rename({ workflowId, displayName: 'x' }, { namespace })
  ).rejects.toBeInstanceOf(ApiError);
});

test('a server refusal surfaces as a typed ApiError with its code', async () => {
  // The fixture body is the server's ACTUAL refusal text
  // (`crates/aion-server/src/api/handlers/workflows.rs`, the rename handler's
  // blank guard), not a paraphrase. A fixture that invents its own wording only
  // ever proves the client agrees with the fixture; this one proves the client
  // carries what the server really says, message included, so an operator sees
  // the server's reason rather than a generic 400.
  const blank = clientReturning(
    {
      code: 'invalid_input',
      message: 'display_name must not be empty; a rename records a non-empty label',
    },
    [],
    { status: 400 }
  );

  try {
    await blank.rename({ workflowId, displayName: 'not blank here' }, { namespace });
    throw new Error('expected rename to throw');
  } catch (error) {
    expect(error).toBeInstanceOf(ApiError);
    expect((error as ApiError).status).toBe(400);
    expect((error as ApiError).code).toBe('invalid_input');
    expect((error as Error).message).toContain(
      'display_name must not be empty; a rename records a non-empty label'
    );
  }

  const missing = clientReturning({ code: 'not_found', message: 'no such workflow' }, [], {
    status: 404,
  });
  await expect(
    missing.rename({ workflowId, displayName: 'x' }, { namespace })
  ).rejects.toBeInstanceOf(ApiError);
});

// #211, minor 10: the REST client refuses a present-but-blank name at its own
// boundary, on BOTH the surfaces that carry one. Before this, `startWorkflow`
// forwarded `display_name: ''` for any library caller that bypassed the start
// form's validation, while the rename path refused blanks — one field, two
// answers, and the forwarded one buys a round trip to a refusal the client
// could have given itself. The blank predicate is shared with the form and the
// rename hook (`@/lib/workflowIdentity`), so all three agree by construction.
test('a present-but-blank display name is refused by the client, never forwarded', async () => {
  for (const blank of ['', '   ', '\t', '\n ']) {
    const renameCalls: Request[] = [];
    const renameClient = clientReturning({ run_id: runId, display_name: 'x' }, renameCalls);
    await expect(
      renameClient.rename({ workflowId, displayName: blank }, { namespace })
    ).rejects.toThrow(/must not be blank/);
    expect(renameCalls).toHaveLength(0);

    const startCalls: Request[] = [];
    const startClient = clientReturning({ workflow_id: workflowId, run_id: runId }, startCalls);
    await expect(
      startClient.startWorkflow(
        { workflowType: 'EmailDigest', input: {}, displayName: blank },
        { namespace }
      )
    ).rejects.toThrow(/must not be blank/);
    expect(startCalls).toHaveLength(0);
  }
});

// The trim is the client's, not the caller's: the server trims before
// recording, so an untrimmed name sent from here would leave every reader
// holding a string the run does not wear.
test('the client sends the trimmed name on both surfaces', async () => {
  const renameCalls: Request[] = [];
  const renameClient = clientReturning(
    { run_id: runId, display_name: 'nightly digest' },
    renameCalls
  );
  await renameClient.rename({ workflowId, displayName: '  nightly digest  ' }, { namespace });
  expect(await renameCalls[0]?.clone().json()).toMatchObject({
    display_name: 'nightly digest',
  });

  const startCalls: Request[] = [];
  const startClient = clientReturning({ workflow_id: workflowId, run_id: runId }, startCalls);
  await startClient.startWorkflow(
    { workflowType: 'EmailDigest', input: {}, displayName: '  nightly digest  ' },
    { namespace }
  );
  expect(await startCalls[0]?.clone().json()).toMatchObject({
    display_name: 'nightly digest',
  });
});
