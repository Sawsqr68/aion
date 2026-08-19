#!/bin/sh
# fleet-teardown.sh <workspace_root> <run_name>
# Remove the run's whole tree (repo, leg worktrees, logs). The root must be
# EXACTLY the provisioned fleet-runs root, the name must be shaped like a
# name, and the check GATES the rm in this same script.
set -u
refuse() { jq -n --arg r "$1" '{ok:false,removed:"",refusal:$r}'; exit 0; }
[ $# -eq 2 ] || refuse "fleet-teardown.sh expects exactly 2 arguments, got $#"
workspace_root=$1; run_name=$2
[ "$workspace_root" = "/home/aion/fleet/runs" ] || refuse "workspace_root is not the provisioned fleet-runs root: $workspace_root — nothing outside it is this script's to delete"
[ -n "$run_name" ] || refuse "empty run_name"
case "$run_name" in *[!A-Za-z0-9-]*) refuse "run_name carries characters outside [A-Za-z0-9-]: $run_name" ;; esac
tree="$workspace_root/$run_name"
[ -d "$tree" ] || refuse "tree already absent: $tree — the census sweeper's to reconcile, not this run's to invent"
rm_err=$(rm -rf "$tree" 2>&1) || refuse "rm failed: $rm_err"
[ -e "$tree" ] && refuse "rm reported success but $tree still exists"
jq -n --arg t "$tree" '{ok:true,removed:$t,refusal:""}'
