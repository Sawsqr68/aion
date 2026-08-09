import { cn } from '@/lib/utils';

import type { DeployOutcome } from '../lib/deploy-action';

/**
 * The deploy outcome as its own persistent line above the status bar.
 *
 * Deliberately NOT the transient status-bar message: that one truncates and
 * is wiped by the next completed check, which would erase a deploy refusal
 * the moment the operator edits toward fixing it. This line wraps in full —
 * a server refusal's words are the operator's only lead and must never be
 * cut — and it outlives every checker pass; only the next deploy starting
 * clears it (see `nextDeployOutcome`).
 */
export function DeployOutcomeLine({ outcome }: { outcome: DeployOutcome | null }) {
  if (outcome === null) return null;
  return (
    <div
      className={cn(
        'break-words border-t px-4 py-2 text-sm',
        outcome.kind === 'success'
          ? 'border-success/30 bg-success/10 text-success'
          : 'border-destructive/30 bg-destructive/10 text-destructive'
      )}
      data-slot="deploy-outcome"
      role="status"
    >
      {outcome.text}
    </div>
  );
}
