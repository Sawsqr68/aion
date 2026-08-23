#!/bin/sh
# fleet-plan-check.sh <workspace_root> <run_name> <leg_ids_csv>
# The planner's answer is a claim; HEAD is the fact. Verifies .fleet/plan.md
# and every dispatch-fixed .fleet/legs/<id>.md exist non-empty AT HEAD and
# that no .fleet state is loose in the working tree (the blackboard is
# committed). Exit 1 means the plan is absent/incomplete; exit 3 means
# committed non-empty briefs exist, but not under every declared id.
#
# STAGED-CHAIN EXTENSION (sequential-legs Resolution D, design section 5).
# When -- and ONLY when -- the committed plan carries a `fleet-stages-v1`
# block, this script additionally validates the staged-chain graph and can
# exit 4 (PLAN GRAPH INVALID) with a stable cause code. THE LEGACY PATH IS
# UNTOUCHED: a plan WITHOUT a block reaches exactly the same `op_emit 0`
# statement, with the same bytes, that it reached before this extension --
# design section 7 clause 5, "a plan without a `fleet-stages-v1` block
# continues to run only through the existing `fleet_dev` entry point". The
# census proves zero such blocks exist today, so every plan that exists
# today takes a bit-identical path. Exits 1 and 3 keep their precedence:
# both are decided, and emitted, before the block is even looked for.
set -u
# Root indirection so the sealed scripts can be exercised off the fleet box
# by a regression harness. The default IS the provisioned path, so nothing
# about a real run changes.
FLEET_HOME=${FLEET_HOME:-/home/aion/fleet}
. "$FLEET_HOME/bin/fleet-lib.sh"
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

# ---------------------------------------------------------------------------
# Everything above is the pre-existing checker, unchanged. Control only
# reaches here when the legacy checker would have emitted exit 0.
#
# The staged-chain arm. `plan_text` is read from the COMMITTED tree object,
# never the working tree -- the same `git cat-file` mechanism section 5.1
# names. An absent block falls straight through to the legacy exit 0 below.
plan_blob=$(git cat-file blob "HEAD:.fleet/plan.md" 2>/dev/null) || plan_blob=""
# ANY `fleet-stages-v*` fence is a CLAIM on this contract, so any of them
# reaches the validator; the validator refuses an unsupported version BY
# NAME rather than letting a v2 block be silently read as prose.
case "$plan_blob" in
    *'```fleet-stages-v'*)
        validator="$FLEET_HOME/bin/aion-fleet-plan"
        [ -x "$validator" ] || op_guard_fail "the committed plan carries a staged-chain block but the validator is not executable at $validator"
        block_plan=$(mktemp) || op_guard_fail "cannot create a temporary file for the committed plan"
        printf '%s\n' "$plan_blob" > "$block_plan" || { rm -f "$block_plan"; op_guard_fail "cannot stage the committed plan for validation"; }
        chain_out=$("$validator" check-plan --plan-file "$block_plan" --legs "$3" 2>&1)
        chain_rc=$?
        rm -f "$block_plan"
        case "$chain_rc" in
            0) op_emit 0 "$chain_out" "" ;;
            4) op_emit 4 "PLAN GRAPH INVALID
$chain_out" "" ;;
            1) op_emit 1 "$chain_out" "" ;;
            *) op_guard_fail "the staged-chain validator exited $chain_rc: $chain_out" ;;
        esac
        ;;
esac

op_emit 0 "plan and all dispatch-fixed leg briefs committed at HEAD" ""
