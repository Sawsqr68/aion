#!/bin/zsh
# Watch #207's attempt-1 agent (pid 59219) and the workflow's history tail.
# Exits (waking the dispatcher) when EITHER:
#   - pid 59219 exits (attempt 1 finished or died), or
#   - the workflow history grows past the 01:25:01Z attempt-2 re-dispatch
#     (delegate completion landing, or an attempt-3 mint after the frozen
#     attempt-2's 90m timer fires ~02:55Z).
# Context: gate-logs/COMPACT-66-STATE.md header block "REDELIVERY FINDING".
WF=7649531a-817f-4168-bc2f-85b25e5ee8d6
LOG=/Users/tom/Developer/ablative/stack/aion/gate-logs/delivery-rejudge/attempt-watch.log
AION=/Users/tom/.cargo/bin/aion
baseline_count=$("$AION" describe "$WF" 2>/dev/null | python3 -c 'import json,sys; print(len(json.load(sys.stdin)["history"]))' 2>/dev/null)
echo "=== attempt-watch armed $(date -u +%Y-%m-%dT%H:%M:%SZ) pid=59219 baseline_events=${baseline_count} ===" >> "$LOG"
while true; do
  now=$(date -u +%H:%M:%SZ)
  if ! kill -0 59219 2>/dev/null; then
    echo "$now PID-59219-EXITED" >> "$LOG"
    echo "attempt-1 agent pid 59219 exited"
    exit 0
  fi
  count=$("$AION" describe "$WF" 2>/dev/null | python3 -c 'import json,sys; print(len(json.load(sys.stdin)["history"]))' 2>/dev/null)
  if [[ -n "$count" && -n "$baseline_count" && "$count" != "$baseline_count" ]]; then
    echo "$now HISTORY-ADVANCED ${baseline_count}->${count}" >> "$LOG"
    echo "history advanced ${baseline_count}->${count} while 59219 still alive"
    exit 0
  fi
  if [[ -z "$count" ]]; then
    echo "$now describe-failed" >> "$LOG"
  fi
  /bin/sleep 60
done
