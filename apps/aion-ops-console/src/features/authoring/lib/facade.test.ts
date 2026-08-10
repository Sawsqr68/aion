import { describe, expect, test } from 'bun:test';

import { createAuthoringFacade, GestureRefusedError, GuidedFlowRefusedError } from './facade';

const fixtures = [
  {
    ok: true,
    deploys_green: true,
    steps: 3,
    diagnostics: [],
    semantic: {
      entries: [],
      graph: { steps: [], edges: [], child_calls: [] },
      studio: {
        builtins: ['String'],
        types: [
          { name: 'Order', kind: 'record', fields: [{ name: 'id', type: 'String' }], variants: [] },
        ],
        workers: [{ name: 'jobs', actions: [{ name: 'run', params: [], return_type: 'String' }] }],
      },
    },
    expected: {
      ok: true,
      deploysGreen: true,
      steps: 3,
      diagnostics: [],
      semantic: {
        entries: [],
        graph: { steps: [], edges: [], childCalls: [] },
        studio: {
          builtins: ['String'],
          types: [
            {
              name: 'Order',
              kind: 'record',
              fields: [{ name: 'id', type: 'String' }],
              variants: [],
            },
          ],
          workers: [{ name: 'jobs', actions: [{ name: 'run', params: [], returnType: 'String' }] }],
        },
      },
    },
  },
  {
    ok: true,
    deploys_green: false,
    steps: 2,
    diagnostics: [{ class: 'emit_subset', message: 'backend subset', line: 4, column: 2 }],
    semantic: null,
    expected: {
      ok: true,
      deploysGreen: false,
      steps: 2,
      diagnostics: [{ class: 'emit_subset', message: 'backend subset', line: 4, column: 2 }],
      semantic: null,
    },
  },
  {
    ok: false,
    deploys_green: false,
    steps: null,
    diagnostics: [{ class: 'error', message: 'expected step', line: 1, column: 1 }],
    semantic: null,
    expected: {
      ok: false,
      deploysGreen: false,
      steps: null,
      diagnostics: [{ class: 'error', message: 'expected step', line: 1, column: 1 }],
      semantic: null,
    },
  },
];

describe('authoring facade parsing', () => {
  for (const [index, fixture] of fixtures.entries()) {
    test(`parses diagnostic state ${index + 1}`, async () => {
      const facade = createAuthoringFacade(async () => Response.json(fixture));
      expect(JSON.stringify(await facade.check('workflow demo {}'))).toBe(
        JSON.stringify(fixture.expected)
      );
    });
  }

  test('parses the flow-shape graph vocabulary across the wire', async () => {
    const span = { start: 0, end: 4, line: 1, column: 1 };
    const wireStep = (name: string, extra: Record<string, unknown>) => ({
      name,
      documentation: '',
      activities: [],
      span,
      kind: 'plain',
      region: null,
      distribution: null,
      collect: null,
      subflow: null,
      substeps: [],
      visits: null,
      decision: false,
      waits: false,
      body: { nodes: [], edges: [] },
      failure: null,
      ...extra,
    });
    const wire = {
      ok: true,
      deploys_green: true,
      steps: 3,
      diagnostics: [],
      semantic: {
        entries: [
          {
            span,
            type: 'String',
            declaration: {
              name: 'coordinator_instructions',
              kind: 'const',
              documentation: null,
              span,
            },
          },
          {
            span: { ...span, start: 5, end: 13 },
            type: null,
            declaration: {
              name: 'dev_item',
              kind: 'subflow',
              documentation: null,
              span: { ...span, start: 5, end: 13 },
            },
          },
        ],
        graph: {
          steps: [
            wireStep('wave', {
              kind: 'distribute',
              distribution: { binding: 'item', collection: 'state.items' },
            }),
            wireStep('build', {
              kind: 'subflow_call',
              region: 'wave',
              subflow: {
                name: 'dev_item',
                graph: {
                  steps: [wireStep('review', { decision: true, visits: '3' })],
                  edges: [],
                  child_calls: [],
                },
              },
              body: {
                nodes: [
                  {
                    id: 'statement:20:40',
                    kind: 'parallel_fork',
                    label: 'item in state.items',
                    detail: 'join → results',
                    span,
                    fork: {
                      binding: 'item',
                      collection: 'state.items',
                      sequential: false,
                      result: 'results',
                    },
                    graph: {
                      nodes: [
                        {
                          id: 'statement:24:32',
                          kind: 'child_call',
                          label: 'build_item',
                          detail: 'build_item(item: Item) -> Result',
                          span,
                          fork: null,
                          graph: null,
                        },
                      ],
                      edges: [],
                    },
                  },
                ],
                edges: [],
              },
            }),
            wireStep('gather', {
              kind: 'collect',
              collect: { binding: 'verdict', tolerant: true, result: 'results' },
            }),
          ],
          edges: [
            {
              id: 'route:fold:wave:otherwise:1',
              source: 'fold',
              target: 'wave',
              kind: 'route',
              label: 'otherwise',
              back: true,
              visits: '3',
            },
          ],
          child_calls: [],
        },
        studio: { builtins: [], types: [], workers: [] },
      },
    };
    const facade = createAuthoringFacade(async () => Response.json(wire));
    const result = await facade.check('workflow demo {}');
    const graph = result.semantic?.graph;
    expect(result.semantic?.entries.map((entry) => entry.declaration?.kind)).toEqual([
      'const',
      'subflow',
    ]);
    expect(graph?.steps[0]?.kind).toBe('distribute');
    expect(graph?.steps[0]?.distribution).toEqual({ binding: 'item', collection: 'state.items' });
    expect(graph?.steps[1]?.region).toBe('wave');
    expect(graph?.steps[1]?.subflow?.name).toBe('dev_item');
    expect(graph?.steps[1]?.subflow?.graph?.steps[0]?.decision).toBe(true);
    expect(graph?.steps[1]?.subflow?.graph?.childCalls).toEqual([]);
    expect(graph?.steps[1]?.body.nodes[0]?.kind).toBe('parallel_fork');
    expect(graph?.steps[1]?.body.nodes[0]?.fork?.collection).toBe('state.items');
    expect(graph?.steps[1]?.body.nodes[0]?.graph?.nodes[0]?.kind).toBe('child_call');
    expect(graph?.steps[2]?.collect).toEqual({
      binding: 'verdict',
      tolerant: true,
      result: 'results',
    });
    expect(graph?.edges[0]?.back).toBe(true);
    expect(graph?.edges[0]?.visits).toBe('3');
  });

  test('sends the documented check shape', async () => {
    let body = '';
    const facade = createAuthoringFacade(async (_input, init) => {
      body = String(init?.body);
      return Response.json(fixtures[0]);
    });
    await facade.check('source', 'nested/demo.awl');
    expect(JSON.parse(body)).toEqual({ source: 'source', path: 'nested/demo.awl' });
  });

  test('surfaces typed gesture refusals without applying source', async () => {
    const facade = createAuthoringFacade(async () =>
      Response.json({
        ok: false,
        diagnostics: [],
        refusal: { code: 'name_collision', message: 'step name already exists' },
      })
    );
    const operation = { type: 'add_step', name: 'existing' } as const;

    try {
      await facade.edit('source', operation);
      throw new Error('gesture unexpectedly succeeded');
    } catch (error) {
      expect(error).toBeInstanceOf(GestureRefusedError);
      expect((error as GestureRefusedError).code).toBe('name_collision');
      expect((error as Error).message).toBe('step name already exists');
    }
  });

  test('requests and parses a worker scaffold file map', async () => {
    let body = '';
    const facade = createAuthoringFacade(async (_input, init) => {
      body = String(init?.body);
      return Response.json({ ok: true, files: { 'worker.toml': '[worker]' } });
    });

    expect(await facade.scaffold('source', 'jobs', 'shell')).toEqual({
      'worker.toml': '[worker]',
    });
    expect(JSON.parse(body)).toEqual({ source: 'source', worker: 'jobs', runtime: 'shell' });
  });

  test('creates a workflow with the documented name body', async () => {
    let body = '';
    const facade = createAuthoringFacade(async (_input, init) => {
      body = String(init?.body);
      return Response.json(
        { path: 'order_flow.awl', name: 'order_flow', source: 'canonical' },
        { status: 201 }
      );
    });
    expect(await facade.createDocument('order_flow')).toEqual({
      path: 'order_flow.awl',
      name: 'order_flow',
    });
    expect(JSON.parse(body)).toEqual({ name: 'order_flow' });
  });

  /**
   * The guide's "Try it" body: a create carrying a source sends BOTH fields,
   * and a create without one still sends only the name — the placeholder
   * synthesis stays a server decision, never an empty-string source.
   */
  test('creates a document carrying its verbatim source when one is given', async () => {
    let body = '';
    const example = '//! Example.\nworkflow guide_example\n  sleep 30s\n';
    const facade = createAuthoringFacade(async (_input, init) => {
      body = String(init?.body);
      return Response.json(
        { path: 'guide_example.awl', name: 'guide_example', source: example },
        { status: 201 }
      );
    });
    expect(await facade.createDocument('guide_example', example)).toEqual({
      path: 'guide_example.awl',
      name: 'guide_example',
    });
    expect(JSON.parse(body)).toEqual({ name: 'guide_example', source: example });
  });

  /**
   * A collision must arrive as the server's own refusal — `DocumentExists`
   * with the message verbatim — because the "Try it" retry loop branches on
   * exactly that code and surfaces anything else to the operator unchanged.
   */
  test('a document collision surfaces the server refusal with its error_type', async () => {
    const facade = createAuthoringFacade(async () =>
      Response.json(
        { message: 'AWL document already exists: guide_example.awl', error_type: 'DocumentExists' },
        { status: 409 }
      )
    );
    try {
      await facade.createDocument('guide_example', 'workflow guide_example\n');
      throw new Error('collision unexpectedly succeeded');
    } catch (error) {
      expect(error).toBeInstanceOf(GuidedFlowRefusedError);
      expect((error as GuidedFlowRefusedError).code).toBe('DocumentExists');
      expect((error as Error).message).toBe('AWL document already exists: guide_example.awl');
    }
  });
});
