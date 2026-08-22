#!/bin/bash
set -u
export PATH=$HOME/.cargo/bin:$PATH
ulimit -n 65536 2>/dev/null
echo "GATE SCOPE: fmt; clippy -D warnings; integrations+norn+worker suites x2; workspace suite"
cargo fmt --all --check || exit 1
cargo clippy --workspace --all-targets -- -D warnings || exit 1
# The crates this brief actually touches, run TWICE: these are the suites a
# classification change can break quietly, and a single green on a suite that
# owns a retry loop is not evidence.
cargo test -p aion-integrations --lib || exit 1
cargo test -p aion-integration-norn --lib || exit 1
cargo test -p aion-worker --lib || exit 1
cargo test -p aion-integrations --lib || exit 1
cargo test -p aion-integration-norn --lib || exit 1
cargo test -p aion-worker --lib || exit 1
cargo test --workspace --no-fail-fast || exit 1
echo GATE-GREEN
