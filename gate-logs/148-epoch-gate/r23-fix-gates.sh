#!/bin/zsh
SP=/private/tmp/claude-501/-Users-tom-Developer-ablative-stack-aion/5849e0d8-4802-4869-8e0f-9f7fd187e198/scratchpad
cd /Users/tom/Developer/ablative/stack/aion || exit 1
load () { sysctl -n vm.loadavg | awk '{print $2}' }
echo "== fmt (write) then fmt --check (load $(load)) =="
cargo fmt
cargo fmt --check > "$SP/r23f-fmt.log" 2>&1; echo "fmt-check EXIT $?"
echo "== module test (load $(load)) =="
cargo test -p aion-rs --lib runtime::nif_continue_as_new > "$SP/r23f-module.log" 2>&1
echo "module EXIT $?"; grep -E "^test result" "$SP/r23f-module.log"
echo "== clippy -p (load $(load)) =="
cargo clippy -p aion-rs --all-targets -- -D warnings > "$SP/r23f-clippy-p.log" 2>&1
echo "clippy-p EXIT $?  checking-lines $(grep -c 'Checking aion-rs' "$SP/r23f-clippy-p.log")"
echo "== clippy --workspace (load $(load)) =="
cargo clippy --workspace --all-targets -- -D warnings > "$SP/r23f-clippy-ws.log" 2>&1
echo "clippy-ws EXIT $?"
echo "== suite (load $(load)) =="
cargo test -p aion-rs > "$SP/r23f-suite.log" 2>&1
echo "suite EXIT $?  verdict-lines $(grep -c '^test result' "$SP/r23f-suite.log")"
grep "^test result" "$SP/r23f-suite.log" | grep -v " 0 failed" || echo "suite: NO FAILURES IN ANY BLOCK"
echo "== shas =="
shasum crates/aion/src/runtime/nif_continue_as_new.rs crates/aion/src/runtime/nif_continue_as_new_tests.rs crates/aion/src/engine/api.rs
echo "DONE (load $(load))"
