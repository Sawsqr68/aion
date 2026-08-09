/**
 * JSON Schema → start-form derivation, and the value plumbing under it.
 *
 * The form never owns a second copy of the input: it reads and writes the SAME
 * JSON object the raw editor serializes, so switching between Form and JSON
 * views cannot lose data — there is exactly one value, rendered two ways.
 *
 * Derivation is deliberately honest about its limits. A property this module
 * cannot render as a typed field (a union, an object-valued array, a schema
 * with no usable type) becomes a `json` field — a per-property raw JSON input —
 * rather than being silently dropped, which would make the form a filter that
 * quietly discards parts of the contract.
 */

export type JsonObject = Record<string, unknown>;

export type ScalarKind = 'string' | 'number' | 'integer' | 'boolean';

/** One enum option: the JSON value sent on the wire and its display label. */
export type EnumOption = { value: string | number | boolean; label: string };

type FieldBase = {
  /** Property name at its level (the last path segment). */
  name: string;
  /** Full path from the input root, e.g. `['retry', 'attempts']`. */
  path: readonly string[];
  /** Whether the ENCLOSING schema lists this property as required. */
  required: boolean;
  /** Schema `description`, rendered as help text. */
  description: string | null;
  /** Schema `default`, applied when the current input carries no value. */
  defaultValue: unknown;
};

export type SchemaField =
  | (FieldBase & { kind: 'scalar'; scalar: ScalarKind })
  | (FieldBase & { kind: 'enum'; options: EnumOption[] })
  | (FieldBase & { kind: 'object'; fields: SchemaField[] })
  | (FieldBase & { kind: 'array'; item: ArrayItem })
  | (FieldBase & { kind: 'json' });

/** What one element of a scalar array is. */
export type ArrayItem =
  | { kind: 'scalar'; scalar: ScalarKind }
  | { kind: 'enum'; options: EnumOption[] };

export type SchemaFormModel = { fields: SchemaField[] };

/**
 * Derive a form model from a workflow input schema.
 *
 * Returns `null` when the schema cannot host a field form AT ALL — it is not an
 * object schema, or it declares no properties. In that case the caller keeps
 * the raw JSON editor as the whole surface; `null` is "no form", never an
 * error.
 */
export function deriveSchemaForm(schema: unknown): SchemaFormModel | null {
  if (!isRecord(schema)) return null;
  if (schema.type !== undefined && schema.type !== 'object') return null;
  const properties = schema.properties;
  if (!isRecord(properties)) return null;
  const names = Object.keys(properties);
  if (names.length === 0) return null;
  const required = requiredNames(schema);
  return {
    fields: names.map((name) => deriveField(name, [name], properties[name], required.has(name))),
  };
}

function deriveField(
  name: string,
  path: readonly string[],
  schema: unknown,
  required: boolean
): SchemaField {
  const base: FieldBase = {
    name,
    path,
    required,
    description:
      isRecord(schema) && typeof schema.description === 'string' ? schema.description : null,
    defaultValue: isRecord(schema) ? schema.default : undefined,
  };
  if (!isRecord(schema)) return { ...base, kind: 'json' };

  const options = enumOptions(schema.enum);
  if (options !== null) return { ...base, kind: 'enum', options };

  switch (schema.type) {
    case 'string':
    case 'number':
    case 'integer':
    case 'boolean':
      return { ...base, kind: 'scalar', scalar: schema.type };
    case 'object': {
      const properties = schema.properties;
      if (!isRecord(properties)) return { ...base, kind: 'json' };
      const nestedRequired = requiredNames(schema);
      return {
        ...base,
        kind: 'object',
        fields: Object.keys(properties).map((nested) =>
          deriveField(nested, [...path, nested], properties[nested], nestedRequired.has(nested))
        ),
      };
    }
    case 'array': {
      const item = arrayItem(schema.items);
      if (item === null) return { ...base, kind: 'json' };
      return { ...base, kind: 'array', item };
    }
    default:
      return { ...base, kind: 'json' };
  }
}

function arrayItem(items: unknown): ArrayItem | null {
  if (!isRecord(items)) return null;
  const options = enumOptions(items.enum);
  if (options !== null) return { kind: 'enum', options };
  if (
    items.type === 'string' ||
    items.type === 'number' ||
    items.type === 'integer' ||
    items.type === 'boolean'
  ) {
    return { kind: 'scalar', scalar: items.type };
  }
  return null;
}

/** `enum` values a `<select>` can host: scalars only, and at least one. */
function enumOptions(candidate: unknown): EnumOption[] | null {
  if (!Array.isArray(candidate) || candidate.length === 0) return null;
  const options: EnumOption[] = [];
  for (const value of candidate) {
    if (typeof value === 'string' || typeof value === 'number' || typeof value === 'boolean') {
      options.push({ value, label: String(value) });
    } else {
      return null;
    }
  }
  return options;
}

function requiredNames(schema: JsonObject): ReadonlySet<string> {
  const names = schema.required;
  if (!Array.isArray(names)) return new Set();
  return new Set(names.filter((name): name is string => typeof name === 'string'));
}

/** Read the value a field renders, walking the path through nested objects. */
export function valueAtPath(root: JsonObject, path: readonly string[]): unknown {
  let current: unknown = root;
  for (const segment of path) {
    if (!isRecord(current)) return undefined;
    current = current[segment];
  }
  return current;
}

/**
 * Write one field's value immutably, creating intermediate objects as needed.
 * `undefined` REMOVES the key (an emptied optional field is absent on the
 * wire, not `""`/`null` — the server sees exactly what the operator sees).
 */
export function withValueAtPath(
  root: JsonObject,
  path: readonly string[],
  value: unknown
): JsonObject {
  const [head, ...rest] = path;
  if (head === undefined) return root;
  const next: JsonObject = { ...root };
  if (rest.length === 0) {
    if (value === undefined) delete next[head];
    else next[head] = value;
    return next;
  }
  const child = next[head];
  next[head] = withValueAtPath(isRecord(child) ? child : {}, rest, value);
  return next;
}

/**
 * Seed the form's value from the current input, filling schema defaults for
 * fields the input does not carry. The existing input always wins — switching
 * to Form view must render what the JSON view holds, not overwrite it.
 */
export function applySchemaDefaults(model: SchemaFormModel, value: JsonObject): JsonObject {
  let seeded = value;
  const visit = (fields: SchemaField[]): void => {
    for (const field of fields) {
      if (field.defaultValue !== undefined && valueAtPath(seeded, field.path) === undefined) {
        seeded = withValueAtPath(seeded, field.path, field.defaultValue);
      }
      if (field.kind === 'object') visit(field.fields);
    }
  };
  visit(model.fields);
  return seeded;
}

/**
 * Names every required field the value does not carry, as dotted paths.
 * Presence is the test — `""` and `null` are values the operator typed and the
 * server is the authority on whether they are acceptable. Requirements inside
 * a nested object apply only once that object exists, matching JSON Schema:
 * an absent optional object is valid regardless of what it would require.
 */
export function missingRequiredPaths(model: SchemaFormModel, value: JsonObject): string[] {
  const missing: string[] = [];
  const visit = (fields: SchemaField[]): void => {
    for (const field of fields) {
      const present = valueAtPath(value, field.path) !== undefined;
      if (field.required && !present) {
        missing.push(field.path.join('.'));
      }
      if (field.kind === 'object' && present) visit(field.fields);
    }
  };
  visit(model.fields);
  return missing;
}

function isRecord(value: unknown): value is JsonObject {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

// ---------------------------------------------------------------------------
// The form's write decisions, extracted pure so each is testable on its own.
// The components hold NO write semantics of their own: what a control writes
// for a given raw input is decided here, and only here.
// ---------------------------------------------------------------------------

/** The result of editing a scalar field's text. */
export type ScalarEditOutcome =
  | {
      /** Write `value` through to the shared input (`undefined` REMOVES the key). */
      kind: 'write';
      value: string | number | undefined;
      /** A visible caveat to show beside the field even though the value was written. */
      warning: string | null;
    }
  | {
      /** Keep the raw text local and visible; write nothing. */
      kind: 'reject';
      warning: string;
    };

/**
 * What a scalar edit writes. An EMPTIED field writes `undefined` — the key is
 * removed, so an optional the operator cleared is absent on the wire, never
 * `""`. Numeric text that does not parse is rejected with a visible reason
 * (never silently discarded), and a non-integer in an integer field is
 * written — the server is the authority — with the violation stated beside
 * the field rather than left undocumented.
 */
export function scalarEditOutcome(
  raw: string,
  scalar: 'string' | 'number' | 'integer'
): ScalarEditOutcome {
  if (raw.length === 0) return { kind: 'write', value: undefined, warning: null };
  if (scalar === 'string') return { kind: 'write', value: raw, warning: null };
  const parsed = Number(raw.trim());
  if (!Number.isFinite(parsed) || raw.trim().length === 0) {
    return { kind: 'reject', warning: 'Not a number — kept here until it parses.' };
  }
  if (scalar === 'integer' && !Number.isInteger(parsed)) {
    return {
      kind: 'write',
      value: parsed,
      warning: 'The schema says integer — the server may refuse this value.',
    };
  }
  return { kind: 'write', value: parsed, warning: null };
}

/**
 * The three-way boolean: `""` is ABSENT (`undefined` — the key is removed),
 * which a checkbox cannot express. `"true"`/`"false"` are the two values.
 */
export function booleanFromToken(token: string): boolean | undefined {
  if (token === 'true') return true;
  if (token === 'false') return false;
  return undefined;
}

/** The select token rendering a boolean's current value ('' = absent/out-of-domain). */
export function booleanToken(current: unknown): '' | 'true' | 'false' {
  if (current === true) return 'true';
  if (current === false) return 'false';
  return '';
}

/**
 * Enum selections round-trip by option INDEX so a numeric or boolean enum
 * keeps its JSON type — mapping through the raw token or the label would send
 * `"3"` for `3`. `""` (the absent choice) writes `undefined`.
 */
export function enumValueFromToken(options: EnumOption[], token: string): unknown {
  if (token === '') return undefined;
  const index = Number(token);
  if (!Number.isInteger(index) || index < 0 || index >= options.length) return undefined;
  return options[index]?.value;
}

/** The select token rendering an enum's current value ('' = absent/out-of-domain). */
export function enumToken(options: EnumOption[], current: unknown): string {
  const index = options.findIndex((option) => option.value === current);
  return index === -1 ? '' : String(index);
}

/**
 * The visible truth for a value the schema's domain does not contain: its raw
 * JSON, named as outside the domain — never displayed as "(not set)", which
 * would be a lie about a value that IS set.
 */
export function domainNote(current: unknown, inDomain: boolean): string | null {
  if (current === undefined || inDomain) return null;
  return `Current value ${JSON.stringify(current)} is outside the schema's domain.`;
}

/**
 * The warning for an object field whose current value is present but not an
 * object: editing a nested field will REPLACE that value. The behavior is
 * kept (the schema says object; the nested write builds one) — but it is
 * stated where the operator can see it, before data is destroyed silently.
 */
export function nonObjectPresenceWarning(root: JsonObject, path: readonly string[]): string | null {
  const current = valueAtPath(root, path);
  if (current === undefined || isRecord(current)) return null;
  return `Current value ${JSON.stringify(current)} is not an object — editing a field below replaces it.`;
}
