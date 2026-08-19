#!/bin/zsh
# Detached norn dev dispatch: #241 TS-stub micro-fix, RESUMING the original
# builder session claude-dev-241-remediation (f7a83452) so its own
# python-stub regen recipe is in hand. Envelope + stderr land beside this.
set -u
umask 077

RIG=/Users/tom/Developer/ablative/stack/aion/gate-logs/241-remediation
WT=/Users/tom/Developer/ablative/stack/aion/.worktrees/241-final-message
SKILL=$HOME/.claude/skills/norn
SESSION_ID=f7a83452-78ad-41c0-bc22-ec4381d67b23

SYS="$(cat "$SKILL/instructions/dev/base.md" "$SKILL/instructions/dev/implementation.md")"

{
  echo "Follow-up on your completed #241 remediation."
  echo "Execute the micro-fix brief below in full. It is your complete charter."
  echo
  cat "$RIG/MICRO-FIX-TS-STUB.md"
  echo
  echo "Work from the current repository state. Return only the value required"
  echo "by the supplied output schema. Do not claim completion or verification"
  echo "without direct evidence."
} > "$RIG/prompt-ts-stub.txt"

date -u +%Y-%m-%dT%H:%M:%SZ > "$RIG/DISPATCHED-ts-stub.marker"

nohup norn --print \
  --account default \
  --model gpt-5.6-sol \
  --reasoning-effort high \
  --fast \
  --working-dir "$WT" \
  --workspace-root "$WT" \
  --allowed-tools "read,search,lsp,bash,write,edit,apply_patch,action_log" \
  --append-system-prompt "$SYS" \
  --resume "$SESSION_ID" \
  --quiet \
  --output-schema "$SKILL/schemas/dev.schema.json" \
  --output-format json \
  < "$RIG/prompt-ts-stub.txt" \
  > "$RIG/envelope-ts-stub.json" \
  2> "$RIG/stderr-ts-stub.log" &

echo "$!" > "$RIG/norn-ts-stub.pid"
echo "dispatched pid $(cat "$RIG/norn-ts-stub.pid")"
