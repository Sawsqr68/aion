import { useEffect, useId, useMemo, useRef, useState } from 'react';
import { Link } from 'react-router';

import { workflowDetailHref } from '@/app/routePaths';
import { ErrorState } from '@/components/ErrorState';
import { Button } from '@/components/ui';
import type { ApiClient } from '@/lib/api';
import { useDraft } from '@/lib/state';
import type { Namespace } from '@/types';

import { type StartSchemaClient, useStartInputSchema } from '../hooks/useStartInputSchema';
import { useStartWorkflow } from '../hooks/useStartWorkflow';
import { parseJsonInput } from '../lib/jsonInput';
import {
  applySchemaDefaults,
  deriveSchemaForm,
  type JsonObject,
  missingRequiredPaths,
} from '../lib/schemaForm';
import { startWorkflowDraftStoreFor } from '../lib/startWorkflowDraft';
import { StartInputEditor, type StartInputMode, serializeInput } from './StartInputEditor';

export type StartWorkflowFormProps = {
  namespace: Namespace | null;
  /** Injected client for tests. */
  apiClient?: Pick<ApiClient, 'startWorkflow'> | undefined;
  /** Preload the workflow-type field (palette deep-link). Editable as usual. */
  initialWorkflowType?: string | undefined;
  /**
   * Whether the resolved caller may read the deployed surface (runtime
   * capability, from `/whoami`). The schema-driven form needs it because
   * `GET /awl/deployed/...` sits behind the deploy grant; without it the
   * raw JSON editor is the whole input surface, exactly as before.
   */
  deployGranted?: boolean;
  /** Injected deployed-read client for tests. */
  schemaClient?: StartSchemaClient | undefined;
  /**
   * Which draft store holds this form's field state. Absent is the Actions
   * page's shared draft (key unchanged — zero change for existing callers);
   * an embedded instance (the authoring run panel) names its own scope so
   * the two surfaces cannot clobber each other's half-filled forms.
   *
   * MUST be constant for the life of a mounted form; callers change it only
   * across remounts. `useDraft` seeds its local state from the store once at
   * mount, so a scope swapped mid-mount would WRITE through to the new store
   * while still SHOWING the old store's draft.
   */
  draftScope?: string | undefined;
};

const FIELD_CLASS =
  'h-9 rounded-md border border-input bg-transparent px-3 text-sm outline-none focus:border-muted-foreground';

/**
 * Start a workflow run. Namespace-scoped command authority (ADR-022): no deploy
 * grant needed. Provenance is honest — the success panel renders only after the
 * server returns real `workflow_id`/`run_id` values, and deep-links to the run.
 *
 * The namespace is the SELECTED namespace when one exists, otherwise the operator
 * types it free-form: `POST /workflows/start` mints an unseen namespace on use
 * (open policy, `start_mint`), so a fresh server with no namespaces is not a
 * dead-end — you type a namespace and the run creates it. The effective namespace
 * is the selected prop, or the trimmed free-form entry when none is selected.
 */
export function StartWorkflowForm({
  namespace,
  apiClient,
  initialWorkflowType,
  deployGranted = false,
  schemaClient,
  draftScope,
}: StartWorkflowFormProps) {
  const ids = useFieldIds();
  // Field state lives in a sessionStorage-backed draft store (src/lib/state):
  // navigating away and back restores a half-filled form. The scope picks
  // WHICH store, so embedded instances keep their own draft.
  const draftStore = useMemo(() => startWorkflowDraftStoreFor(draftScope), [draftScope]);
  const { draft, updateDraft, clearDraft } = useDraft(draftStore);
  const [inputError, setInputError] = useState<string | null>(null);
  // Snapshot of the namespace a submit targeted: the draft clears on a
  // confirmed start, but the success panel must keep reporting the namespace
  // the run actually used.
  const [startedNamespace, setStartedNamespace] = useState<Namespace | null>(null);

  // A palette deep-link (`/actions?workflow_type=X`) can land while this route
  // is already mounted — same-route navigation re-renders without remounting —
  // so a deep-link value must sync into the field (and its draft), not just seed it.
  useEffect(() => {
    if (initialWorkflowType !== undefined) {
      updateDraft({ workflowType: initialWorkflowType });
    }
  }, [initialWorkflowType, updateDraft]);

  // No selected namespace on a fresh server: fall back to the free-form entry,
  // which the start path mints on use. A selected namespace always takes it.
  const trimmedEntry = draft.namespaceEntry.trim();
  const effectiveNamespace: Namespace | null =
    namespace ?? (trimmedEntry.length > 0 ? (trimmedEntry as Namespace) : null);

  const start = useStartWorkflow({ namespace: effectiveNamespace, apiClient });
  const trimmedType = draft.workflowType.trim();
  const canSubmit = effectiveNamespace !== null && trimmedType.length > 0 && !start.isPending;

  // The route-active deployed version's start-input schema, when this caller
  // can read the deployed surface. `null` keeps the raw JSON editor as the
  // entire input surface — exactly the pre-schema behavior.
  const { schema, isLoading: schemaLoading } = useStartInputSchema({
    workflowType: trimmedType,
    deployGranted,
    client: schemaClient,
  });
  const schemaModel = useMemo(() => (schema === null ? null : deriveSchemaForm(schema)), [schema]);
  const [inputMode, setInputMode] = useState<StartInputMode>('form');

  // When a schema arrives, seed its declared defaults into the shared input —
  // ONCE PER MODEL (the ref gate), for keys the operator's input does not
  // already carry. Re-seeding on later edits would fight an operator who
  // removed a defaulted key on purpose. The draft is the one serialized truth,
  // so the seeding is equally visible in both views.
  const seededModelRef = useRef<typeof schemaModel>(null);
  useEffect(() => {
    if (schemaModel === null || seededModelRef.current === schemaModel) return;
    const parsed = parseJsonInput(draft.inputText);
    // Unparseable input is not consumed: the gate ref is NOT advanced, so the
    // seeding retries once the input parses again instead of being skipped
    // forever because it happened to be broken at the moment the schema
    // landed.
    if (!parsed.ok) return;
    seededModelRef.current = schemaModel;
    const seeded = applySchemaDefaults(schemaModel, parsed.value as JsonObject);
    if (seeded !== parsed.value) {
      updateDraft({ inputText: serializeInput(seeded) });
    }
  }, [schemaModel, draft.inputText, updateDraft]);

  function handleSubmit(event: React.FormEvent) {
    event.preventDefault();
    setInputError(null);

    const parsed = parseJsonInput(draft.inputText);
    if (!parsed.ok) {
      setInputError(parsed.error);
      return;
    }

    if (schemaModel !== null) {
      const missing = missingRequiredPaths(schemaModel, parsed.value as JsonObject);
      if (missing.length > 0) {
        setInputError(
          `Missing required input field${missing.length === 1 ? '' : 's'}: ${missing.join(', ')}`
        );
        return;
      }
    }

    const trimmedRoutingKey = draft.routingKey.trim();
    const trimmedTaskQueue = draft.taskQueue.trim();
    setStartedNamespace(effectiveNamespace);
    start.mutate(
      {
        workflowType: trimmedType,
        input: parsed.value,
        ...(trimmedRoutingKey.length === 0 ? {} : { routingKey: trimmedRoutingKey }),
        ...(trimmedTaskQueue.length === 0 ? {} : { taskQueue: trimmedTaskQueue }),
      },
      {
        // A confirmed start consumes the draft; a failed one keeps it for retry.
        onSuccess: () => clearDraft(),
      }
    );
  }

  return (
    <div className="space-y-4">
      <form className="grid gap-3 rounded-lg border border-border p-4" onSubmit={handleSubmit}>
        {namespace === null ? (
          <label className="flex flex-col gap-2 font-medium text-sm" htmlFor={ids.namespace}>
            Namespace
            <input
              className={FIELD_CLASS}
              id={ids.namespace}
              placeholder="e.g. orders — created on start if new"
              value={draft.namespaceEntry}
              onChange={(event) => updateDraft({ namespaceEntry: event.currentTarget.value })}
            />
            <span className="font-normal text-muted-foreground text-xs">
              No namespace is selected. Type one to target — an unseen namespace is created when the
              run starts.
            </span>
          </label>
        ) : null}
        <label className="flex flex-col gap-2 font-medium text-sm" htmlFor={ids.workflowType}>
          Workflow type
          <input
            className={FIELD_CLASS}
            id={ids.workflowType}
            placeholder="e.g. EmailDigest"
            value={draft.workflowType}
            onChange={(event) => updateDraft({ workflowType: event.currentTarget.value })}
          />
        </label>
        <StartInputEditor
          inputId={ids.input}
          inputText={draft.inputText}
          loading={schemaLoading}
          mode={inputMode}
          model={schemaModel}
          onInputTextChange={(inputText) => updateDraft({ inputText })}
          onModeChange={setInputMode}
        />
        <label className="flex flex-col gap-2 font-medium text-sm" htmlFor={ids.routingKey}>
          Routing key (optional)
          <input
            className={FIELD_CLASS}
            id={ids.routingKey}
            placeholder="steers placement; blank = unsteered"
            value={draft.routingKey}
            onChange={(event) => updateDraft({ routingKey: event.currentTarget.value })}
          />
        </label>
        <label className="flex flex-col gap-2 font-medium text-sm" htmlFor={ids.taskQueue}>
          Task queue (optional)
          <input
            className={FIELD_CLASS}
            id={ids.taskQueue}
            placeholder="default activity queue; blank = namespace default"
            value={draft.taskQueue}
            onChange={(event) => updateDraft({ taskQueue: event.currentTarget.value })}
          />
        </label>
        <div className="flex items-center gap-3">
          <Button disabled={!canSubmit} type="submit">
            {start.isPending ? 'Starting…' : 'Start workflow'}
          </Button>
          {inputError === null ? null : <p className="text-destructive text-sm">{inputError}</p>}
        </div>
      </form>

      <StartOutcome
        error={start.error}
        isError={start.isError}
        namespace={startedNamespace ?? effectiveNamespace}
        result={start.data ?? null}
      />
    </div>
  );
}

export type StartOutcomeProps = {
  isError: boolean;
  error: unknown;
  /** The namespace the run targeted; non-null whenever `result` is present. */
  namespace: Namespace | null;
  result: { workflowId: string; runId: string } | null;
};

export function StartOutcome({ isError, error, namespace, result }: StartOutcomeProps) {
  if (isError) {
    return <ErrorState error={error} title="Start failed" />;
  }

  if (result === null) {
    return null;
  }

  return (
    <div className="rounded-lg border border-primary/40 bg-primary-glow p-4">
      <h3 className="font-medium text-foreground text-sm">Workflow started</h3>
      <dl className="mt-2 space-y-1 text-sm">
        <Detail label="Namespace" value={namespace ?? ''} />
        <Detail label="Workflow ID" value={result.workflowId} />
        <Detail label="Run ID" value={result.runId} />
      </dl>
      <Link
        className="mt-3 inline-block text-primary text-sm underline-offset-4 hover:underline"
        to={workflowDetailHref(result.workflowId)}
      >
        Open run →
      </Link>
    </div>
  );
}

function Detail({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex gap-2">
      <dt className="text-muted-foreground">{label}</dt>
      <dd className="font-mono text-foreground">{value}</dd>
    </div>
  );
}

function useFieldIds() {
  const prefix = useId();

  return {
    namespace: `${prefix}-namespace`,
    workflowType: `${prefix}-workflow-type`,
    input: `${prefix}-input`,
    routingKey: `${prefix}-routing-key`,
    taskQueue: `${prefix}-task-queue`,
  };
}
