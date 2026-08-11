#!/bin/zsh -l
# 0.14.0 release battery — test-workspace leg run-5, --no-fail-fast.
# run-2 red = #85 flake (outbox_e2e::race_settles, API-misuse face).
# run-3 red = environmental fd limit 256 (436x os error 24) — cured by ulimit.
# run-4 red = #85 flake again (outbox_e2e::crash_recovery_rearms — DIFFERENT test,
#   same binary, same face; moving target = flake-shaped).
# run-5 runs every binary regardless, so one #85 hit cannot drop coverage.
set -o pipefail
ulimit -n 8192 || exit 91
cd /Users/deanwhiting/Developer/stack/aion || exit 90
LOG=/tmp/battery-0140/test-workspace-run5.log
{
  echo "# LEG=test-workspace RUN=5"
  echo "# ARGV=cargo test --workspace --no-fail-fast"
  echo "# CWD=$(pwd)"
  echo "# TREE=$(git rev-parse HEAD)"
  echo "# ULIMIT_N=$(ulimit -n)"
  echo "# START=$(date -u +%Y-%m-%dT%H:%M:%S%z)"
} > "$LOG"
cargo test --workspace --no-fail-fast >> "$LOG" 2>&1
EXIT=$?
echo "# EXIT=$EXIT" >> "$LOG"
echo "test-workspace-run5 EXIT=$EXIT" >> /tmp/battery-0140/MANIFEST.txt
echo "END5=$(date -u +%Y-%m-%dT%H:%M:%S%z)" >> /tmp/battery-0140/MANIFEST.txt
touch /tmp/battery-0140/DONE5
