import { useCallback } from 'react';
import { useSearchParams } from 'react-router';

import { cn } from '@/lib/utils';

import { DeployedPanel } from './DeployedPanel';
import { WorkspacePanel } from './WorkspacePanel';

/** Which side of the studio is on screen. */
export type AuthoringSource = 'workspace' | 'deployed';

/**
 * The AWL studio: the operator's editable workspace, and the read-only view of
 * what is actually deployed on this server.
 *
 * The two are deliberately separate surfaces rather than one list with a badge.
 * A workspace document is a draft the operator owns and can change; a deployed
 * version is a record of running code, named by content hash, that nothing in
 * this console can edit. Showing them as one list would invite the operator to
 * treat a deployment as a file — and a deploy has never written into their
 * workspace, so the two can legitimately disagree.
 */
export function AuthoringView() {
  const [searchParams, setSearchParams] = useSearchParams();
  const source: AuthoringSource =
    searchParams.get('source') === 'deployed' ? 'deployed' : 'workspace';
  const setSource = useCallback(
    (next: AuthoringSource) => {
      setSearchParams((previous) => {
        const params = new URLSearchParams(previous);
        if (next === 'workspace') params.delete('source');
        else params.set('source', next);
        return params;
      });
    },
    [setSearchParams]
  );

  return (
    <div className="overflow-hidden rounded-xl border border-border bg-surface-elevated shadow-sm">
      <header className="flex items-center justify-between gap-4 border-border border-b bg-surface-base px-4 py-3">
        <div>
          <h1 className="font-semibold text-foreground text-lg">Authoring</h1>
          <p className="text-muted-foreground text-sm">
            {source === 'workspace'
              ? 'AWL documents and deployment checks'
              : 'AWL source carried by the packages deployed on this server'}
          </p>
        </div>
        <fieldset
          aria-label="Authoring source"
          className="flex rounded-lg border border-border bg-surface-elevated p-0.5"
        >
          <SourceButton current={source} label="Workspace" onSelect={setSource} value="workspace" />
          <SourceButton current={source} label="Deployed" onSelect={setSource} value="deployed" />
        </fieldset>
      </header>
      {source === 'workspace' ? <WorkspacePanel /> : <DeployedPanel />}
    </div>
  );
}

function SourceButton({
  current,
  label,
  onSelect,
  value,
}: {
  current: AuthoringSource;
  label: string;
  onSelect: (value: AuthoringSource) => void;
  value: AuthoringSource;
}) {
  return (
    <button
      aria-pressed={current === value}
      className={cn(
        'rounded-md px-2.5 py-1 text-xs outline-none focus-visible:ring-2 focus-visible:ring-accent-primary',
        current === value
          ? 'bg-accent text-foreground'
          : 'text-muted-foreground hover:text-foreground'
      )}
      onClick={() => onSelect(value)}
      type="button"
    >
      {label}
    </button>
  );
}
