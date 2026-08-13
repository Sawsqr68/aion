#!/usr/bin/env bash
# Publish the Aion crates to crates.io.
#
# The publish ORDER and the publish SET both come from cargo, derived from the
# workspace's own dependency graph at the moment of the run. Neither is written
# down here.
#
# This script used to carry a hand-maintained `PUBLISH_ORDER` array and a
# validator that checked that array was topologically sound. The validator could
# only ever consult the crates already ON the list, so it could not see a crate
# MISSING from it — and by 2026-08-13 eleven of the twenty-two publishable
# members were missing (aion-awl, aion-awl-lsp, aion-awl-package, aion-mcp,
# aion-integrations, aion-integration-{acp,cli,norn}, aion-proto-generated,
# aion-store-haematite, aion-toolchain). A run would have reported success
# having shipped half a release. The list is gone rather than corrected: a
# second copy of the dependency graph is a thing that drifts, and cargo already
# holds the first.
set -euo pipefail

readonly SCRIPT_NAME="$(basename "$0")"

usage() {
  cat <<USAGE
Usage: ${SCRIPT_NAME} [--live] [--help]

Publishes every publishable crate in the Aion workspace. Cargo resolves the
leaf-first order from the workspace dependency graph; crates marked
\`publish = false\` are excluded by cargo itself.

By default this performs a dry run:
  cargo publish --workspace --dry-run

Options:
  --live    publish for real with cargo publish --workspace
  -h, --help
            show this help text
USAGE
}

live=0
for arg in "$@"; do
  case "$arg" in
    --live) live=1 ;;
    -h|--help) usage; exit 0 ;;
    *)
      echo "error: unknown argument: ${arg}" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if ! git rev-parse --show-toplevel >/dev/null 2>&1; then
  echo "error: ${SCRIPT_NAME} must be run from inside the Aion git repository" >&2
  exit 1
fi
cd "$(git rev-parse --show-toplevel)"

# Announce the SET before acting on it, so the operator sees what a run covers
# rather than trusting that it covered everything. The set is READ OUT of cargo's
# own metadata, not maintained here — this prints what cargo will do, it does not
# decide it.
#
# jq rather than python3: on 2026-08-13 the first `python3` on this box's PATH was
# a Linux ELF binary (a broken uv install under a directory named macos-aarch64),
# so every `python3` call died with "cannot execute binary file". A release script
# must not fail on an interpreter it does not need. jq is the tool the estate's
# gate commands already speak, and its absence is refused by name rather than
# silently skipping the announcement — an announcement that can quietly not happen
# is worse than none, because its silence reads as "nothing to report".
if ! command -v jq >/dev/null 2>&1; then
  echo "error: jq is required to announce the publish set before acting; install it and re-run" >&2
  exit 1
fi

echo "==> Publishable workspace members (from cargo metadata)"
metadata="$(cargo metadata --format-version=1 --no-deps)"
publishable="$(printf '%s' "$metadata" | jq -r '[.packages[] | select(.publish != []) | .name] | sort | .[]')"
excluded="$(printf '%s' "$metadata" | jq -r '[.packages[] | select(.publish == []) | .name] | sort | join(", ")')"

if [ -z "$publishable" ]; then
  echo "error: cargo metadata reported no publishable members — refusing to run a vacuous publish" >&2
  exit 1
fi

printf '    %s\n' $publishable
printf '==> %s publishable, excluded: %s\n' \
  "$(printf '%s\n' "$publishable" | wc -l | tr -d ' ')" \
  "${excluded:-none}"

if (( live )); then
  echo "==> LIVE publish: cargo publish --workspace"
  cargo publish --workspace
else
  echo "==> Dry run: cargo publish --workspace --dry-run"
  cargo publish --workspace --dry-run
fi

echo "==> Done"
