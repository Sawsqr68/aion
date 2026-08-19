#!/bin/zsh
# norn_dev workflow dispatch: #246 adoption-at-recovery (brief RATIFIED
# 01:59Z, same-worker amendment 03:24Z; #254 folded in — floors 4-6 +
# Red A2). PRE-STAGED 08:4xZ; FIRE ONLY after Waffles' K9 quiet-box
# battery + #244-rem completion + his landings clear (his wall ruling).
# FRESH session, brief self-contained (lesson 08:35Z: never resume a
# near-full session — compaction loses the brief's walls).
set -eu
umask 077

REPO=/Users/tom/Developer/ablative/stack/aion
RIG=$REPO/gate-logs/246-adoption
WT=$REPO/.worktrees/246-adoption

if [[ ! -d "$WT" ]]; then
  git -C "$REPO" worktree add "$WT" -b 246-adoption main || exit 1
fi

/usr/bin/python3 "$RIG/build-input.py" "$WT" > "$RIG/input.json"
aion start norn_dev \
  --input-file "$RIG/input.json" \
  --name "246-adoption" \
  > "$RIG/start-response.json"
date -u +%Y-%m-%dT%H:%M:%SZ > "$RIG/DISPATCHED.marker"

cat "$RIG/start-response.json"
