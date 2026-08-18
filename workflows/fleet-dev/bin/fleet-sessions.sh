#!/bin/sh
# fleet-sessions.sh <mode> ...
# Copy named agent session transcripts INTO THE TREE and COMMIT them, so a
# landed run carries the conversations that produced it. The mode word is a
# literal in the action's command template, never a caller's value.
#
#   fleet-sessions.sh primary <workspace_root> <run_name> <counsel_control_session> <dev_control_session> <plan_session> <judge_session>
#   fleet-sessions.sh one     <workspace_root> <run_name> <tree> <step_label> <session_id>
#
# EXIT ZERO ALWAYS, INNER CODE AS DATA: 0 every named session captured and
# verified at HEAD, 1 at least one session's transcript was unreachable (a
# COMMITTED typed absence, never a silent skip), 2 a script-face refusal.
set -u
. /home/aion/fleet/bin/fleet-lib.sh

: "${FLEET_NORN_STORE:=/home/aion/.norn/session-store}"
: "${FLEET_CLAUDE_PROJECTS:=/home/aion/.claude/projects}"

note=""
absent=0

id_guard() {
    case "$1" in ""|*[!A-Za-z0-9-]*) op_guard_fail "session id shape refused for $2: $1" ;; esac
}

label_guard() {
    case "$1" in ""|*[!A-Za-z0-9_-]*) op_guard_fail "step_label shape refused: $1" ;; esac
}

# tree_guard <root> <tree> — the tree must be this run's primary repo or one of
# its leg worktrees, and must be a git tree. Nothing else is writable here.
tree_guard() {
    case "$2" in
        "$1/repo") : ;;
        "$1/legs/"*)
            leaf=${2#"$1/legs/"}
            case "$leaf" in ""|*[!A-Za-z0-9-]*) op_guard_fail "leg worktree name shape refused: $2" ;; esac
            ;;
        *) op_guard_fail "tree is not this run's primary repo or a leg worktree: $2" ;;
    esac
    # A linked worktree's `.git` is a FILE, not a directory, so `[ -d ]` (the
    # test the primary-tree-only scripts use) would refuse every leg.
    git -C "$2" rev-parse --is-inside-work-tree >/dev/null 2>&1 || op_guard_fail "no git tree at $2"
}

# locate <session_id> — echo the transcript path, or nothing with status 1.
locate() {
    if [ -f "$FLEET_NORN_STORE/$1.jsonl" ]; then
        printf '%s' "$FLEET_NORN_STORE/$1.jsonl"
        return 0
    fi
    for candidate in "$FLEET_CLAUDE_PROJECTS"/*/"$1.jsonl"; do
        if [ -f "$candidate" ]; then
            printf '%s' "$candidate"
            return 0
        fi
    done
    return 1
}

# commit_path <tree> <relpath> <message> — stage and commit exactly that path,
# never the rest of an agent's index. Status 1 means there was nothing new to
# commit (an activity retry re-capturing what is already in).
commit_path() {
    git -C "$1" add -- "$2" || op_guard_fail "git add refused for $2 in $1"
    if [ -z "$(git -C "$1" status --porcelain -- "$2" 2>&1)" ]; then
        return 1
    fi
    git -C "$1" commit -q -m "$3" -- "$2" || op_guard_fail "git commit refused for $2 in $1"
    return 0
}

# capture <tree> <step_label> <session_id>
capture() {
    tree=$1; label=$2; sid=$3
    label_guard "$label"
    id_guard "$sid" "$label"
    mkdir -p "$tree/.fleet/sessions" || op_guard_fail "cannot create $tree/.fleet/sessions"
    if src=$(locate "$sid"); then
        rel=".fleet/sessions/$label-$sid.jsonl"
        cp "$src" "$tree/$rel" || op_guard_fail "cannot copy $src to $tree/$rel"
        [ -s "$tree/$rel" ] && bytes=$(wc -c < "$tree/$rel" | tr -d ' ') || op_guard_fail "copied transcript is empty: $src"
        tail_state=intact
        tail -n 1 "$tree/$rel" | jq -e . >/dev/null 2>&1 || tail_state=trailing-line-unflushed
        if commit_path "$tree" "$rel" "sessions: $label transcript"; then state=committed; else state=already-committed; fi
        git -C "$tree" cat-file -e "HEAD:$rel" 2>/dev/null || op_guard_fail "$rel is not at HEAD in $tree after $state"
        note="$note
$label $sid $src $bytes bytes $tail_state $state"
    else
        absent=1
        rel=".fleet/sessions/ABSENT-$label.md"
        {
            printf '# ABSENT — %s (session %s)\n\n' "$label" "$sid"
            printf 'No transcript for this session was reachable when the run captured it.\n\n'
            printf 'Looked for, in order:\n\n'
            printf -- '- `%s/%s.jsonl` (norn session store)\n' "$FLEET_NORN_STORE" "$sid"
            printf -- '- `%s/*/%s.jsonl` (ACP/Claude project transcripts)\n\n' "$FLEET_CLAUDE_PROJECTS" "$sid"
            printf 'What was there instead:\n\n```\n'
            ls -la "$FLEET_NORN_STORE" 2>&1 | head -n 5
            ls -d "$FLEET_CLAUDE_PROJECTS"/*/ 2>&1 | head -n 5
            printf '```\n'
        } > "$tree/$rel" || op_guard_fail "cannot write $rel in $tree"
        if commit_path "$tree" "$rel" "sessions: $label transcript ABSENT"; then state=committed; else state=already-committed; fi
        git -C "$tree" cat-file -e "HEAD:$rel" 2>/dev/null || op_guard_fail "$rel is not at HEAD in $tree after $state"
        note="$note
$label $sid ABSENT $state"
    fi
}

[ $# -ge 1 ] || op_guard_fail "fleet-sessions.sh expects a mode word (primary|one)"
mode=$1
case "$mode" in
    primary)
        [ $# -eq 7 ] || op_guard_fail "fleet-sessions.sh primary expects exactly 7 arguments, got $#"
        fleet_root_guard "$2" "$3"
        root="$2/$3"; tree="$root/repo"
        tree_guard "$root" "$tree"
        capture "$tree" counsel_control "$4"
        capture "$tree" dev_control "$5"
        capture "$tree" plan_legs "$6"
        capture "$tree" judgement "$7"
        ;;
    one)
        [ $# -eq 6 ] || op_guard_fail "fleet-sessions.sh one expects exactly 6 arguments, got $#"
        fleet_root_guard "$2" "$3"
        root="$2/$3"
        tree_guard "$root" "$4"
        capture "$4" "$5" "$6"
        ;;
    *) op_guard_fail "unknown mode: $mode (expected primary or one)" ;;
esac

op_emit "$absent" "$note" ""
