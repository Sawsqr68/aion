#!/bin/sh
# clone.sh <repo_url> <workspace_root> <workflow_id>
# Clone to <workspace_root>/<workflow_id>/repo. EXIT ZERO ALWAYS on
# anticipated faces: every refusal is typed JSON with its reason named,
# because the workflow routes on these, never crashes on them. A leftover
# destination REFUSES — it is a fact to surface, never to delete in passing.
set -u

refuse() {
    jq -n --arg r "$1" '{ok: false, refusal: $r, path: "", head_commit: "", clone_bytes: 0}'
    exit 0
}

[ $# -eq 3 ] || refuse "clone.sh expects exactly 3 arguments, got $#"
repo_url=$1
workspace_root=$2
workflow_id=$3
[ -n "$repo_url" ] || refuse "empty repo_url"
[ -n "$workspace_root" ] || refuse "empty workspace_root"
[ -n "$workflow_id" ] || refuse "empty workflow_id"
case "$workflow_id" in
    *[!A-Za-z0-9-]*) refuse "workflow_id carries characters outside [A-Za-z0-9-]: $workflow_id" ;;
esac

dest="$workspace_root/$workflow_id/repo"
[ -e "$dest" ] && refuse "destination already exists: $dest — a leftover tree from a collided dispatch, the dispatcher's to inspect"

mkdir -p "$workspace_root/$workflow_id" || refuse "could not create run directory $workspace_root/$workflow_id"

clone_err=$(git clone --quiet "$repo_url" "$dest" 2>&1)
if [ $? -ne 0 ]; then
    rm -rf "$dest"
    refuse "git clone failed: $clone_err"
fi

head_commit=$(git -C "$dest" rev-parse HEAD 2>&1) || refuse "clone landed but HEAD is unreadable: $head_commit"
# POSIX du: -k is kibibytes everywhere; -b is GNU-only and absent on BSD/mac.
clone_kib=$(du -sk "$dest" | cut -f1)
clone_bytes=$((clone_kib * 1024))

jq -n --arg p "$dest" --arg h "$head_commit" --argjson b "$clone_bytes" \
    '{ok: true, refusal: "", path: $p, head_commit: $h, clone_bytes: $b}'
