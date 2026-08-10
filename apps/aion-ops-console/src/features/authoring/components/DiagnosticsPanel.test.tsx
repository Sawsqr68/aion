import { describe, expect, test } from 'bun:test';
import { renderToStaticMarkup } from 'react-dom/server';

import type { AwlDiagnostic } from '../lib/facade';
import { DiagnosticsPanel } from './DiagnosticsPanel';

const DIAGNOSTICS: AwlDiagnostic[] = [
  {
    class: 'error',
    message: 'unknown after target `shp` — declare the step or reference `ship` instead',
    line: 4,
    column: 9,
    range: { start: { line: 3, character: 8 }, end: { line: 3, character: 11 } },
  },
  {
    class: 'error',
    message: 'expected step',
    line: 7,
    column: 1,
    range: { start: { line: 6, character: 0 }, end: { line: 6, character: 0 } },
  },
];

describe('DiagnosticsPanel', () => {
  test('lists every diagnostic with its position, as buttons that navigate', () => {
    const html = renderToStaticMarkup(
      <DiagnosticsPanel
        diagnostics={DIAGNOSTICS}
        onClose={() => undefined}
        onNavigate={() => undefined}
      />
    );
    expect(html).toContain('aria-label="Diagnostics"');
    expect(html).toContain('4:9');
    expect(html).toContain('7:1');
    expect(html).toContain('unknown after target');
    expect(html).toContain('expected step');
    // One navigation button per diagnostic, plus the close button.
    expect(html.split('<button').length - 1).toBe(DIAGNOSTICS.length + 1);
    expect(html).toContain('aria-label="Close the diagnostics panel"');
  });

  test('renders backticked identifiers as code and the remedy as its own line', () => {
    const html = renderToStaticMarkup(
      <DiagnosticsPanel
        diagnostics={[DIAGNOSTICS[0] as AwlDiagnostic]}
        onClose={() => undefined}
        onNavigate={() => undefined}
      />
    );
    expect(html).toContain('<code');
    expect(html).toContain('shp');
    expect(html).toContain('declare the step or reference');
    // The separator itself is presentation, not content to repeat.
    expect(html).not.toContain(' — declare');
  });

  test('is a non-modal complementary region: no dialog, no aria-modal', () => {
    const html = renderToStaticMarkup(
      <DiagnosticsPanel
        diagnostics={DIAGNOSTICS}
        onClose={() => undefined}
        onNavigate={() => undefined}
      />
    );
    expect(html).toContain('<aside');
    expect(html).not.toContain('role="dialog"');
    expect(html).not.toContain('aria-modal');
  });
});
