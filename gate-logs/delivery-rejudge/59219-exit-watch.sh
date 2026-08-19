#!/bin/zsh
# Watch attempt-1 agent 59219 to natural exit (run 7649531a already terminal
# Failed 02:56:02Z; this agent is the salvage — worktree commits + envelope).
LOG=/Users/tom/Developer/ablative/stack/aion/gate-logs/delivery-rejudge/attempt-watch.log
echo "=== 59219 exit-watch armed $(date -u +%Y-%m-%dT%H:%M:%SZ) (run already terminal) ===" >> "$LOG"
while kill -0 59219 2>/dev/null; do /bin/sleep 60; done
echo "$(date -u +%H:%M:%SZ) PID-59219-EXITED (post-terminal)" >> "$LOG"
echo "attempt-1 agent pid 59219 exited"
