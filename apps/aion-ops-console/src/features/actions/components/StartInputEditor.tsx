import { Skeleton } from '@/components/ui';
import { cn } from '@/lib/utils';

import { parseJsonInput } from '../lib/jsonInput';
import type { JsonObject, SchemaFormModel } from '../lib/schemaForm';
import { SchemaFormFields } from './SchemaFormFields';

const AREA_CLASS =
  'min-h-28 rounded-md border border-input bg-transparent px-3 py-2 font-mono text-sm outline-none focus:border-muted-foreground';

export type StartInputMode = 'form' | 'json';

export type StartInputEditorProps = {
  /** Id for the raw JSON textarea (label wiring). */
  inputId: string;
  /** The serialized input — the single source both views read and write. */
  inputText: string;
  onInputTextChange: (text: string) => void;
  /** Derived form model, or `null` when the type carries no usable schema. */
  model: SchemaFormModel | null;
  /**
   * True while a schema might still arrive for the entered type. Rendered as
   * a placeholder rather than the raw editor, so the surface does not flash
   * from the JSON editor to the form the moment the schema lands.
   */
  loading: boolean;
  mode: StartInputMode;
  onModeChange: (mode: StartInputMode) => void;
};

/**
 * The start-input surface. With no schema it is exactly the raw JSON editor.
 * With one, a visible toggle offers a generated form and the raw editor over
 * the SAME serialized input: the form parses `inputText` and writes every edit
 * back through it, so switching views can never lose data — there is nothing
 * view-local to lose.
 */
export function StartInputEditor({
  inputId,
  inputText,
  onInputTextChange,
  model,
  loading,
  mode,
  onModeChange,
}: StartInputEditorProps) {
  if (model === null) {
    // While the schema is still being resolved, hold the space with a
    // placeholder instead of the raw editor: rendering JSON first and
    // swapping to the form when the schema lands is exactly the mode flash
    // this surface promises not to make.
    if (loading) {
      return (
        <div className="flex flex-col gap-2 font-medium text-sm">
          <span>Input</span>
          <Skeleton className="min-h-28 w-full rounded-md" />
          <span className="font-normal text-muted-foreground text-xs">
            Checking the deployed contract for an input schema…
          </span>
        </div>
      );
    }
    return (
      <label className="flex flex-col gap-2 font-medium text-sm" htmlFor={inputId}>
        Input (JSON)
        <RawJsonArea
          inputId={inputId}
          inputText={inputText}
          onInputTextChange={onInputTextChange}
        />
        <span className="font-normal text-muted-foreground text-xs">
          Blank sends an empty object. The engine requires a payload.
        </span>
      </label>
    );
  }

  const parsed = parseJsonInput(inputText);
  // A form cannot render input that does not parse as a JSON object; forcing
  // the JSON view (with the reason visible) keeps the broken text editable
  // instead of hiding it behind a form that shows something else.
  const formAvailable = parsed.ok;
  const effectiveMode: StartInputMode = formAvailable ? mode : 'json';

  // The whole surface is a semantically LABELLED group (fieldset + legend):
  // in form mode there is no single input for a `<label htmlFor>` to point
  // at, and a bare heading span would name nothing accessibly. The legend is
  // visually hidden; the visible "Input" heading is aria-hidden so assistive
  // tech hears one name, not two.
  return (
    <fieldset className="flex flex-col gap-2">
      <legend className="sr-only">Input</legend>
      <div className="flex items-center justify-between gap-3">
        <span aria-hidden="true" className="font-medium text-sm">
          Input
        </span>
        <fieldset
          aria-label="Input mode"
          className="flex rounded-lg border border-border bg-surface-elevated p-0.5"
        >
          <ModeButton
            disabled={!formAvailable}
            label="Form"
            mode="form"
            onSelect={onModeChange}
            selected={effectiveMode}
          />
          <ModeButton
            disabled={false}
            label="JSON"
            mode="json"
            onSelect={onModeChange}
            selected={effectiveMode}
          />
        </fieldset>
      </div>
      {formAvailable ? null : (
        <p className="text-destructive text-xs" role="status">
          The form view needs the input to parse as a JSON object — fix it here first.
        </p>
      )}
      {effectiveMode === 'form' && parsed.ok ? (
        <SchemaFormFields
          fields={model.fields}
          onChange={(next: JsonObject) => onInputTextChange(serializeInput(next))}
          value={parsed.value as JsonObject}
        />
      ) : (
        <label className="flex flex-col gap-2 font-medium text-sm" htmlFor={inputId}>
          <span className="sr-only">Input (JSON)</span>
          <RawJsonArea
            inputId={inputId}
            inputText={inputText}
            onInputTextChange={onInputTextChange}
          />
          <span className="font-normal text-muted-foreground text-xs">
            Blank sends an empty object. The engine requires a payload.
          </span>
        </label>
      )}
    </fieldset>
  );
}

/** Serialize a form edit back into the shared input text. */
export function serializeInput(value: JsonObject): string {
  return JSON.stringify(value, null, 2);
}

function RawJsonArea({
  inputId,
  inputText,
  onInputTextChange,
}: {
  inputId: string;
  inputText: string;
  onInputTextChange: (text: string) => void;
}) {
  return (
    <textarea
      className={AREA_CLASS}
      id={inputId}
      placeholder='{} or {"to": "ops@aion"}'
      value={inputText}
      onChange={(event) => onInputTextChange(event.currentTarget.value)}
    />
  );
}

function ModeButton({
  disabled,
  label,
  mode,
  onSelect,
  selected,
}: {
  disabled: boolean;
  label: string;
  mode: StartInputMode;
  onSelect: (mode: StartInputMode) => void;
  selected: StartInputMode;
}) {
  return (
    <button
      aria-pressed={selected === mode}
      className={cn(
        'rounded-md px-2.5 py-1 text-xs outline-none focus-visible:ring-2 focus-visible:ring-accent-primary',
        selected === mode
          ? 'bg-accent text-foreground'
          : 'text-muted-foreground hover:text-foreground',
        disabled && 'cursor-not-allowed opacity-50'
      )}
      disabled={disabled}
      onClick={() => onSelect(mode)}
      type="button"
    >
      {label}
    </button>
  );
}
