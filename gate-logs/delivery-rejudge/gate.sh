# Gate script — manifold #207 delivery-verdict re-judge (single dev run).
# Run by the aion server via norn_dev's `gates` action: `zsh $script`.
# The package's gate: the full Rust battery on the worktree. R5's red-first
# test and the two-process re-judge proof live INSIDE the battery (the brief
# demands they be tests the battery runs), so this one exit code measures
# them mechanically. Waffles' desk re-runs the battery and does the live
# two-process walk on a scratch estate before landing — this gate refuses
# early, it does not replace his.
WT=/Users/tom/Developer/ablative/apps/manifold/.worktrees/delivery-rejudge
LOGS=$WT/gate-logs/delivery-rejudge
mkdir -p "$LOGS" || { echo "gate setup FAILED: cannot create log directory $LOGS" >&2; exit 90; }
cd "$WT" || { echo "gate setup FAILED: cannot enter worktree $WT" >&2; exit 91; }

cargo test --workspace > "$LOGS/gate-rust.log" 2>&1
rust_exit=$?
echo "EXIT=$rust_exit" >> "$LOGS/gate-rust.log"

echo "rust=$rust_exit"
if [ "$rust_exit" -ne 0 ]; then
  {
    echo "GATE RED — 1 of 1 legs failed:"
    echo "  rust exited $rust_exit  (full output: $LOGS/gate-rust.log)"
    echo "log directory: $LOGS"
  } >&2
fi
exit $rust_exit
