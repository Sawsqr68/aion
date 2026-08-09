import type { ManagedWorkersClient } from '@/features/workers';
import { ManagedWorkersPanel } from '@/features/workers';
import type { ApiClient } from '@/lib/api';
import type { Namespace } from '@/types';
import type { DeployClient } from '../hooks/useDeployPackage';
import type { StartSchemaClient } from '../hooks/useStartInputSchema';
import { DeployPackagePanel } from './DeployPackagePanel';
import { StartWorkflowForm } from './StartWorkflowForm';

export type ActionsViewProps = {
  namespace: Namespace | null;
  /** Injected start client (tests). */
  startClient?: Pick<ApiClient, 'startWorkflow'> | undefined;
  /** Injected deploy client (tests). */
  deployClient?: DeployClient | undefined;
  /** Injected deployed-read client for the schema-driven start form (tests). */
  startSchemaClient?: StartSchemaClient | undefined;
  /** Injected managed-workers client (tests). */
  workersClient?: Pick<ManagedWorkersClient, 'report' | 'act'> | undefined;
  /**
   * Whether the resolved caller may deploy, discovered at runtime from
   * `GET /whoami` (NOT a build-time flag). When `false` the deploy affordance is
   * not rendered at all — the console only ever shows what the server grants
   * this caller. Defaults to `false`: an ungated render is unprivileged.
   */
  deployGranted?: boolean;
  /** Preload the start form's workflow type (palette deep-link `?workflow_type=`). */
  initialWorkflowType?: string | undefined;
};

/**
 * Operator actions: start a workflow run (namespace-scoped command authority),
 * deploy a package, and drive the managed-worker fleet (both deployment-scoped
 * under the deploy grant). The authorities are distinct (ADR-022), and each
 * surface renders its own honest loading / error / confirmed-success state.
 *
 * The deploy and managed-worker affordances are gated on the runtime-discovered
 * `deployGranted` capability, so they appear only for a caller the server
 * actually authorizes (the single-tenant operator, or a deploy-claim bearer
 * under real auth).
 */
export function ActionsView({
  namespace,
  startClient,
  deployClient,
  startSchemaClient,
  workersClient,
  deployGranted = false,
  initialWorkflowType,
}: ActionsViewProps) {
  return (
    <div className="space-y-8 py-4">
      <header>
        <h1 className="font-medium text-foreground text-xl">Actions</h1>
        <p className="text-muted-foreground text-sm">
          Start a workflow run, deploy a package, or drive the managed workers. Success is shown
          only after the server confirms.
        </p>
      </header>

      <section className="space-y-3" aria-label="Start workflow">
        <h2 className="font-medium text-secondary-foreground text-sm uppercase tracking-[0.15em]">
          Start workflow
        </h2>
        <StartWorkflowForm
          apiClient={startClient}
          deployGranted={deployGranted}
          initialWorkflowType={initialWorkflowType}
          namespace={namespace}
          schemaClient={startSchemaClient}
        />
      </section>

      {deployGranted ? (
        <>
          <section className="space-y-3" aria-label="Managed workers">
            <h2 className="font-medium text-secondary-foreground text-sm uppercase tracking-[0.15em]">
              Managed workers
            </h2>
            <ManagedWorkersPanel client={workersClient} />
          </section>

          <section className="space-y-3" aria-label="Deploy package">
            <h2 className="font-medium text-secondary-foreground text-sm uppercase tracking-[0.15em]">
              Deploy package
            </h2>
            <DeployPackagePanel apiClient={deployClient} />
          </section>
        </>
      ) : null}
    </div>
  );
}
