import { generatePath, Link } from 'react-router';
import { InstantText } from '@/components/InstantText';
import { StatusBadge } from '@/components/StatusBadge';
import { TableCell, TableRow } from '@/components/ui';
import { normalizeDisplayName, shortWorkflowId } from '@/lib/workflowIdentity';
import type { WorkflowSummary } from '@/types';

export type WorkflowRowProps = {
  workflow: WorkflowSummary;
};

export function WorkflowRow({ workflow }: WorkflowRowProps) {
  return (
    <TableRow>
      <TableCell className="font-mono text-xs">
        <WorkflowIdentityCell workflow={workflow} />
      </TableCell>
      <TableCell>{workflow.workflow_type}</TableCell>
      <TableCell>
        <div className="flex flex-col gap-1">
          <StatusBadge status={workflow.status} />
          <FailureContext failedStep={workflow.failed_step} reason={workflow.failure_reason} />
        </div>
      </TableCell>
      <TableCell>
        <InstantText value={workflow.started_at} />
      </TableCell>
      <TableCell>
        <InstantText value={workflow.ended_at} />
      </TableCell>
    </TableRow>
  );
}

/**
 * The row's identity cell (#211): the display name when there is one, with the
 * short id ALWAYS beside it (R2) — a name is a label, never an address, so the
 * row can never leave an operator without the id to address the run by. An
 * unnamed run shows its bare id exactly as before.
 *
 * The name reads back per WORKFLOW, not per run: the server folds the recorded
 * attribute over the whole history, so every run of a workflow — including a
 * continue-as-new successor, which inherits it — shows the same current name. Both forms link to the same
 * id-addressed detail route; nothing routes by name.
 */
function WorkflowIdentityCell({ workflow }: { workflow: WorkflowSummary }) {
  // The SHARED normalizer, not a local re-reading of the same rule: the list and
  // the detail view must decide "named or not" identically, or one surface shows
  // a name the other does not. (It is what treats a blank as unnamed rather than
  // as empty space — the server refuses to record one, so a blank arriving here
  // is foreign data, not a label.)
  const displayName = normalizeDisplayName(workflow.display_name);
  const to = generatePath('/workflows/:id', { id: workflow.workflow_id });

  if (displayName === null) {
    return (
      <Link className="text-primary underline-offset-4 hover:underline" to={to}>
        {workflow.workflow_id}
      </Link>
    );
  }

  return (
    <div className="flex flex-col gap-0.5">
      <span className="font-sans text-foreground text-sm">{displayName}</span>
      <Link className="text-primary underline-offset-4 hover:underline" to={to}>
        {shortWorkflowId(workflow.workflow_id)}
      </Link>
    </div>
  );
}

/**
 * The failure attribution for a failed row: the step that failed and the terminal
 * reason. Rendered ONLY when the server populated them (a failed row) — a
 * healthy/running/completed row carries `null`/`undefined` for both and shows
 * nothing, so no empty failure text appears under a non-failed status.
 */
function FailureContext({
  failedStep,
  reason,
}: {
  failedStep: string | null | undefined;
  reason: string | null | undefined;
}) {
  const hasStep = failedStep !== null && failedStep !== undefined && failedStep.length > 0;
  const hasReason = reason !== null && reason !== undefined && reason.length > 0;
  if (!hasStep && !hasReason) {
    return null;
  }

  return (
    <div className="flex flex-col gap-0.5 text-muted-foreground text-xs">
      {hasStep ? (
        <span>
          Failed at <span className="font-mono text-secondary-foreground">{failedStep}</span>
        </span>
      ) : null}
      {hasReason ? <span className="line-clamp-2 max-w-xs">{reason}</span> : null}
    </div>
  );
}
