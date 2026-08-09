/**
 * Whether the editor header offers its Deploy button, and in what state.
 *
 * One pure decision, so the header only renders what this module rules and
 * every arm is testable without a DOM. Deploy is a SEPARATE action from Run
 * (operator ruling): it ships the saved document and never starts anything,
 * so a dirty buffer is deliberately NOT a reason to withhold it — the deploy
 * action saves the visible buffer first and ships exactly what the operator
 * is looking at.
 */

import type { CheckResult } from './facade-check';

/** The slice of a check verdict the decision reads. */
export type DeployCheckSlice = Pick<CheckResult, 'deploysGreen' | 'diagnostics'>;

export type DeployEligibilityInput = {
  /** The open document declares `worker <queue>` (deploys via DeployWorkerPanel). */
  workerDocument: boolean;
  /** The resolved caller may deploy (runtime capability, from `/whoami`). */
  deployGranted: boolean;
  /** The latest debounced check of the buffer, or null while one is in flight. */
  check: DeployCheckSlice | null;
};

/**
 * `hidden` — the button does not render at all: a worker document deploys as
 * a managed worker through its own panel, and a caller without the deploy
 * grant would only be refused by the server. `disabled` — the button renders
 * greyed with `reason` as its title. `ready` — deploy away.
 */
export type DeployEligibility =
  | { kind: 'hidden' }
  | { kind: 'disabled'; reason: string }
  | { kind: 'ready' };

export function deployEligibility({
  workerDocument,
  deployGranted,
  check,
}: DeployEligibilityInput): DeployEligibility {
  if (workerDocument || !deployGranted) {
    return { kind: 'hidden' };
  }
  if (check === null) {
    return { kind: 'disabled', reason: 'The document is still being checked' };
  }
  if (!check.deploysGreen) {
    return {
      kind: 'disabled',
      reason: check.diagnostics[0]?.message ?? 'The document does not deploy green',
    };
  }
  return { kind: 'ready' };
}
