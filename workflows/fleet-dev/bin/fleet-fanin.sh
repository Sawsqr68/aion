#!/bin/sh
# fleet-fanin.sh <workspace_root> <run_name> <leg_ids_csv>
# Ordered no-ff merges of leg branches into the primary tree. A conflict
# aborts clean, names the files, and exits AS DATA — a finding about the
# plan's leg boundaries. A clean fanin commits the exact leg heads and merge
# commits so teardown cannot erase the branch-ref evidence.
set -u
. /home/aion/fleet/bin/fleet-lib.sh
[ $# -eq 3 ] || op_guard_fail "fleet-fanin.sh expects exactly 3 arguments, got $#"
fleet_root_guard "$1" "$2"; csv_guard "$3"
repo="$1/$2/repo"
[ -d "$repo/.git" ] || op_guard_fail "no repo at $repo"
cd "$repo" || op_guard_fail "cannot enter $repo"
# Spawn-guard marker reconciliation (carrier kill #6, speech r3 ae8f57df,
# 2026-08-23): a 0.17.0 norn dev-control session plants
# .aion-held-execution.json in the tree it guards, and a session that dies
# mid-flight leaves it as a corpse the clean-tree check below then trips
# over — the rig forbids the leg to touch it, so fanin must reconcile it
# with the engine's own spawn-guard semantics: a DEAD planter's claim is
# cleared (preserved, never rm'd — the bytes are the attribution); a LIVE
# pid refuses by name (fail-closed: a reused pid earns a hand check, an
# auto-clear could merge over a genuinely held tree). RETIREMENT: #148
# (aion 0.21.0) moves claims out of the tree entirely — this block dies
# with that deploy, on the deploy checklist by name.
marker=".aion-held-execution.json"
if [ -e "$marker" ]; then
    git ls-files --error-unmatch -- "$marker" >/dev/null 2>&1 && op_guard_fail "$marker is TRACKED in this tree — not a spawn-guard corpse; refusing to touch it"
    mpid=$(jq -r '.pid // empty' "$marker" 2>/dev/null)
    case "$mpid" in ''|*[!0-9]*) op_guard_fail "$marker exists but carries no parseable pid — refusing fanin over an unattributable claim: $(cat "$marker" 2>/dev/null | head -c 400)" ;; esac
    if kill -0 "$mpid" 2>/dev/null; then
        mwf=$(jq -r '.workflow_id // "unknown"' "$marker" 2>/dev/null)
        mact=$(jq -r '.activity_id // "unknown"' "$marker" 2>/dev/null)
        op_guard_fail "live spawn-guard claim holds this tree: pid $mpid (workflow $mwf, activity $mact) is still running — fanin refuses rather than merge over a held tree"
    fi
    preserved="$1/$2/held-execution-marker-cleared.json"
    mv "$marker" "$preserved" || op_guard_fail "dead spawn-guard marker (pid $mpid gone) could not be preserved to $preserved"
    marker_note=" [reconciled dead spawn-guard marker: planter pid $mpid gone; bytes preserved at $preserved]"
else
    marker_note=""
fi
dirty=$(git status --porcelain 2>&1)
[ -n "$dirty" ] && op_guard_fail "primary tree is not clean before fanin: $dirty"
merged=""
rows=""
for id in $(printf '%s' "$3" | tr ',' ' '); do
    head=$(git rev-parse --verify --quiet "leg/$id^{commit}") || op_guard_fail "leg branch leg/$id has no commit to merge"
    if ! out=$(git merge --no-ff "leg/$id" -m "fanin: leg $id" 2>&1); then
        conflicts=$(git diff --name-only --diff-filter=U 2>/dev/null)
        git merge --abort 2>/dev/null
        op_emit 1 "merge of leg/$id refused. Conflicting files: $conflicts. Merged so far:$merged. Raw: $out" ""
    fi
    mc=$(git rev-parse HEAD 2>&1) || op_guard_fail "merge of leg/$id landed but HEAD is unreadable: $mc"
    rows="$rows| $id | leg/$id | $head | $mc |
"
    merged="$merged $id"
done
mkdir -p .fleet || op_guard_fail "cannot create .fleet"
{
    printf '# legs merged — %s\n\n' "$2"
    printf 'Recorded by fleet-fanin.sh at merge time: the branch refs the run tree held.\n'
    printf 'Teardown discards the branches, so this file is how the exact pre-merge state\n'
    printf 'of every leg stays recoverable from the result branch alone.\n\n'
    printf '| leg | branch | leg head at merge | merge commit |\n'
    printf '|---|---|---|---|\n'
    printf '%s' "$rows"
} > .fleet/legs-merged.md || op_guard_fail "cannot write .fleet/legs-merged.md"
git add -- .fleet/legs-merged.md || op_guard_fail "cannot stage .fleet/legs-merged.md"
git commit -q -m "fanin: leg branch refs recorded" -- .fleet/legs-merged.md || op_guard_fail "cannot commit .fleet/legs-merged.md"
git cat-file -e HEAD:.fleet/legs-merged.md 2>/dev/null || op_guard_fail ".fleet/legs-merged.md is not at HEAD after commit"
for id in $(printf '%s' "$3" | tr ',' ' '); do
    git show HEAD:.fleet/legs-merged.md | grep -q "^| $id | leg/$id |" || op_guard_fail "no row for leg $id in the committed .fleet/legs-merged.md"
done
for sha in $(git show HEAD:.fleet/legs-merged.md | tr '|' '\n' | tr -d ' ' | grep -E '^[0-9a-f]{40}$'); do
    git cat-file -e "$sha^{commit}" 2>/dev/null || op_guard_fail "recorded sha does not resolve to a commit: $sha"
done
op_emit 0 "merged in order:$merged; branch refs recorded in .fleet/legs-merged.md$marker_note" ""
