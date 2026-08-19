#!/bin/zsh
# Detached norn dev dispatch: #253 bun-runner shim gap micro-fix.
# FRESH session (lesson 08:35Z: micro-fixes never resume near-full
# sessions — compaction loses the brief's walls). Brief self-contained.
set -u
umask 077

RIG=/Users/tom/Developer/ablative/stack/aion/gate-logs/241-remediation
WT=/Users/tom/Developer/ablative/stack/aion/.worktrees/241-final-message
SKILL=$HOME/.claude/skills/norn

SYS="$(cat "$SKILL/instructions/dev/base.md" "$SKILL/instructions/dev/implementation.md")"

{
  cat "$RIG/MICRO-FIX-253.md"
  echo
  echo "Work from the current repository state. Return only the value required"
  echo "by the supplied output schema. Do not claim completion or verification"
  echo "without direct evidence."
} > "$RIG/prompt-253.txt"

date -u +%Y-%m-%dT%H:%M:%SZ > "$RIG/DISPATCHED-253.marker"

nohup norn --print \
  --account default \
  --model gpt-5.6-sol \
  --reasoning-effort high \
  --fast \
  --working-dir "$WT" \
  --workspace-root "$WT" \
  --allowed-tools "read,search,lsp,bash,write,edit,apply_patch,action_log" \
  --session-name "claude-dev-253-bun-shim" \
  --quiet \
  --output-schema "$SKILL/schemas/dev.schema.json" \
  --output-format json \
  < "$RIG/prompt-253.txt" \
  > "$RIG/envelope-253.json" \
  2> "$RIG/stderr-253.log" &

echo "$!" > "$RIG/norn-253.pid"
echo "dispatched pid $(cat "$RIG/norn-253.pid")"
