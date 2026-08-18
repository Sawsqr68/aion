#!/bin/sh
# fleet-fanin.sh <workspace_root> <run_name> <leg_ids_csv>
# Ordered no-ff merges of leg branches into the primary tree. A conflict
# aborts clean, names the files, and exits AS DATA — a finding about the
# plan's leg boundaries.
set -u
. /home/aion/fleet/bin/fleet-lib.sh
[ $# -eq 3 ] || op_guard_fail "fleet-fanin.sh expects exactly 3 arguments, got $#"
fleet_root_guard "$1" "$2"; csv_guard "$3"
repo="$1/$2/repo"
[ -d "$repo/.git" ] || op_guard_fail "no repo at $repo"
cd "$repo" || op_guard_fail "cannot enter $repo"
dirty=$(git status --porcelain 2>&1)
[ -n "$dirty" ] && op_guard_fail "primary tree is not clean before fanin: $dirty"
merged=""
for id in $(printf '%s' "$3" | tr ',' ' '); do
    if ! out=$(git merge --no-ff "leg/$id" -m "fanin: leg $id" 2>&1); then
        conflicts=$(git diff --name-only --diff-filter=U 2>/dev/null)
        git merge --abort 2>/dev/null
        op_emit 1 "merge of leg/$id refused. Conflicting files: $conflicts. Merged so far:$merged. Raw: $out" ""
    fi
    merged="$merged $id"
done
op_emit 0 "merged in order:$merged" ""
