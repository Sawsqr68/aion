import type { AwlDocument } from './facade';
import { GuidedFlowRefusedError } from './facade';

/**
 * The guide's "Try it" flow, as data: how an example document becomes a
 * workspace document the operator lands on.
 *
 * The pieces are pure and dependency-injected so the whole flow — name
 * derivation, atomic create with collision retry, list refresh, navigation —
 * is testable without a server or a router. `useTryExample` wires them to the
 * real facade, query cache, and URL.
 */

/**
 * The workspace document name for a guide example: the name the example's own
 * top-level `workflow <name>` / `worker <name>` declaration carries. The first
 * declaration wins — every guide example is a complete checked document, so
 * one is always there; `null` says the source carries none (and the caller
 * must refuse visibly, not invent a name).
 */
export function deriveExampleName(source: string): string | null {
  for (const line of source.split('\n')) {
    const match = /^(?:workflow|worker)[ \t]+([a-z_][a-z0-9_]*)/.exec(line);
    const name = match?.[1];
    if (name !== undefined) return name;
  }
  return null;
}

/**
 * Creates a workspace document carrying the example verbatim, retrying
 * `{name}_2`, `{name}_3`, … while the server answers `DocumentExists`.
 *
 * Every attempt is the server's own atomic create — there is no
 * list-then-pick-a-free-name window — so two operators trying the same
 * example at once each get their own document. Any refusal that is NOT
 * `DocumentExists` stops the loop and propagates verbatim: the server said
 * something else is wrong, and inventing more names would not fix it.
 */
export async function createExampleDocument(
  create: (name: string) => Promise<AwlDocument>,
  source: string
): Promise<AwlDocument> {
  const base = deriveExampleName(source);
  if (base === null) {
    throw new GuidedFlowRefusedError(
      'ExampleNameUnderivable',
      'This example declares no workflow or worker to name a document after.'
    );
  }
  for (let attempt = 1; ; attempt += 1) {
    const name = attempt === 1 ? base : `${base}_${attempt}`;
    try {
      return await create(name);
    } catch (error) {
      const isCollision =
        error instanceof GuidedFlowRefusedError && error.code === 'DocumentExists';
      if (!isCollision) throw error;
    }
  }
}

/**
 * The one-update navigation to a just-created document: `?doc=<path>` on the
 * workspace tab. Deleting `source` IS selecting the workspace — the studio's
 * canonical URL for the workspace tab carries no `source` parameter (the tab
 * switcher in `AuthoringView` writes it the same way), and `AuthoringView`
 * reads any absent or unknown source as the workspace. One `URLSearchParams`
 * result, so the tab switch and the document selection land in a single
 * history entry from the Language tab and the overlay alike.
 */
export function paramsForCreatedDocument(previous: URLSearchParams, path: string): URLSearchParams {
  const next = new URLSearchParams(previous);
  next.delete('source');
  next.set('doc', path);
  return next;
}

/** What `runTryExample` needs from the world, injected so the flow is pure. */
export type TryExampleDependencies = {
  /** One atomic server create of `name` carrying the example verbatim. */
  create: (name: string) => Promise<AwlDocument>;
  /** Resolves only once the document list is FRESH in the cache. */
  refreshDocuments: () => Promise<void>;
  /** One URL update landing on the created document in the workspace editor. */
  navigate: (path: string) => void;
};

/**
 * The whole "Try it" flow in order: create (with collision retry) → refresh
 * the document list → navigate.
 *
 * The ordering is LOAD-BEARING, not stylistic: the workspace panel treats a
 * cached list as loaded and deselects any `?doc` the list does not contain,
 * so navigating before `refreshDocuments` has resolved would bounce the
 * operator straight off the document that was just created. `navigate` runs
 * only after the refresh settles, and never at all when any earlier step
 * rejects — the rejection propagates for the caller to surface.
 */
export async function runTryExample(
  dependencies: TryExampleDependencies,
  source: string
): Promise<AwlDocument> {
  const created = await createExampleDocument(dependencies.create, source);
  await dependencies.refreshDocuments();
  dependencies.navigate(created.path);
  return created;
}
