export function nullableString(value: unknown, field: string): string | null {
  return value === null ? null : expectString(value, field);
}

export function expectRecord(value: unknown, field?: string): Record<string, unknown> {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    throw new Error(
      field === undefined
        ? 'Invalid authoring response: expected object'
        : `Invalid authoring response: ${field}`
    );
  }
  return value as Record<string, unknown>;
}

export function expectArray(value: unknown, field: string): unknown[] {
  if (!Array.isArray(value)) throw new Error(`Invalid authoring response: ${field}`);
  return value;
}

export function expectString(value: unknown, field: string): string {
  if (typeof value !== 'string') throw new Error(`Invalid authoring response: ${field}`);
  return value;
}

export function expectNumber(value: unknown, field: string): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    throw new Error(`Invalid authoring response: ${field}`);
  }
  return value;
}

/**
 * A position-like value: a non-negative integer. Layout coordinates are
 * legitimately fractional ([`expectNumber`]); lines, columns, and UTF-16
 * characters are not — a fractional or negative one is a malformed response
 * and is refused at the boundary, never fed to the editor as an offset.
 */
export function expectNonNegativeInteger(value: unknown, field: string): number {
  const number = expectNumber(value, field);
  if (!Number.isInteger(number) || number < 0) {
    throw new Error(`Invalid authoring response: ${field}`);
  }
  return number;
}

export function expectBoolean(value: unknown, field: string): boolean {
  if (typeof value !== 'boolean') throw new Error(`Invalid authoring response: ${field}`);
  return value;
}
