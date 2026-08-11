#!/bin/sh
# Run one gate into a log that records what was run and how it ended.
#
# m10, practised rather than claimed: a log of output alone cannot be re-run,
# and a suite that died before printing its verdict line looks identical to one
# that printed nothing on purpose. So every log opens with its exact argv and
# closes with the exit code.
#
#   sh slice1-logs/gate.sh <log-name> <command...>
set -u
LOG="slice1-logs/$1"
shift

{
    echo "# argv:  $*"
    echo "# cwd:   $(pwd)"
    echo "# tree:  $(git rev-parse HEAD)"
    echo "# date:  $(date '+%Y-%m-%dT%H:%M:%S%z')"
    echo "# ---"
} > "$LOG"

"$@" >> "$LOG" 2>&1
STATUS=$?
{
    echo "# ---"
    echo "# exit:  $STATUS"
} >> "$LOG"
echo "$1 -> exit=$STATUS  ($LOG)"
exit $STATUS
