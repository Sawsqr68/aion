#!/bin/sh
# fleet-push.sh <workspace_root> <run_name> <result_branch>
# Deliver the merged branch home with a receipt. Refusals typed, never
# forced — a non-fast-forward is a fact about the remote, not an obstacle.
set -u
refuse() { jq -n --arg r "$1" '{ok:false,refusal:$r,branch:"",pushed_commit:"",remote:""}'; exit 0; }
[ $# -eq 3 ] || refuse "fleet-push.sh expects exactly 3 arguments, got $#"
workspace_root=$1; run_name=$2; result_branch=$3
[ "$workspace_root" = "/home/aion/fleet/runs" ] || refuse "workspace_root is not the provisioned fleet-runs root: $workspace_root"
case "$run_name" in ""|*[!A-Za-z0-9-]*) refuse "run_name shape refused: $run_name" ;; esac
case "$result_branch" in ""|-*|*[!A-Za-z0-9._/-]*) refuse "result_branch shape refused: $result_branch" ;; esac
repo="$workspace_root/$run_name/repo"
[ -d "$repo/.git" ] || refuse "no repo at $repo"
# The ops worker's declared-command executor passes ONLY PATH plus declared
# env (env_clear by design — determinism). git resolves the per-user
# credential config through HOME, so a push under the executor finds no
# helper and dies prompting: "could not read Username … Device not
# configured" (wire-go-1, 2026-08-23). Derive HOME from the passwd entry
# of the uid we run as and re-export it — the gate-entry standard's
# CARGO_HOME -> HOME -> passwd chain, applied at the limb that broke.
if [ -z "${HOME:-}" ]; then
    HOME=$(getent passwd "$(id -u)" 2>/dev/null | cut -d: -f6)
    [ -n "$HOME" ] || HOME=$(dscl . -read "/Users/$(id -un)" NFSHomeDirectory 2>/dev/null | awk '{print $2}')
    [ -n "$HOME" ] || refuse "cannot derive HOME for uid $(id -u): git credential config unreachable"
    export HOME
fi
out=$(git -C "$repo" push origin "HEAD:refs/heads/$result_branch" 2>&1) || refuse "git push refused: $out"
head=$(git -C "$repo" rev-parse HEAD 2>&1) || refuse "pushed but HEAD unreadable: $head"
remote=$(git -C "$repo" remote get-url origin 2>&1) || refuse "pushed but origin unreadable: $remote"
jq -n --arg b "$result_branch" --arg h "$head" --arg r "$remote" '{ok:true,refusal:"",branch:$b,pushed_commit:$h,remote:$r}'
