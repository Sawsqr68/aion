/**
 * Pure presentation segmentation for diagnostic prose.
 *
 * The server's checker messages are remedy-bearing sentences: what is wrong,
 * then — after an em-dash separator — what to do instead, with identifiers
 * quoted in backticks. This module turns that ONE wire field into display
 * structure. It is a display convention only: no server field encodes it, and
 * a message without the separator or without backticks renders whole.
 *
 * Deliberately free of `@codemirror/*` and DOM imports so the tooltip
 * renderer and the diagnostics panel share one segmentation, testable
 * headlessly (the seam guard keeps CodeMirror imports in `codemirror.ts`).
 */

export type ProseSegment = { kind: 'text' | 'code'; text: string };

export type DiagnosticProse = {
  headline: readonly ProseSegment[];
  /** The remedy clause after the first ` — `, or null when the message has none. */
  remedy: readonly ProseSegment[] | null;
};

const REMEDY_SEPARATOR = ' — ';

/** Splits a checker message into headline and (optional) remedy prose. */
export function splitDiagnosticProse(message: string): DiagnosticProse {
  const separator = message.indexOf(REMEDY_SEPARATOR);
  if (separator === -1) {
    return { headline: proseSegments(message), remedy: null };
  }
  return {
    headline: proseSegments(message.slice(0, separator)),
    remedy: proseSegments(message.slice(separator + REMEDY_SEPARATOR.length)),
  };
}

/**
 * Splits prose into text and backtick-quoted code segments. An unpaired
 * backtick is literal text — degradation, never a parse failure.
 */
export function proseSegments(text: string): ProseSegment[] {
  const segments: ProseSegment[] = [];
  let cursor = 0;
  while (cursor < text.length) {
    const open = text.indexOf('`', cursor);
    if (open === -1) break;
    const close = text.indexOf('`', open + 1);
    if (close === -1) break;
    if (open > cursor) segments.push({ kind: 'text', text: text.slice(cursor, open) });
    segments.push({ kind: 'code', text: text.slice(open + 1, close) });
    cursor = close + 1;
  }
  if (cursor < text.length) segments.push({ kind: 'text', text: text.slice(cursor) });
  return segments;
}
