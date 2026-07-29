#!/bin/sh
# Reconcile a nextest run's arithmetic, and VOID the result if it does not add up.
#
# A pinned test denominator that nothing mechanically compares is an
# instruction, not a control. A test that STARTS and never resolves — killed
# by a signal, hung until the harness moved on, silently filtered between the
# start line and the summary — simply vanishes from the count, and the run
# still prints a green summary. Every published green is then one absence away
# from being wrong.
#
# This checks the run against itself three ways:
#   1. the summary's "N tests run" equals the start line's "Starting N tests";
#   2. that same N equals passed + failed as the summary reports them;
#   3. that same N equals the number of per-test result lines actually emitted,
#      which is the check that catches a vanished test — a test that never
#      resolves prints no result line, so this count comes up short even when
#      the summary looks consistent with itself.
#
# Usage: scripts/reconcile-nextest.sh <nextest-output-log>
# Exit:  0 reconciled, 1 VOID (with the discrepancy named), 2 unusable input.

set -eu

if [ $# -ne 1 ]; then
    echo "usage: $0 <nextest-output-log>" >&2
    exit 2
fi
log=$1
if [ ! -r "$log" ]; then
    echo "VOID: cannot read '$log'" >&2
    exit 2
fi

field() {
    # field <regex> <log> -> the LAST match, or empty
    grep -oE "$1" "$2" | tail -1
}

started=$(field 'Starting [0-9]+ tests' "$log" | grep -oE '[0-9]+' || true)
summary=$(field 'Summary \[[^]]*\] [0-9]+ tests run: [0-9]+ passed' "$log" || true)

if [ -z "$started" ] || [ -z "$summary" ]; then
    echo "VOID: '$log' has no nextest start line and/or summary line — a run that" >&2
    echo "      did not report cannot be reconciled, and must not be cited." >&2
    exit 1
fi

ran=$(echo "$summary" | sed -n 's/.*] \([0-9][0-9]*\) tests run.*/\1/p')
passed=$(echo "$summary" | sed -n 's/.*run: \([0-9][0-9]*\) passed.*/\1/p')
failed=$(field '[0-9]+ failed' "$log" | grep -oE '^[0-9]+' || true)
[ -n "$failed" ] || failed=0

# Distinct tests that reached a verdict. Every terminal nextest verdict prints
# a line here; a test that started and never resolved prints none.
#
# Keyed on the trailing "<binary> <test-name>" pair rather than counted raw,
# for two reasons found by running this against a real red log:
#   - nextest prints each FAILURE twice, once inline at its progress position
#     and once in the end-of-run recap, so a raw count over-reports by exactly
#     the failure count and would VOID every red run for the wrong reason;
#   - the same test NAME legitimately appears in several binaries, so keying on
#     the name alone would under-report and VOID a perfectly good green.
results=$(
    grep -E '^[[:space:]]+(PASS|FAIL|TIMEOUT|SIGSEGV|SIGABRT|ABORT|LEAK)[[:space:]]' "$log" |
        awk '{ print $(NF - 1), $NF }' |
        sort -u |
        wc -l |
        tr -d ' '
)
[ -n "$results" ] || results=0

status=0
if [ "$started" -ne "$ran" ]; then
    echo "VOID: $started tests started but the summary reports $ran run —" >&2
    echo "      $((started - ran)) test(s) vanished between start and summary." >&2
    status=1
fi
if [ "$ran" -ne $((passed + failed)) ]; then
    echo "VOID: $ran run != $passed passed + $failed failed." >&2
    status=1
fi
if [ "$results" -ne "$ran" ]; then
    echo "VOID: $ran tests reported run but $results per-test result lines were" >&2
    echo "      emitted — a test that starts and never resolves prints no result." >&2
    status=1
fi

if [ "$status" -eq 0 ]; then
    echo "reconciled: $started started == $ran run == $passed passed + $failed failed"
    echo "            == $results per-test result lines emitted"
fi
exit "$status"
