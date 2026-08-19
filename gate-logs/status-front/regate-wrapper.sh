#!/bin/zsh
# Waffles re-gate wrapper: runs the supplied gate.sh detached, records pid + exit.
LOGS=/Users/tom/Developer/ablative/stack/aion/gate-logs/status-front
echo $$ > "$LOGS/regate.pid"
/bin/zsh "$LOGS/gate.sh" > "$LOGS/regate-summary.log" 2>&1
code=$?
echo "$code" > "$LOGS/regate.exit"
exit $code
