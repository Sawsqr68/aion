# waffles-regate2 — corrected legs after regate1: graceful_shutdown_park_e2e belongs
# to aion-server (regate1's scoped leg died at target resolution, ran nothing), and
# the single outbox red needs solo attribution before it counts against the chain.
WT=/Users/tom/Developer/ablative/stack/aion/.worktrees/supersession-record
OUT=/Users/tom/Developer/ablative/stack/aion/gate-logs/266-defect-b
M=$OUT/waffles-regate2-MANIFEST.txt
cd "$WT" || { echo "cd failed" > "$M"; echo 90 > "$OUT/waffles-regate2.exit"; exit 90; }
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
leg scoped-e2e-aion cargo test -j 4 --manifest-path crates/aion/Cargo.toml --test recovery_supersession_e2e --test settled_fanout_e2e --test nested_workflows_e2e
leg gsp-e2e cargo test -j 4 --manifest-path crates/aion-server/Cargo.toml --test graceful_shutdown_park_e2e
leg outbox-solo1 cargo test -j 4 --manifest-path crates/aion-server/Cargo.toml --lib failed_dispatch_retries_with_backoff_and_bumps_attempt
leg outbox-solo2 cargo test -j 4 --manifest-path crates/aion-server/Cargo.toml --lib failed_dispatch_retries_with_backoff_and_bumps_attempt
leg outbox-solo3 cargo test -j 4 --manifest-path crates/aion-server/Cargo.toml --lib failed_dispatch_retries_with_backoff_and_bumps_attempt
leg test-aion-server-rerun cargo test -j 4 --manifest-path crates/aion-server/Cargo.toml
echo "finished=$(date +%Y-%m-%dT%H:%M:%S%z)" >> "$M"
echo "OVERALL=$overall" >> "$M"
echo $overall > "$OUT/waffles-regate2.exit"
exit $overall
