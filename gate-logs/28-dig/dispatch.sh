#!/bin/zsh
# norn_dev workflow dispatch: 28-dig (five-run wedge, invalid correlation id).
# RIG COMPLETE 2026-08-17 ~02:2xZ. FIRE ONLY on a freed heavy-run slot per
# Waffles' two-runs box law (02:18Z) and his sequencing word vs the K9 rig.
# Pre-fire duties: STANDING-CONSTRAINTS.md check; verify control_expect
# against the root Cargo.toml workspace version AT FIRE TIME; verify the
# box is not already carrying two heavy runs.
set -eu
umask 077

REPO=/Users/tom/Developer/ablative/stack/aion
RIG=$REPO/gate-logs/28-dig
WT=$REPO/.worktrees/28-dig

if [[ ! -d "$WT" ]]; then
  git -C "$REPO" worktree add "$WT" -b 28-dig main || exit 1
fi

/usr/bin/python3 "$RIG/build-input.py" "$WT" > "$RIG/input.json"
aion start norn_dev \
  --input-file "$RIG/input.json" \
  --name "28-dig" \
  > "$RIG/start-response.json"
date -u +%Y-%m-%dT%H:%M:%SZ > "$RIG/DISPATCHED.marker"

cat "$RIG/start-response.json"
