#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 2 ]]; then
  printf 'usage: %s PROOF_NUMBER ENGINE_ENDPOINT\n' "$0" >&2
  exit 2
fi
proof=$1
endpoint=$2
repo=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../../.." && pwd)
root=/tmp/repo-battery-live-proof-$proof
rm -rf "$root"
mkdir -p "$root"
fixture=green
case "$proof" in
  1) fixture=green ;;
  2) fixture=red ;;
  3) fixture=green ;;
  4) fixture=no-declaration ;;
  6) fixture=green ;;
  7) fixture=green ;;
  *) printf 'proof must be 1, 2, 3, 4, 6, or 7\n' >&2; exit 2 ;;
esac
"$repo/workflows/repo-battery/proofs/build-fixture.sh" "$fixture" "$repo" "$root/source"
if [[ "$proof" == 7 ]]; then
  jq '.legs[0].cmd = "sleep 60"' "$root/source/gates.json" >"$root/source/gates.tmp"
  mv "$root/source/gates.tmp" "$root/source/gates.json"
  git -C "$root/source" add gates.json
  git -C "$root/source" commit -qm 'proof: live cancellation leg'
fi
waivers='[]'
if [[ "$proof" == 6 ]]; then waivers='[{"leg":"fixture:true","reason":"malformed control","ruled_by":""}]'; fi
input=$(jq -cn --arg subject_repo "$root/source" --arg subject_ref HEAD --arg workspace_root "$root/work" --argjson waivers "$waivers" \
  '{subject_repo:$subject_repo,subject_ref:$subject_ref,workspace_root:$workspace_root,waivers:$waivers}')
worker_env=()
if [[ "$proof" == 3 ]]; then worker_env=(AION_BATTERY_ONLY=fixture:true); fi
env "${worker_env[@]}" aion worker awl --endpoint "$endpoint" --concurrency 1 \
  "$repo/workflows/repo-battery/repo_battery_worker.awl" >"$root/worker.log" 2>&1 &
worker_pid=$!
cleanup() { kill "$worker_pid" 2>/dev/null || true; wait "$worker_pid" 2>/dev/null || true; }
trap cleanup EXIT
sleep 2
start=$(aion start repo_battery --endpoint "$endpoint" --input "$input" --pretty)
printf '%s\n' "$start"
workflow_id=$(jq -r '.workflow_id // .id // empty' <<<"$start")
if [[ "$proof" == 7 ]]; then
  sleep 2
  aion cancel --endpoint "$endpoint" --reason 'proof 7 mid-battery cancellation' "$workflow_id" --pretty
fi
sleep 2
aion describe --endpoint "$endpoint" "$workflow_id" --pretty
