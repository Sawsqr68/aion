import { describe, expect, test } from 'bun:test';

import {
  applySchemaDefaults,
  booleanFromToken,
  booleanToken,
  deriveSchemaForm,
  domainNote,
  enumToken,
  enumValueFromToken,
  missingRequiredPaths,
  nonObjectPresenceWarning,
  scalarEditOutcome,
  valueAtPath,
  withValueAtPath,
} from './schemaForm';

/**
 * The reference schema exercises every field family the form generator
 * supports at once: scalars, required markers, enum, defaults, descriptions,
 * a nested object, and an array of scalars.
 */
const SCHEMA = {
  type: 'object',
  properties: {
    to: { type: 'string', description: 'Recipient address' },
    attempts: { type: 'integer', default: 3 },
    threshold: { type: 'number' },
    dryRun: { type: 'boolean' },
    priority: { enum: ['low', 'high'] },
    retry: {
      type: 'object',
      properties: {
        backoff: { type: 'number' },
        jitter: { type: 'boolean' },
      },
      required: ['backoff'],
    },
    tags: { type: 'array', items: { type: 'string' } },
  },
  required: ['to', 'priority'],
};

describe('deriveSchemaForm', () => {
  // Catches: a generator that drops properties, loses required flags, or
  // flattens kinds — every property must arrive as its own typed field.
  test('derives one typed field per property with required, defaults, and descriptions', () => {
    const model = deriveSchemaForm(SCHEMA);
    expect(model).not.toBeNull();
    if (model === null) return;
    const byName = new Map(model.fields.map((field) => [field.name, field]));
    expect(byName.get('to')).toMatchObject({
      kind: 'scalar',
      scalar: 'string',
      required: true,
      description: 'Recipient address',
    });
    expect(byName.get('attempts')).toMatchObject({
      kind: 'scalar',
      scalar: 'integer',
      defaultValue: 3,
      required: false,
    });
    expect(byName.get('threshold')).toMatchObject({ kind: 'scalar', scalar: 'number' });
    expect(byName.get('dryRun')).toMatchObject({ kind: 'scalar', scalar: 'boolean' });
    expect(byName.get('priority')).toMatchObject({
      kind: 'enum',
      required: true,
      options: [
        { value: 'low', label: 'low' },
        { value: 'high', label: 'high' },
      ],
    });
    expect(byName.get('tags')).toMatchObject({
      kind: 'array',
      item: { kind: 'scalar', scalar: 'string' },
    });
  });

  // Catches: nested objects rendered flat, or nested required lists read from
  // the wrong level.
  test('nested objects derive nested fields with their own required markers and full paths', () => {
    const model = deriveSchemaForm(SCHEMA);
    const retry = model?.fields.find((field) => field.name === 'retry');
    expect(retry?.kind).toBe('object');
    if (retry?.kind !== 'object') return;
    const backoff = retry.fields.find((field) => field.name === 'backoff');
    expect(backoff).toMatchObject({ required: true, path: ['retry', 'backoff'] });
    const jitter = retry.fields.find((field) => field.name === 'jitter');
    expect(jitter).toMatchObject({ required: false });
  });

  // Catches: a fallback that silently DROPS what it cannot type. An
  // un-renderable property must still surface (as a raw JSON field).
  test('an unsupported property becomes a json field, never disappears', () => {
    const model = deriveSchemaForm({
      type: 'object',
      properties: {
        union: { oneOf: [{ type: 'string' }, { type: 'number' }] },
        table: { type: 'array', items: { type: 'object' } },
      },
    });
    expect(model?.fields.map((field) => [field.name, field.kind])).toEqual([
      ['union', 'json'],
      ['table', 'json'],
    ]);
  });

  // Catches: rendering a form for schemas that cannot host one — these must
  // fall back to the raw JSON editor (model null), not an empty form.
  test('non-object and property-less schemas yield no form', () => {
    expect(deriveSchemaForm(null)).toBeNull();
    expect(deriveSchemaForm(undefined)).toBeNull();
    expect(deriveSchemaForm({ type: 'string' })).toBeNull();
    expect(deriveSchemaForm({ type: 'object' })).toBeNull();
    expect(deriveSchemaForm({ type: 'object', properties: {} })).toBeNull();
    expect(deriveSchemaForm({ type: 'array', items: { type: 'string' } })).toBeNull();
  });

  // Catches: a non-scalar enum shoved into a <select> (its values could not
  // round-trip through an option).
  test('an enum with non-scalar members falls back to a json field', () => {
    const model = deriveSchemaForm({
      type: 'object',
      properties: { odd: { enum: [{ nested: true }] } },
    });
    expect(model?.fields[0]?.kind).toBe('json');
  });
});

describe('value plumbing', () => {
  // Catches: a writer that mutates the input or loses sibling keys — the form
  // and the raw editor share ONE object, so both defects would corrupt it.
  test('withValueAtPath writes immutably and keeps siblings', () => {
    const before = { to: 'ops', retry: { backoff: 1 } };
    const after = withValueAtPath(before, ['retry', 'jitter'], true);
    expect(after).toEqual({ to: 'ops', retry: { backoff: 1, jitter: true } });
    expect(before).toEqual({ to: 'ops', retry: { backoff: 1 } });
  });

  test('withValueAtPath creates intermediate objects for absent parents', () => {
    expect(withValueAtPath({}, ['retry', 'backoff'], 2)).toEqual({ retry: { backoff: 2 } });
  });

  // Catches: an emptied optional field serialized as "" / null instead of
  // being ABSENT on the wire.
  test('writing undefined removes the key', () => {
    expect(withValueAtPath({ to: 'ops', extra: 1 }, ['extra'], undefined)).toEqual({ to: 'ops' });
    expect(
      withValueAtPath({ retry: { backoff: 1, jitter: true } }, ['retry', 'jitter'], undefined)
    ).toEqual({
      retry: { backoff: 1 },
    });
  });

  test('valueAtPath reads nested values and answers undefined off the map', () => {
    const value = { retry: { backoff: 7 } };
    expect(valueAtPath(value, ['retry', 'backoff'])).toBe(7);
    expect(valueAtPath(value, ['retry', 'missing'])).toBeUndefined();
    expect(valueAtPath(value, ['absent', 'deep'])).toBeUndefined();
  });

  // Round trip: form edits serialize to JSON and parse back to the same value,
  // so switching Form ↔ JSON views preserves data by construction.
  test('a form-built value survives JSON serialization unchanged', () => {
    let value: Record<string, unknown> = {};
    value = withValueAtPath(value, ['to'], 'ops@aion');
    value = withValueAtPath(value, ['attempts'], 3);
    value = withValueAtPath(value, ['retry', 'backoff'], 1.5);
    value = withValueAtPath(value, ['tags'], ['a', 'b']);
    expect(JSON.parse(JSON.stringify(value))).toEqual(value);
  });
});

describe('applySchemaDefaults', () => {
  const model = deriveSchemaForm(SCHEMA);

  // Catches: defaults not applied, or applied OVER what the operator typed.
  test('fills declared defaults only where the input carries no value', () => {
    expect(model).not.toBeNull();
    if (model === null) return;
    expect(applySchemaDefaults(model, {})).toEqual({ attempts: 3 });
    expect(applySchemaDefaults(model, { attempts: 9 })).toEqual({ attempts: 9 });
  });

  // Catches: a seeding pass that reports change when nothing changed (the
  // caller writes through to the draft only on a new reference).
  test('returns the same reference when nothing needed seeding', () => {
    expect(model).not.toBeNull();
    if (model === null) return;
    const input = { attempts: 5 };
    expect(applySchemaDefaults(model, input)).toBe(input);
  });
});

describe('missingRequiredPaths', () => {
  const model = deriveSchemaForm(SCHEMA);

  // Catches: submit-time validation that misses a required field or names the
  // wrong path.
  test('names every absent required field as a dotted path', () => {
    expect(model).not.toBeNull();
    if (model === null) return;
    expect(missingRequiredPaths(model, {})).toEqual(['to', 'priority']);
    expect(missingRequiredPaths(model, { to: 'ops', priority: 'low' })).toEqual([]);
  });

  // Catches: nested requirements enforced while their optional parent is
  // absent (JSON Schema semantics: an absent optional object is valid), and
  // the inverse — nested requirements ignored once the parent exists.
  test('nested required fields apply exactly when their parent object exists', () => {
    expect(model).not.toBeNull();
    if (model === null) return;
    expect(missingRequiredPaths(model, { to: 'x', priority: 'low' })).toEqual([]);
    expect(
      missingRequiredPaths(model, { to: 'x', priority: 'low', retry: { jitter: true } })
    ).toEqual(['retry.backoff']);
  });

  // Catches: presence conflated with truthiness — "" and 0 and null are typed
  // values the server judges, not absences.
  test('empty-string, zero, and null count as present', () => {
    expect(model).not.toBeNull();
    if (model === null) return;
    expect(missingRequiredPaths(model, { to: '', priority: null })).toEqual([]);
  });
});

describe('scalarEditOutcome (the emptied-optional and numeric write decisions)', () => {
  // M4. Catches: an emptied field written as `""` instead of REMOVING the
  // key — the exact "emptied-optional → ''" mutation. `value: undefined`
  // means absent on the wire.
  test('an emptied field writes undefined (the key is removed), never ""', () => {
    expect(scalarEditOutcome('', 'string')).toEqual({
      kind: 'write',
      value: undefined,
      warning: null,
    });
    expect(scalarEditOutcome('', 'number')).toEqual({
      kind: 'write',
      value: undefined,
      warning: null,
    });
  });

  test('string text writes through verbatim', () => {
    expect(scalarEditOutcome('ops@aion', 'string')).toEqual({
      kind: 'write',
      value: 'ops@aion',
      warning: null,
    });
  });

  // Catches: numbers written as strings (type coercion on the wire).
  test('numeric text writes a NUMBER, not its string', () => {
    expect(scalarEditOutcome('1.5', 'number')).toEqual({
      kind: 'write',
      value: 1.5,
      warning: null,
    });
    expect(scalarEditOutcome('42', 'integer')).toEqual({ kind: 'write', value: 42, warning: null });
  });

  // Minor 5. Catches: an unparseable numeric edit silently discarded — it is
  // REJECTED with a visible reason, and nothing is written.
  test('unparseable numeric text is rejected with a visible reason', () => {
    expect(scalarEditOutcome('1e', 'number')).toEqual({
      kind: 'reject',
      warning: 'Not a number — kept here until it parses.',
    });
    expect(scalarEditOutcome('  ', 'number').kind).toBe('reject');
  });

  // Minor 6. Catches: a non-integer accepted into an integer field with no
  // visible account — the value is written (the server judges) but the
  // schema violation is stated.
  test('a non-integer in an integer field writes with a visible caveat', () => {
    expect(scalarEditOutcome('1.5', 'integer')).toEqual({
      kind: 'write',
      value: 1.5,
      warning: 'The schema says integer — the server may refuse this value.',
    });
  });
});

describe('booleanFromToken / booleanToken (the three-way boolean)', () => {
  // M4. Catches: the three-way collapse into checkbox semantics ('' → false).
  // Absent, true, and false are three distinct answers.
  test('the empty token is ABSENT (undefined), not false', () => {
    expect(booleanFromToken('')).toBeUndefined();
    expect(booleanFromToken('true')).toBe(true);
    expect(booleanFromToken('false')).toBe(false);
  });

  test('rendering round-trips all three states, and out-of-domain shows unselected', () => {
    expect(booleanToken(true)).toBe('true');
    expect(booleanToken(false)).toBe('false');
    expect(booleanToken(undefined)).toBe('');
    expect(booleanToken('yes')).toBe('');
  });
});

describe('enumValueFromToken / enumToken (by-index, JSON type preserved)', () => {
  const options = [
    { value: 3, label: '3' },
    { value: 'high', label: 'high' },
    { value: true, label: 'true' },
  ];

  // M4. Catches: the enum round-trip degrading to the raw token or the label
  // — a numeric enum would send `"3"` for `3`, a boolean `"true"` for `true`.
  test('selection by index preserves the option value JSON type', () => {
    expect(enumValueFromToken(options, '0')).toBe(3);
    expect(enumValueFromToken(options, '1')).toBe('high');
    expect(enumValueFromToken(options, '2')).toBe(true);
  });

  test('the empty token and out-of-range tokens write ABSENT', () => {
    expect(enumValueFromToken(options, '')).toBeUndefined();
    expect(enumValueFromToken(options, '9')).toBeUndefined();
    expect(enumValueFromToken(options, 'high')).toBeUndefined();
  });

  test('rendering finds the current value by identity, out-of-domain unselected', () => {
    expect(enumToken(options, 3)).toBe('0');
    expect(enumToken(options, 'high')).toBe('1');
    expect(enumToken(options, '3')).toBe('');
    expect(enumToken(options, undefined)).toBe('');
  });
});

describe('domainNote / nonObjectPresenceWarning (the visible truths)', () => {
  // Minor 4. Catches: an out-of-domain value described as "(not set)" — the
  // note must carry the value's raw JSON.
  test('an out-of-domain value is named by its raw JSON', () => {
    expect(domainNote('urgent', false)).toBe(
      `Current value "urgent" is outside the schema's domain.`
    );
    expect(domainNote(5, true)).toBeNull();
    expect(domainNote(undefined, false)).toBeNull();
  });

  // Minor 3. Catches: a non-object intermediate silently replaced with `{}`
  // — the replacement behavior is kept, but it must be STATED first.
  test('a present non-object under an object field is warned about, with its value', () => {
    expect(nonObjectPresenceWarning({ retry: 5 }, ['retry'])).toBe(
      'Current value 5 is not an object — editing a field below replaces it.'
    );
    expect(nonObjectPresenceWarning({ retry: { backoff: 1 } }, ['retry'])).toBeNull();
    expect(nonObjectPresenceWarning({}, ['retry'])).toBeNull();
  });
});
