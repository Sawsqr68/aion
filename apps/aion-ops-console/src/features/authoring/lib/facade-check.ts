import { parseSemanticIndex } from './facade-semantic';
import {
  expectArray,
  expectBoolean,
  expectNonNegativeInteger,
  expectNumber,
  expectRecord,
  expectString,
} from './facade-values';
import type { SemanticIndex } from './projection-types';

/**
 * The server's diagnostic classes. The checker emits exactly one today; the
 * union stays so a second class is a type-level event here, not a silent
 * string.
 */
export type DiagnosticClass = 'error';

/** One zero-based position: `line` counts lines, `character` UTF-16 units. */
export type DiagnosticPosition = { line: number; character: number };

/**
 * A diagnostic's full extent, converted to UTF-16 SERVER-SIDE (the editors'
 * index space — CodeMirror's and the LSP's alike). Half-open: `start`
 * inclusive, `end` exclusive.
 */
export type DiagnosticRange = { start: DiagnosticPosition; end: DiagnosticPosition };

export type AwlDiagnostic = {
  class: DiagnosticClass;
  message: string;
  line: number;
  column: number;
  range: DiagnosticRange;
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
  if (diagnosticClass !== 'error') {
    throw new Error('Invalid authoring response: class');
  }
  return {
    class: diagnosticClass,
    message: expectString(record.message, 'message'),
    line: expectNonNegativeInteger(record.line, 'line'),
    column: expectNonNegativeInteger(record.column, 'column'),
    range: parseDiagnosticRange(record.range),
  };
}

function parseDiagnosticRange(value: unknown): DiagnosticRange {
  const record = expectRecord(value, 'range');
  return {
    start: parseDiagnosticPosition(record.start, 'range.start'),
    end: parseDiagnosticPosition(record.end, 'range.end'),
  };
}

function parseDiagnosticPosition(value: unknown, field: string): DiagnosticPosition {
  const record = expectRecord(value, field);
  return {
    line: expectNonNegativeInteger(record.line, `${field}.line`),
    character: expectNonNegativeInteger(record.character, `${field}.character`),
  };
}
