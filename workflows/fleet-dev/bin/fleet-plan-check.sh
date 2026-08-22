#!/bin/sh
# fleet-plan-check.sh <workspace_root> <run_name> <leg_ids_csv>
# The planner's answer is a claim; HEAD is the fact. Verifies .fleet/plan.md
# and every dispatch-fixed .fleet/legs/<id>.md exist non-empty AT HEAD and
# that no .fleet state is loose in the working tree (the blackboard is
# committed). Exit 1 means the plan is absent/incomplete; exit 3 means
# committed non-empty briefs exist, but not under every declared id.
set -u
. /home/aion/fleet/bin/fleet-lib.sh
[ $# -eq 3 ] || op_guard_fail "fleet-plan-check.sh expects exactly 3 arguments, got $#"
fleet_root_guard "$1" "$2"; csv_guard "$3"
repo="$1/$2/repo"
[ -d "$repo/.git" ] || op_guard_fail "no repo at $repo"
cd "$repo" || op_guard_fail "cannot enter $repo"

append_absence() {
    if [ -n "$absence" ]; then
        absence="$absence
$1"
    else
        absence="$1"
    fi
}

append_csv() {
    if [ -n "$1" ]; then printf '%s, %s' "$1" "$2"; else printf '%s' "$2"; fi
}

absence=""
declared_missing=""
found_ids=""
found_ids_csv=""
found_nonempty_csv=""
found_nonempty_count=0
plan_seen=0
plan_nonempty=0

if dirty=$(git status --porcelain -- .fleet 2>&1); then
    if [ -n "$dirty" ]; then
        append_absence "uncommitted .fleet state (the blackboard is committed): $dirty"
    fi
else
    append_absence "git status failed while checking committed-plan cleanliness: $dirty"
fi

if tree_listing=$(git ls-tree -r --name-only HEAD -- .fleet/plan.md .fleet/legs/ 2>&1); then
    old_ifs=$IFS
    IFS='
'
    for path in $tree_listing; do
        case "$path" in
            .fleet/plan.md)
                plan_seen=1
                if size=$(git cat-file -s "HEAD:$path" 2>&1); then
                    case "$size" in
                        *[!0-9]*|'') append_absence "git returned a non-numeric size for $path at HEAD: $size" ;;
                        0) ;;
                        *) plan_nonempty=1 ;;
                    esac
                else
                    append_absence "git could not inspect $path at HEAD: $size"
                fi
                ;;
            .fleet/legs/*.md)
                id=${path#.fleet/legs/}
                id=${id%.md}
                case "$id" in
                    */*|'') continue ;;
                esac
                found_ids=$(append_csv "$found_ids" "$id")
                if [ -n "$found_ids_csv" ]; then
                    found_ids_csv="$found_ids_csv,$id"
                else
                    found_ids_csv="$id"
                fi
                if size=$(git cat-file -s "HEAD:$path" 2>&1); then
                    case "$size" in
                        *[!0-9]*|'') append_absence "git returned a non-numeric size for $path at HEAD: $size" ;;
                        0) ;;
                        *)
                            if [ -n "$found_nonempty_csv" ]; then
                                found_nonempty_csv="$found_nonempty_csv,$id"
                            else
                                found_nonempty_csv="$id"
                            fi
                            found_nonempty_count=$((found_nonempty_count + 1))
                            ;;
                    esac
                else
                    append_absence "git could not inspect $path at HEAD: $size"
                fi
                ;;
        esac
    done
    IFS=$old_ifs
else
    append_absence "git ls-tree failed while enumerating the committed plan at HEAD: $tree_listing"
fi

[ "$plan_seen" -eq 1 ] && [ "$plan_nonempty" -eq 1 ] || append_absence ".fleet/plan.md absent or empty at HEAD"

for id in $(printf '%s' "$3" | tr ',' ' '); do
    case ",$found_ids_csv," in
        *",$id,"*)
            case ",$found_nonempty_csv," in
                *",$id,"*) ;;
                *) append_absence ".fleet/legs/$id.md is empty at HEAD" ;;
            esac
            ;;
        *) declared_missing=$(append_csv "$declared_missing" "$id") ;;
    esac
done

if [ -n "$absence" ]; then
    if [ -n "$declared_missing" ]; then
        append_absence "declared brief files absent at HEAD: $declared_missing"
    fi
    op_emit 1 "$absence" ""
fi

if [ -n "$declared_missing" ] && [ "$found_nonempty_count" -gt 0 ]; then
    declared=$(printf '%s' "$3" | sed 's/,/, /g')
    op_emit 3 "PLAN MISMATCH: leg ids are fixed at dispatch; write briefs under the declared names, do not re-plan.
DECLARED: $declared
FOUND: $found_ids
MISSING DECLARED: $declared_missing" ""
fi

if [ -n "$declared_missing" ]; then
    op_emit 1 "no committed non-empty leg briefs establish a naming mismatch.
Declared brief files absent at HEAD: $declared_missing" ""
fi

op_emit 0 "plan and all dispatch-fixed leg briefs committed at HEAD" ""
