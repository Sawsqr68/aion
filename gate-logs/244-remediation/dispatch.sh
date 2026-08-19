#!/bin/zsh
# Detached norn dev dispatch for the #244 remediation leg (FAIL verdict
# remediation, OPUS-REVIEW.md must-list). HELD until: (1) Waffles ratifies
# BRIEF.md, (2) his 64G target sweep in this worktree COMPLETES (verify
# with lsof + du — no in-flight rm may race the cold build), (3) load
# sane. Envelope + stderr land beside this script.
set -u
umask 077

RIG=/Users/tom/Developer/ablative/stack/aion/gate-logs/244-remediation
WT=/Users/tom/Developer/ablative/stack/aion/.worktrees/244-child-visibility
REVIEW=/Users/tom/Developer/ablative/stack/aion/gate-logs/244-child-visibility/OPUS-REVIEW.md
SKILL=$HOME/.claude/skills/norn

SYS="$(cat "$SKILL/instructions/dev/base.md" "$SKILL/instructions/dev/implementation.md")"

{
  echo "Delegation mode: dev"
  echo "Specialist preset: implementation"
  echo "Repository: $WT (isolated git worktree, branch 244-child-visibility @ dd5073bf2)"
  echo
  echo "Task: execute the remediation brief below in full. It is your complete charter."
  echo
  cat "$RIG/BRIEF.md"
  echo
  echo "===== OPUS-REVIEW.md (the review of record, part of the charter) ====="
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
  --session-name "claude-dev-244-remediation" \
  --quiet \
  --output-schema "$SKILL/schemas/dev.schema.json" \
  --output-format json \
  < "$RIG/prompt.txt" \
  > "$RIG/envelope.json" \
  2> "$RIG/stderr.log" &

echo "$!" > "$RIG/norn.pid"
echo "dispatched pid $(cat "$RIG/norn.pid")"
