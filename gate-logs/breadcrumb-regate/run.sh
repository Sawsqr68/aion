#!/bin/bash
# Waffles' landing re-gate for death-note-breadcrumb (afe84d147). Crate-scoped:
# delta touches only crates/aion-server + docs, zero manifest movement vs my
# full six-leg re-gate at 04f8e976 40 min prior. Subshell legs per the law.
set -u
cd "$(dirname "$0")/../.." || exit 90
L="gate-logs/breadcrumb-regate"
overall=0
for leg in "fmt:cargo fmt --all" "test:cargo test -p aion-server" "clippy:cargo clippy -p aion-server --all-targets -- -D warnings"; do
  name="${leg%%:*}"; cmd="${leg#*:}"
  ( echo "LEG=$name HEAD=$(git rev-parse HEAD) start=$(date -u +%H:%M:%SZ)"; $cmd 2>&1; c=$?; echo "LEG=$name EXIT=$c"; exit $c ) > "$L/$name.log"
  c=$?
  echo "LEG=$name EXIT=$c"
  [ "$c" -ne 0 ] && overall=1
done
git status --short | /usr/bin/grep -v "breadcrumb-regate" > "$L/post-fmt-status.txt"
echo "OVERALL=$overall"; echo "$overall" > "$L/OVERALL.txt"
exit $overall
