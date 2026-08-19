#!/bin/zsh
L=/Users/tom/Developer/ablative/stack/aion/gate-logs/status-front/waffles-main-control.log
W=/Users/tom/Developer/ablative/stack/aion/.worktrees/main-control
echo $$ > "${L%.log}.pid"
{ echo "=== CONTROL ARM: unmodified main 0bed60196 ==="; echo "HEAD=$(git -C $W rev-parse HEAD)"; echo "LOAD-BEFORE: $(uptime)"; } > "$L"
cd "$W" || { echo 90 > "${L%.log}.exit"; exit 90; }
for i in 1 2 3; do
  echo "--- run $i default ---" >> "$L"; cargo test -p aion-server --test namespace_restart >> "$L" 2>&1; echo "EXIT=$?" >> "$L"
  echo "--- run $i all-features ---" >> "$L"; cargo test -p aion-server --test namespace_restart --all-features >> "$L" 2>&1; echo "EXIT=$?" >> "$L"
done
echo "LOAD-AFTER: $(uptime)" >> "$L"
echo 0 > "${L%.log}.exit"
