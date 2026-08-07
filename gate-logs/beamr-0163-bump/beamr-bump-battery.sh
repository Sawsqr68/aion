#!/bin/zsh
# beamr 0.16.2 -> 0.16.3 pin-bump battery. Detached; transcript + per-gate logs.
# Entry gate: process-table exclusion + load (1-min AND 5-min) vs cores + disk floor.
set -u
SP=/private/tmp/claude-501/-Users-tom-Developer-ablative-stack-aion/5849e0d8-4802-4869-8e0f-9f7fd187e198/scratchpad
T=$SP/beamr-bump.transcript
cd /Users/tom/Developer/ablative/stack/aion || exit 3

say() { print -r -- "$1" >> "$T" }
loadnow() { sysctl -n vm.loadavg | awk '{print $2" "$3}' }

: > "$T"
NCPU=$(sysctl -n hw.ncpu)
DISK=$(df -g / | tail -1 | awk '{print $4}')
FOREIGN=$(pgrep -x cargo | wc -l | tr -d ' ')
read L1 L5 <<< "$(loadnow)"
say "== entry gate: ncpu=$NCPU load1=$L1 load5=$L5 disk=${DISK}G foreign-cargo=$FOREIGN =="
if (( FOREIGN > 0 )); then say "REFUSED: foreign cargo live"; exit 2; fi
if (( ${L1%.*} >= NCPU || ${L5%.*} >= NCPU )); then say "REFUSED: load >= ncpu"; exit 2; fi
if (( DISK < 28 )); then say "REFUSED: disk < 28G"; exit 2; fi

say "== beamr in lock =="
grep -A1 'name = "beamr"' Cargo.lock | head -2 >> "$T"

say "== clippy --workspace (load $(loadnow)) =="
cargo clippy --workspace --all-targets -- -D warnings > "$SP/bb-clippy-ws.log" 2>&1
say "clippy-ws EXIT $? checking-lines $(grep -c '^\s*Checking' "$SP/bb-clippy-ws.log")"

say "== fmt --check (load $(loadnow)) =="
cargo fmt --check > "$SP/bb-fmt.log" 2>&1
say "fmt-check EXIT $?"

say "== suite -p aion-rs (load $(loadnow)) =="
cargo test -p aion-rs > "$SP/bb-suite-aion.log" 2>&1
rc=$?
say "suite-aion EXIT $rc verdict-lines $(grep -c '^test result' "$SP/bb-suite-aion.log")"
grep '^test result' "$SP/bb-suite-aion.log" | grep -v ' 0 failed' >> "$T"

say "== suite --workspace (load $(loadnow)) =="
cargo test --workspace > "$SP/bb-suite-ws.log" 2>&1
rc=$?
say "suite-ws EXIT $rc verdict-lines $(grep -c '^test result' "$SP/bb-suite-ws.log")"
grep '^test result' "$SP/bb-suite-ws.log" | grep -v ' 0 failed' >> "$T"

say "== mid/exit disk $(df -g / | tail -1 | awk '{print $4}')G =="
say "DONE (load $(loadnow))"
