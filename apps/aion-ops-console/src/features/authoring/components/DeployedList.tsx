import { PackageCheck } from 'lucide-react';

import { EmptyState } from '@/components/EmptyState';
import { cn } from '@/lib/utils';

import type { DeployedSourceState, DeployedVersion } from '../lib/deployed-facade';

export type DeployedVersionKey = { workflowType: string; contentHash: string };

export type DeployedListProps = {
  versions: readonly DeployedVersion[];
  selected: DeployedVersionKey | null;
  unavailable?: boolean;
  /**
   * How many deployed versions the panel withheld because their archives carry
   * no readable AWL source. Keeps the empty state honest: "nothing is
   * deployed" and "nothing deployed here can be read" are different facts.
   */
  sourcelessCount?: number;
  onSelect: (key: DeployedVersionKey) => void;
};

/**
 * The deployed workflow versions this server holds, each stating what it
 * carries by way of authored source.
 *
 * The list makes NO claim it has not measured. The panel withholds versions
 * without readable source (this is a read surface; an entry that cannot open
 * is not listed), but the rows stay honest for any input: a version whose
 * archive has no AWL document says so and is not selectable; a version whose
 * archive would not read back says that instead, with the server's own reason
 * — the two are different facts and are never rendered as one.
 */
export function DeployedList({
  versions,
  selected,
  unavailable = false,
  sourcelessCount = 0,
  onSelect,
}: DeployedListProps) {
  if (unavailable) {
    return (
      <div className="p-3">
        <EmptyState
          description="deployed workflow surface not available"
          icon={<PackageCheck aria-hidden="true" className="size-5" />}
          title="Deployed versions unavailable"
        />
      </div>
    );
  }
  if (versions.length === 0) {
    return (
      <p className="p-4 text-center text-muted-foreground text-xs">
        {sourcelessCount > 0
          ? `${sourcelessCount} deployed version${sourcelessCount === 1 ? ' carries' : 's carry'} no archived AWL source, so there is nothing to read here.`
          : 'Nothing is deployed on this server yet.'}
      </p>
    );
  }
  return (
    <ul aria-label="Deployed workflow versions" className="space-y-1 p-2">
      {versions.map((version) => (
        <li key={`${version.workflowType}@${version.contentHash}`}>
          <DeployedRow
            onSelect={onSelect}
            selected={
              selected !== null &&
              selected.workflowType === version.workflowType &&
              selected.contentHash === version.contentHash
            }
            version={version}
          />
        </li>
      ))}
    </ul>
  );
}

function DeployedRow({
  version,
  selected,
  onSelect,
}: {
  version: DeployedVersion;
  selected: boolean;
  onSelect: (key: DeployedVersionKey) => void;
}) {
  const readable = version.source.state === 'available';
  const label = sourceLabel(version.source);
  return (
    <button
      aria-current={selected ? 'true' : undefined}
      className={cn(
        'group w-full rounded-md border border-transparent px-3 py-2.5 text-left transition-colors',
        readable ? 'hover:border-border-subtle hover:bg-surface-hover' : 'cursor-not-allowed',
        !readable && 'opacity-70',
        selected && 'border-border bg-surface-elevated shadow-sm hover:bg-surface-elevated'
      )}
      // A version with no readable archived source has nothing to open. It is
      // still LISTED — it is genuinely deployed — but the row does not pretend
      // to lead somewhere.
      disabled={!readable}
      onClick={() =>
        onSelect({ workflowType: version.workflowType, contentHash: version.contentHash })
      }
      type="button"
    >
      <span className="flex items-center gap-2">
        <PackageCheck
          aria-hidden="true"
          className={cn(
            'size-3.5 shrink-0 text-muted-foreground transition-colors',
            selected && 'text-primary'
          )}
        />
        <span className="min-w-0 flex-1 truncate font-medium text-foreground text-sm">
          {version.workflowType}
        </span>
        {version.routeActive && (
          <span className="shrink-0 rounded-full border border-border px-1.5 py-0.5 font-medium text-[0.625rem] text-muted-foreground uppercase tracking-wide">
            routed
          </span>
        )}
      </span>
      <span className="mt-1.5 flex items-center pl-5.5">
        <span
          className="min-w-0 truncate font-mono text-muted-foreground text-[0.6875rem]"
          title={version.contentHash}
        >
          {shortHash(version.contentHash)}
        </span>
      </span>
      <span className="mt-1 block pl-5.5 text-[0.6875rem] text-muted-foreground">{label}</span>
    </button>
  );
}

/**
 * The plain-language statement of one source state.
 *
 * Every branch says something the server measured. There is deliberately no
 * fallthrough "unknown" wording: an unrecognised state never reaches here,
 * because the facade refuses to parse one.
 */
export function sourceLabel(source: DeployedSourceState): string {
  switch (source.state) {
    case 'available':
      return source.schemaCount === 0
        ? `AWL source: ${source.documentName}`
        : `AWL source: ${source.documentName} (+${source.schemaCount} schema${
            source.schemaCount === 1 ? '' : 's'
          })`;
    case 'absent':
      return 'No archived AWL source — authored in Gleam, or deployed before source archiving';
    case 'not_persisted':
      return 'No archive stored for this version — loaded from an operator-supplied file';
    default:
      return `Archive could not be read: ${source.reason}`;
  }
}

/** First and last eight hex characters, the shape an operator reads a hash by. */
export function shortHash(contentHash: string): string {
  return contentHash.length <= 20
    ? contentHash
    : `${contentHash.slice(0, 8)}…${contentHash.slice(-8)}`;
}
