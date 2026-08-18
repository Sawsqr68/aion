#!/bin/sh
# fleet-fanout.sh <workspace_root> <run_name> <leg_ids_csv>
# One worktree + one branch per leg at <root>/legs/<id>, cut from the
# planned HEAD. Committed state rides in for free. Refuses loudly on
# anything in its way; never deletes.
set -u
. /home/aion/fleet/bin/fleet-lib.sh
[ $# -eq 3 ] || op_guard_fail "fleet-fanout.sh expects exactly 3 arguments, got $#"
fleet_root_guard "$1" "$2"; csv_guard "$3"
root="$1/$2"; repo="$root/repo"
[ -d "$repo/.git" ] || op_guard_fail "no repo at $repo"
cut=""
for id in $(printf '%s' "$3" | tr ',' ' '); do
    dest="$root/legs/$id"
    [ -e "$dest" ] && op_emit 1 "leg worktree already exists: $dest — a leftover, the dispatcher's to inspect. Cut so far:$cut" ""
    out=$(git -C "$repo" worktree add "$dest" -b "leg/$id" 2>&1) || op_emit 1 "worktree cut failed for leg $id: $out. Cut so far:$cut" ""
    cut="$cut $id"
done
op_emit 0 "cut:$cut" ""
