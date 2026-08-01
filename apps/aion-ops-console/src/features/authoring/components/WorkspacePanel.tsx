import { FileCode2 } from 'lucide-react';
import { AnimatePresence, motion, useReducedMotion } from 'motion/react';
import { useCallback, useEffect } from 'react';
import { useSearchParams } from 'react-router';

import { EmptyState } from '@/components/EmptyState';
import { ErrorState } from '@/components/ErrorState';
import { degradeToFade, SPRING_SIGNATURE } from '@/components/kit';
import { LoadingSkeleton } from '@/components/LoadingSkeleton';

import { useAuthoringDocument, useAuthoringDocuments } from '../hooks/useAuthoringDocuments';
import { AuthoringWorkspaceNotConfiguredError, authoringFacade } from '../lib/facade';
import { DocumentList } from './DocumentList';
import { EditorPane } from './EditorPane';

/**
 * The operator's editable AWL workspace: the documents on disk beside the
 * server, opened in the full authoring editor.
 *
 * This is the WRITE side of the studio. Its deployed counterpart is
 * `DeployedPanel`, which reads archived source out of deployed packages and
 * never touches this directory.
 */
export function WorkspacePanel() {
  const documentsQuery = useAuthoringDocuments();
  const documents = documentsQuery.data ?? [];
  // The URL is the source of truth for the open document (`?doc=<path>`), so
  // any view is one shareable link (operator ruling, 2026-07-12). Selecting
  // pushes history; the default/fallback selection replaces it so a plain
  // /authoring visit never pollutes the back stack.
  const [searchParams, setSearchParams] = useSearchParams();
  const selectedPath = searchParams.get('doc');
  const setSelectedPath = useCallback(
    (path: string | null, options?: { replace: boolean }) => {
      setSearchParams(
        (previous) => {
          const next = new URLSearchParams(previous);
          if (path === null) next.delete('doc');
          else next.set('doc', path);
          return next;
        },
        { replace: options?.replace ?? false }
      );
    },
    [setSearchParams]
  );
  const reducedMotion = useReducedMotion() ?? false;
  const transition = degradeToFade(SPRING_SIGNATURE, reducedMotion);
  const createDocument = useCallback(
    async (name: string) => {
      const created = await authoringFacade.createDocument(name);
      await documentsQuery.refetch();
      setSelectedPath(created.path);
    },
    [documentsQuery, setSelectedPath]
  );

  const documentsLoaded = documentsQuery.isSuccess;
  useEffect(() => {
    // Only reconcile once the list has actually loaded — a deep link must not
    // be discarded because the fetch hasn't returned yet.
    if (!documentsLoaded) return;
    if (selectedPath === null && documents[0] !== undefined) {
      setSelectedPath(documents[0].path, { replace: true });
    }
    if (selectedPath !== null && documents.every((document) => document.path !== selectedPath)) {
      setSelectedPath(documents[0]?.path ?? null, { replace: true });
    }
  }, [documents, documentsLoaded, selectedPath, setSelectedPath]);

  const documentQuery = useAuthoringDocument(selectedPath);
  const unconfigured =
    documentsQuery.error instanceof AuthoringWorkspaceNotConfiguredError ||
    documentQuery.error instanceof AuthoringWorkspaceNotConfiguredError;

  if (unconfigured) {
    return (
      <DocumentList documents={[]} onSelect={() => undefined} selectedPath={null} unconfigured />
    );
  }
  if (documentsQuery.isPending) return <LoadingSkeleton />;
  if (documentsQuery.isError) {
    return (
      <ErrorState
        message={documentsQuery.error.message}
        onRetry={() => void documentsQuery.refetch()}
        title="Documents unavailable"
      />
    );
  }

  return (
    <div className="flex min-h-[38rem] flex-col md:flex-row">
      <aside
        aria-label="Documents"
        className="max-h-64 shrink-0 overflow-auto border-border border-b bg-surface-base md:max-h-none md:w-56 md:border-r md:border-b-0"
      >
        <div className="sticky top-0 z-10 flex items-center justify-between border-border border-b bg-surface-base/95 px-3 py-2 backdrop-blur">
          <span className="font-medium text-muted-foreground text-xs uppercase tracking-wider">
            Documents
          </span>
          <span className="font-mono text-muted-foreground text-[0.6875rem]">
            {documents.length}
          </span>
        </div>
        <DocumentList
          documents={documents}
          onCreate={createDocument}
          onSelect={setSelectedPath}
          selectedPath={selectedPath}
        />
      </aside>
      <div className="relative flex min-w-0 flex-1">
        <AnimatePresence initial={false} mode="wait">
          <motion.div
            animate={{ opacity: 1, y: 0 }}
            className="flex min-w-0 flex-1"
            exit={reducedMotion ? { opacity: 0 } : { opacity: 0, y: 3 }}
            initial={reducedMotion ? { opacity: 0 } : { opacity: 0, y: -3 }}
            key={selectedPath ?? 'empty'}
            transition={transition}
          >
            <WorkspaceBody
              documentQuery={documentQuery}
              documents={documents}
              onOpenDocument={setSelectedPath}
              selectedPath={selectedPath}
            />
          </motion.div>
        </AnimatePresence>
      </div>
    </div>
  );
}

function WorkspaceBody({
  documentQuery,
  documents,
  onOpenDocument,
  selectedPath,
}: {
  documentQuery: ReturnType<typeof useAuthoringDocument>;
  documents: ReturnType<typeof useAuthoringDocuments>['data'] & object;
  onOpenDocument: (path: string) => void;
  selectedPath: string | null;
}) {
  if (selectedPath === null) {
    return (
      <div className="m-auto w-full max-w-md p-6">
        <EmptyState
          description="Choose a document from the list to inspect and edit its source."
          icon={<FileCode2 aria-hidden="true" className="size-5" />}
          title="Select an AWL document"
        />
      </div>
    );
  }
  if (documentQuery.isPending) return <LoadingSkeleton />;
  if (documentQuery.isError) {
    return (
      <ErrorState
        message={documentQuery.error.message}
        onRetry={() => void documentQuery.refetch()}
        title="Document unavailable"
      />
    );
  }
  return (
    <EditorPane
      documents={documents}
      initialSource={documentQuery.data}
      onOpenDocument={onOpenDocument}
      path={selectedPath}
    />
  );
}
