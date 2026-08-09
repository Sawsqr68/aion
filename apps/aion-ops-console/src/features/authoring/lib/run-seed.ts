/**
 * The workflow-type seed for the editor's run panel.
 *
 * A pure reading so the fallback ORDER is pinned by its own test: the last
 * deploy's type when one landed this session (the deployed name is the
 * authoritative one — it is what the server will route), else the document's
 * own `workflow <name>` declaration, else undefined and the operator types
 * it. The panel captures the seed once per open; this function only decides
 * what that capture sees.
 */

import type { DeploymentRecord } from './guided-facade';
import { workflowTypeFromSource } from './worker-deploy';

export function runSeedWorkflowType(
  lastDeployment: Pick<DeploymentRecord, 'workflowType'> | null,
  source: string
): string | undefined {
  return lastDeployment?.workflowType ?? workflowTypeFromSource(source) ?? undefined;
}
