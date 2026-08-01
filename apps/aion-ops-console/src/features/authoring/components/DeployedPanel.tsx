import { PackageSearch } from 'lucide-react';
import { useCallback, useEffect } from 'react';
import { useSearchParams } from 'react-router';

import { EmptyState } from '@/components/EmptyState';
import { ErrorState } from '@/components/ErrorState';
import { LoadingSkeleton } from '@/components/LoadingSkeleton';

import { useDeployedDocument, useDeployedVersions } from '../hooks/useDeployedVersions';
import { DeployedSurfaceUnavailableError } from '../lib/deployed-facade';
import { DeployedDocumentPane } from './DeployedDocumentPane';
import { DeployedList, type DeployedVersionKey } from './DeployedList';

/**
 * The READ side of the studio: workflow versions deployed on this server,
 * shown from the source their own deployed packages carry.
 *
 * Nothing here writes. The selection lives in the URL (`?type=…&version=…`)
 * exactly as the workspace document does, so a deployed view is one shareable
 * link naming an immutable artifact.
 */
export function DeployedPanel() {
  const versionsQuery = useDeployedVersions();
  const versions = versionsQuery.data ?? [];
  const [searchParams, setSearchParams] = useSearchParams();
  const workflowType = searchParams.get('type');
  const contentHash = searchParams.get('version');
  const select = useCallback(
    (key: DeployedVersionKey | null, options?: { replace: boolean }) => {
      setSearchParams(
        (previous) => {
          const next = new URLSearchParams(previous);
          if (key === null) {
            next.delete('type');
            next.delete('version');
          } else {
            next.set('type', key.workflowType);
            next.set('version', key.contentHash);
          }
          return next;
        },
        { replace: options?.replace ?? false }
      );
    },
    [setSearchParams]
  );

  const loaded = versionsQuery.isSuccess;
  useEffect(() => {
    if (!loaded) return;
    if (workflowType === null || contentHash === null) return;
    // A deep link naming a version this server does not hold is cleared rather
    // than left to 404 in the pane: the listing beside it is the truth.
    const known = versions.some(
      (version) => version.workflowType === workflowType && version.contentHash === contentHash
    );
    if (!known) select(null, { replace: true });
  }, [contentHash, loaded, select, versions, workflowType]);

  const unavailable = versionsQuery.error instanceof DeployedSurfaceUnavailableError;
  const documentQuery = useDeployedDocument(
    unavailable ? null : workflowType,
    unavailable ? null : contentHash
  );

  if (versionsQuery.isPending) return <LoadingSkeleton />;
  if (versionsQuery.isError && !unavailable) {
    return (
      <ErrorState
        message={versionsQuery.error.message}
        onRetry={() => void versionsQuery.refetch()}
        title="Deployed versions unavailable"
      />
    );
  }

  return (
    <div className="flex min-h-[38rem] flex-col md:flex-row">
      <aside
        aria-label="Deployed versions"
        className="max-h-64 shrink-0 overflow-auto border-border border-b bg-surface-base md:max-h-none md:w-64 md:border-r md:border-b-0"
      >
        <div className="sticky top-0 z-10 flex items-center justify-between border-border border-b bg-surface-base/95 px-3 py-2 backdrop-blur">
          <span className="font-medium text-muted-foreground text-xs uppercase tracking-wider">
            Deployed
          </span>
          <span className="font-mono text-muted-foreground text-[0.6875rem]">
            {versions.length}
          </span>
        </div>
        <DeployedList
          onSelect={select}
          selected={
            workflowType === null || contentHash === null ? null : { workflowType, contentHash }
          }
          unavailable={unavailable}
          versions={versions}
        />
      </aside>
      <div className="relative flex min-w-0 flex-1">
        <DeployedBody documentQuery={documentQuery} selected={workflowType !== null} />
      </div>
    </div>
  );
}

function DeployedBody({
  documentQuery,
  selected,
}: {
  documentQuery: ReturnType<typeof useDeployedDocument>;
  selected: boolean;
}) {
  if (!selected) {
    return (
      <div className="m-auto w-full max-w-md p-6">
        <EmptyState
          description="Choose a deployed version to read the AWL document its package carries."
          icon={<PackageSearch aria-hidden="true" className="size-5" />}
          title="Select a deployed version"
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
        title="Deployed source unavailable"
      />
    );
  }
  return <DeployedDocumentPane document={documentQuery.data} />;
}
