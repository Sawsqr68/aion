import { type FormEvent, useId, useState } from 'react';

import { Button } from '@/components/ui';
import type { Namespace, RunId, WorkflowId } from '@/types';

import { type RenameClient, useRename } from '../hooks/useRename';

export type RenameControlProps = {
  /** The run being labelled. The rename TARGETS this id — never a name. */
  workflowId: WorkflowId;
  /** Auth scope for the command; `null` disables the commit. */
  namespace: Namespace | null;
  /** The name the run currently wears (projected from history); seeds the editor. */
  displayName: string | null;
  /** Pin the rename to a concrete run; absent targets the latest run. */
  runId?: RunId | undefined;
  /** Injected client (tests); defaults to the namespace-scoped client. */
  apiClient?: RenameClient | undefined;
  /** Open the editor on first render (tests only). */
  initialOpen?: boolean;
};

const INPUT_CLASS =
  'h-7 rounded-md border border-input bg-transparent px-2 text-sm outline-none focus:border-muted-foreground';

/**
 * Set or change a run's display name (#211).
 *
 * The name is a LABEL: this control never becomes a way to FIND a workflow, and
 * the id it renames stays visible beside it in the header. The outcome is
 * reported honestly — the confirmation quotes the name the server actually
 * RECORDED (trimmed), and a refusal (blank name, absent workflow, denied
 * namespace grant) is shown as a visible error rather than swallowed.
 */
export function RenameControl({
  workflowId,
  namespace,
  displayName,
  runId,
  apiClient,
  initialOpen = false,
}: RenameControlProps) {
  const inputId = useId();
  const [open, setOpen] = useState(initialOpen);
  const [value, setValue] = useState(displayName ?? '');
  const rename = useRename({ namespace, apiClient });
  const canSubmit = value.trim().length > 0 && namespace !== null && !rename.isPending;

  function openEditor() {
    // Seed from the name the run wears RIGHT NOW (it may have changed since
    // this component mounted), and drop any previous outcome so a stale error
    // never reads as the result of the edit about to be made.
    setValue(displayName ?? '');
    rename.reset();
    setOpen(true);
  }

  function handleSubmit(event: FormEvent) {
    event.preventDefault();

    if (!canSubmit) {
      return;
    }

    rename.mutate(
      { workflowId, displayName: value, ...(runId === undefined ? {} : { runId }) },
      // Close only on a CONFIRMED record; a refusal keeps the editor open with
      // the operator's text intact so it can be corrected and retried.
      { onSuccess: () => setOpen(false) }
    );
  }

  if (!open) {
    return (
      <div className="flex items-center gap-2">
        <Button
          className="h-7 px-3 text-xs"
          data-testid="rename-open"
          onClick={openEditor}
          type="button"
          variant="outline"
        >
          {displayName === null ? 'Name run' : 'Rename'}
        </Button>
        <RenameOutcome
          displayName={rename.data?.displayName ?? null}
          error={rename.error}
          isError={rename.isError}
        />
      </div>
    );
  }

  return (
    <form className="flex flex-wrap items-center gap-2" onSubmit={handleSubmit}>
      <label className="sr-only" htmlFor={inputId}>
        Display name
      </label>
      <input
        className={INPUT_CLASS}
        data-testid="rename-input"
        id={inputId}
        onChange={(event) => setValue(event.currentTarget.value)}
        placeholder="a name for this run"
        value={value}
      />
      <Button
        className="h-7 px-3 text-xs"
        data-testid="rename-submit"
        disabled={!canSubmit}
        type="submit"
        variant="default"
      >
        {rename.isPending ? 'Saving…' : 'Save name'}
      </Button>
      <Button
        className="h-7 px-3 text-xs"
        data-testid="rename-cancel"
        onClick={() => setOpen(false)}
        type="button"
        variant="ghost"
      >
        Cancel
      </Button>
      <RenameGate error={rename.error} isError={rename.isError} namespace={namespace} />
    </form>
  );
}

/**
 * The reason a rename cannot be committed, or the refusal that came back.
 * Distinct, honest visible state — never a disabled button with no explanation.
 */
function RenameGate({
  error,
  isError,
  namespace,
}: {
  error: Error | null;
  isError: boolean;
  namespace: Namespace | null;
}) {
  if (isError && error !== null) {
    return (
      <p className="text-destructive text-xs" data-testid="rename-error" role="alert">
        {error.message}
      </p>
    );
  }

  if (namespace === null) {
    return (
      <p className="text-muted-foreground text-xs" role="note">
        Select a namespace to scope the rename command.
      </p>
    );
  }

  return null;
}

/** The last settled outcome once the editor is closed: a refusal, or the recorded name. */
function RenameOutcome({
  displayName,
  error,
  isError,
}: {
  displayName: string | null;
  error: Error | null;
  isError: boolean;
}) {
  if (isError && error !== null) {
    return (
      <p className="text-destructive text-xs" data-testid="rename-error" role="alert">
        {error.message}
      </p>
    );
  }

  if (displayName === null) {
    return null;
  }

  return (
    <p className="text-muted-foreground text-xs" data-testid="rename-recorded" role="note">
      Recorded as "{displayName}"
    </p>
  );
}
