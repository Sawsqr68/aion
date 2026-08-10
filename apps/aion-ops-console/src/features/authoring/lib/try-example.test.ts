import { describe, expect, test } from 'bun:test';

import type { AwlDocument } from './facade';
import { GuidedFlowRefusedError } from './facade';
import {
  createExampleDocument,
  deriveExampleName,
  paramsForCreatedDocument,
  runTryExample,
} from './try-example';

const EXAMPLE = '//! Cool-down between retries.\nworkflow cool_down\n  sleep 30s\n';
const WORKER_EXAMPLE =
  '//! A typed worker contract.\nworker payments\n  action charge() -> String\n';

describe('deriveExampleName', () => {
  test('takes the name of the top-level workflow declaration', () => {
    expect(deriveExampleName(EXAMPLE)).toBe('cool_down');
  });

  test('takes the name of a top-level worker declaration', () => {
    expect(deriveExampleName(WORKER_EXAMPLE)).toBe('payments');
  });

  test('the FIRST declaration wins in a document carrying both', () => {
    expect(deriveExampleName(`${EXAMPLE}\n${WORKER_EXAMPLE}`)).toBe('cool_down');
  });

  test('an indented or mid-line mention of the keywords is not a declaration', () => {
    const source = '//! The word workflow appears here.\n  workflow indented\nstep call_worker\n';
    expect(deriveExampleName(source)).toBeNull();
  });

  test('a source with no declaration derives no name', () => {
    expect(deriveExampleName('//! Nothing declared.\n')).toBeNull();
  });
});

function collisionRefusal(name: string): GuidedFlowRefusedError {
  return new GuidedFlowRefusedError('DocumentExists', `AWL document already exists: ${name}.awl`);
}

function createdDocument(name: string): AwlDocument {
  return { path: `${name}.awl`, name };
}

describe('createExampleDocument', () => {
  test('creates under the derived name on the first attempt', async () => {
    const attempts: string[] = [];
    const created = await createExampleDocument(async (name) => {
      attempts.push(name);
      return createdDocument(name);
    }, EXAMPLE);
    expect(attempts).toEqual(['cool_down']);
    expect(created).toEqual({ path: 'cool_down.awl', name: 'cool_down' });
  });

  test('a collision retries `_2`, `_3`, … until a create succeeds', async () => {
    const attempts: string[] = [];
    const created = await createExampleDocument(async (name) => {
      attempts.push(name);
      if (name !== 'cool_down_3') throw collisionRefusal(name);
      return createdDocument(name);
    }, EXAMPLE);
    expect(attempts).toEqual(['cool_down', 'cool_down_2', 'cool_down_3']);
    expect(created).toEqual({ path: 'cool_down_3.awl', name: 'cool_down_3' });
  });

  test('a non-collision refusal stops the loop and propagates verbatim', async () => {
    const attempts: string[] = [];
    const refusal = new GuidedFlowRefusedError(
      'AuthoringWorkspaceUnconfigured',
      'AWL workspace is not configured'
    );
    let caught: unknown = null;
    try {
      await createExampleDocument(async (name) => {
        attempts.push(name);
        throw refusal;
      }, EXAMPLE);
    } catch (error) {
      caught = error;
    }
    expect(caught).toBe(refusal);
    expect(attempts).toEqual(['cool_down']);
  });

  test('a source with no declaration is refused without calling the server', async () => {
    const attempts: string[] = [];
    let caught: unknown = null;
    try {
      await createExampleDocument(async (name) => {
        attempts.push(name);
        return createdDocument(name);
      }, '//! Nothing declared.\n');
    } catch (error) {
      caught = error;
    }
    expect(caught).toBeInstanceOf(GuidedFlowRefusedError);
    expect((caught as GuidedFlowRefusedError).code).toBe('ExampleNameUnderivable');
    expect(attempts).toEqual([]);
  });
});

describe('paramsForCreatedDocument', () => {
  test('one update selects the workspace tab and the created document', () => {
    const next = paramsForCreatedDocument(new URLSearchParams('source=language'), 'cool_down.awl');
    // The studio's canonical workspace URL carries NO `source` parameter —
    // `AuthoringView` writes the workspace tab the same way and reads its
    // absence as the workspace — so deleting it IS `?source=workspace`.
    expect(next.get('source')).toBeNull();
    expect(next.get('doc')).toBe('cool_down.awl');
  });

  test('navigating from the deployed tab replaces the open document too', () => {
    const next = paramsForCreatedDocument(
      new URLSearchParams('source=deployed&doc=other.awl&node=n1'),
      'cool_down_2.awl'
    );
    expect(next.get('source')).toBeNull();
    expect(next.get('doc')).toBe('cool_down_2.awl');
    // Unrelated parameters survive: the update touches only the two the
    // navigation owns.
    expect(next.get('node')).toBe('n1');
  });
});

/** A promise settled by hand, for holding one step of the flow open. */
function deferred(): { promise: Promise<void>; resolve: () => void } {
  let resolve: () => void = () => undefined;
  const promise = new Promise<void>((settle) => {
    resolve = settle;
  });
  return { promise, resolve };
}

describe('runTryExample', () => {
  /**
   * The ordering is LOAD-BEARING: the workspace panel deselects a `?doc` its
   * cached list does not contain, so navigating before the refresh has
   * RESOLVED — not merely started — would bounce the operator off the
   * document they just created. Held open with a deferred refresh: navigate
   * must not have run while the refresh is pending, and must run once it
   * settles.
   */
  test('navigate waits for the document-list refresh to resolve', async () => {
    const order: string[] = [];
    const refresh = deferred();
    const flow = runTryExample(
      {
        create: async (name) => {
          order.push(`create:${name}`);
          return createdDocument(name);
        },
        refreshDocuments: () => {
          order.push('refresh-called');
          return refresh.promise;
        },
        navigate: (path) => {
          order.push(`navigate:${path}`);
        },
      },
      EXAMPLE
    );

    // A macrotask turn: the flow has run as far as it can — past the create,
    // into the pending refresh — and navigate must still not have fired.
    await new Promise((settle) => setTimeout(settle, 0));
    expect(order).toEqual(['create:cool_down', 'refresh-called']);

    refresh.resolve();
    const created = await flow;
    expect(order).toEqual(['create:cool_down', 'refresh-called', 'navigate:cool_down.awl']);
    expect(created).toEqual({ path: 'cool_down.awl', name: 'cool_down' });
  });

  test('a create refusal propagates and neither refresh nor navigate ever runs', async () => {
    const calls: string[] = [];
    const refusal = new GuidedFlowRefusedError(
      'AuthoringWorkspaceUnconfigured',
      'AWL workspace is not configured'
    );
    let caught: unknown = null;
    try {
      await runTryExample(
        {
          create: async () => {
            calls.push('create');
            throw refusal;
          },
          refreshDocuments: async () => {
            calls.push('refresh');
          },
          navigate: () => {
            calls.push('navigate');
          },
        },
        EXAMPLE
      );
    } catch (error) {
      caught = error;
    }
    expect(caught).toBe(refusal);
    expect(calls).toEqual(['create']);
  });

  test('a refresh failure propagates and navigate never runs', async () => {
    const calls: string[] = [];
    const failure = new Error('the document list could not be refreshed');
    let caught: unknown = null;
    try {
      await runTryExample(
        {
          create: async (name) => {
            calls.push('create');
            return createdDocument(name);
          },
          refreshDocuments: async () => {
            calls.push('refresh');
            throw failure;
          },
          navigate: () => {
            calls.push('navigate');
          },
        },
        EXAMPLE
      );
    } catch (error) {
      caught = error;
    }
    expect(caught).toBe(failure);
    expect(calls).toEqual(['create', 'refresh']);
  });
});

/**
 * The whole flow against a fake server enforcing the real create semantics —
 * atomic create-or-`DocumentExists`, exactly like `ConfinedDir::create_new`
 * behind `POST /awl/documents`. Two operators clicking the same example land
 * on two distinct documents, each carrying the verbatim source, and the
 * navigation from the Language tab ends on the workspace tab.
 */
describe('the try-it flow end to end', () => {
  function fakeServer() {
    const documents = new Map<string, string>();
    return {
      documents,
      async create(name: string, source: string): Promise<AwlDocument> {
        const path = `${name}.awl`;
        if (documents.has(path)) throw collisionRefusal(name);
        documents.set(path, source);
        return { path, name };
      },
    };
  }

  test('two clicks on the same example create two documents, verbatim, and navigate to each', async () => {
    const server = fakeServer();
    let params = new URLSearchParams('source=language');
    const dependenciesFor = () => ({
      create: (name: string) => server.create(name, EXAMPLE),
      refreshDocuments: async () => undefined,
      navigate: (path: string) => {
        params = paramsForCreatedDocument(params, path);
      },
    });

    const first = await runTryExample(dependenciesFor(), EXAMPLE);
    expect(first.path).toBe('cool_down.awl');
    expect(params.get('doc')).toBe('cool_down.awl');
    expect(params.get('source')).toBeNull();

    const second = await runTryExample(dependenciesFor(), EXAMPLE);
    expect(second.path).toBe('cool_down_2.awl');
    expect(params.get('doc')).toBe('cool_down_2.awl');

    expect(server.documents.get('cool_down.awl')).toBe(EXAMPLE);
    expect(server.documents.get('cool_down_2.awl')).toBe(EXAMPLE);
  });
});
