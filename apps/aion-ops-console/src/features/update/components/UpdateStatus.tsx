import { useMemo } from 'react';

import { useNamespace } from '@/features/namespace';
import { createConfiguredApiClient } from '@/lib/config';
import type { WorkflowId, WorkflowStatus } from '@/types';

import { updateCheckStartParams } from '../lib/contract';
import { fetchCheckRunStatus } from '../lib/followCheck';
import { UpdateStatusPanel } from './UpdateStatusPanel';

/**
 * The shell-facing update entry point: {@link UpdateStatusPanel} bound to the
 * configured API client. The two reads (`/update-status`, `/build`) are
 * namespace-independent, so one client built once outside render is correct
 * for the app's lifetime — the What's-new pattern exactly.
 *
 * "Check now" is namespace-scoped like every workflow start: the run is
 * created in the operator's SELECTED namespace, where its transcript and
 * history are then readable through the ordinary workflow surfaces. The start
 * carries an explicit empty `input` object — the document declares no inputs,
 * and the start route requires a payload to bind (the live e2e caught the
 * omission; `UpdateStatusPanel.test.tsx` pins it now). With no namespace
 * selected the affordance is disabled and says why — it never guesses a
 * namespace to start in.
 *
 * A started check is FOLLOWED to a terminal status (reviewed, r1 m6): the
 * panel polls the run's projected status through the ordinary list query so
 * "check started" can become "completed" or a loud failure, never the last
 * word forever. The lookup is TIME-FILTERED (`lib/followCheck.ts`) because
 * the list wire returns EVERY matching summary — the r3 review measured the
 * route ignoring the client's page limit entirely (#194) — so a type-wide
 * lookup would haul the namespace's whole update-check history on every
 * poll; the floor keeps it to ~one row.
 */
const fetchUpdateStatus = () => createConfiguredApiClient().getUpdateStatus();
const fetchBuild = () => createConfiguredApiClient().getBuild();

export function UpdateStatus() {
  const { selectedNamespace } = useNamespace();

  const actions = useMemo(() => {
    if (selectedNamespace === null) {
      return null;
    }
    const client = createConfiguredApiClient({ namespace: selectedNamespace });
    const options = { namespace: selectedNamespace };
    return {
      startCheck: () => client.startWorkflow(updateCheckStartParams(), options),
      fetchCheckStatus: (
        workflowId: WorkflowId,
        startedAfter: string
      ): Promise<WorkflowStatus | null> =>
        fetchCheckRunStatus(client, selectedNamespace, workflowId, startedAfter),
    };
  }, [selectedNamespace]);

  return (
    <UpdateStatusPanel
      checkDisabledReason={actions === null ? 'Select a namespace to run the check in.' : null}
      fetchBuild={fetchBuild}
      fetchCheckStatus={
        actions?.fetchCheckStatus ??
        (() => Promise.reject(new Error('A namespace must be selected before checking.')))
      }
      fetchUpdateStatus={fetchUpdateStatus}
      startCheck={
        actions?.startCheck ??
        (() => Promise.reject(new Error('A namespace must be selected before checking.')))
      }
    />
  );
}
