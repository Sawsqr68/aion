/**
 * The console's ONE data-freshness cadence, stated once.
 *
 * `QUERY_STALE_TIME_MS` is the app-wide react-query `staleTime` (how long a
 * fetched answer is treated as current), and the update pill's poll interval
 * reuses the SAME number: a surface that must notice server-side change
 * without a user gesture re-asks exactly as often as the app considers an
 * answer fresh. One constant, two readers — a second cadence would be a
 * second truth to drift.
 */
export const QUERY_STALE_TIME_MS = 30_000;

/** App-wide react-query `gcTime`: how long an unused cache entry is kept. */
export const QUERY_GC_TIME_MS = 5 * 60_000;
