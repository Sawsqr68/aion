import { describe, expect, test } from 'bun:test';
import { renderToStaticMarkup } from 'react-dom/server';

import { DeployOutcomeLine } from './DeployOutcomeLine';

describe('DeployOutcomeLine', () => {
  test('no outcome renders nothing', () => {
    expect(renderToStaticMarkup(<DeployOutcomeLine outcome={null} />)).toBe('');
  });

  test('a success renders as its own success-styled status line', () => {
    const markup = renderToStaticMarkup(
      <DeployOutcomeLine
        outcome={{
          kind: 'success',
          text: 'Deployed hello_flow to queue default (package hello_flow$abc)',
        }}
      />
    );
    expect(markup).toContain('data-slot="deploy-outcome"');
    expect(markup).toContain('role="status"');
    expect(markup).toContain('text-success');
    expect(markup).toContain('Deployed hello_flow to queue default (package hello_flow$abc)');
  });

  // Catches: the refusal's words truncated — the server's message is the
  // operator's only lead and the line must WRAP, not cut.
  test('a refusal renders danger-styled, wrapping, never truncating', () => {
    const markup = renderToStaticMarkup(
      <DeployOutcomeLine
        outcome={{
          kind: 'refused',
          text: 'RevisionMismatch: the saved document does not carry content hash abc123',
        }}
      />
    );
    expect(markup).toContain('text-destructive');
    expect(markup).toContain(
      'RevisionMismatch: the saved document does not carry content hash abc123'
    );
    expect(markup).toContain('break-words');
    expect(markup).not.toContain('truncate');
  });
});
