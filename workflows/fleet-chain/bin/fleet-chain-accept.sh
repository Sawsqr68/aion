#!/bin/sh
# fleet-chain-accept.sh <state-json>
#
# One recorded boundary of the staged chain, and exactly one. The
# coordinator threads its whole `ChainState` in and gets a new one back, so
# each boundary is a SEPARATELY RECORDED activity result and replay
# consumes the record rather than redoing the work (design §6).
#
# EXIT ZERO ALWAYS, INNER CODE AS DATA: every anticipated face exits zero
# and carries its status in the returned state. A halted state passes
# through UNCHANGED, which is how a failing stage dispatches nothing
# downstream.
#
# The decision itself is never made here. `aion-fleet-chain-step` is a pure
# function of the normalized graph plus the recorded ledger; this script
# only carries the external result across the seam. It resolves no ref,
# reads no worktree, greps no prose, and consults no clock.
set -u
FLEET_HOME=${FLEET_HOME:-/home/aion/fleet}
. "$FLEET_HOME/bin/fleet-chain-lib.sh"
[ $# -eq 1 ] || chain_guard_fail "fleet-chain-accept.sh expects exactly 1 argument, got $#"

halted=$(printf '%s' "$1" | jq -r '.halted // false' 2>/dev/null) || chain_guard_fail "state is not decodable JSON"
if [ "$halted" = "true" ]; then
    printf '%s' "$1"
    exit 0
fi

run_dir=$(printf '%s' "$1" | jq -r '.kept_root')
[ -n "$run_dir" ] || chain_guard_fail "state carries no run root"
step=$(chain_binary)

# Poll + TOTAL DEADLINE + timeout as a DISTINCT outcome. Never one blocking
# sleep sized to the expected duration: a stage flight is hours long and a
# sleep that guessed its duration would be both wrong and unobservable.
out=$("$step" --graph "$run_dir/.fleet/chain/graph.json" \
               --ledger "$run_dir/.fleet/chain/ledger.json" \
               --root-base "$(printf '%s' "$1" | jq -r '.base_commit')" \
               --pid-file "$run_dir/.fleet/chain/accept.pid" \
               --inbox "$run_dir/.fleet/chain/inbox" \
               --deadline-secs "${AION_FLEET_CHAIN_DEADLINE_SECS:?the chain step deadline is bound configuration and must be provisioned, never defaulted}" \
               --poll-millis "${AION_FLEET_CHAIN_POLL_MILLIS:?the chain step poll interval is bound configuration and must be provisioned, never defaulted}" 2>&1)
rc=$?
case "$rc" in
    0)  printf '%s' "$out" | jq -c '.' >/dev/null 2>&1 || chain_guard_fail "the step binary returned undecodable output: $out"
        printf '%s' "$out" ;;
    75) chain_guard_fail "the accept boundary reached its TOTAL DEADLINE waiting at the barrier; this is a distinct timeout outcome, never a success and never a silent retry" ;;
    *)  chain_guard_fail "the step binary exited $rc at the accept boundary: $out" ;;
esac
exit 0
