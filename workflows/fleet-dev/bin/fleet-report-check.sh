#!/bin/sh
# fleet-report-check.sh <workspace_root> <run_name> <leg_ids_csv>
# A builder's answer is a claim; the merged HEAD is the fact. Every
# .fleet/reports/<id>.md must be committed non-empty in the merged tree.
set -u
. /home/aion/fleet/bin/fleet-lib.sh
[ $# -eq 3 ] || op_guard_fail "fleet-report-check.sh expects exactly 3 arguments, got $#"
fleet_root_guard "$1" "$2"; csv_guard "$3"
repo="$1/$2/repo"
[ -d "$repo/.git" ] || op_guard_fail "no repo at $repo"
cd "$repo" || op_guard_fail "cannot enter $repo"
missing=""
for id in $(printf '%s' "$3" | tr ',' ' '); do
    if ! git cat-file -e "HEAD:.fleet/reports/$id.md" 2>/dev/null || [ "$(git cat-file -s "HEAD:.fleet/reports/$id.md" 2>/dev/null || echo 0)" -eq 0 ]; then
        missing="$missing
.fleet/reports/$id.md absent or empty in the merged tree"
    fi
done
if [ -n "$missing" ]; then op_emit 1 "$missing" ""; else op_emit 0 "all leg reports committed in the merged tree" ""; fi
