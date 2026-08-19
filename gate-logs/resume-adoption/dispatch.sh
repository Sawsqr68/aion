#!/bin/zsh
# norn_dev workflow dispatch: resume-adoption (brief r1-approved + 2 riders).
# GO given by Waffles 06:07Z 2026-08-17 — first dev run on the v0.17.0 binary.
# Pre-fire duties (executed at Vesper's hand before this script runs):
# STANDING-CONSTRAINTS.md check; server /build == 0.17.0 @ 168baa5f4
# dirty:false; workers lsof-ESTABLISHED to :50061; control_expect matches
# origin/main root Cargo.toml workspace version (0.17.0).
set -eu
umask 077

REPO=/Users/tom/Developer/ablative/stack/aion
RIG=$REPO/gate-logs/resume-adoption
WT=$REPO/.worktrees/resume-adoption

if [[ ! -d "$WT" ]]; then
  git -C "$REPO" worktree add "$WT" -b resume-adoption origin/main || exit 1
fi

/usr/bin/python3 "$RIG/build-input.py" "$WT" > "$RIG/input.json"
aion start norn_dev \
  --input-file "$RIG/input.json" \
  --name "resume-adoption" \
  > "$RIG/start-response.json"
date -u +%Y-%m-%dT%H:%M:%SZ > "$RIG/DISPATCHED.marker"

cat "$RIG/start-response.json"
