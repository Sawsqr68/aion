import { describe, expect, test } from 'bun:test';
import { renderToStaticMarkup } from 'react-dom/server';

import { deriveSchemaForm } from '../lib/schemaForm';
import { SchemaFormFields } from './SchemaFormFields';
import { StartInputEditor, serializeInput } from './StartInputEditor';

const MODEL = deriveSchemaForm({
  type: 'object',
  properties: {
    to: { type: 'string', description: 'Recipient address' },
    attempts: { type: 'integer' },
    dryRun: { type: 'boolean' },
    priority: { enum: ['low', 'high'] },
    retry: { type: 'object', properties: { backoff: { type: 'number' } }, required: ['backoff'] },
    tags: { type: 'array', items: { type: 'string' } },
  },
  required: ['to'],
});

function editor(overrides: Partial<Parameters<typeof StartInputEditor>[0]> = {}) {
  return renderToStaticMarkup(
    <StartInputEditor
      inputId="input"
      inputText=""
      loading={false}
      mode="form"
      model={MODEL}
      onInputTextChange={() => undefined}
      onModeChange={() => undefined}
      {...overrides}
    />
  );
}

describe('StartInputEditor', () => {
  // Catches: the no-schema path drifting from today's raw editor — same label,
  // same textarea, same help line, and NO mode toggle to click.
  test('with no schema it renders exactly the raw JSON editor, no toggle', () => {
    const markup = editor({ model: null });
    expect(markup).toContain('Input (JSON)');
    expect(markup).toContain('Blank sends an empty object.');
    expect(markup).toContain('<textarea');
    expect(markup).not.toContain('Input mode');
  });

  // M8. Catches: the mode flash — rendering the raw JSON editor while the
  // schema is still being resolved, then swapping to the form when it lands.
  // While loading, the surface is a placeholder: no textarea, no form.
  test('while the schema is loading it holds a placeholder, never the raw editor', () => {
    const markup = editor({ model: null, loading: true });
    expect(markup).not.toContain('<textarea');
    expect(markup).not.toContain('Input (JSON)');
    expect(markup).toContain('Checking the deployed contract');
  });

  // Catches: a resolved model ignoring `loading` (a stale flag must not hide
  // a form that is already renderable).
  test('a resolved model renders the form even if a refetch is in flight', () => {
    const markup = editor({ loading: true });
    expect(markup).toContain('Input mode');
    expect(markup).toContain('Recipient address');
  });

  // Minor 8. Catches: the form-mode surface losing its accessible name — in
  // form mode there is no single input for a label to point at, so the whole
  // surface must be a fieldset whose legend names it (the visible heading is
  // aria-hidden so assistive tech hears one name, not two).
  test('form mode names the surface with a fieldset legend', () => {
    const markup = editor({ inputText: '{}' });
    expect(markup).toContain('<legend class="sr-only">Input</legend>');
    expect(markup).toContain('aria-hidden="true"');
  });

  // Catches: the toggle disappearing, or one of the two views becoming
  // unreachable — both must always be offered when a schema exists.
  test('with a schema both Form and JSON are offered and the form is shown', () => {
    const markup = editor();
    expect(markup).toContain('Input mode');
    expect(markup).toContain('>Form<');
    expect(markup).toContain('>JSON<');
    // Form mode renders generated fields, not the textarea.
    expect(markup).toContain('Recipient address');
    expect(markup).not.toContain('<textarea');
  });

  // Catches: losing the raw-JSON option when a schema exists.
  test('JSON mode keeps the raw textarea with the shared input text', () => {
    const markup = editor({ mode: 'json', inputText: '{"to":"ops"}' });
    expect(markup).toContain('<textarea');
    expect(markup).toContain('&quot;to&quot;:&quot;ops&quot;');
  });

  // Catches: the form rendering over input it cannot represent. Broken JSON
  // must force the JSON view WITH the reason visible, keeping the text
  // editable rather than hiding it behind a form showing something else.
  test('unparseable input forces the JSON view and says why', () => {
    const markup = editor({ mode: 'form', inputText: '{not json' });
    expect(markup).toContain('<textarea');
    expect(markup).toContain('needs the input to parse as a JSON object');
  });

  // Catches: form mode inventing its own state instead of pre-populating from
  // the shared serialized input.
  test('the form pre-populates from the current JSON text', () => {
    const markup = editor({ inputText: '{"to": "ops@aion", "attempts": 4}' });
    expect(markup).toContain('value="ops@aion"');
    expect(markup).toContain('value="4"');
  });
});

describe('SchemaFormFields', () => {
  const fields = MODEL?.fields ?? [];

  // Catches: dropped required markers or descriptions-as-help.
  test('renders required markers and descriptions', () => {
    const markup = renderToStaticMarkup(
      <SchemaFormFields fields={fields} onChange={() => undefined} value={{}} />
    );
    expect(markup).toContain('(required)');
    expect(markup).toContain('Recipient address');
  });

  // Catches: an enum rendered as a free-text input, or options going missing.
  test('an enum renders a select carrying every option plus the absent choice', () => {
    const markup = renderToStaticMarkup(
      <SchemaFormFields fields={fields} onChange={() => undefined} value={{}} />
    );
    expect(markup).toContain('(not set)');
    expect(markup).toContain('>low<');
    expect(markup).toContain('>high<');
  });

  // Catches: booleans rendered as a checkbox, which cannot express "absent".
  test('a boolean renders the three-way select', () => {
    const markup = renderToStaticMarkup(
      <SchemaFormFields fields={fields} onChange={() => undefined} value={{}} />
    );
    expect(markup).toContain('>true<');
    expect(markup).toContain('>false<');
  });

  // Catches: nested objects flattened out of their fieldset, or array rows
  // losing their remove/add affordances.
  test('nested objects render as fieldsets and arrays render rows with add/remove', () => {
    const markup = renderToStaticMarkup(
      <SchemaFormFields fields={fields} onChange={() => undefined} value={{ tags: ['a', 'b'] }} />
    );
    expect(markup).toContain('<fieldset');
    expect(markup).toContain('backoff');
    expect(markup).toContain('Add item');
    expect(markup).toContain('Remove item 1');
    expect(markup).toContain('Remove item 2');
    expect(markup).toContain('value="a"');
    expect(markup).toContain('value="b"');
  });

  // M4. Catches: JsonField never rendering (its body could be `return null`
  // and every other test would still pass). An un-typable property must
  // surface as a per-property raw JSON editor carrying the current value.
  test('an un-typable property renders as a JSON field carrying its value', () => {
    const model = deriveSchemaForm({
      type: 'object',
      properties: { union: { oneOf: [{ type: 'string' }, { type: 'number' }] } },
    });
    expect(model).not.toBeNull();
    const markup = renderToStaticMarkup(
      <SchemaFormFields
        fields={model?.fields ?? []}
        onChange={() => undefined}
        value={{ union: { pick: 'a' } }}
      />
    );
    expect(markup).toContain('placeholder="JSON value"');
    expect(markup).toContain('&quot;pick&quot;: &quot;a&quot;');
  });

  // Minor 4. Catches: an out-of-domain value displayed as "(not set)" — the
  // form must state the value it cannot represent, as raw JSON.
  test('out-of-domain enum and boolean values are named, not shown as unset', () => {
    const markup = renderToStaticMarkup(
      <SchemaFormFields
        fields={fields}
        onChange={() => undefined}
        value={{ priority: 'urgent', dryRun: 'yes' }}
      />
    );
    expect(markup).toContain('&quot;urgent&quot; is outside the schema');
    expect(markup).toContain('&quot;yes&quot; is outside the schema');
  });

  // Minor 3. Catches: a non-object value under an object field being
  // silently replaced on the first nested edit — the warning must be visible
  // BEFORE the data is destroyed.
  test('an object field whose current value is not an object warns before replacing it', () => {
    const markup = renderToStaticMarkup(
      <SchemaFormFields fields={fields} onChange={() => undefined} value={{ retry: 5 }} />
    );
    expect(markup).toContain('is not an object');
    expect(markup).toContain('replaces it');
  });
});

describe('serializeInput', () => {
  // Catches: form serialization drifting from what the JSON view parses —
  // the round-trip IS the mode-switch guarantee.
  test('round-trips through JSON.parse', () => {
    const value = { to: 'ops', retry: { backoff: 1.5 }, tags: ['a'] };
    expect(JSON.parse(serializeInput(value))).toEqual(value);
  });
});
