#!/bin/bash
set -u
export PATH=$HOME/.cargo/bin:$PATH
ulimit -n 65536 2>/dev/null
echo "GATE SCOPE: fmt; clippy -D warnings; aion-server lib suite x2; workspace suite"
cargo fmt --all --check || exit 1
cargo clippy --workspace --all-targets -- -D warnings || exit 1
cargo test -p aion-server --lib || exit 1
cargo test -p aion-server --lib || exit 1
cargo test --workspace --no-fail-fast || exit 1
echo GATE-GREEN
