import { parseSemanticIndex } from './facade-semantic';
import {
  expectArray,
  expectBoolean,
  expectNumber,
  expectRecord,
  expectString,
} from './facade-values';
import type { SemanticIndex } from './projection-types';

export type DiagnosticClass = 'error' | 'emit_subset';

export type AwlDiagnostic = {
  class: DiagnosticClass;
  message: string;
  line: number;
  column: number;
};

export type CheckResult = {
  ok: boolean;
  deploysGreen: boolean;
  steps: number | null;
  diagnostics: AwlDiagnostic[];
  semantic: SemanticIndex | null;
};

/**
 * Parses a server `CheckResponse`.
 *
 * One parser, used by both the workspace check (`POST /awl/check`) and the
 * projection a deployed version carries in `GET /awl/deployed/...`. A second
 * copy would let the two views of the same server type drift.
 */
export function parseCheckResult(value: unknown): CheckResult {
  const record = expectRecord(value);
  return {
    ok: expectBoolean(record.ok, 'ok'),
    deploysGreen: expectBoolean(record.deploys_green, 'deploys_green'),
    steps: record.steps === null ? null : expectNumber(record.steps, 'steps'),
    diagnostics: expectArray(record.diagnostics, 'diagnostics').map(parseDiagnostic),
    semantic: record.semantic === null ? null : parseSemanticIndex(record.semantic),
  };
}

export function parseDiagnostic(value: unknown): AwlDiagnostic {
  const record = expectRecord(value);
  const diagnosticClass = record.class;
  if (diagnosticClass !== 'error' && diagnosticClass !== 'emit_subset') {
    throw new Error('Invalid authoring response: class');
  }
  return {
    class: diagnosticClass,
    message: expectString(record.message, 'message'),
    line: expectNumber(record.line, 'line'),
    column: expectNumber(record.column, 'column'),
  };
}
