/**
 * The header Deploy action, as data: save the operator's visible buffer, then
 * deploy the saved revision under the hash the save returned.
 *
 * This is NOT a deploy-then-run chain (that chain is forbidden — Deploy never
 * starts a run). The save exists because `POST /awl/deploy` verifies its
 * `content_hash` against the SAVED document and refuses on RevisionMismatch;
 * saving first means what ships is exactly the buffer the operator is looking
 * at. Composed here as one pure-ish sequence over an injected client so the
 * ordering, the hash threading, and the success message are testable without
 * a DOM, and the panel handler only relays.
 */

import type { DeploymentRecord, GuidedDeployResult } from './guided-facade';

/** The two facade calls the action makes, injectable for tests. */
export type DeployDocumentClient = {
  saveDocument(path: string, source: string): Promise<string>;
  deploy(path: string, contentHash: string): Promise<GuidedDeployResult>;
};

/**
 * Save `source` under `path`, then deploy that saved revision.
 *
 * `onSaved` fires as soon as the save lands — BEFORE the deploy — so the
 * editor can mark the buffer clean even when the deploy is later refused:
 * the save happened either way, and pretending otherwise would resurrect a
 * "Modified" badge for bytes already on disk.
 */
export async function saveThenDeploy(
  client: DeployDocumentClient,
  path: string,
  source: string,
  onSaved: (savedSource: string) => void
): Promise<GuidedDeployResult> {
  const contentHash = await client.saveDocument(path, source);
  onSaved(source);
  return client.deploy(path, contentHash);
}

/** The outcome line for a landed deploy: what shipped, where, and AS WHAT — the
 * package id is the content-hash identity the operator can find again later. */
export function deploySuccessMessage(deployment: DeploymentRecord): string {
  return `Deployed ${deployment.workflowType} to queue ${deployment.taskQueue} (package ${deployment.packageId})`;
}

/** The deploy outcome the editor keeps on screen: landed or refused, in words. */
export type DeployOutcome = { kind: 'success' | 'refused'; text: string };

export type DeployOutcomeEvent =
  | { event: 'deploy-started' }
  | { event: 'deploy-landed'; deployment: DeploymentRecord }
  | { event: 'deploy-refused'; text: string }
  | { event: 'check-completed' };

/**
 * The deploy-outcome line's whole lifecycle as one pure transition.
 *
 * The outcome is NOT the transient status-bar message: a refusal must stay on
 * screen while the operator edits toward a fix, and the debounced checker
 * completes on every one of those edits. The checker therefore reports
 * `check-completed` THROUGH this transition — where it is defined to change
 * nothing — instead of owning any right to clear the line. Only the next
 * deploy starting clears it.
 */
export function nextDeployOutcome(
  current: DeployOutcome | null,
  event: DeployOutcomeEvent
): DeployOutcome | null {
  switch (event.event) {
    case 'deploy-started':
      return null;
    case 'deploy-landed':
      return { kind: 'success', text: deploySuccessMessage(event.deployment) };
    case 'deploy-refused':
      return { kind: 'refused', text: event.text };
    case 'check-completed':
      return current;
  }
}
