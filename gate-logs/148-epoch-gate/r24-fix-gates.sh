#!/bin/zsh
SP=/private/tmp/claude-501/-Users-tom-Developer-ablative-stack-aion/5849e0d8-4802-4869-8e0f-9f7fd187e198/scratchpad
cd /Users/tom/Developer/ablative/stack/aion || exit 1
load () { sysctl -n vm.loadavg | awk '{print $2}' }
cargo fmt
cargo fmt --check > "$SP/r24f-fmt.log" 2>&1; echo "fmt-check EXIT $? (load $(load))"
cargo test -p aion-rs --lib runtime::nif_continue_as_new > "$SP/r24f-module.log" 2>&1
echo "module EXIT $? (load $(load))"; grep -E "^test result" "$SP/r24f-module.log"
cargo clippy -p aion-rs --all-targets -- -D warnings > "$SP/r24f-clippy-p.log" 2>&1
echo "clippy-p EXIT $?  checking-lines $(grep -c 'Checking aion-rs' "$SP/r24f-clippy-p.log") (load $(load))"
cargo clippy --workspace --all-targets -- -D warnings > "$SP/r24f-clippy-ws.log" 2>&1
echo "clippy-ws EXIT $? (load $(load))"
shasum crates/aion/src/runtime/nif_continue_as_new.rs
echo "DONE (load $(load))"
