import { formatInstant, formatInstantMs, parseInstantMs } from '@/lib/time';

/**
 * A wire instant rendered for the viewer (issue #212): the visible text is the
 * viewer-local rendering from the shared `@/lib/time` helpers, and the raw UTC
 * instant is preserved on the `<time>` element (`dateTime` attribute + hover
 * `title`). An absent or unparseable value renders as plain text — a
 * `<time dateTime>` never carries a value that is not an RFC 3339 instant.
 * The accepted path parses once ({@link parseInstantMs}) and formats from the
 * parsed milliseconds.
 */
export function InstantText({
  value,
  className,
}: {
  value: string | null | undefined;
  className?: string;
}) {
  const parsedMs = value === null || value === undefined ? null : parseInstantMs(value);
  if (value === null || value === undefined || parsedMs === null) {
    const text = formatInstant(value);
    return className === undefined ? text : <span className={className}>{text}</span>;
  }

  return (
    <time className={className} dateTime={value} title={value}>
      {formatInstantMs(parsedMs)}
    </time>
  );
}
