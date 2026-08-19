#!/bin/zsh
# Waffles: pre-warm the release binary at the final CODE state (e790ada74) in
# parallel with the amendment run's gates. The amendment is test-only, so this
# target is byte-valid for the cut; the version-bump commit at cut time costs
# only a relink. Detached, pid+exit files.
LOGS=/Users/tom/Developer/ablative/stack/aion/gate-logs/status-front
echo $$ > "$LOGS/prebuild.pid"
cd /Users/tom/Developer/ablative/stack/aion/.worktrees/release-build || { echo 90 > "$LOGS/prebuild.exit"; exit 90; }
cargo build --release -p aion-cli > "$LOGS/prebuild.log" 2>&1
code=$?
echo "$code" > "$LOGS/prebuild.exit"
exit $code
