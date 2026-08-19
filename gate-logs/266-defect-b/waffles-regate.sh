# waffles-regate — dispatcher's own gates on the FINAL tree of the #266+#33 chain.
# Mirrors Vesper's 33-battery4 manifest legs + adds fmt. Exit codes read directly,
# each leg recorded before the next runs. Never lands on an executor's pass-claim.
WT=/Users/tom/Developer/ablative/stack/aion/.worktrees/supersession-record
OUT=/Users/tom/Developer/ablative/stack/aion/gate-logs/266-defect-b
M=$OUT/waffles-regate-MANIFEST.txt
cd "$WT" || { echo "cd failed" > "$M"; echo 90 > "$OUT/waffles-regate.exit"; exit 90; }
echo "HEAD=$(git rev-parse HEAD)" > "$M"
echo "started=$(date +%Y-%m-%dT%H:%M:%S%z)" >> "$M"
overall=0
leg() {
  name=$1; shift
  echo "load_before[$name]={$(sysctl -n vm.loadavg | tr -d '{}')}" >> "$M"
  "$@" > "$OUT/waffles-$name.log" 2>&1
  rc=$?
  echo "exit[$name]=$rc" >> "$M"
  [ $rc -ne 0 ] && overall=1
}
leg fmt cargo fmt --all --check
leg scoped-e2e cargo test -j 4 --manifest-path crates/aion/Cargo.toml --test recovery_supersession_e2e --test settled_fanout_e2e --test nested_workflows_e2e --test graceful_shutdown_park_e2e
leg test-aion cargo test -j 4 --manifest-path crates/aion/Cargo.toml --lib --bins
leg test-aion-server cargo test -j 4 --manifest-path crates/aion-server/Cargo.toml
leg test-integrations cargo test -j 4 --manifest-path crates/aion-integrations/Cargo.toml
leg test-integration-norn cargo test -j 4 --manifest-path crates/aion-integration-norn/Cargo.toml
leg test-aion-worker cargo test -j 4 --manifest-path crates/aion-worker/Cargo.toml
leg clippy-workspace cargo clippy -j 4 --workspace --all-targets -- -D warnings
echo "finished=$(date +%Y-%m-%dT%H:%M:%S%z)" >> "$M"
echo "OVERALL=$overall" >> "$M"
echo $overall > "$OUT/waffles-regate.exit"
exit $overall
