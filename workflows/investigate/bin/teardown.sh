#!/bin/sh
# teardown.sh <workspace_root> <workflow_id>
# Remove the run's tree. Deletion inherits every adjective of its
# justification: the root must be EXACTLY the provisioned fleet-runs root,
# the id must be shaped like an id, and the check GATES the rm in this same
# script — never a separate command. EXIT ZERO ALWAYS on anticipated faces.
set -u

refuse() {
    jq -n --arg r "$1" '{ok: false, removed: "", refusal: $r}'
    exit 0
}

[ $# -eq 2 ] || refuse "teardown.sh expects exactly 2 arguments, got $#"
workspace_root=$1
workflow_id=$2

[ "$workspace_root" = "/home/aion/fleet/runs" ] || refuse "workspace_root is not the provisioned fleet-runs root: $workspace_root — nothing outside it is this script's to delete"
[ -n "$workflow_id" ] || refuse "empty workflow_id"
case "$workflow_id" in
    *[!A-Za-z0-9-]*) refuse "workflow_id carries characters outside [A-Za-z0-9-]: $workflow_id" ;;
esac

tree="$workspace_root/$workflow_id"
[ -d "$tree" ] || refuse "tree already absent: $tree — the census sweeper's to reconcile, not this run's to invent"

rm_err=$(rm -rf "$tree" 2>&1) || refuse "rm failed: $rm_err"
[ -e "$tree" ] && refuse "rm reported success but $tree still exists"

jq -n --arg t "$tree" '{ok: true, removed: $t, refusal: ""}'
