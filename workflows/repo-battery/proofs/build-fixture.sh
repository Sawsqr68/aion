#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 3 ]]; then
  printf 'usage: %s FIXTURE SOURCE_TREE DESTINATION\n' "$0" >&2
  exit 2
fi
fixture=$1
source_tree=$(realpath "$2")
destination=$3
proof_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
definition="$proof_root/fixtures/$fixture"
[[ -d "$definition" ]] || { printf 'unknown fixture: %s\n' "$fixture" >&2; exit 2; }
[[ ! -e "$destination" ]] || { printf 'destination exists: %s\n' "$destination" >&2; exit 2; }
mkdir -p "$destination/scripts/gates"
if [[ "$fixture" != "no-declaration" ]]; then cp "$definition/gates.json" "$destination/gates.json"; fi
if [[ -f "$definition/fixture.txt" ]]; then cp "$definition/fixture.txt" "$destination/fixture.txt"; fi
cp "$source_tree/scripts/battery.sh" "$destination/scripts/battery.sh"
cp "$source_tree/scripts/battery-costly-announcements.sh" "$destination/scripts/battery-costly-announcements.sh"
cp "$source_tree/scripts/gates/lib.sh" "$destination/scripts/gates/lib.sh"
cp "$source_tree/scripts/gates/verify-declaration.sh" "$destination/scripts/gates/verify-declaration.sh"
chmod +x "$destination/scripts/battery.sh" "$destination/scripts/gates/verify-declaration.sh"
git -C "$destination" init -q
git -C "$destination" config user.name 'repo-battery proof'
git -C "$destination" config user.email 'repo-battery@example.invalid'
git -C "$destination" add .
git -C "$destination" commit -qm "fixture: $fixture"
for path in scripts/battery-costly-announcements.sh scripts/battery.sh scripts/gates/lib.sh scripts/gates/verify-declaration.sh; do
  printf '%s  %s\n' "$(sha256sum "$source_tree/$path" | awk '{print $1}')" "$path"
done
printf 'fixture_commit %s\n' "$(git -C "$destination" rev-parse HEAD)"
