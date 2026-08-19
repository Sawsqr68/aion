#!/bin/zsh
# norn_dev workflow dispatch: #247 transcript spool (brief RATIFIED
# 07:49:48Z — fork cut: v1 accepts worker-death loss, server-authored
# window-only gap event; 3 amendments law). PRE-STAGED 08:4xZ; FIRE
# ONLY after #246 lands (ratified queue #246 → #247) and Waffles'
# walls stay clear. FRESH session, brief self-contained (lesson
# 08:35Z: never resume a near-full session).
set -eu
umask 077

REPO=/Users/tom/Developer/ablative/stack/aion
RIG=$REPO/gate-logs/247-transcript-spool
WT=$REPO/.worktrees/247-transcript-spool

if [[ ! -d "$WT" ]]; then
  git -C "$REPO" worktree add "$WT" -b 247-transcript-spool main || exit 1
fi

/usr/bin/python3 "$RIG/build-input.py" "$WT" > "$RIG/input.json"
aion start norn_dev \
  --input-file "$RIG/input.json" \
  --name "247-transcript-spool" \
  > "$RIG/start-response.json"
date -u +%Y-%m-%dT%H:%M:%SZ > "$RIG/DISPATCHED.marker"

cat "$RIG/start-response.json"
