import { useQuery } from '@tanstack/react-query';
import { Play, X } from 'lucide-react';
import { useId, useState } from 'react';

import { Button } from '@/components/ui/button';
import { StartWorkflowForm } from '@/features/actions';
import type { Namespace } from '@/types';

import { authoringFacade, type DeploymentRecord, type WorkerAvailability } from '../lib/facade';

/** The one facade call this panel makes on its own, injectable for tests. */
export type AvailabilityClient = {
  workerAvailability(namespace: string, taskQueue: string): Promise<WorkerAvailability>;
};

export type RunPanelProps = {
  /** The selected namespace; the form falls back to free-form entry when null. */
  namespace: Namespace | null;
  /** Whether the caller may read the deployed surface (schema-driven input form). */
  deployGranted: boolean;
  /**
   * Seed for the workflow-type field (see `runSeedWorkflowType`). The panel
   * body captures it ONCE per open — the header recomputes it as the buffer
   * changes, and letting it track live edits would clobber what the operator
   * typed into the form.
   */
  initialWorkflowType: string | undefined;
  /** This session's last deploy of the open document, if any. */
  lastDeployment: DeploymentRecord | null;
  /** Workspace path of the open document — scopes the form's draft to it. */
  path: string;
  /** Open on first render — the launcher toggle owns it afterwards (test seam). */
  defaultOpen?: boolean;
  /** Injected availability client for tests. */
  availabilityClient?: AvailabilityClient | undefined;
};

/**
 * The open/closed transition for the run panel, as data: the launcher
 * toggles; the close button and Escape only ever close. Pure so the toggle
 * path is pinned by its own test — static markup cannot click.
 */
export function nextOpenState(open: boolean, intent: 'toggle' | 'close'): boolean {
  return intent === 'toggle' ? !open : false;
}

/**
 * The Run surface of the editor header: a launcher button in the action row
 * and, when open, a floating panel hosting the real start-workflow form —
 * the operator starts a run without leaving the editor (operator ruling).
 *
 * Running is a SEPARATE act from deploying: this panel never deploys, and it
 * is offered whether or not this session deployed — a run targets whatever
 * revision the server holds. Mirrors the guide overlay's shape deliberately:
 * NON-MODAL — no backdrop, no focus trap, no click-outside dismiss — a
 * floating `complementary` landmark beside the editor, closed only from its
 * own close button, Escape while inside it, or the launcher toggle.
 *
 * The body MOUNTS FRESH on every open (`open && <RunPanelBody>`): that is
 * what lets it capture the workflow-type seed once per open and re-ask
 * availability per open, instead of tracking the live buffer or a stale
 * cache.
 */
export function RunPanel({ defaultOpen = false, ...body }: RunPanelProps) {
  const [open, setOpen] = useState(defaultOpen);
  const panelId = useId();

  return (
    <>
      <Button
        aria-controls={panelId}
        aria-expanded={open}
        data-slot="run-panel-launcher"
        onClick={() => setOpen((current) => nextOpenState(current, 'toggle'))}
        size="sm"
        variant="outline"
      >
        <Play aria-hidden="true" className="size-3.5" />
        Run
      </Button>
      {open && (
        <RunPanelBody
          {...body}
          id={panelId}
          onClose={() => setOpen((current) => nextOpenState(current, 'close'))}
        />
      )}
    </>
  );
}

/**
 * The open panel. A separate component so each open is a fresh MOUNT: the
 * seed capture below runs once per open by construction, never on the
 * re-renders the live buffer causes while the panel sits open.
 */
function RunPanelBody({
  namespace,
  deployGranted,
  initialWorkflowType,
  lastDeployment,
  path,
  availabilityClient,
  id,
  onClose,
}: Omit<RunPanelProps, 'defaultOpen'> & { id: string; onClose: () => void }) {
  // Captured once at mount. StartWorkflowForm syncs any CHANGE of this prop
  // into the draft (the Actions deep-link contract), so passing the header's
  // live value would overwrite the operator's typed workflow type whenever
  // the buffer's declaration changed. The constant makes that sync fire at
  // most once, at open.
  const [seed] = useState(initialWorkflowType);

  return (
    <aside
      aria-label="Run workflow"
      className="fixed top-20 right-6 z-40 flex max-h-[80vh] w-[28rem] max-w-[calc(100vw-3rem)] flex-col overflow-hidden rounded-xl border border-border bg-surface-elevated shadow-xl"
      data-slot="run-panel"
      id={id}
      onKeyDown={(event) => {
        // Escape closes from anywhere inside — non-modal, so this is the
        // panel's own listener, not a document-level trap.
        if (event.key === 'Escape') onClose();
      }}
    >
      <header className="flex items-center justify-between gap-2 border-border border-b px-4 py-2.5">
        <span className="font-semibold text-foreground text-xs uppercase tracking-wide">
          Run workflow
        </span>
        <button
          aria-label="Close the run panel"
          className="rounded-md p-1 text-muted-foreground outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-accent-primary"
          onClick={onClose}
          type="button"
        >
          <X aria-hidden="true" className="size-3.5" />
        </button>
      </header>
      <div className="min-h-0 flex-1 space-y-3 overflow-y-auto px-4 py-3">
        {lastDeployment !== null && (
          <QueueAvailability
            client={availabilityClient ?? authoringFacade}
            namespace={namespace}
            taskQueue={lastDeployment.taskQueue}
          />
        )}
        <StartWorkflowForm
          deployGranted={deployGranted}
          draftScope={`authoring:${path}`}
          initialWorkflowType={seed}
          namespace={namespace}
        />
      </div>
    </aside>
  );
}

/**
 * Asks the server what serves the last-deployed queue and relays the verdict
 * to {@link AvailabilityLine}. One ask PER OPEN: the body remounts each time
 * the panel opens and `refetchOnMount: 'always'` re-asks then — workers come
 * and go between opens — while the infinite staleness keeps it from turning
 * into a poll while the panel sits open. The line INFORMS the operator; it
 * never blocks the start.
 */
function QueueAvailability({
  client,
  namespace,
  taskQueue,
}: {
  client: AvailabilityClient;
  namespace: Namespace | null;
  taskQueue: string;
}) {
  const query = useQuery({
    queryKey: ['authoring', 'run-availability', namespace, taskQueue],
    queryFn: () => {
      if (namespace === null) {
        throw new Error('No namespace selected');
      }
      return client.workerAvailability(namespace, taskQueue);
    },
    enabled: namespace !== null,
    staleTime: Number.POSITIVE_INFINITY,
    refetchOnMount: 'always',
    refetchOnWindowFocus: false,
    refetchOnReconnect: false,
    retry: false,
  });

  return (
    <AvailabilityLine
      availability={query.data ?? null}
      error={query.error}
      namespace={namespace}
      pending={query.isPending}
      taskQueue={taskQueue}
    />
  );
}

/**
 * The availability verdict as one inline status line: exactly one of the
 * no-namespace note, the served/unserved verdict, the failed check's error
 * text, or the in-flight note. Exported for its own tests — every arm is
 * this component's contract, independent of react-query's cache mechanics.
 * A failed check renders its error verbatim; nothing here is ever silent,
 * and nothing here blocks the start.
 */
export function AvailabilityLine({
  availability,
  error,
  namespace,
  pending,
  taskQueue,
}: {
  availability: WorkerAvailability | null;
  error: unknown;
  namespace: Namespace | null;
  pending: boolean;
  taskQueue: string;
}) {
  if (namespace === null) {
    return (
      <p className="text-muted-foreground text-xs" data-slot="run-panel-availability">
        Select a namespace to check what serves task queue {taskQueue}.
      </p>
    );
  }

  if (availability !== null) {
    if (availability.available) {
      return (
        <p className="text-success text-xs" data-slot="run-panel-availability">
          Task queue {availability.taskQueue} is served — {availability.connectedWorkers} connected
          worker{availability.connectedWorkers === 1 ? '' : 's'}.
        </p>
      );
    }
    return (
      <p className="text-warning text-xs" data-slot="run-panel-availability">
        {availability.scaffoldHint ?? `Nothing serves task queue ${availability.taskQueue} yet.`}
      </p>
    );
  }

  if (error !== null && error !== undefined) {
    return (
      <p className="text-destructive text-xs" data-slot="run-panel-availability">
        Availability check failed: {describeAvailabilityError(error)}
      </p>
    );
  }

  if (pending) {
    return (
      <p className="text-muted-foreground text-xs" data-slot="run-panel-availability">
        Checking what serves task queue {taskQueue}…
      </p>
    );
  }

  return null;
}

/**
 * Words for a thrown value of any shape. `String(...)` on a plain object is
 * "[object Object]" — a line that says nothing — so non-Error values render
 * as their JSON instead, and a value JSON cannot express (cycles, BigInt,
 * bare undefined from a symbol) is NAMED as unrenderable rather than
 * silently blanked.
 */
function describeAvailabilityError(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === 'string') return error;
  try {
    return JSON.stringify(error) ?? 'unrenderable error value';
  } catch {
    return 'unrenderable error value';
  }
}
