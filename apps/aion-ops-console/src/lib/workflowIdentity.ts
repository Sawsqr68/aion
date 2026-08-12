import type { Event } from '@/types';
import { DISPLAY_NAME_ATTRIBUTE } from '@/types';

/**
 * Workflow identity and labelling (#211).
 *
 * A run's identity is its UUID; a display name is only ever a LABEL over that
 * identity. Nothing addresses a workflow by name — not a route, not a request
 * parameter — so every surface that shows a name shows the id beside it and an
 * operator can always address the run they are looking at.
 *
 * This module is shared rather than feature-local on purpose: the workflow list
 * and the detail view must read a name by exactly the same rules, or one
 * surface would show a name the other does not.
 */

/** The canonical UUID shape (8-4-4-4-12 hex) aion mints workflow/run ids in. */
const UUID_PATTERN = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

/**
 * The short form of a workflow id: a canonical UUID's FIRST GROUP, which is the
 * standard short form for a UUID.
 *
 * This is not a truncation policy and imposes no length cap: a value that is
 * not a canonical UUID — a caller-chosen id, a fixture — is returned WHOLE,
 * never cut, so the short form can never silently drop the characters that
 * distinguish one id from another.
 */
export function shortWorkflowId(workflowId: string): string {
  return UUID_PATTERN.test(workflowId) ? workflowId.slice(0, 8) : workflowId;
}

/** Search-attribute map as carried by a recorded `SearchAttributesUpdated`. */
export type SearchAttributes = Extract<
  Event,
  { type: 'SearchAttributesUpdated' }
>['data']['attributes'];

/**
 * The attribute that carries a run's operator-facing display name.
 *
 * NOT declared here — re-exported from the generated bindings, which the ts-rs
 * pipeline emits from `aion_core::DISPLAY_NAME_ATTRIBUTE`. There is therefore
 * ONE declaration of this key in the world: renaming the Rust constant makes
 * the committed bindings stale and fails `export_ops_console_wire_types` in the
 * lane that renamed it, and regenerating carries the new key here with no hand
 * edit. A retyped literal would instead have failed SILENTLY — every named run
 * would simply have rendered as unnamed.
 *
 * It is re-exported through this module (rather than imported from `@/types` at
 * each site) because reading a name is this module's job, and
 * `display-name-attribute-parity.test.ts` holds every other TS file to it.
 */
export { DISPLAY_NAME_ATTRIBUTE };

/**
 * Read a name that arrived as a plain field (a `WorkflowSummary.display_name`).
 *
 * Absent, null, and blank all mean UNNAMED: the server refuses to record a
 * blank name, so a blank reaching a renderer is foreign data, not a label, and
 * must never take the place the id would have occupied.
 *
 * A surviving name is returned TRIMMED, for the same reason it is returned at
 * all. Every writer in the system trims before recording — the server, and the
 * Rust, Python, and TypeScript SDKs — so a padded name reaching a reader is
 * likewise foreign data. Returning it as-is would render the padding, and two
 * readers holding `"X"` and `"  X  "` would disagree about whether they are
 * looking at the same label. Trimming here makes this module the single place
 * that decides what a name IS, which is what it exists for.
 */
export function normalizeDisplayName(value: string | null | undefined): string | null {
  if (typeof value !== 'string') {
    return null;
  }
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : null;
}

/**
 * Whether a value the operator supplied is a blank name — the writer-side twin
 * of {@link normalizeDisplayName}.
 *
 * ONE predicate, three surfaces. The start form, the rename submit path, and
 * the REST client all refuse a present-but-blank name, because the server and
 * every caller SDK do; what differs between them is only the sentence shown to
 * the operator, which is surface-appropriate and belongs at each surface. The
 * RULE lives here so it cannot drift into three slightly different rules.
 *
 * Note what this does NOT decide: whether an ABSENT name is allowed. Omitting a
 * name is how a start says "unnamed" and is perfectly legal; a rename has no
 * name to omit. Each caller answers that for itself.
 */
export function isBlankDisplayName(value: string): boolean {
  return value.trim().length === 0;
}

/**
 * The display name carried by one recorded attribute map, or `null` when this
 * update did not touch the name. A non-String value is NOT a name: the server
 * only ever records the display name as a String, so anything else is foreign
 * data and is ignored rather than coerced into a label.
 */
export function displayNameFromAttributes(attributes: SearchAttributes): string | null {
  const value = attributes[DISPLAY_NAME_ATTRIBUTE];

  if (value === undefined || value.type !== 'String') {
    return null;
  }

  return normalizeDisplayName(value.data);
}

/** The display name a single event records, or `null` if it records none. */
export function displayNameFromEvent(event: Event): string | null {
  return event.type === 'SearchAttributesUpdated'
    ? displayNameFromAttributes(event.data.attributes)
    : null;
}

/**
 * The run's display name folded from the recorded events the caller HOLDS,
 * last write wins.
 *
 * An update that carries OTHER attributes but not {@link DISPLAY_NAME_ATTRIBUTE}
 * leaves the name standing (the server's fold merges maps; an absent key means
 * unchanged, never cleared). The winner is chosen by recorded sequence rather
 * than array position, so an unsorted window cannot resurrect a superseded
 * name.
 *
 * SCOPE — this reads only the events in hand, which is why it is not the sole
 * authority. `WorkflowSummary.display_name`, projected server-side from the
 * visibility row's search attributes and refreshed on every append, is the
 * authority for a run's CURRENT name and is not window-limited. This fold is
 * the reader for recorded history itself (the timeline), and the reader that
 * supersedes a fetched summary when a NEWER naming event has since arrived in
 * view. Do not "fix" a missing name here by widening a history window; read the
 * summary instead.
 */
export function displayNameFromEvents(events: readonly Event[]): string | null {
  let winner: { seq: number; name: string } | null = null;

  for (const event of events) {
    const name = displayNameFromEvent(event);
    const seq = event.data.envelope.seq;

    if (name !== null && (winner === null || seq >= winner.seq)) {
      winner = { seq, name };
    }
  }

  return winner === null ? null : winner.name;
}
