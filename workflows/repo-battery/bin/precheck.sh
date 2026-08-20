#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=lib.sh
source "$SCRIPT_DIR/lib.sh"

if [[ $# -ne 2 ]]; then
  refuse_json "usage: precheck.sh WORKSPACE_ROOT WORKFLOW_ID"
  exit 0
fi

workspace_root=$1
workflow_id=$2
run_dir=$(run_root "$workspace_root" "$workflow_id")
tree=$(subject_tree "$workspace_root" "$workflow_id")
census_path="$run_dir/census.json"
result_path="$run_dir/precheck.json"

bash "$SCRIPT_DIR/census-declaration.sh" "$tree" >"$census_path"
if ! jq -e '.ok' "$census_path" >/dev/null; then
  jq -c '{ok:false, kind:.kind, refusal:.refusal, selector_path:.selector_path}' "$census_path" | tee "$result_path"
  exit 0
fi

venue=$(hostname)
refusal=""
toolchain="not-declared"
targets='[]'
if [[ -f "$tree/rust-toolchain.toml" ]]; then
  toolchain=$(awk -F'"' '/^[[:space:]]*channel[[:space:]]*=/{print $2; exit}' "$tree/rust-toolchain.toml")
  mapfile -t declared_targets < <(awk '
    /^[[:space:]]*targets[[:space:]]*=/ { inside=1 }
    inside { while (match($0, /"[^"]+"/)) { print substr($0, RSTART+1, RLENGTH-2); $0=substr($0, RSTART+RLENGTH) } }
    inside && /]/ { exit }
  ' "$tree/rust-toolchain.toml")
  targets=$(printf '%s\n' "${declared_targets[@]:-}" | jq -Rsc 'split("\n") | map(select(length > 0))')
  actual=$(rustc --version | awk '{print $2}')
  if [[ -n "$toolchain" && "$actual" != "$toolchain" ]]; then
    refusal="rust toolchain mismatch: tree requests $toolchain but venue rustc is $actual"
  fi
  rustup_path=$(command -v rustup || true)
  cargo_path=$(command -v cargo || true)
  rustup_dir=${rustup_path%/*}
  if [[ -z "$rustup_path" || "$cargo_path" != "$rustup_dir/cargo" ]]; then
    [[ -n "$refusal" ]] || refusal="PATH order refusal: rustup cargo shim is not first (resolved $cargo_path)"
  fi
  while IFS= read -r target; do
    if ! rustup target list --installed --toolchain "$toolchain" | grep -Fxq "$target"; then
      [[ -n "$refusal" ]] || refusal="rust target missing for $toolchain: $target"
    fi
  done < <(jq -r '.[]' <<<"$targets")
fi

if [[ -f "$tree/bun.lock" || -f "$tree/bun.lockb" ]]; then
  if [[ ! -d "$tree/node_modules" && -z "$refusal" ]]; then
    refusal="hydration refusal: repository documents bun dependencies but node_modules is absent; run bun install --frozen-lockfile outside the measurement"
  fi
fi

environment='[]'
runtime_environment='[]'
while IFS=$'\t' read -r leg name; do
  [[ -n "$name" ]] || continue
  if value=$(worker_env_value "$name"); then
    runtime_record=$(jq -cn --arg name "$name" --arg value "$value" '{name:$name, value:$value}')
    runtime_environment=$(jq -c --argjson record "$runtime_record" '. + [$record]' <<<"$runtime_environment")
    if secret_shaped "$name"; then
      digest=$(printf '%s' "$value" | sha256sum | awk '{print $1}')
      record=$(jq -cn --arg leg "$leg" --arg name "$name" --arg digest "$digest" \
        '{leg:$leg, name:$name, present:true, secret:true, value:"", digest:$digest}')
    else
      record=$(jq -cn --arg leg "$leg" --arg name "$name" --arg value "$value" \
        '{leg:$leg, name:$name, present:true, secret:false, value:$value, digest:""}')
    fi
  else
    record=$(jq -cn --arg leg "$leg" --arg name "$name" \
      '{leg:$leg, name:$name, present:false, secret:false, value:"", digest:""}')
    [[ -n "$refusal" ]] || refusal="declared environment variable is absent: $name"
  fi
  environment=$(jq -c --argjson record "$record" '. + [$record]' <<<"$environment")
done < <(jq -r '.environment_specs[] | [.leg, .name] | @tsv' "$census_path")
printf '%s\n' "$runtime_environment" >"$run_dir/runtime-environment.json"
chmod 600 "$run_dir/runtime-environment.json"

script_witnesses='[]'
while IFS= read -r script; do
  path=${script#"$tree/"}
  [[ "$path" != "$script" ]] || path="workflow/bin/${script##*/}"
  witness=$(jq -cn --arg path "$path" --arg sha256 "$(sha256_file "$script")" '{path:$path, sha256:$sha256}')
  script_witnesses=$(jq -c --argjson witness "$witness" '. + [$witness]' <<<"$script_witnesses")
done < <(printf '%s\n' \
  "$tree/scripts/battery-costly-announcements.sh" \
  "$tree/scripts/battery.sh" \
  "$tree/scripts/gates/lib.sh" \
  "$tree/scripts/gates/verify-declaration.sh" \
  "$SCRIPT_DIR"/*.sh | sort)

ok=true
kind="ready"
if [[ -n "$refusal" ]]; then ok=false; kind="venue_refusal"; fi
jq -cn \
  --argjson ok "$ok" --arg kind "$kind" --arg refusal "$refusal" \
  --arg selector_path "$(jq -r '.selector_path' "$census_path")" \
  --arg venue "$venue" --arg toolchain "$toolchain" \
  --argjson targets "$targets" --argjson environment "$environment" \
  --argjson script_witnesses "$script_witnesses" \
  '{ok:$ok, kind:$kind, refusal:$refusal, selector_path:$selector_path, venue:$venue, toolchain:$toolchain, targets:$targets, environment:$environment, script_witnesses:$script_witnesses}' | tee "$result_path"
