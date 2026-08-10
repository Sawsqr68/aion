/**
 * The diagnostic tooltip's DOM, built from the shared prose segmentation.
 *
 * Pure DOM construction with no `@codemirror` dependency (the seam guard
 * keeps CodeMirror imports in `codemirror.ts`), extracted here so it sits
 * beside `diagnostic-prose.ts`, which owns the segmentation both this
 * renderer and the `DiagnosticsPanel` consume.
 */

import { type ProseSegment, splitDiagnosticProse } from './diagnostic-prose';

export function diagnosticTooltipDom(messages: readonly string[]): HTMLElement {
  const dom = document.createElement('div');
  dom.className = 'awl-diagnostic-tooltip';
  dom.setAttribute('role', 'tooltip');
  for (const message of messages) {
    const prose = splitDiagnosticProse(message);
    const entry = document.createElement('div');
    entry.className = 'awl-diagnostic-tooltip-entry';
    entry.appendChild(proseLine(prose.headline, 'awl-diagnostic-headline'));
    if (prose.remedy !== null) entry.appendChild(proseLine(prose.remedy, 'awl-diagnostic-remedy'));
    dom.appendChild(entry);
  }
  return dom;
}

function proseLine(segments: readonly ProseSegment[], className: string): HTMLElement {
  const line = document.createElement('div');
  line.className = className;
  for (const segment of segments) {
    if (segment.kind === 'code') {
      const code = document.createElement('code');
      code.textContent = segment.text;
      line.appendChild(code);
    } else {
      line.appendChild(document.createTextNode(segment.text));
    }
  }
  return line;
}
