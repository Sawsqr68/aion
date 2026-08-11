#!/bin/zsh
# Run one build/test leg with a hard disk floor.
#
# A suite killed by a full disk reads as red and costs a re-run. This starts the
# leg, then polls free space every 10s; if it falls below FLOOR_GIB the leg is
# killed at once and the script exits 99 with a HALT line, so the halt is never
# mistaken for a test failure.
#
# Usage: guarded.sh <logfile> <cargo args...>
set -u
FLOOR_GIB=8
LOG="$1"; shift
avail_gib() { echo $(( $(df -k / | tail -1 | awk '{print $4}') / 1024 / 1024 )); }
START=$(avail_gib)
echo "== leg start: avail=${START}Gi :: cargo $* ==" | tee "$LOG"
cargo "$@" >> "$LOG" 2>&1 &
LEG=$!
while kill -0 "$LEG" 2>/dev/null; do
  NOW=$(avail_gib)
  if [ "$NOW" -lt "$FLOOR_GIB" ]; then
    echo "HALT: avail=${NOW}Gi below ${FLOOR_GIB}Gi floor; killing this leg (NOT a test result)" | tee -a "$LOG"
    pkill -P "$LEG" 2>/dev/null
    kill -9 "$LEG" 2>/dev/null
    wait "$LEG" 2>/dev/null
    exit 99
  fi
  sleep 10
done
wait "$LEG"
CODE=$?
echo "== leg end: exit=${CODE} avail=$(avail_gib)Gi ==" | tee -a "$LOG"
exit "$CODE"
