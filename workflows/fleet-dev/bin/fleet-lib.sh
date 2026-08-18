#!/bin/sh
# fleet-lib.sh — shared guards for the OpProbe-faced scripts. Sourced, not
# executed. op_guard_fail emits an OpProbe with exit_code 2 (script-face
# refusal, distinct from the inner command's own codes) and exits zero.
op_guard_fail() { jq -n --arg e "$1" '{exit_code:2,stdout:"",stderr:$e}'; exit 0; }
op_emit() { jq -n --argjson c "$1" --arg o "$2" --arg e "$3" '{exit_code:$c,stdout:$o,stderr:$e}'; exit 0; }
fleet_root_guard() {
    [ "$1" = "/home/aion/fleet/runs" ] || op_guard_fail "workspace_root is not the provisioned fleet-runs root: $1"
    [ -n "$2" ] || op_guard_fail "empty run_name"
    case "$2" in *[!A-Za-z0-9-]*) op_guard_fail "run_name carries characters outside [A-Za-z0-9-]: $2" ;; esac
}
csv_guard() {
    [ -n "$1" ] || op_guard_fail "empty leg_ids_csv"
    case "$1" in *[!A-Za-z0-9,-]*) op_guard_fail "leg_ids_csv carries characters outside [A-Za-z0-9,-]: $1" ;; esac
}
