#!/bin/zsh
# norn_dev workflow dispatch: manifold #210 — attach path drives the
# ObserverBackpressure recovery (Waffles' YES 14:51Z 2026-08-15, slot
# math ratified: #246 + this = two; #247 takes #246's slot at landing).
# Worktree cut HERE at fire time off manifold main.
set -eu
umask 077

MANIFOLD=/Users/tom/Developer/ablative/apps/manifold
RIG=/Users/tom/Developer/ablative/stack/aion/gate-logs/m210-attach-backpressure
WT=$MANIFOLD/.worktrees/m210-attach-backpressure

if [[ ! -d "$WT" ]]; then
  git -C "$MANIFOLD" worktree add "$WT" -b m210-attach-backpressure main || exit 1
fi

/usr/bin/python3 "$RIG/build-input.py" "$WT" > "$RIG/input.json"
aion start norn_dev \
  --input-file "$RIG/input.json" \
  --name "m210-attach-backpressure" \
  > "$RIG/start-response.json"
date -u +%Y-%m-%dT%H:%M:%SZ > "$RIG/DISPATCHED.marker"

cat "$RIG/start-response.json"
