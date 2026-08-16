# Gate script — workflow-dispatch conversion lane. Run by the aion server
# via norn_dev's `gates` action: `zsh $script`. Light by design: the box is
# carrying another seat's cargo battery, and this lane's subject is shell
# text, not compiled code. Every leg named, every leg's verdict echoed —
# a summed silence is not a pass.
REPO=/Users/tom/Developer/ablative/stack/aion
GL=$REPO/gate-logs
fail=0

check() {
  local name=$1 rc=$2
  if [[ $rc -eq 0 ]]; then
    echo "PASS  $name"
  else
    echo "FAIL  $name"
    fail=1
  fi
}

for lane in 246-adoption 247-transcript-spool libsql-retirement; do
  s=$GL/$lane/dispatch.sh

  [[ -f "$s" ]]; check "${lane}:dispatch-exists" $?
  [[ -f "$s" ]] || continue

  zsh -n "$s"; check "${lane}:zsh-syntax" $?

  # Detached dispatch is dead: no nohup, no direct norn invocation.
  ! grep -qE '(^|[^[:alnum:]_-])nohup([^[:alnum:]_-]|$)' "$s"
  check "${lane}:no-nohup" $?
  ! grep -qE 'norn[[:space:]]+--' "$s"
  check "${lane}:no-direct-norn" $?

  # The workflow path is present.
  grep -q 'aion start norn_dev' "$s"; check "${lane}:aion-start" $?
  grep -q -- '--input-file' "$s"; check "${lane}:input-file" $?

  # Worktree logic survives.
  grep -q 'worktree add' "$s"; check "${lane}:worktree-add" $?

  # A standing gate script exists for the lane and parses.
  g=$GL/$lane/gate.sh
  [[ -f "$g" ]]; check "${lane}:gate-exists" $?
  if [[ -f "$g" ]]; then
    zsh -n "$g"; check "${lane}:gate-syntax" $?
  fi
done

# The builder touched no tracked file. Live workers append to tracked
# *.log files under gate-logs (their dirt predates this lane and never
# stops accruing), so those paths are excluded; anything else modified
# is a fence breach.
cd "$REPO" || exit 91
dirt=$(git -c diff.external= diff --no-ext-diff --name-only | grep -vE '^gate-logs/.*\.log$')
[[ -z "$dirt" ]]; check "repo:unstaged-clean" $?
[[ -n "$dirt" ]] && echo "$dirt"
git -c diff.external= diff --no-ext-diff --cached --quiet; check "repo:staged-clean" $?

exit $fail
