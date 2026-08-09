import { describe, expect, test } from 'bun:test';
import { deployEligibility } from './deploy-eligibility';
import type { AwlDiagnostic } from './facade-check';

const RED_DIAGNOSTIC: AwlDiagnostic = {
  class: 'error',
  message: 'unknown step `shp`',
  line: 3,
  column: 5,
};
const EMIT_SUBSET_DIAGNOSTIC: AwlDiagnostic = {
  class: 'emit_subset',
  message: 'step `notify` is prose-only and will emit as a stub',
  line: 7,
  column: 3,
};

const GREEN = { deploysGreen: true, diagnostics: [] };
const RED = { deploysGreen: false, diagnostics: [RED_DIAGNOSTIC] };

describe('deployEligibility', () => {
  // Catches: the header offering workflow-deploy on a worker document, which
  // deploys as a managed worker through DeployWorkerPanel instead.
  test('a worker document never sees the button, whatever else holds', () => {
    expect(deployEligibility({ workerDocument: true, deployGranted: true, check: GREEN })).toEqual({
      kind: 'hidden',
    });
    expect(deployEligibility({ workerDocument: true, deployGranted: false, check: null })).toEqual({
      kind: 'hidden',
    });
    expect(deployEligibility({ workerDocument: true, deployGranted: true, check: RED })).toEqual({
      kind: 'hidden',
    });
  });

  // Catches: a deploy affordance shown to a caller the server would refuse.
  test('no deploy grant hides the button even on a green check', () => {
    expect(
      deployEligibility({ workerDocument: false, deployGranted: false, check: GREEN })
    ).toEqual({ kind: 'hidden' });
    expect(deployEligibility({ workerDocument: false, deployGranted: false, check: null })).toEqual(
      { kind: 'hidden' }
    );
  });

  // Catches: deploying a buffer no check has judged yet.
  test('a missing check disables with a reason, never enables', () => {
    expect(deployEligibility({ workerDocument: false, deployGranted: true, check: null })).toEqual({
      kind: 'disabled',
      reason: 'The document is still being checked',
    });
  });

  // Catches: swallowing the checker's own words — the first diagnostic IS the
  // reason the operator reads on the disabled button.
  test('a red check disables with the first diagnostic message', () => {
    expect(deployEligibility({ workerDocument: false, deployGranted: true, check: RED })).toEqual({
      kind: 'disabled',
      reason: 'unknown step `shp`',
    });
  });

  test('a red check with no diagnostics still names why it is disabled', () => {
    expect(
      deployEligibility({
        workerDocument: false,
        deployGranted: true,
        check: { deploysGreen: false, diagnostics: [] },
      })
    ).toEqual({ kind: 'disabled', reason: 'The document does not deploy green' });
  });

  test('a green check on a granted workflow document is ready', () => {
    expect(deployEligibility({ workerDocument: false, deployGranted: true, check: GREEN })).toEqual(
      { kind: 'ready' }
    );
  });

  // Catches: gating on `ok`/empty-diagnostics instead of `deploysGreen` —
  // emit-subset notes ride a deployable document and must not withhold it.
  test('deploys-green with emit-subset diagnostics is still ready', () => {
    expect(
      deployEligibility({
        workerDocument: false,
        deployGranted: true,
        check: { deploysGreen: true, diagnostics: [EMIT_SUBSET_DIAGNOSTIC] },
      })
    ).toEqual({ kind: 'ready' });
  });

  // The workflow-document sweep over grant × check: every combination lands
  // on exactly one arm — no grant hides regardless of the check, a grant is
  // ready only on green and disabled otherwise. (Dirty-buffer state cannot
  // leak in: the input type carries no dirty flag at all.)
  test('workflow-document sweep: no grant hides; a grant is ready only on green', () => {
    for (const check of [GREEN, RED, null]) {
      for (const deployGranted of [true, false]) {
        const verdict = deployEligibility({ workerDocument: false, deployGranted, check });
        if (!deployGranted) {
          expect(verdict).toEqual({ kind: 'hidden' });
        } else if (check === GREEN) {
          expect(verdict).toEqual({ kind: 'ready' });
        } else {
          expect(verdict.kind).toBe('disabled');
        }
      }
    }
  });
});
