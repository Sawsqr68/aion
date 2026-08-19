#!/bin/zsh
# Detached norn dev dispatch for the #241 remediation round.
# Same worktree/branch as the original build (Waffles' disposition).
# Envelope + stderr land beside this script.
set -u
umask 077

RIG=/Users/tom/Developer/ablative/stack/aion/gate-logs/241-remediation
WT=/Users/tom/Developer/ablative/stack/aion/.worktrees/241-final-message
SKILL=$HOME/.claude/skills/norn
REVIEW=/Users/tom/Developer/ablative/stack/aion/gate-logs/241-final-message/OPUS-REVIEW.md

SYS="$(cat "$SKILL/instructions/dev/base.md" "$SKILL/instructions/dev/implementation.md")"

{
  echo "Delegation mode: dev"
  echo "Specialist preset: implementation"
  echo "Repository: $WT (isolated git worktree, branch 241-final-message)"
  echo
  echo "Task: execute the remediation brief below in full. It is your complete charter."
  echo
  cat "$RIG/BRIEF.md"
  echo
  echo "--- REVIEW OF RECORD (verbatim; findings carry file:line) ---"
  echo
  cat "$REVIEW"
  echo
  echo "Work from the current repository state. Return only the value required"
  echo "by the supplied output schema. Do not claim completion or verification"
  echo "without direct evidence."
} > "$RIG/prompt.txt"

date -u +%Y-%m-%dT%H:%M:%SZ > "$RIG/DISPATCHED.marker"

nohup norn --print \
  --model gpt-5.6-sol \
  --reasoning-effort xhigh \
  --fast \
  --working-dir "$WT" \
  --workspace-root "$WT" \
  --allowed-tools "read,search,lsp,bash,write,edit,apply_patch,action_log" \
  --append-system-prompt "$SYS" \
  --session-name "claude-dev-241-remediation" \
  --quiet \
  --output-schema "$SKILL/schemas/dev.schema.json" \
  --output-format json \
  < "$RIG/prompt.txt" \
  > "$RIG/envelope.json" \
  2> "$RIG/stderr.log" &

echo "$!" > "$RIG/norn.pid"
echo "dispatched pid $(cat "$RIG/norn.pid")"
