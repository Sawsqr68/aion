#!/bin/sh
# fleet-chain-plan-check.sh <workspace_root> <run_name> <leg_ids_csv> <max_stages>
#
# BOUNDARY 1 — parse complete (design §6 clause 1). Validates the committed
# `fleet-stages-v1` block and returns the NORMALIZED TYPED GRAPH, which is
# the run's ordering authority for the whole run (§5.3 final ¶).
#
# This delegates to the SAME `fleet-plan-check.sh` the legacy flight uses,
# so there is exactly one implementation of the exit contract and the chain
# cannot drift from it. Inner code 0 with a block is a valid chain; 0
# without a block is the coordinator's missing contract; 1 and 3 keep their
# pre-existing meanings; 4 is PLAN GRAPH INVALID with a stable cause code.
set -u
FLEET_HOME=${FLEET_HOME:-/home/aion/fleet}
. "$FLEET_HOME/bin/fleet-chain-lib.sh"
[ $# -eq 4 ] || chain_guard_fail "fleet-chain-plan-check.sh expects exactly 4 arguments, got $#"
chain_root_guard "$1" "$2"

probe=$(sh "$FLEET_HOME/bin/fleet-plan-check.sh" "$1" "$2" "$3")
inner=$(printf '%s' "$probe" | jq -r '.exit_code')
out=$(printf '%s' "$probe" | jq -r '.stdout')
err=$(printf '%s' "$probe" | jq -r '.stderr')

# `stdout` on a valid chain is the validator's own JSON reply. Parse it for
# the graph and, on exit 4, the cause code — never re-derive either.
graph=$(printf '%s' "$out" | jq -c '.graph // empty' 2>/dev/null || printf '')
block=$(printf '%s' "$out" | jq -r '.chain_block // false' 2>/dev/null || printf 'false')
cause=$(printf '%s' "$out" | tail -n +2 | jq -r '.cause_code // ""' 2>/dev/null || printf '')
stage=$(printf '%s' "$out" | tail -n +2 | jq -r '.stage_id // ""' 2>/dev/null || printf '')

jq -n --argjson c "$inner" --argjson b "${block:-false}" --arg g "${graph:-}" \
      --arg cc "${cause:-}" --arg sid "${stage:-}" --arg o "$out" --arg e "$err" \
  '{exit_code:$c,chain_block:$b,graph:$g,cause_code:$cc,stage_id:$sid,stdout:$o,stderr:$e}'
exit 0
