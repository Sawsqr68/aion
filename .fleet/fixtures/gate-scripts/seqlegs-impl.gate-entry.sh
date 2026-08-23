#!/bin/bash
set -u
export PATH=$HOME/.cargo/bin:$PATH
ulimit -n 65536 2>/dev/null
# SEQLEGS implementation flight gate (brief: .fleet/BRIEF.md).
# Scope: deliverable positive controls (fast, before any cargo), then the
# aion repo battery pattern: fmt --check, clippy -D warnings, workspace
# suite --no-fail-fast. R-substance (R1 refusal specimens, R3 cause codes,
# R6 replay boundaries, R7 byte-identity, R8 exemplar paths) is proven by
# the flight's OWN red-first tests and re-run at the judge — a green here
# is battery health + wall, never acceptance.
BASE=c590712f57a2fe8353c6cd42823200587f25d70e
echo "GATE SCOPE: deliverable controls; cargo fmt --check; clippy -D warnings; workspace suite (R-substance = tests + judge)"

changed=$(git diff --name-only "${BASE}"..HEAD)
if [ -z "$changed" ]; then
  echo "GATE-RED: empty diff against base — no implementation committed"
  exit 1
fi
if ! printf '%s\n' "$changed" | grep -qE '^crates/.*\.rs$'; then
  echo "GATE-RED: no Rust deliverable in the diff (named cause: deliverable_absent)"
  exit 1
fi
if ! printf '%s\n' "$changed" | grep -qE '^docs/evidence/'; then
  echo "GATE-RED: no red-first evidence committed under docs/evidence/ (named cause: evidence_absent)"
  exit 1
fi

cargo fmt --all --check || { echo "GATE-RED: fmt"; exit 1; }
cargo clippy --workspace --all-targets -- -D warnings || { echo "GATE-RED: clippy"; exit 1; }
cargo test --workspace --no-fail-fast || { echo "GATE-RED: workspace suite"; exit 1; }
echo "GATE-GREEN: deliverables present, battery green against base ${BASE}"
exit 0
