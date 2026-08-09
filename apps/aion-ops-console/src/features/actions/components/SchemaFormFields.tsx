import { useId, useState } from 'react';

import {
  type ArrayItem,
  booleanFromToken,
  booleanToken,
  domainNote,
  type EnumOption,
  enumToken,
  enumValueFromToken,
  type JsonObject,
  nonObjectPresenceWarning,
  type SchemaField,
  scalarEditOutcome,
  valueAtPath,
  withValueAtPath,
} from '../lib/schemaForm';

const FIELD_CLASS =
  'h-9 rounded-md border border-input bg-transparent px-3 text-sm outline-none focus:border-muted-foreground';
const AREA_CLASS =
  'min-h-20 rounded-md border border-input bg-transparent px-3 py-2 font-mono text-sm outline-none focus:border-muted-foreground';

export type SchemaFormFieldsProps = {
  fields: SchemaField[];
  /** The ONE input value — the same object the raw JSON editor serializes. */
  value: JsonObject;
  onChange: (next: JsonObject) => void;
};

/**
 * The generated start form: one control per schema property, all reading and
 * writing the single shared input object. Every write decision — what an
 * emptied field, a select token, or a numeric text becomes on the wire — is a
 * pure function in `../lib/schemaForm`; these components only render and
 * relay, so the semantics are testable without a DOM.
 */
export function SchemaFormFields({ fields, value, onChange }: SchemaFormFieldsProps) {
  return (
    <div className="grid gap-3">
      {fields.map((field) => (
        <Field field={field} key={field.path.join('.')} onChange={onChange} root={value} />
      ))}
    </div>
  );
}

function Field({
  field,
  root,
  onChange,
}: {
  field: SchemaField;
  root: JsonObject;
  onChange: (next: JsonObject) => void;
}) {
  const id = useId();
  const current = valueAtPath(root, field.path);
  const write = (next: unknown) => onChange(withValueAtPath(root, field.path, next));

  if (field.kind === 'object') {
    const conflict = nonObjectPresenceWarning(root, field.path);
    return (
      <fieldset className="grid gap-3 rounded-md border border-border p-3">
        <legend className="px-1 font-medium text-sm">
          <FieldName field={field} />
        </legend>
        <Description field={field} />
        {conflict === null ? null : <Warning text={conflict} />}
        {field.fields.map((nested) => (
          <Field field={nested} key={nested.path.join('.')} onChange={onChange} root={root} />
        ))}
      </fieldset>
    );
  }

  return (
    <label className="flex flex-col gap-2 font-medium text-sm" htmlFor={id}>
      <span>
        <FieldName field={field} />
      </span>
      <FieldControl current={current} field={field} id={id} write={write} />
      <Description field={field} />
    </label>
  );
}

function FieldControl({
  current,
  field,
  id,
  write,
}: {
  current: unknown;
  field: Exclude<SchemaField, { kind: 'object' }>;
  id: string;
  write: (next: unknown) => void;
}) {
  switch (field.kind) {
    case 'scalar':
      return field.scalar === 'boolean' ? (
        <BooleanSelect current={current} id={id} write={write} />
      ) : (
        <ScalarInput current={current} id={id} scalar={field.scalar} write={write} />
      );
    case 'enum':
      return <EnumSelect current={current} id={id} options={field.options} write={write} />;
    case 'array':
      return <ArrayEditor current={current} id={id} item={field.item} write={write} />;
    case 'json':
      return <JsonField current={current} id={id} write={write} />;
  }
}

/**
 * Local text with write-through: what the operator typed stays visible even
 * while it does not parse (a rejected numeric edit is KEPT and named, never
 * silently discarded), and an external change to the underlying value — a
 * default seeding, a JSON-view edit — resets the text to the new truth.
 */
function useTextDraft(externalText: string) {
  const [draft, setDraft] = useState({ base: externalText, text: externalText });
  // External change: the value this text was derived from is no longer what
  // the input holds — re-derive. Our own successful writes advance `base` to
  // the serialization they produce, so they do not reset the operator's text.
  const text = draft.base === externalText ? draft.text : externalText;
  return {
    text,
    /** Record an edit whose write went through, moving the external baseline. */
    accept: (nextText: string, nextExternal: string) =>
      setDraft({ base: nextExternal, text: nextText }),
    /** Record an edit that was kept local (nothing written). */
    keep: (nextText: string) => setDraft({ base: externalText, text: nextText }),
  };
}

function ScalarInput({
  current,
  id,
  scalar,
  write,
}: {
  current: unknown;
  id: string;
  scalar: 'string' | 'number' | 'integer';
  write: (next: unknown) => void;
}) {
  const externalText = scalarDisplay(current);
  const { text, accept, keep } = useTextDraft(externalText);
  const outcome = scalarEditOutcome(text, scalar);
  return (
    <>
      <input
        className={FIELD_CLASS}
        id={id}
        inputMode={scalar === 'string' ? undefined : 'decimal'}
        type="text"
        value={text}
        onChange={(event) => {
          const raw = event.currentTarget.value;
          const decided = scalarEditOutcome(raw, scalar);
          if (decided.kind === 'write') {
            accept(raw, scalarDisplay(decided.value));
            write(decided.value);
          } else {
            keep(raw);
          }
        }}
      />
      {text.length === 0 || outcome.warning === null ? null : <Warning text={outcome.warning} />}
    </>
  );
}

function scalarDisplay(current: unknown): string {
  if (current === undefined || current === null) return '';
  if (typeof current === 'string') return current;
  if (typeof current === 'number' || typeof current === 'boolean') return String(current);
  return JSON.stringify(current);
}

/**
 * Booleans are a three-way choice, not a checkbox: an optional flag the
 * operator never touched must stay ABSENT, which a checkbox cannot express.
 * The write decision is `booleanFromToken`.
 */
function BooleanSelect({
  current,
  id,
  write,
}: {
  current: unknown;
  id: string;
  write: (next: unknown) => void;
}) {
  const note = domainNote(current, current === true || current === false);
  return (
    <>
      <select
        className={FIELD_CLASS}
        id={id}
        value={booleanToken(current)}
        onChange={(event) => write(booleanFromToken(event.currentTarget.value))}
      >
        <option value="">(not set)</option>
        <option value="true">true</option>
        <option value="false">false</option>
      </select>
      {note === null ? null : <Warning text={note} />}
    </>
  );
}

/** The write decision is `enumValueFromToken` (by-index, JSON type preserved). */
function EnumSelect({
  current,
  id,
  options,
  write,
}: {
  current: unknown;
  id: string;
  options: EnumOption[];
  write: (next: unknown) => void;
}) {
  const note = domainNote(
    current,
    options.some((option) => option.value === current)
  );
  return (
    <>
      <select
        className={FIELD_CLASS}
        id={id}
        value={enumToken(options, current)}
        onChange={(event) => write(enumValueFromToken(options, event.currentTarget.value))}
      >
        <option value="">(not set)</option>
        {options.map((option, index) => (
          <option key={option.label} value={String(index)}>
            {option.label}
          </option>
        ))}
      </select>
      {note === null ? null : <Warning text={note} />}
    </>
  );
}

function ArrayEditor({
  current,
  id,
  item,
  write,
}: {
  current: unknown;
  id: string;
  item: ArrayItem;
  write: (next: unknown) => void;
}) {
  const elements = Array.isArray(current) ? current : [];
  // The array is written AS IT IS, including empty: removing the last row
  // leaves `[]` — an intentional empty list the operator can send (and a
  // required array that stays present). Absence remains expressible by never
  // adding a row, or through the JSON view.
  const writeElements = (next: unknown[]) => write(next);
  return (
    <div className="grid gap-2" id={id}>
      {elements.map((element, index) => (
        <div
          className="flex items-center gap-2"
          // biome-ignore lint/suspicious/noArrayIndexKey: rows are positional values with no identity of their own
          key={index}
        >
          <div className="flex-1">
            <ArrayElement
              element={element}
              item={item}
              write={(next) => {
                const updated = [...elements];
                updated[index] = next;
                writeElements(updated);
              }}
            />
          </div>
          <button
            aria-label={`Remove item ${index + 1}`}
            className="rounded-md border border-input px-2 py-1 text-muted-foreground text-xs hover:text-foreground"
            onClick={() => writeElements(elements.filter((_, at) => at !== index))}
            type="button"
          >
            Remove
          </button>
        </div>
      ))}
      <button
        className="justify-self-start rounded-md border border-input px-2 py-1 text-muted-foreground text-xs hover:text-foreground"
        onClick={() =>
          writeElements([
            ...elements,
            item.kind === 'scalar' && item.scalar === 'string' ? '' : null,
          ])
        }
        type="button"
      >
        Add item
      </button>
    </div>
  );
}

function ArrayElement({
  element,
  item,
  write,
}: {
  element: unknown;
  item: ArrayItem;
  write: (next: unknown) => void;
}) {
  const elementId = useId();
  // A row leaves the array through its Remove button; an emptied input inside
  // a row holds `null` so the row the operator added stays visible in both
  // views instead of vanishing.
  const writeElement = (next: unknown) => write(next === undefined ? null : next);
  if (item.kind === 'enum') {
    return (
      <EnumSelect current={element} id={elementId} options={item.options} write={writeElement} />
    );
  }
  if (item.scalar === 'boolean') {
    return <BooleanSelect current={element} id={elementId} write={writeElement} />;
  }
  return <ScalarInput current={element} id={elementId} scalar={item.scalar} write={writeElement} />;
}

/**
 * The honest fallback for a property the form cannot type: a per-property raw
 * JSON input. Invalid JSON stays local (with a visible note) and never
 * corrupts the shared input value; an external change to the property resets
 * the text to the new truth (`useTextDraft`).
 */
function JsonField({
  current,
  id,
  write,
}: {
  current: unknown;
  id: string;
  write: (next: unknown) => void;
}) {
  const externalText = current === undefined ? '' : JSON.stringify(current, null, 2);
  const { text, accept, keep } = useTextDraft(externalText);
  const [invalid, setInvalid] = useState(false);
  return (
    <>
      <textarea
        className={AREA_CLASS}
        id={id}
        placeholder="JSON value"
        value={text}
        onChange={(event) => {
          const raw = event.currentTarget.value;
          if (raw.trim().length === 0) {
            setInvalid(false);
            accept(raw, '');
            write(undefined);
            return;
          }
          try {
            const parsed: unknown = JSON.parse(raw);
            accept(raw, JSON.stringify(parsed, null, 2));
            write(parsed);
            setInvalid(false);
          } catch {
            keep(raw);
            setInvalid(true);
          }
        }}
      />
      {invalid ? (
        <Warning text="Not valid JSON — the last valid value is kept until this parses." />
      ) : null}
    </>
  );
}

function FieldName({ field }: { field: SchemaField }) {
  return (
    <>
      {field.name}
      {field.required ? (
        <span className="text-destructive">
          {' *'}
          <span className="sr-only">(required)</span>
        </span>
      ) : null}
    </>
  );
}

function Description({ field }: { field: SchemaField }) {
  if (field.description === null) return null;
  return <span className="font-normal text-muted-foreground text-xs">{field.description}</span>;
}

function Warning({ text }: { text: string }) {
  return (
    <span className="font-normal text-warning text-xs" role="status">
      {text}
    </span>
  );
}
