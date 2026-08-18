#!/bin/sh
# .fleet-gate.sh — fleet gate for the #70 beamr >=0.19.2 migration, box-scoped.
#
# THE FORMER EXCLUSION IS THE PROOF. Every prior gate on this box excluded the
# gleam-runtime populations under #64 (validator refuses gleam-1.18.1/OTP-27
# beams). beamr >=0.19.2 carries the validator fix, so this gate runs the FULL
# workspace suite with NO exclusions: runtime_codecs going green on this box's
# own mix is the migration's proof, measured where the refusal lived.
# HOME can be EMPTY in the workflow gate context (learned 2026-08-18: fleet-53
# round 1-3 red on "cargo not found on PATH (/.cargo/bin:...)"). Absolute box
# path first; $HOME fallback kept for other hosts.
PATH="/home/aion/.cargo/bin:$HOME/.cargo/bin:/usr/local/bin:/usr/bin:/bin"
export PATH
# cargo itself resolves ~/.cargo and ~/.rustup through HOME — default it too.
if [ -z "${HOME:-}" ]; then HOME=/home/aion; export HOME; fi
# Concurrent flights exhaust the 1024-fd soft limit (EMFILE reds, 2026-08-18);
# the hard limit is 524288 — raise within it so contention cannot fake a red.
ulimit -n 65536 2>/dev/null || true

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
# lld segfaulted under 16-way parallel links of workspace test binaries
# (round-2 red, 2026-08-18: LLVM stack dumps, multiple binaries, 31G box) —
# bound build parallelism so concurrent link memory stays inside the box.
cargo test --workspace -j 4

# Leg 3 — workspace lint wall.
cargo clippy --workspace --all-targets -j 4 -- -D warnings
