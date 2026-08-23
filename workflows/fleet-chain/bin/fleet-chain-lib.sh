#!/bin/sh
# fleet-chain-lib.sh — shared guards for the staged-chain scripts. Sourced,
# not executed.
#
# Follows the established law of `workflows/fleet-dev/bin/fleet-lib.sh`:
# EXIT ZERO ALWAYS, INNER CODE AS DATA. Under a worker `run json` action a
# nonzero exit is an activity FAILURE and its stdout is DISCARDED, so every
# anticipated face here exits zero and carries its status back as typed
# data. `chain_guard_fail` is the script-face refusal, distinct from any
# inner command's own codes.
#
# `fleet-dev/bin/fleet-lib.sh` is NOT sourced and NOT modified: these are
# new scripts for a new document, and the two guard families compose
# without either widening the other.
FLEET_HOME=${FLEET_HOME:-/home/aion/fleet}

# A script-face refusal: halt the chain with a named refusal, as DATA.
chain_guard_fail() {
    jq -n --arg r "$1" '{halted:true,verdict:"chain_refused",stage_id:"",child_workflow_id:"",base_commit:"",result_commit:"",accepted_commit:"",conclusion:"",cause:$r,cause_code:"chain_guard",observed_bytes:"",kept_root:"",last_accepted_commit:"",ledger:""}'
    exit 0
}

# Emit a ChainState verbatim from a JSON document on stdin.
chain_emit() { cat; exit 0; }

# The provisioned root, pinned exactly — the same shape as
# `fleet_root_guard`, and refused rather than defaulted.
chain_root_guard() {
    [ "$1" = "$FLEET_HOME/runs" ] || chain_guard_fail "workspace_root is not the provisioned fleet-runs root: $1"
    [ -n "$2" ] || chain_guard_fail "empty run_name"
    case "$2" in *[!A-Za-z0-9-]*) chain_guard_fail "run_name carries characters outside [A-Za-z0-9-]: $2" ;; esac
}

# The staged-chain binary. Refused, never assumed absent-and-skipped.
chain_binary() {
    b="$FLEET_HOME/bin/aion-fleet-chain-step"
    [ -x "$b" ] || chain_guard_fail "the staged-chain step binary is not executable at $b"
    printf '%s' "$b"
}
