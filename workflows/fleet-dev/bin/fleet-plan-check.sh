#!/bin/sh
# fleet-plan-check.sh <workspace_root> <run_name> <leg_ids_csv>
# The planner's answer is a claim; HEAD is the fact. Verifies .fleet/plan.md
# and every .fleet/legs/<id>.md exist non-empty AT HEAD and that no .fleet
# state is loose in the working tree (the blackboard is committed).
set -u
. /home/aion/fleet/bin/fleet-lib.sh
[ $# -eq 3 ] || op_guard_fail "fleet-plan-check.sh expects exactly 3 arguments, got $#"
fleet_root_guard "$1" "$2"; csv_guard "$3"
repo="$1/$2/repo"
[ -d "$repo/.git" ] || op_guard_fail "no repo at $repo"
cd "$repo" || op_guard_fail "cannot enter $repo"
missing=""
dirty=$(git status --porcelain -- .fleet 2>&1)
[ -n "$dirty" ] && missing="uncommitted .fleet state (the blackboard is committed): $dirty"
if ! git cat-file -e HEAD:.fleet/plan.md 2>/dev/null || [ "$(git cat-file -s HEAD:.fleet/plan.md 2>/dev/null || echo 0)" -eq 0 ]; then
    missing="$missing
.fleet/plan.md absent or empty at HEAD"
fi
for id in $(printf '%s' "$3" | tr ',' ' '); do
    if ! git cat-file -e "HEAD:.fleet/legs/$id.md" 2>/dev/null || [ "$(git cat-file -s "HEAD:.fleet/legs/$id.md" 2>/dev/null || echo 0)" -eq 0 ]; then
        missing="$missing
.fleet/legs/$id.md absent or empty at HEAD"
    fi
done
if [ -n "$missing" ]; then op_emit 1 "$missing" ""; else op_emit 0 "plan and all leg briefs committed at HEAD" ""; fi
