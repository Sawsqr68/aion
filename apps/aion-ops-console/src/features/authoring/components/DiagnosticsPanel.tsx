import { X } from 'lucide-react';

import { type ProseSegment, splitDiagnosticProse } from '../editor-seam/diagnostic-prose';
import type { AwlDiagnostic } from '../lib/facade';

/**
 * The current diagnostics as a clickable list, docked above the status bar —
 * the lint-panel convention. Each row shows the diagnostic's position and its
 * full prose (headline, then remedy, backticked identifiers as code — the
 * same segmentation the editor tooltip renders); activating a row hands the
 * diagnostic back to the host, which moves the editor to its range.
 *
 * Deliberately NON-MODAL, like `GuideOverlay`: no backdrop, no focus trap,
 * nothing takes focus uninvited. It opens from the status-bar count and
 * closes from its own button or that same toggle.
 */
export function DiagnosticsPanel({
  diagnostics,
  onNavigate,
  onClose,
}: {
  diagnostics: readonly AwlDiagnostic[];
  onNavigate: (diagnostic: AwlDiagnostic) => void;
  onClose: () => void;
}) {
  return (
    <aside
      aria-label="Diagnostics"
      className="flex max-h-48 flex-col border-border border-t bg-surface-base"
      data-slot="diagnostics-panel"
      id="authoring-diagnostics-panel"
    >
      <header className="flex items-center justify-between gap-2 border-border border-b px-4 py-1.5">
        <span className="font-semibold text-muted-foreground text-xs uppercase tracking-wide">
          Diagnostics
        </span>
        <button
          aria-label="Close the diagnostics panel"
          className="rounded-md p-1 text-muted-foreground outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-accent-primary"
          onClick={onClose}
          type="button"
        >
          <X aria-hidden="true" className="size-3.5" />
        </button>
      </header>
      <ul className="min-h-0 flex-1 overflow-y-auto" data-slot="diagnostics-panel-list">
        {keyedDiagnostics(diagnostics).map(({ key, diagnostic }) => (
          <li key={key}>
            <DiagnosticRow diagnostic={diagnostic} onNavigate={onNavigate} />
          </li>
        ))}
      </ul>
    </aside>
  );
}

/**
 * Content-derived keys: position plus message, with an occurrence counter so
 * even two byte-identical findings stay distinct without leaning on the
 * array index.
 */
function keyedDiagnostics(diagnostics: readonly AwlDiagnostic[]) {
  const seen = new Map<string, number>();
  return diagnostics.map((diagnostic) => {
    const base = `${diagnostic.line}:${diagnostic.column}:${diagnostic.message}`;
    const occurrence = seen.get(base) ?? 0;
    seen.set(base, occurrence + 1);
    return { key: occurrence === 0 ? base : `${base}#${occurrence}`, diagnostic };
  });
}

function DiagnosticRow({
  diagnostic,
  onNavigate,
}: {
  diagnostic: AwlDiagnostic;
  onNavigate: (diagnostic: AwlDiagnostic) => void;
}) {
  const prose = splitDiagnosticProse(diagnostic.message);
  return (
    <button
      className="flex w-full items-start gap-3 px-4 py-2 text-left text-xs outline-none hover:bg-surface-elevated focus-visible:ring-2 focus-visible:ring-accent-primary"
      onClick={() => onNavigate(diagnostic)}
      type="button"
    >
      <span className="shrink-0 font-mono text-destructive">
        {diagnostic.line}:{diagnostic.column}
      </span>
      <span className="min-w-0 flex-1">
        <ProseLine className="text-foreground" segments={prose.headline} />
        {prose.remedy !== null && (
          <ProseLine className="mt-0.5 block text-muted-foreground" segments={prose.remedy} />
        )}
      </span>
    </button>
  );
}

function ProseLine({
  segments,
  className,
}: {
  segments: readonly ProseSegment[];
  className?: string;
}) {
  let offset = 0;
  return (
    <span className={className}>
      {segments.map((segment) => {
        const key = `${offset}:${segment.kind}`;
        offset += segment.text.length + (segment.kind === 'code' ? 2 : 0);
        return segment.kind === 'code' ? (
          <code className="rounded bg-accent px-1 font-mono text-[0.6875rem]" key={key}>
            {segment.text}
          </code>
        ) : (
          <span key={key}>{segment.text}</span>
        );
      })}
    </span>
  );
}
