#!/bin/zsh
cd /Users/deanwhiting/Developer/stack/aion
echo "TREE=" > /tmp/battery-0140/MANIFEST.txt
echo "START=2026-08-11T11:47:10+0000" >> /tmp/battery-0140/MANIFEST.txt
run_leg() {
  local name=""; shift
  echo "argv: " > /tmp/battery-0140/.log
  echo "cwd: /Users/deanwhiting  tree:   date: 2026-08-11T11:47:10+0000" >> /tmp/battery-0140/.log
  echo "# ---" >> /tmp/battery-0140/.log
  "" >> /tmp/battery-0140/.log 2>&1
  local code=0
  echo "# ---" >> /tmp/battery-0140/.log
  echo "EXIT=" >> /tmp/battery-0140/.log
  echo " EXIT=" >> /tmp/battery-0140/MANIFEST.txt
}
run_leg fmt-check cargo fmt --check
run_leg clippy-workspace cargo clippy --workspace --all-targets -- -D warnings
run_leg clippy-percrate cargo clippy -p aion-cli -p aion-server -p aion-awl -p aion-worker --all-targets -- -D warnings
run_leg test-workspace cargo test --workspace
echo "END=2026-08-11T11:47:10+0000" >> /tmp/battery-0140/MANIFEST.txt
touch /tmp/battery-0140/DONE
