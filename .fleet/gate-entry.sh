#!/bin/sh
# .fleet/gate-entry.sh — VENUE-SCOPED gate for the repo-battery build wave.
# SCOPE (named, per the receipt scope law): fmt + workspace clippy + workspace
# tests. The repo's full declared battery (gates.json, 24 legs — gleam, bun,
# uv, console, estate-guard legs among them) does NOT run here: this venue
# does not carry those tools, and their require_tool refusals are venue facts,
# not code verdicts. The full battery runs at the landing desk. A green here
# is a green of THIS scope and nothing wider.
set -e
echo "GATE SCOPE: cargo fmt --all --check; cargo clippy --workspace --all-targets -D warnings; cargo test --workspace --no-fail-fast (venue-scoped; full 24-leg battery at landing desk)"
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
