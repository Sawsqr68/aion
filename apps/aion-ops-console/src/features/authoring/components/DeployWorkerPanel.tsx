import { HardHat, LoaderCircle } from 'lucide-react';
import { useId, useState } from 'react';
import { Link } from 'react-router';

import { actionsHref } from '@/app/routePaths';
import { ErrorState } from '@/components/ErrorState';
import { Button } from '@/components/ui/button';

import {
  buildWorkerDeploymentSubmission,
  type PutWorkerDeploymentResult,
  type WorkerDeploymentsClient,
  workerDeploymentsClient,
  workerQueueFromSource,
} from '../lib/worker-deploy';

const FIELD_CLASS =
  'h-9 rounded-md border border-input bg-transparent px-3 font-mono text-sm outline-none focus:border-muted-foreground';

export type DeployWorkerPanelProps = {
  /** Workspace path of the open worker document. */
  path: string;
  /** The editor buffer (the queue is read from it). */
  buffer: string;
  /** Injected client (tests). */
  client?: Pick<WorkerDeploymentsClient, 'putWorkerDeployment'> | undefined;
};

/**
 * Deploy the open WORKER document as a durable managed-worker deployment:
 * `PUT /worker-deployments/{name}` with the builtin `aion worker awl` verb.
 *
 * Two facts this panel refuses to fake:
 *
 * - The DOCUMENT PATH the spawned worker reads is resolved by the SERVER
 *   process (a relative path resolves against the server's working directory,
 *   not this studio's workspace root, whose absolute location the console
 *   cannot see). The field is prefilled with the workspace-relative path and
 *   stays editable so the operator can pin an absolute path.
 * - The connection settings (`--concurrency`, the reconnect trio) have NO
 *   defaults anywhere in the stack, deliberately. A field left blank sends no
 *   flag, and the worker then requires the matching environment variable in
 *   the server's environment — or refuses at spawn, naming what is missing.
 */
export function DeployWorkerPanel({ path, buffer, client }: DeployWorkerPanelProps) {
  const ids = useFieldIds();
  const queue = workerQueueFromSource(buffer);
  const resolved = client ?? workerDeploymentsClient;

  const [name, setName] = useState(() => deploymentNameFromPath(path));
  const [documentPath, setDocumentPath] = useState(path);
  const [namespacesEntry, setNamespacesEntry] = useState('');
  const [node, setNode] = useState('');
  const [desired, setDesired] = useState<'running' | 'stopped'>('running');
  const [identity, setIdentity] = useState('');
  const [concurrency, setConcurrency] = useState('');
  const [reconnectInitialBackoff, setReconnectInitialBackoff] = useState('');
  const [reconnectMaxBackoff, setReconnectMaxBackoff] = useState('');
  const [reconnectMaxAttempts, setReconnectMaxAttempts] = useState('');
  const [endpoint, setEndpoint] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<Error | null>(null);
  const [result, setResult] = useState<PutWorkerDeploymentResult | null>(null);

  const trimmedName = name.trim();
  const canSubmit =
    queue !== null && trimmedName.length > 0 && documentPath.trim().length > 0 && !busy;

  async function deploy(event: React.FormEvent) {
    event.preventDefault();
    if (!canSubmit) return;
    // The submission — verb argv, namespaces, queue-from-document, node
    // nulling, trims — is composed by ONE pure function; this handler only
    // relays its decision, so the semantics are testable without a DOM.
    const submission = buildWorkerDeploymentSubmission(
      {
        name,
        documentPath,
        namespacesEntry,
        node,
        desired,
        identity,
        concurrency,
        reconnectInitialBackoff,
        reconnectMaxBackoff,
        reconnectMaxAttempts,
        endpoint,
      },
      queue
    );
    if (!submission.ok) {
      setError(new Error(submission.reason));
      return;
    }
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      setResult(await resolved.putWorkerDeployment(submission.name, submission.request));
    } catch (reason) {
      setError(reason instanceof Error ? reason : new Error('Worker deployment refused'));
    } finally {
      setBusy(false);
    }
  }

  if (queue === null) {
    return (
      <section
        aria-label="Deploy worker"
        className="flex min-h-[32rem] min-w-0 flex-1 bg-surface-elevated"
      >
        <div className="m-auto flex max-w-md flex-col items-center gap-3 p-8 text-center">
          <HardHat className="size-7 text-accent-primary" />
          <h3 className="font-semibold text-foreground">Not a worker document</h3>
          <p className="text-muted-foreground text-sm">
            Deploying as a managed worker needs a document whose opening declaration is{' '}
            <code className="font-mono">worker &lt;queue&gt;</code>. This document does not declare
            one.
          </p>
        </div>
      </section>
    );
  }

  return (
    <section
      aria-label="Deploy worker"
      className="min-h-[32rem] min-w-0 flex-1 overflow-auto bg-surface-elevated p-4"
    >
      <div className="mx-auto max-w-2xl space-y-4">
        <header>
          <h3 className="font-semibold text-foreground">Deploy as managed worker</h3>
          <p className="text-muted-foreground text-sm">
            Creates (or replaces) a durable worker deployment that runs{' '}
            <code className="font-mono">aion worker awl</code> against this document under the
            server's supervision, serving queue{' '}
            <code className="font-mono text-foreground">{queue}</code>. Save the document first —
            the spawned worker reads the file, not this buffer.
          </p>
        </header>

        <form
          className="grid gap-3 rounded-lg border border-border p-4"
          onSubmit={(event) => void deploy(event)}
        >
          <Field
            help="Primary key of the durable deployment. Re-deploying the same name replaces it."
            id={ids.name}
            label="Deployment name"
            onChange={setName}
            value={name}
          />
          <Field
            help="Path the SERVER resolves when spawning the worker. Relative paths resolve against the server process's working directory — not this studio's workspace — so give an absolute path unless the server runs from the workspace's parent."
            id={ids.documentPath}
            label="Document path (as the server sees it)"
            onChange={setDocumentPath}
            value={documentPath}
          />
          <Field
            help="Namespace routing set, comma or space separated. Leave blank for none."
            id={ids.namespaces}
            label="Namespaces"
            onChange={setNamespacesEntry}
            value={namespacesEntry}
          />
          <Field
            help="Optional node/locality routing axis."
            id={ids.node}
            label="Node (optional)"
            onChange={setNode}
            value={node}
          />
          <label className="flex flex-col gap-2 font-medium text-sm" htmlFor={ids.desired}>
            Desired state
            <select
              className={FIELD_CLASS}
              id={ids.desired}
              onChange={(event) =>
                setDesired(event.currentTarget.value === 'stopped' ? 'stopped' : 'running')
              }
              value={desired}
            >
              <option value="running">running — supervise and start now</option>
              <option value="stopped">stopped — record it, start it later</option>
            </select>
          </label>

          <fieldset className="grid gap-3 rounded-md border border-border p-3">
            <legend className="px-1 font-medium text-sm">Worker connection settings</legend>
            <p className="font-normal text-muted-foreground text-xs">
              These are operator policy with no defaults anywhere in the stack. A field left blank
              sends no flag; the spawned worker then requires the matching environment variable
              (AION_WORKER_CONCURRENCY, AION_RECONNECT_*) in the server's environment, and refuses
              at spawn naming any setting that has no source.
            </p>
            <Field
              help={`Registered worker identity. Blank derives ${queue}-awl-worker from the document.`}
              id={ids.identity}
              label="Identity (--identity, optional)"
              onChange={setIdentity}
              value={identity}
            />
            <Field
              help={null}
              id={ids.concurrency}
              label="Concurrency (--concurrency)"
              onChange={setConcurrency}
              value={concurrency}
            />
            <Field
              help={null}
              id={ids.reconnectInitialBackoff}
              label="Reconnect initial backoff, seconds (--reconnect-initial-backoff)"
              onChange={setReconnectInitialBackoff}
              value={reconnectInitialBackoff}
            />
            <Field
              help={null}
              id={ids.reconnectMaxBackoff}
              label="Reconnect max backoff, seconds (--reconnect-max-backoff)"
              onChange={setReconnectMaxBackoff}
              value={reconnectMaxBackoff}
            />
            <Field
              help={null}
              id={ids.reconnectMaxAttempts}
              label="Reconnect max attempts (--reconnect-max-attempts)"
              onChange={setReconnectMaxAttempts}
              value={reconnectMaxAttempts}
            />
            <Field
              help="gRPC endpoint the worker dials. Blank defers to the spawned CLI's own resolution (AION_ENDPOINT or its default)."
              id={ids.endpoint}
              label="Endpoint (--endpoint, optional)"
              onChange={setEndpoint}
              value={endpoint}
            />
          </fieldset>

          <div className="flex items-center gap-3">
            <Button disabled={!canSubmit} type="submit">
              {busy ? (
                <LoaderCircle className="size-4 animate-spin" />
              ) : (
                <HardHat className="size-4" />
              )}
              {busy ? 'Deploying…' : 'Deploy worker'}
            </Button>
          </div>
        </form>

        {error === null ? null : (
          <ErrorState error={error} message={error.message} title="Worker deployment refused" />
        )}
        {result === null ? null : (
          <div className="rounded-lg border border-primary/40 bg-primary-glow p-4">
            <h4 className="font-medium text-foreground text-sm">
              {result.outcome === 'created'
                ? 'Worker deployment created'
                : 'Worker deployment replaced'}
            </h4>
            <dl className="mt-2 space-y-1 text-sm">
              <Detail label="Name" value={result.name} />
              <Detail label="Task queue" value={result.taskQueue} />
              <Detail label="Desired" value={result.desired} />
            </dl>
            <Link
              className="mt-3 inline-block text-primary text-sm underline-offset-4 hover:underline"
              to={actionsHref()}
            >
              Manage it under Actions → Managed workers →
            </Link>
          </div>
        )}
      </div>
    </section>
  );
}

function Field({
  help,
  id,
  label,
  onChange,
  value,
}: {
  help: string | null;
  id: string;
  label: string;
  onChange: (value: string) => void;
  value: string;
}) {
  return (
    <label className="flex flex-col gap-2 font-medium text-sm" htmlFor={id}>
      {label}
      <input
        className={FIELD_CLASS}
        id={id}
        value={value}
        onChange={(event) => onChange(event.currentTarget.value)}
      />
      {help === null ? null : (
        <span className="font-normal text-muted-foreground text-xs">{help}</span>
      )}
    </label>
  );
}

function Detail({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex gap-2">
      <dt className="text-muted-foreground">{label}</dt>
      <dd className="font-mono text-foreground">{value}</dd>
    </div>
  );
}

/** `pipelines/build-worker.awl` → `build-worker`: a reading of the path, editable. */
function deploymentNameFromPath(path: string): string {
  const base = path.split('/').at(-1) ?? path;
  return base.endsWith('.awl') ? base.slice(0, -'.awl'.length) : base;
}

function useFieldIds() {
  const prefix = useId();
  return {
    name: `${prefix}-name`,
    identity: `${prefix}-identity`,
    documentPath: `${prefix}-document-path`,
    namespaces: `${prefix}-namespaces`,
    node: `${prefix}-node`,
    desired: `${prefix}-desired`,
    concurrency: `${prefix}-concurrency`,
    reconnectInitialBackoff: `${prefix}-reconnect-initial-backoff`,
    reconnectMaxBackoff: `${prefix}-reconnect-max-backoff`,
    reconnectMaxAttempts: `${prefix}-reconnect-max-attempts`,
    endpoint: `${prefix}-endpoint`,
  };
}
