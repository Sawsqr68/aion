import { describe, expect, test } from 'bun:test';

import { compareForUpdate, compareSemver, parseSemver, type SemVer } from './semver';

function parsed(text: string): SemVer {
  const result = parseSemver(text);
  if (!result.ok) {
    throw new Error(result.reason);
  }
  return result.version;
}

describe('parseSemver', () => {
  test('parses a plain release and a full form', () => {
    expect(parsed('0.13.7')).toEqual({ major: 0, minor: 13, patch: 7, prerelease: [] });
    expect(parsed('1.2.3-rc.1+exp.sha')).toEqual({
      major: 1,
      minor: 2,
      patch: 3,
      prerelease: [
        { kind: 'alphanumeric', value: 'rc' },
        { kind: 'numeric', value: 1 },
      ],
    });
  });

  test('refuses every named malformation instead of guessing', () => {
    for (const text of [
      '1.0',
      '1.2.3.4',
      '01.2.3',
      'v1.2.3',
      '1.2.x',
      '1.2.3-',
      '1.2.3-rc..1',
      '1.2.3-01',
      '1.2.3+a_b',
      ' 1.2.3',
      '1.2.3 ',
      '',
    ]) {
      const result = parseSemver(text);
      expect(result.ok).toBe(false);
      if (!result.ok) {
        expect(result.reason).toContain('is not a spec-exact semver version');
      }
    }
  });
});

describe('compareSemver', () => {
  test("the spec's own precedence chain ranks strictly ascending", () => {
    const chain = [
      '1.0.0-alpha',
      '1.0.0-alpha.1',
      '1.0.0-alpha.beta',
      '1.0.0-beta',
      '1.0.0-beta.2',
      '1.0.0-beta.11',
      '1.0.0-rc.1',
      '1.0.0',
      '2.0.0',
      '2.1.0',
      '2.1.1',
    ];
    for (let index = 1; index < chain.length; index += 1) {
      const lower = chain[index - 1];
      const higher = chain[index];
      if (lower === undefined || higher === undefined) {
        throw new Error('the chain indexing is broken');
      }
      expect(compareSemver(parsed(lower), parsed(higher))).toBeLessThan(0);
      expect(compareSemver(parsed(higher), parsed(lower))).toBeGreaterThan(0);
    }
  });

  test('numeric version parts compare numerically, not lexically', () => {
    // Lexically "0.10.1" < "0.9.0"; numerically it is the reverse.
    expect(compareSemver(parsed('0.10.1'), parsed('0.9.0'))).toBeGreaterThan(0);
  });

  test('build metadata never separates precedence', () => {
    expect(compareSemver(parsed('1.0.0+a'), parsed('1.0.0+b'))).toBe(0);
  });
});

describe('compareForUpdate', () => {
  test('a strictly greater latest is newer; equal or lesser is not', () => {
    expect(compareForUpdate('0.13.7', '0.14.0')).toEqual({ state: 'newer' });
    expect(compareForUpdate('0.13.7', '0.13.7')).toEqual({ state: 'not-newer' });
    expect(compareForUpdate('0.13.7', '0.13.6')).toEqual({ state: 'not-newer' });
  });

  test('a prerelease latest is NEVER announced — both arms (r1 Major 1)', () => {
    // The higher-core arm: 0.14.0-rc.1 IS semver-greater than 0.13.7, and a
    // plain `cargo install` still refuses it — announcing it would point the
    // operator at an upgrade that hands them their own version back.
    expect(compareForUpdate('0.13.7', '0.14.0-rc.1')).toEqual({ state: 'not-newer' });
    // The same-core arm.
    expect(compareForUpdate('1.0.0', '1.0.0-rc.1')).toEqual({ state: 'not-newer' });
    // The rule is about installability, not about parsing: the same
    // prerelease still RANKS above the stable release in raw precedence.
    expect(compareSemver(parsed('0.14.0-rc.1'), parsed('0.13.7'))).toBeGreaterThan(0);
  });

  test('malformation on EITHER side is its own loud outcome, naming the side', () => {
    const badInstalled = compareForUpdate('unknown', '0.14.0');
    expect(badInstalled.state).toBe('malformed');
    if (badInstalled.state === 'malformed') {
      expect(badInstalled.reason).toContain('installed version');
      expect(badInstalled.reason).toContain('`unknown`');
    }

    const badLatest = compareForUpdate('0.13.7', 'not-a-version');
    expect(badLatest.state).toBe('malformed');
    if (badLatest.state === 'malformed') {
      expect(badLatest.reason).toContain('latest known version');
    }
  });
});
