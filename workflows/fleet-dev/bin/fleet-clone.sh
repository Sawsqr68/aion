#!/bin/sh
# fleet-clone.sh <repo_url> <base_ref> <workspace_root> <run_name>
# Provision the run tree at the named ref. EXIT ZERO ALWAYS on anticipated
# faces; refusals are typed. A leftover root REFUSES — the dispatcher's to
# inspect, never a script's to delete in passing.
set -u
refuse() { jq -n --arg r "$1" '{ok:false,refusal:$r,root:"",repo_path:"",head_commit:""}'; exit 0; }
[ $# -eq 4 ] || refuse "fleet-clone.sh expects exactly 4 arguments, got $#"
repo_url=$1; base_ref=$2; workspace_root=$3; run_name=$4
[ "$workspace_root" = "/home/aion/fleet/runs" ] || refuse "workspace_root is not the provisioned fleet-runs root: $workspace_root"
[ -n "$run_name" ] || refuse "empty run_name"
case "$run_name" in *[!A-Za-z0-9-]*) refuse "run_name carries characters outside [A-Za-z0-9-]: $run_name" ;; esac
case "$base_ref" in ""|-*|*[!A-Za-z0-9._/-]*) refuse "base_ref shape refused: $base_ref" ;; esac
root="$workspace_root/$run_name"; repo="$root/repo"
[ -e "$root" ] && refuse "run root already exists: $root — a leftover from a collided dispatch, the dispatcher's to inspect"
mkdir -p "$root" || refuse "could not create run root $root"
err=$(git clone --quiet "$repo_url" "$repo" 2>&1) || { rm -rf "$root"; refuse "git clone failed: $err"; }
err=$(git -C "$repo" checkout --quiet "$base_ref" 2>&1) || refuse "checkout of $base_ref failed: $err — tree kept at $root"
git -C "$repo" config user.name "aion-fleet" || refuse "could not set repo-local git user.name"
git -C "$repo" config user.email "fleet@aion.local" || refuse "could not set repo-local git user.email"
head=$(git -C "$repo" rev-parse HEAD 2>&1) || refuse "clone landed but HEAD is unreadable: $head"
jq -n --arg root "$root" --arg p "$repo" --arg h "$head" '{ok:true,refusal:"",root:$root,repo_path:$p,head_commit:$h}'
