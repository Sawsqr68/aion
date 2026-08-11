import { describe, expect, test } from 'bun:test';

import { formatInstant, formatInstantMs, parseInstantMs } from './time';

/** A known instant; its Sydney rendering pins the test-run timezone (test-setup.ts). */
const INSTANT = '2026-07-15T00:00:02.500Z';
const INSTANT_EPOCH_MS = 1_784_073_602_500;

describe('formatInstant acceptance gate', () => {
  test('absent instants render as an em-dash', () => {
    expect(formatInstant(null)).toBe('—');
    expect(formatInstant(undefined)).toBe('—');
    expect(formatInstant('')).toBe('—');
    expect(formatInstant('   ')).toBe('—');
  });

  test('unparseable input is returned raw and named, never "Invalid Date"', () => {
    expect(formatInstant('not-a-timestamp')).toBe('not-a-timestamp (unparseable timestamp)');
    expect(formatInstant('garbage')).not.toContain('Invalid Date');
  });

  test('anything short of an RFC 3339 date-time WITH offset refuses', () => {
    // Each of these would otherwise silently render viewer-dependently
    // (ECMA-262: offset-less date-time = local, bare date = UTC) or pass a
    // non-instant into a <time dateTime>.
    for (const value of [
      '2026-07-15T00:00:02',
      '2026-07-15',
      '2026-07-15 00:00:02',
      '2026',
      '0',
      'July 15, 2026',
    ]) {
      expect(formatInstant(value)).toBe(`${value} (unparseable timestamp)`);
      expect(parseInstantMs(value)).toBeNull();
    }
  });

  test('a well-shaped but calendar-invalid date-time refuses', () => {
    expect(formatInstant('2026-13-15T00:00:02Z')).toBe(
      '2026-13-15T00:00:02Z (unparseable timestamp)'
    );
  });

  test('an explicit numeric offset is accepted and renders the same instant as Z', () => {
    expect(formatInstant('2026-07-15T10:00:02.500+10:00', { locales: 'en-US' })).toBe(
      formatInstant(INSTANT, { locales: 'en-US' })
    );
  });
});

describe('viewer-local rendering', () => {
  test('a known instant renders exactly under an explicit timeZone option', () => {
    expect(formatInstant(INSTANT, { locales: 'en-US', timeZone: 'UTC' })).toBe(
      'Jul 15, 2026 at 12:00:02 AM UTC'
    );
    expect(formatInstant(INSTANT, { locales: 'en-US', timeZone: 'Australia/Sydney' })).toBe(
      'Jul 15, 2026 at 10:00:02 AM GMT+10'
    );
  });

  test('the pinned system zone (Australia/Sydney) IS the default rendering zone', () => {
    // Machine-independence rests on test-setup.ts pinning TZ (and guarding the
    // locale) before any Date exists — an exact assertion proves the pin bit.
    expect(formatInstant(INSTANT)).toBe('Jul 15, 2026 at 10:00:02 AM GMT+10');
  });

  test('every rendering carries a visible timezone label, never the word "local"', () => {
    const full = formatInstant(INSTANT, { locales: 'en-US' });
    const timeOnly = formatInstant(INSTANT, { locales: 'en-US', style: 'time' });
    expect(full).toContain('GMT+10');
    expect(timeOnly).toBe('10:00:02 AM GMT+10');
    expect(full.toLowerCase()).not.toContain('local');
    expect(timeOnly.toLowerCase()).not.toContain('local');
  });
});

describe('formatInstantMs', () => {
  test('renders a known epoch and refuses non-finite input', () => {
    expect(formatInstantMs(INSTANT_EPOCH_MS, { locales: 'en-US' })).toBe(
      'Jul 15, 2026 at 10:00:02 AM GMT+10'
    );
    expect(formatInstantMs(INSTANT_EPOCH_MS, { locales: 'en-US', style: 'time' })).toBe(
      '10:00:02 AM GMT+10'
    );
    expect(
      formatInstantMs(INSTANT_EPOCH_MS, { locales: 'en-US', style: 'time', timeZone: 'UTC' })
    ).toBe('12:00:02 AM UTC');
    expect(formatInstantMs(Number.NaN)).toBe('NaN (unparseable timestamp)');
    expect(formatInstantMs(Number.POSITIVE_INFINITY)).toBe('Infinity (unparseable timestamp)');
  });

  test('out-of-range epoch milliseconds refuse instead of throwing', () => {
    // Beyond the ECMAScript maximum time value: a finite number no Date can hold.
    expect(formatInstantMs(8.7e15)).toBe('8700000000000000 (unparseable timestamp)');
  });

  test("the 'time-ms' style distinguishes adjacent milliseconds (scrubber steps in ms)", () => {
    const at2500 = formatInstantMs(INSTANT_EPOCH_MS, { locales: 'en-US', style: 'time-ms' });
    const at2501 = formatInstantMs(INSTANT_EPOCH_MS + 1, { locales: 'en-US', style: 'time-ms' });
    expect(at2500).toBe('10:00:02.500 AM GMT+10');
    expect(at2501).toBe('10:00:02.501 AM GMT+10');
    expect(at2500).not.toBe(at2501);
  });
});

describe('parseInstantMs', () => {
  test('separates instants from garbage and yields the exact epoch', () => {
    expect(parseInstantMs(INSTANT)).toBe(INSTANT_EPOCH_MS);
    expect(parseInstantMs('2026-07-15T10:00:02.500+10:00')).toBe(INSTANT_EPOCH_MS);
    expect(parseInstantMs('not-a-timestamp')).toBeNull();
    expect(parseInstantMs('2026-13-15T00:00:02Z')).toBeNull();
  });
});
