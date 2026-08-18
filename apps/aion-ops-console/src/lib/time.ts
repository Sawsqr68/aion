/**
 * Viewer-local rendering for wire instants (issue #212).
 *
 * Storage and the wire are UTC (RFC 3339) and stay UTC — this module is the
 * render layer's one truth for turning those instants into strings for the
 * viewer's eyes. Every rendered string carries a seconds-precision time and a
 * visible timezone label (plus the date, unless a compact site asks for a
 * time-only style), in the viewer's system timezone and locale: no locale is
 * hardcoded and no timezone is assumed — the system default IS the feature.
 * The `timeZone` and `locales` options exist so tests can pin what they
 * assert.
 *
 * Acceptance is strict: only an RFC 3339 date-time WITH an explicit offset is
 * an instant. An offset-less date-time or a bare date would silently render
 * viewer-dependently (local vs UTC per ECMA-262 date parsing), so anything
 * else refuses loudly instead of the browser's "Invalid Date" (the contract
 * UpdateStatusPanel documented as r1 m12, hosted here now): an unrenderable
 * value is shown raw and NAMED unparseable.
 */

/** How much of the instant to render. */
export type InstantStyle = 'full' | 'time' | 'time-ms';

export type FormatInstantOptions = {
  /** IANA zone override so tests can pin a zone; omitted means the viewer's system zone. */
  timeZone?: string;
  /** BCP 47 locale override so tests can pin a locale; omitted means the viewer's locale. */
  locales?: string | string[];
  /**
   * `'full'` (date + time + zone) unless a compact site asks for `'time'`
   * (time + zone) or `'time-ms'` (time with milliseconds + zone, for callers
   * whose positions step in milliseconds).
   */
  style?: InstantStyle;
};

/** What an absent instant renders as. */
const ABSENT = '—';

/**
 * An RFC 3339 date-time with a MANDATORY offset (`Z` or `±HH:MM`; `T`/`Z` are
 * case-insensitive per the RFC). The offset is required because ECMA-262 parses
 * an offset-less date-time as LOCAL time and a bare date as UTC — either would
 * render a different instant per viewer with no visible failure.
 */
const RFC3339_DATE_TIME =
  /^\d{4}-\d{2}-\d{2}[Tt]\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:[Zz]|[+-]\d{2}:\d{2})$/;

/**
 * Memoized formatters keyed by style, zone, and locale. The assumptions this
 * memoization rests on: the system default zone and locale are resolved once
 * per formatter and therefore once per session (an OS-level zone or locale
 * change mid-session is picked up on reload, not live), and every key
 * component is code-supplied — styles are the closed {@link InstantStyle}
 * union and zones/locales come from call sites and tests, never from user
 * input — so the map is bounded by the style × zone × locale sets appearing
 * in code.
 */
/**
 * The date-time junction in `'full'` style is THIS literal, never ICU's.
 * CLDR 42 changed the en-US combined-format glue from ", " to " at ", so two
 * hosts with different ICU data rendered different bytes from the same
 * instant and the same runtime version (issue #57 — the bun gate leg could
 * never be green off-macOS). Date and time are formatted separately and
 * joined here, so the rendering is byte-identical on every platform; the
 * halves themselves are locale-faithful as before. A comma is the junction
 * because it asserts no language — an English "at" would be wrong under
 * every non-English viewer locale this module refuses to hardcode away.
 */
const DATE_TIME_GLUE = ', ';

type InstantRenderer = (ms: number | Date) => string;

const renderers = new Map<string, InstantRenderer>();

function rendererFor(options: FormatInstantOptions | undefined): InstantRenderer {
  const style = options?.style ?? 'full';
  const timeZone = options?.timeZone;
  const locales = options?.locales;
  const localesKey = Array.isArray(locales) ? locales.join(',') : (locales ?? '');
  const key = `${style}|${timeZone ?? ''}|${localesKey}`;
  const cached = renderers.get(key);
  if (cached !== undefined) {
    return cached;
  }

  const timeOptions: Intl.DateTimeFormatOptions = {
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
    timeZoneName: 'short',
  };
  if (style === 'time-ms') {
    timeOptions.fractionalSecondDigits = 3;
  }
  if (timeZone !== undefined) {
    timeOptions.timeZone = timeZone;
  }
  const timeFormatter = new Intl.DateTimeFormat(locales, timeOptions);

  let renderer: InstantRenderer;
  if (style === 'full') {
    const dateOptions: Intl.DateTimeFormatOptions = {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
    };
    if (timeZone !== undefined) {
      dateOptions.timeZone = timeZone;
    }
    const dateFormatter = new Intl.DateTimeFormat(locales, dateOptions);
    renderer = (instant) =>
      `${dateFormatter.format(instant)}${DATE_TIME_GLUE}${timeFormatter.format(instant)}`;
  } else {
    renderer = (instant) => timeFormatter.format(instant);
  }
  renderers.set(key, renderer);
  return renderer;
}

/**
 * The epoch milliseconds of a wire instant, or `null` when `value` is not an
 * RFC 3339 date-time with an explicit offset (or is calendar-invalid, e.g. a
 * thirteenth month). The single parse gate every render path goes through —
 * callers use it to decide whether a raw ISO `dateTime` attribute on a
 * `<time>` element would be honest, and to parse exactly once before
 * {@link formatInstantMs}.
 */
export function parseInstantMs(value: string): number | null {
  if (!RFC3339_DATE_TIME.test(value)) {
    return null;
  }

  const ms = Date.parse(value);
  return Number.isNaN(ms) ? null : ms;
}

/**
 * A wire instant (RFC 3339 string) rendered viewer-local. Absent input
 * (`null` / `undefined` / blank) renders as an em-dash; anything
 * {@link parseInstantMs} refuses is returned raw and named unparseable, never
 * "Invalid Date" and never a silently viewer-dependent reading.
 */
export function formatInstant(
  value: string | null | undefined,
  options?: FormatInstantOptions
): string {
  if (value === null || value === undefined || value.trim().length === 0) {
    return ABSENT;
  }

  const parsedMs = parseInstantMs(value);
  if (parsedMs === null) {
    return `${value} (unparseable timestamp)`;
  }

  return rendererFor(options)(parsedMs);
}

/**
 * An epoch-milliseconds instant rendered viewer-local — for callers that carry
 * numeric time (the swimlane axis, the scrubber). Non-finite or out-of-range
 * milliseconds refuse with the same labelled string, never "Invalid Date".
 */
export function formatInstantMs(ms: number, options?: FormatInstantOptions): string {
  const date = new Date(ms);
  if (Number.isNaN(date.getTime())) {
    return `${String(ms)} (unparseable timestamp)`;
  }

  return rendererFor(options)(date);
}
