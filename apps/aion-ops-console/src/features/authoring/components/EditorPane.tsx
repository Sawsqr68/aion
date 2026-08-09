import { LoaderCircle } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useCallback, useEffect, useReducer, useRef, useState } from 'react';

import { StatusDot } from '@/components/kit';
import { useCapabilities } from '@/features/capabilities';
import { useNamespace } from '@/features/namespace';
import { ACTION_IDS } from '@/lib/keybindings/actions';
import { useAction } from '@/lib/keybindings/context';
import { cn } from '@/lib/utils';

import { createCodeMirrorEditor } from '../editor-seam/codemirror';
import type { AuthoringEditor, HighlightSpan } from '../editor-seam/types';
import { nextDeployOutcome, saveThenDeploy } from '../lib/deploy-action';
import { deployEligibility } from '../lib/deploy-eligibility';
import { mapDiagnostics, minimalTextDelta, statusForCheck } from '../lib/diagnostics';
import {
  type AwlDocument,
  authoringFacade,
  type CheckResult,
  type DeploymentRecord,
  type EditResult,
  type GestureOperation,
} from '../lib/facade';
import { applyGestureThroughSeam } from '../lib/gestures';
import { byteOffsetToUtf16, stepAtPosition } from '../lib/projection';
import type { SemanticIndex } from '../lib/projection-types';
import { runSeedWorkflowType } from '../lib/run-seed';
import { isWorkerDocument } from '../lib/worker-deploy';
import { DeployOutcomeLine } from './DeployOutcomeLine';
import { DeployWorkerPanel } from './DeployWorkerPanel';
import { EditorHeader, type EditorWorking } from './EditorHeader';
import { type AuthoringViewMode, ProjectionPane } from './ProjectionPane';
import { ShipRunPanel } from './ShipRunPanel';
import { StudioPane } from './StudioPane';

export type EditorPaneProps = {
  path: string;
  initialSource: string;
  documents: readonly AwlDocument[];
  onOpenDocument: (path: string) => void;
};

type ViewMode = AuthoringViewMode;

type HighlightReply = { id: number; spans?: HighlightSpan[]; error?: string };

export function EditorPane({ path, initialSource, documents, onOpenDocument }: EditorPaneProps) {
  const hostRef = useRef<HTMLDivElement>(null);
  const editorRef = useRef<AuthoringEditor | null>(null);
  const semanticRef = useRef<SemanticIndex | null>(null);
  const cursorPositionRef = useRef(0);
  const highlightWorkerRef = useRef<Worker | null>(null);
  const sourceRef = useRef(initialSource);
  const savedSourceRef = useRef(initialSource);
  const requestIdRef = useRef(0);
  const [source, setSource] = useState(initialSource);
  const [dirty, setDirty] = useState(false);
  const [check, setCheck] = useState<CheckResult | null>(null);
  const [semantic, setSemantic] = useState<SemanticIndex | null>(null);
  const [selectedStep, setSelectedStep] = useState<string | null>(null);
  const [viewMode, setViewMode] = useState<ViewMode>('text');
  const [message, setMessage] = useState<string | null>(null);
  const [working, setWorking] = useState<EditorWorking>(null);
  // This session's last landed deploy of this document: seeds the run panel's
  // workflow type and names the queue its availability line asks about.
  const [lastDeployment, setLastDeployment] = useState<DeploymentRecord | null>(null);
  // The deploy outcome has its OWN state, apart from the transient `message`:
  // every state change routes through the pure transition, including the
  // checker's completions — which are defined there to change nothing.
  const [deployOutcome, dispatchDeployOutcome] = useReducer(nextDeployOutcome, null);
  const { deployGranted } = useCapabilities();
  const { selectedNamespace } = useNamespace();

  useEffect(() => {
    const host = hostRef.current;
    if (host === null) return;
    const editor = createCodeMirrorEditor(host, initialSource);
    editorRef.current = editor;
    sourceRef.current = initialSource;
    savedSourceRef.current = initialSource;
    setSource(initialSource);
    setDirty(false);
    const unsubscribe = editor.onChange((change) => {
      sourceRef.current = change.content;
      setSource(change.content);
      setDirty(change.content !== savedSourceRef.current);
    });
    const unsubscribeCursor = editor.onCursorChange(({ position }) => {
      cursorPositionRef.current = position;
      const graph = semanticRef.current?.graph;
      setSelectedStep(
        graph === undefined
          ? null
          : (stepAtPosition(graph, sourceRef.current, position)?.name ?? null)
      );
    });
    return () => {
      unsubscribe();
      unsubscribeCursor();
      editor.destroy();
      editorRef.current = null;
    };
  }, [initialSource]);

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
    return () => {
      highlightWorkerRef.current = null;
      worker.terminate();
    };
  }, []);

  useEffect(() => {
    const id = ++requestIdRef.current;
    highlightWorkerRef.current?.postMessage({ id, source });
  }, [source]);

  useEffect(() => {
    let active = true;
    const timer = window.setTimeout(() => {
      void authoringFacade
        .check(source, path)
        .then((result) => {
          if (!active) return;
          setCheck(result);
          if (result.semantic !== null) {
            semanticRef.current = result.semantic;
            setSemantic(result.semantic);
            setSelectedStep(
              stepAtPosition(result.semantic.graph, source, cursorPositionRef.current)?.name ?? null
            );
          }
          editorRef.current?.setDecorations(mapDiagnostics(source, result.diagnostics));
          setMessage(null);
          // Explicitly through the outcome transition — a completed check
          // must NOT clear a deploy refusal the operator is editing to fix.
          dispatchDeployOutcome({ event: 'check-completed' });
        })
        .catch((error: unknown) => {
          if (active) setMessage(error instanceof Error ? error.message : 'Check failed');
        });
    }, 400);
    return () => {
      active = false;
      window.clearTimeout(timer);
    };
  }, [path, source]);

  async function save() {
    if (!dirty || working !== null) return;
    setWorking('save');
    setMessage(null);
    try {
      await authoringFacade.saveDocument(path, sourceRef.current);
      savedSourceRef.current = sourceRef.current;
      setDirty(false);
      setMessage('Saved');
    } catch (error) {
      setMessage(error instanceof Error ? error.message : 'Save failed');
    } finally {
      setWorking(null);
    }
  }

  // Deploy is save-then-deploy of the operator's VISIBLE buffer — the server
  // verifies the content hash against the saved document, so shipping without
  // saving first could only ever refuse or ship stale bytes. It never starts
  // a run (Deploy and Run are separate acts, by operator ruling).
  async function deploy() {
    if (working !== null) return;
    setWorking('deploy');
    dispatchDeployOutcome({ event: 'deploy-started' });
    try {
      const result = await saveThenDeploy(authoringFacade, path, sourceRef.current, (saved) => {
        savedSourceRef.current = saved;
        // Compare against the LIVE buffer: keystrokes typed while the save
        // was in flight keep the Modified badge they earned.
        setDirty(sourceRef.current !== saved);
      });
      setLastDeployment(result.deployment);
      dispatchDeployOutcome({ event: 'deploy-landed', deployment: result.deployment });
    } catch (error) {
      // The server's refusal reaches the outcome line verbatim.
      dispatchDeployOutcome({
        event: 'deploy-refused',
        text: error instanceof Error ? error.message : 'Deploy failed',
      });
    } finally {
      setWorking(null);
    }
  }

  useAction(ACTION_IDS.authoringSave, () => void save());
  useAction(ACTION_IDS.authoringViewText, () => setViewMode('text'));
  useAction(ACTION_IDS.authoringViewCanvas, () => setViewMode('canvas'));
  useAction(ACTION_IDS.authoringViewSplit, () => setViewMode('split'));
  useAction(ACTION_IDS.authoringViewStudio, () => setViewMode('studio'));

  const jumpToSpan = useCallback((byteOffset: number) => {
    setViewMode((current) => (current === 'canvas' ? 'text' : current));
    const editor = editorRef.current;
    editor?.setCursor(byteOffsetToUtf16(sourceRef.current, byteOffset));
    window.requestAnimationFrame(() => editor?.focus());
  }, []);

  async function format() {
    if (working !== null) return;
    setWorking('format');
    setMessage(null);
    try {
      const formatted = await authoringFacade.format(sourceRef.current);
      editorRef.current?.applyTextDelta(minimalTextDelta(sourceRef.current, formatted));
    } catch (error) {
      setMessage(error instanceof Error ? error.message : 'Format failed');
    } finally {
      setWorking(null);
    }
  }

  const applyGesture = useCallback(async (operation: GestureOperation): Promise<EditResult> => {
    setMessage(null);
    try {
      const editor = editorRef.current;
      if (editor === null) throw new Error('Editor is not available');
      // This is the architectural seam: every server-printed gesture reaches
      // CodeMirror in exactly one applyTextDelta call, hence one undo unit.
      const result = await applyGestureThroughSeam(editor, authoringFacade, operation);
      setMessage('Canvas gesture applied');
      return result;
    } catch (error) {
      setMessage(error instanceof Error ? error.message : 'Canvas gesture failed');
      throw error;
    }
  }, []);

  // A worker document (`worker <queue>`) has no flow to ship-and-run; its
  // "run" surface is deploying it as a supervised managed worker instead.
  const workerDocument = isWorkerDocument(source);

  return (
    <section className="flex min-w-0 flex-1 flex-col" aria-label={`Editing ${path}`}>
      <EditorHeader
        deploy={deployEligibility({ check, deployGranted, workerDocument })}
        dirty={dirty}
        onDeploy={() => void deploy()}
        onFormat={() => void format()}
        onSave={() => void save()}
        onSelectViewMode={setViewMode}
        path={path}
        run={{
          deployGranted,
          initialWorkflowType: runSeedWorkflowType(lastDeployment, source),
          lastDeployment,
          namespace: selectedNamespace,
          path,
        }}
        viewMode={viewMode}
        workerDocument={workerDocument}
        working={working}
      />
      <div className="flex min-h-[32rem] min-w-0 flex-1 overflow-hidden">
        <div
          className={cn(
            'min-h-[32rem] min-w-0 overflow-hidden',
            viewMode === 'canvas' || viewMode === 'studio' || viewMode === 'run'
              ? 'hidden'
              : 'flex-1',
            viewMode === 'split' && 'border-border border-r'
          )}
          ref={hostRef}
        />
        <ProjectionPane
          check={check}
          documents={documents}
          onJumpToSpan={jumpToSpan}
          onGesture={applyGesture}
          onOpenDocument={onOpenDocument}
          path={path}
          selectedStep={selectedStep}
          semantic={semantic}
          viewMode={viewMode}
        />
        {viewMode === 'studio' && semantic !== null && (
          <StudioPane
            documents={documents}
            onGesture={applyGesture}
            onJumpToSpan={jumpToSpan}
            onOpenDocument={onOpenDocument}
            path={path}
            selectedStep={selectedStep}
            semantic={semantic}
            source={source}
          />
        )}
        {viewMode === 'run' &&
          (workerDocument ? (
            <DeployWorkerPanel buffer={source} path={path} />
          ) : (
            <ShipRunPanel buffer={source} documents={documents} path={path} />
          ))}
      </div>
      <DeployOutcomeLine outcome={deployOutcome} />
      <EditorStatusBar check={check} message={message} />
    </section>
  );
}

/** Debounced check verdict on the left, latest transient message on the right. */
function EditorStatusBar({
  check,
  message,
}: {
  check: CheckResult | null;
  message: string | null;
}) {
  const status = check === null ? null : statusForCheck(check);
  const statusStyle = statusPresentation(status?.tone);
  return (
    <div className="flex min-h-11 items-center justify-between gap-4 border-border border-t bg-surface-base px-4 py-2 text-xs">
      <span
        className={cn(
          'inline-flex items-center gap-2 rounded-full border px-2.5 py-1 font-medium',
          statusStyle.className
        )}
      >
        <CheckStatusIcon checking={status === null} status={statusStyle.status} />
        {status?.label ?? 'Checking…'}
      </span>
      <AnimatePresence initial={false} mode="wait">
        <motion.span
          animate={{ opacity: 1, y: 0 }}
          className="truncate text-muted-foreground"
          exit={{ opacity: 0, y: 2 }}
          initial={{ opacity: 0, y: -2 }}
          key={message ?? check?.steps ?? 'checking'}
        >
          {message ?? (check === null ? 'Validating source' : `${check.steps} steps`)}
        </motion.span>
      </AnimatePresence>
    </div>
  );
}

function CheckStatusIcon({
  checking,
  status,
}: {
  checking: boolean;
  status: 'healthy' | 'running' | 'failed' | 'idle';
}) {
  if (checking) {
    return <LoaderCircle aria-hidden="true" className="size-3 animate-spin" />;
  }
  return <StatusDot className="size-1.5" status={status} />;
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
