#!/bin/zsh
# Detached norn dev dispatch — the #36 follow-on lane (board rows aion#37-#41).
# Plain builder, fresh worktree from landed main (c58079469). Brief r1 approved
# by Waffles 09:51Z with three amendments folded; dispatch at Vesper's word.
set -u
umask 077

RIG=/Users/tom/Developer/ablative/stack/aion/gate-logs/36-follow-on
WT=/Users/tom/Developer/ablative/stack/aion/.worktrees/36-follow-on
SKILL=$HOME/.claude/skills/norn

SYS="$(cat "$SKILL/instructions/dev/base.md" "$SKILL/instructions/dev/implementation.md")"

{
  echo "Delegation mode: dev"
  echo "Specialist preset: implementation"
  echo "Repository: $WT (isolated git worktree, branch 36-follow-on)"
  echo
  echo "Task: execute the follow-on lane brief below in full. It is your complete charter."
  echo "The opening-state documents it names live in the worktree at the paths given."
  echo
  cat "$RIG/../resume-adoption/FOLLOW-ON-BRIEF-DRAFT.md"
  echo
  echo "Work from the current repository state. Write REPORT.md at the worktree"
  echo "root before you finish, whatever the outcome. Return only the value"
  echo "required by the supplied output schema. Do not claim completion or"
  echo "verification without direct evidence."
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
  --session-name "claude-dev-36-follow-on" \
  --quiet \
  --output-schema "$SKILL/schemas/dev.schema.json" \
  --output-format json \
  < "$RIG/prompt.txt" \
  > "$RIG/envelope.json" \
  2> "$RIG/stderr.log" &

echo "$!" > "$RIG/norn.pid"
echo "dispatched pid $(cat "$RIG/norn.pid")"
