/**
 * Test-run preload (wired via bunfig.toml `[test].preload`): pins the process
 * to a named timezone BEFORE any test file loads or constructs a `Date`, so
 * viewer-local timestamp renderings (issue #212) are machine-independent under
 * `bun test`. The console itself never pins a zone or locale — the viewer's
 * system defaults are the feature; only the test run is pinned.
 */
process.env.TZ = 'Australia/Sydney';

// Exact-string timestamp assertions that render through components (no
// `locales` option reaches Intl there) are calibrated against the 'en-US' ICU
// default. A runner resolving any other locale would fail those assertions
// with misleading diffs — refuse loudly up front instead.
const resolvedLocale = new Intl.DateTimeFormat().resolvedOptions().locale;
if (resolvedLocale !== 'en-US') {
  throw new Error(
    `test-setup: timestamp assertions are calibrated for the 'en-US' default locale, ` +
      `but this runner resolved '${resolvedLocale}' — pin the runner's locale to en-US ` +
      `or re-calibrate the exact-string expectations`
  );
}
