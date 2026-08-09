import { Columns2, FileText, Lock, Workflow } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';

import { StatusDot } from '@/components/kit';
import { Badge } from '@/components/ui';
import { cn } from '@/lib/utils';

import { createCodeMirrorEditor } from '../editor-seam/codemirror';
import type { AuthoringEditor, HighlightSpan } from '../editor-seam/types';
import type { DeployedDocument } from '../lib/deployed-facade';
import { mapDiagnostics, statusForCheck } from '../lib/diagnostics';
import { byteOffsetToUtf16, stepAtPosition } from '../lib/projection';
import { shortHash } from './DeployedList';
import { ProjectionPane } from './ProjectionPane';

type DeployedViewMode = 'text' | 'canvas' | 'split';

type HighlightReply = { id: number; spans?: HighlightSpan[]; error?: string };

/**
 * A DEPLOYED AWL document, rendered in the same editor and canvas surfaces the
 * workspace uses — and read-only in every one of them.
 *
 * The read-only claim is structural, not decorative:
 *
 * - the editor is created read-only, so the CodeMirror STATE refuses document
 *   changes from keystrokes, paste, and dispatched transactions alike;
 * - the canvas runs in `deployed` mode, which renders no gesture control and
 *   no prose-edit affordance and persists no layout;
 * - this component holds only the deployed facade, whose every method is a GET.
 *   There is no save, no format, and no gesture to call.
 *
 * The projection comes from the server with the document, resolved against the
 * schemas the ARCHIVE carries. It is deliberately not re-checked through
 * `/awl/check`, which would resolve `schema("…")` imports against the
 * operator's workspace and render a composite that exists nowhere.
 */
export function DeployedDocumentPane({ document }: { document: DeployedDocument }) {
  const hostRef = useRef<HTMLDivElement>(null);
  const editorRef = useRef<AuthoringEditor | null>(null);
  const highlightWorkerRef = useRef<Worker | null>(null);
  const requestIdRef = useRef(0);
  const [viewMode, setViewMode] = useState<DeployedViewMode>('text');
  const [selectedStep, setSelectedStep] = useState<string | null>(null);

  const source = document.source;
  const semantic = document.projection.semantic;

  useEffect(() => {
    const host = hostRef.current;
    if (host === null) return;
    const editor = createCodeMirrorEditor(host, source, { readOnly: true });
    editorRef.current = editor;
    editor.setDecorations(mapDiagnostics(source, document.projection.diagnostics));
    const unsubscribeCursor = editor.onCursorChange(({ position }) => {
      setSelectedStep(
        semantic === null ? null : (stepAtPosition(semantic.graph, source, position)?.name ?? null)
      );
    });
    return () => {
      unsubscribeCursor();
      editor.destroy();
      editorRef.current = null;
    };
  }, [source, semantic, document.projection.diagnostics]);

  useEffect(() => {
    const worker = new Worker(new URL('../lib/highlight.worker.ts', import.meta.url), {
      type: 'module',
    });
    highlightWorkerRef.current = worker;
    worker.onmessage = (event: MessageEvent<HighlightReply>) => {
      if (event.data.id === requestIdRef.current && event.data.spans !== undefined) {
        editorRef.current?.setHighlightSpans(event.data.spans);
      }
    };
    const id = ++requestIdRef.current;
    worker.postMessage({ id, source });
    return () => {
      highlightWorkerRef.current = null;
      worker.terminate();
    };
  }, [source]);

  const status = statusForCheck(document.projection);
  const statusStyle = statusPresentation(status.tone);

  return (
    <section
      aria-label={`Deployed ${document.workflowType}`}
      className="flex min-w-0 flex-1 flex-col"
    >
      <header className="flex flex-wrap items-center justify-between gap-3 border-border border-b bg-surface-base px-4 py-3">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <h2 className="truncate font-semibold text-foreground text-sm">
              {document.documentName}
            </h2>
            <Badge className="shrink-0 gap-1" variant="outline">
              <Lock aria-hidden="true" className="size-3" />
              Deployed · read-only
            </Badge>
          </div>
          <p className="mt-0.5 truncate font-mono text-muted-foreground text-[0.6875rem]">
            <span>{document.workflowType}</span>
            <span aria-hidden="true"> @ </span>
            <span title={document.contentHash}>{shortHash(document.contentHash)}</span>
          </p>
        </div>
        <fieldset
          aria-label="Deployed view"
          className="flex rounded-lg border border-border bg-surface-elevated p-0.5"
        >
          <ViewButton
            icon={<FileText className="size-3.5" />}
            label="Text"
            mode="text"
            onSelect={setViewMode}
            selected={viewMode}
          />
          <ViewButton
            icon={<Workflow className="size-3.5" />}
            label="Canvas"
            mode="canvas"
            onSelect={setViewMode}
            selected={viewMode}
          />
          <ViewButton
            icon={<Columns2 className="size-3.5" />}
            label="Split"
            mode="split"
            onSelect={setViewMode}
            selected={viewMode}
          />
        </fieldset>
      </header>
      <div className="flex min-h-[32rem] min-w-0 flex-1 overflow-hidden">
        <div
          className={cn(
            'min-h-[32rem] min-w-0 overflow-hidden',
            viewMode === 'canvas' ? 'hidden' : 'flex-1',
            viewMode === 'split' && 'border-border border-r'
          )}
          ref={hostRef}
        />
        <ProjectionPane
          canvasMode="deployed"
          check={document.projection}
          documents={[]}
          onGesture={refuseGesture}
          onJumpToSpan={jumpToSpan(editorRef, source, setViewMode)}
          onOpenDocument={openNothing}
          path={`${document.workflowType}@${document.contentHash}`}
          selectedStep={selectedStep}
          semantic={semantic}
          viewMode={viewMode}
        />
      </div>
      <div className="flex min-h-11 items-center justify-between gap-4 border-border border-t bg-surface-base px-4 py-2 text-xs">
        <span
          className={cn(
            'inline-flex items-center gap-2 rounded-full border px-2.5 py-1 font-medium',
            statusStyle.className
          )}
        >
          <StatusDot className="size-1.5" status={statusStyle.status} />
          {status.label}
        </span>
        <span className="truncate text-muted-foreground">
          {schemaSummary(document)}
          <span aria-hidden="true"> · </span>
          {contractSummary(document)}
        </span>
      </div>
    </section>
  );
}

/**
 * What the version's IDENTITY-COMMITTED contract says about starting and
 * signalling this entry. Absence is stated as absence: a `null` signal set
 * (an additional entry, a pre-contract archive, or an older server) is "no
 * committed signal set", which is a different fact from "signals: none".
 */
function contractSummary(document: DeployedDocument): string {
  const start =
    document.inputSchema === null ? 'no committed start schema' : 'start schema committed';
  if (document.signals === null) return `${start} · no committed signal set`;
  if (document.signals.length === 0) return `${start} · signals: none committed`;
  return `${start} · signals: ${document.signals.map((signal) => signal.name).join(', ')}`;
}

/**
 * The tripwire. No affordance in `deployed` mode calls this — the canvas
 * renders no gesture control, no connectable handle, and no prose-edit button —
 * so it is unreachable by construction. It refuses loudly rather than
 * returning, so that if an affordance ever leaks into this mode the deployed
 * artifact is not quietly edited: the attempt fails and says why.
 */
function refuseGesture(): Promise<never> {
  return Promise.reject(
    new Error('a deployed workflow is a record of what is running and cannot be edited')
  );
}

/** The deployed pane opens no other document: it lists none to open. */
function openNothing() {
  return undefined;
}

function jumpToSpan(
  editorRef: React.RefObject<AuthoringEditor | null>,
  source: string,
  setViewMode: (mode: DeployedViewMode) => void
) {
  return (byteOffset: number) => {
    setViewMode('text');
    const editor = editorRef.current;
    editor?.setCursor(byteOffsetToUtf16(source, byteOffset));
    window.requestAnimationFrame(() => editor?.focus());
  };
}

function schemaSummary(document: DeployedDocument): string {
  const steps = document.projection.steps;
  const stepPart = steps === null ? 'projection unavailable' : `${steps} steps`;
  if (document.schemas.length === 0) return `${stepPart} · no imported schemas`;
  return `${stepPart} · ${document.schemas.length} archived schema${
    document.schemas.length === 1 ? '' : 's'
  }: ${document.schemas.map((schema) => schema.path).join(', ')}`;
}

function ViewButton({
  icon,
  label,
  mode,
  selected,
  onSelect,
}: {
  icon: React.ReactNode;
  label: string;
  mode: DeployedViewMode;
  selected: DeployedViewMode;
  onSelect: (mode: DeployedViewMode) => void;
}) {
  return (
    <button
      aria-label={`${label} view`}
      aria-pressed={selected === mode}
      className={cn(
        'flex items-center gap-1.5 rounded-md px-2 py-1 text-xs outline-none focus-visible:ring-2 focus-visible:ring-accent-primary',
        selected === mode
          ? 'bg-accent text-foreground'
          : 'text-muted-foreground hover:text-foreground'
      )}
      onClick={() => onSelect(mode)}
      type="button"
    >
      {icon}
      <span className="hidden lg:inline">{label}</span>
    </button>
  );
}

function statusPresentation(tone: 'green' | 'amber' | 'red' | undefined) {
  switch (tone) {
    case 'green':
      return {
        status: 'healthy' as const,
        className: 'border-success/30 bg-success/10 text-success',
      };
    case 'amber':
      return {
        status: 'running' as const,
        className: 'border-warning/30 bg-warning/10 text-warning',
      };
    case 'red':
      return {
        status: 'failed' as const,
        className: 'border-destructive/30 bg-destructive/10 text-destructive',
      };
    default:
      return {
        status: 'idle' as const,
        className: 'border-border bg-surface-elevated text-muted-foreground',
      };
  }
}
