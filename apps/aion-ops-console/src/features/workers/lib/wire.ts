/** Strict field readers for managed-worker responses: absence is an error HERE
 * (the caller decides which fields tolerate absence, and reads those with the
 * nullable variants). */

export function expectRecord(value: unknown, field: string): Record<string, unknown> {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    throw new Error(`Invalid managed-worker response: ${field}`);
  }
  return value as Record<string, unknown>;
}

export function expectArray(value: unknown, field: string): unknown[] {
  if (!Array.isArray(value)) throw new Error(`Invalid managed-worker response: ${field}`);
  return value;
}

export function expectString(value: unknown, field: string): string {
  if (typeof value !== 'string') throw new Error(`Invalid managed-worker response: ${field}`);
  return value;
}

export function expectNumber(value: unknown, field: string): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    throw new Error(`Invalid managed-worker response: ${field}`);
  }
  return value;
}

export function expectBoolean(value: unknown, field: string): boolean {
  if (typeof value !== 'boolean') throw new Error(`Invalid managed-worker response: ${field}`);
  return value;
}

export function nullableString(value: unknown, field: string): string | null {
  return value === null || value === undefined ? null : expectString(value, field);
}

export function nullableNumber(value: unknown, field: string): number | null {
  return value === null || value === undefined ? null : expectNumber(value, field);
}
