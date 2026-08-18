#!/bin/sh
# .fleet-gate.sh — fleet gate for the #70 beamr >=0.19.2 migration, box-scoped.
#
# THE FORMER EXCLUSION IS THE PROOF. Every prior gate on this box excluded the
# gleam-runtime populations under #64 (validator refuses gleam-1.18.1/OTP-27
# beams). beamr >=0.19.2 carries the validator fix, so this gate runs the FULL
# workspace suite with NO exclusions: runtime_codecs going green on this box's
# own mix is the migration's proof, measured where the refusal lived.
PATH="$HOME/.cargo/bin:/usr/local/bin:/usr/bin:/bin"
export PATH

if ! command -v cargo >/dev/null 2>&1; then
    echo "FAIL: cargo not found on PATH ($PATH)" >&2
    exit 1
fi

set -e

# Leg 1 — LOCK WITNESS (a patch/req is witnessed at the lock, never the line):
# the resolved graph must contain zero beamr rows below 0.19.2.
bad=$(grep -A1 '^name = "beamr"' Cargo.lock | grep '^version' | grep -Ev 'version = "0\.(19\.([2-9]|[1-9][0-9])|[2-9][0-9]*\.)' || true)
if [ -n "$bad" ]; then
    echo "FAIL: pre-0.19.2 beamr in the resolved lock:"; echo "$bad"; exit 1
fi
echo "gate leg 1 ok: lock witness clean (all beamr rows >=0.19.2)"

# Leg 2 — full workspace tests, NO gleam exclusions. This includes
# runtime_codecs (28 tests) on this box's gleam-1.18.1/OTP-27 mix.
cargo test --workspace

# Leg 3 — workspace lint wall.
cargo clippy --workspace --all-targets -- -D warnings
