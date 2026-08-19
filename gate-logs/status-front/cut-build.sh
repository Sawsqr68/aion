#!/bin/zsh
L=/Users/tom/Developer/ablative/stack/aion/gate-logs/status-front/cut-build.log
cd /Users/tom/Developer/ablative/stack/aion/.worktrees/release-build || { echo 90 > "${L%.log}.exit"; exit 90; }
{ echo "=== 0.16.0 cut build at $(git rev-parse --short HEAD) ==="; } > "$L"
cargo build --release -p aion-cli >> "$L" 2>&1
code=$?
echo "$code" > "${L%.log}.exit"
