/**
 * Strict Semantic Versioning 2.0.0 parsing and precedence for the update
 * pill — the console-side twin of the server's
 * `crates/aion-server/src/update_check/semver.rs`, implementing the spec
 * grammar and precedence (items 9–11) with the same refusal posture and the
 * same one deliberate bound: a numeric identifier beyond
 * `Number.isSafeInteger` (the server bounds at `u64`) is REFUSED rather than
 * parsed — a refuse-direction divergence from the spec's unbounded
 * integers, mirroring the server's documented bound (r2-m11).
 *
 * The pill's one question is "is `latest_known` an INSTALLABLE upgrade over
 * `installed`?", and a wrong answer points an operator at a phantom upgrade
 * (or hides a real one). So malformation REFUSES loudly instead of guessing:
 * every parse returns a discriminated result, and the comparison surfaces
 * `malformed` as its own outcome rather than coercing to false — the caller
 * must render the refusal, not swallow it.
 */

/** One prerelease identifier, already classified per the spec grammar. */
export type PrereleaseIdent =
  | { kind: 'numeric'; value: number }
  | { kind: 'alphanumeric'; value: string };

/** A parsed SemVer 2.0.0 version. Build metadata is display-only. */
export type SemVer = {
  major: number;
  minor: number;
  patch: number;
  /** Empty means a release, which outranks every prerelease of its core. */
  prerelease: PrereleaseIdent[];
};

/** A parse outcome: the version, or the named refusal. Never a guess. */
export type SemVerParse = { ok: true; version: SemVer } | { ok: false; reason: string };

/** Parse a spec-exact version string; anything else refuses with a reason. */
export function parseSemver(text: string): SemVerParse {
  const refusal = (reason: string): SemVerParse => ({
    ok: false,
    reason: `\`${text}\` is not a spec-exact semver version: ${reason}`,
  });

  const { core, prereleaseText, build } = splitVersion(text);

  const numbers = parseCore(core);
  if (typeof numbers === 'string') {
    return refusal(numbers);
  }
  const prerelease = parsePrereleaseList(prereleaseText);
  if (typeof prerelease === 'string') {
    return refusal(prerelease);
  }
  const buildRefusal = validateBuild(build);
  if (buildRefusal !== null) {
    return refusal(buildRefusal);
  }

  const [major, minor, patch] = numbers;
  return { ok: true, version: { major, minor, patch, prerelease } };
}

/** Split `core[-prerelease][+build]` without validating any of the pieces. */
function splitVersion(text: string): {
  core: string;
  prereleaseText: string | null;
  build: string | null;
} {
  const buildSplit = text.indexOf('+');
  const beforeBuild = buildSplit === -1 ? text : text.slice(0, buildSplit);
  const build = buildSplit === -1 ? null : text.slice(buildSplit + 1);
  const dashSplit = beforeBuild.indexOf('-');
  const core = dashSplit === -1 ? beforeBuild : beforeBuild.slice(0, dashSplit);
  const prereleaseText = dashSplit === -1 ? null : beforeBuild.slice(dashSplit + 1);
  return { core, prereleaseText, build };
}

/** The three core numbers, or the refusal reason as a string. */
function parseCore(core: string): [number, number, number] | string {
  const parts = core.split('.');
  const [major, minor, patch] = parts;
  if (parts.length !== 3 || major === undefined || minor === undefined || patch === undefined) {
    return 'the version core must be exactly major.minor.patch';
  }
  const numbers: number[] = [];
  for (const part of [major, minor, patch]) {
    const parsed = numericIdentifier(part);
    if (parsed === null) {
      return `core part \`${part}\` is not a numeric identifier without leading zeros`;
    }
    numbers.push(parsed);
  }
  const [first, second, third] = numbers;
  if (first === undefined || second === undefined || third === undefined) {
    return 'the version core must be exactly major.minor.patch';
  }
  return [first, second, third];
}

/** The classified prerelease identifiers, or the refusal reason as a string. */
function parsePrereleaseList(prereleaseText: string | null): PrereleaseIdent[] | string {
  if (prereleaseText === null) {
    return [];
  }
  const prerelease: PrereleaseIdent[] = [];
  for (const ident of prereleaseText.split('.')) {
    const classified = prereleaseIdentifier(ident);
    if (classified === null) {
      return `prerelease identifier \`${ident}\` is not spec-legal`;
    }
    prerelease.push(classified);
  }
  return prerelease;
}

/** `null` when the build metadata is legal, else the refusal reason. */
function validateBuild(build: string | null): string | null {
  if (build === null) {
    return null;
  }
  for (const ident of build.split('.')) {
    if (!/^[0-9A-Za-z-]+$/.test(ident)) {
      return `build identifier \`${ident}\` is not spec-legal`;
    }
  }
  return null;
}

/** A spec numeric identifier: digits only, no leading zero. */
function numericIdentifier(part: string): number | null {
  if (!/^(0|[1-9][0-9]*)$/.test(part)) {
    return null;
  }
  const value = Number(part);
  return Number.isSafeInteger(value) ? value : null;
}

/** Classify one prerelease identifier, or refuse it. */
function prereleaseIdentifier(ident: string): PrereleaseIdent | null {
  if (!/^[0-9A-Za-z-]+$/.test(ident)) {
    return null;
  }
  if (/^[0-9]+$/.test(ident)) {
    const value = numericIdentifier(ident);
    return value === null ? null : { kind: 'numeric', value };
  }
  return { kind: 'alphanumeric', value: ident };
}

/** Spec item 11 precedence: negative, zero, or positive as `a` <, ==, > `b`. */
export function compareSemver(a: SemVer, b: SemVer): number {
  if (a.major !== b.major) {
    return a.major - b.major;
  }
  if (a.minor !== b.minor) {
    return a.minor - b.minor;
  }
  if (a.patch !== b.patch) {
    return a.patch - b.patch;
  }
  return comparePrerelease(a.prerelease, b.prerelease);
}

/** Item 11.3/11.4: release > prerelease; identifiers pairwise; longer wins. */
function comparePrerelease(a: PrereleaseIdent[], b: PrereleaseIdent[]): number {
  if (a.length === 0 && b.length === 0) {
    return 0;
  }
  if (a.length === 0) {
    return 1;
  }
  if (b.length === 0) {
    return -1;
  }
  const shared = Math.min(a.length, b.length);
  for (let index = 0; index < shared; index += 1) {
    const left = a[index];
    const right = b[index];
    if (left === undefined || right === undefined) {
      break;
    }
    const ordering = compareIdent(left, right);
    if (ordering !== 0) {
      return ordering;
    }
  }
  return a.length - b.length;
}

/** Item 11.4: numeric < alphanumeric; numeric by value; alphanumeric ASCII. */
function compareIdent(a: PrereleaseIdent, b: PrereleaseIdent): number {
  if (a.kind === 'numeric' && b.kind === 'numeric') {
    return a.value - b.value;
  }
  if (a.kind === 'numeric') {
    return -1;
  }
  if (b.kind === 'numeric') {
    return 1;
  }
  return a.value < b.value ? -1 : a.value > b.value ? 1 : 0;
}

/** The pill's question, with malformation as its own visible outcome. */
export type UpdateComparison =
  | { state: 'newer' }
  | { state: 'not-newer' }
  | { state: 'malformed'; reason: string };

/**
 * Is `latest` an INSTALLABLE upgrade over `installed`? Either side failing to
 * parse yields `malformed` with the refusal — never a silent `not-newer`,
 * because a pill that hides itself on garbage input is indistinguishable from
 * a pill honestly reporting "up to date".
 *
 * A prerelease `latest` is never announced (r1 review, Major 1): a plain
 * `cargo install aion-cli` refuses prereleases, so `0.14.0-rc.1` against a
 * stable `0.13.7` would send the operator through the documented upgrade and
 * hand them their own version back. The server's index parse excludes
 * prereleases at the source (`crates/aion-server/src/update_check/index.rs`),
 * so a prerelease can only reach this comparison from an older server or a
 * hand-fed value — and this rule keeps the pill truthful even then.
 */
export function compareForUpdate(installed: string, latest: string): UpdateComparison {
  const installedParse = parseSemver(installed);
  if (!installedParse.ok) {
    return { state: 'malformed', reason: `installed version: ${installedParse.reason}` };
  }
  const latestParse = parseSemver(latest);
  if (!latestParse.ok) {
    return { state: 'malformed', reason: `latest known version: ${latestParse.reason}` };
  }
  if (latestParse.version.prerelease.length > 0) {
    return { state: 'not-newer' };
  }
  return compareSemver(latestParse.version, installedParse.version) > 0
    ? { state: 'newer' }
    : { state: 'not-newer' };
}
