#!/bin/zsh
# norn_dev workflow dispatch: libsql-retirement (brief RATIFIED 09:09:16Z,
# lane response 10283385 — 3 forks as recommended + 3 amendments law).
# PRE-STAGED 09:1xZ; FIRE ONLY after #247 lands (ratified shelf order
# #246 → #247 → libsql-retirement) and Waffles' walls stay clear.
# FRESH session, self-contained brief (the K9 lesson). Worktree is
# created HERE at fire time (not pre-staged — no box IO during his
# battery). Real ticket number minted at dispatch per Amendment 1:
# record it in BRIEF.md header before firing.
set -eu
umask 077

REPO=/Users/tom/Developer/ablative/stack/aion
RIG=$REPO/gate-logs/libsql-retirement
WT=$REPO/.worktrees/libsql-retirement

if [[ ! -d "$WT" ]]; then
  git -C "$REPO" worktree add "$WT" -b libsql-retirement main || exit 1
fi

/usr/bin/python3 "$RIG/build-input.py" "$WT" > "$RIG/input.json"
aion start norn_dev \
  --input-file "$RIG/input.json" \
  --name "libsql-retirement" \
  > "$RIG/start-response.json"
date -u +%Y-%m-%dT%H:%M:%SZ > "$RIG/DISPATCHED.marker"

cat "$RIG/start-response.json"
