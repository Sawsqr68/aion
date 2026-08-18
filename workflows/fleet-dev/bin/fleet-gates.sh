#!/bin/sh
# fleet-gates.sh <workspace_root> <run_name> <gate_entry>
# Run the repo's own gate entry; the inner exit code is the verdict, carried
# as data. On red, the findings are COMMITTED WHOLE to .fleet/gates/ —
# numbered history plus the stable RED-LATEST.md address the fix-round
# prompt names — so the dev seat reads the verbatim evidence from the tree.
# Payload streams are tails; the full log stays at <root>/gates-round-N.log.
set -u
. /home/aion/fleet/bin/fleet-lib.sh
[ $# -eq 3 ] || op_guard_fail "fleet-gates.sh expects exactly 3 arguments, got $#"
fleet_root_guard "$1" "$2"
gate_entry=$3
case "$gate_entry" in ""|/*|*..*) op_guard_fail "gate_entry must be a repo-relative path without dot-dot: $gate_entry" ;; esac
root="$1/$2"; repo="$root/repo"
[ -f "$repo/$gate_entry" ] || op_guard_fail "gate entry absent in tree: $gate_entry"
cd "$repo" || op_guard_fail "cannot enter $repo"
n=$(ls .fleet/gates/red-*.md 2>/dev/null | wc -l | tr -d ' '); n=$((n + 1))
log="$root/gates-round-$n.log"
sh "$gate_entry" > "$log" 2>&1
rc=$?
if [ "$rc" -ne 0 ]; then
    mkdir -p .fleet/gates || op_guard_fail "cannot create .fleet/gates"
    cp "$log" ".fleet/gates/red-$n.md" && cp "$log" ".fleet/gates/RED-LATEST.md" || op_guard_fail "cannot write findings into the tree"
    git add .fleet/gates && git commit -q -m "gates: red round $n — findings verbatim" || op_guard_fail "cannot commit gate findings"
fi
tail_out=$(tail -n 120 "$log")
op_emit "$rc" "$tail_out" "full log: $log"
