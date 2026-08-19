# Leg `capture` — harden the fleet_dev family's data capture

You are the only builder on this run. This brief is complete: everything you need is
here. Read it end to end before you touch a file.

---

## 0. What you are working on

This repository (`aion`) contains a workflow family that runs *itself* — `fleet_dev`
dispatches a planning counsel, parallel builders, mechanical merges, gates, and a judge,
on the Aion engine. It flew live for the first time tonight and worked. Three things it
produced were destroyed instead of recorded. You are closing exactly those three gaps
and nothing else.

The family is three kinds of file:

| Path | What it is |
|---|---|
| `workflows/fleet-dev/fleet_dev.awl` | the workflow: 16 steps, from `provision` to `clean_up` |
| `workflows/fleet-dev/fleet_ops_worker.awl` | the ops worker document: 8 actions, each a `run json` shell command |
| `workflows/fleet-dev/bin/*.sh` | the nine operation scripts those commands run |

`aion` is the CLI (`/home/aion/.cargo/bin/aion` on this box). `aion awl check <file>`
type-checks an AWL document and is one of your acceptance commands.

**Where the scripts actually run from.** `workflows/fleet-dev/bin/` is the source of
truth; provisioning copies those files to `/home/aion/fleet/bin/`, and the seated worker
executes the copies. Today all nine deployed copies are byte-identical to the committed
ones. So your edits harden the **next** provisioning — they do not change this flight
mid-air. Nothing you are asked to verify depends on a live re-dispatch.

## 1. The four laws that bind every edit

1. **Every operation script exits zero, always; the inner exit code is typed data.**
   Under a worker's `run json` body, a non-zero exit is an *activity failure* and the
   command's **stdout is discarded** — the evidence is thrown away at exactly the moment
   it matters (stated at `workflows/gates/repo_gates_worker.awl:38-42`, and again in the
   two fleet documents at `fleet_ops_worker.awl:7-12` and `fleet_dev.awl:12-17`). The
   mechanism is `bin/fleet-lib.sh`, which every script sources:
   - `op_emit <inner_exit_code> <stdout> <stderr>` (`:6`) — emits
     `{exit_code, stdout, stderr}` and exits 0. `exit_code` is the *inner* verdict.
   - `op_guard_fail <message>` (`:5`) — emits `exit_code: 2`, a script-face refusal
     (bad arguments, unusable tree, git refusing), and exits 0.
   - `fleet_root_guard <workspace_root> <run_name>` (`:7-11`) — pins the root to exactly
     `/home/aion/fleet/runs` and the run name to `[A-Za-z0-9-]`.
   **Never write `exit 1` in an operation script. Never let a script die on an
   unhandled error.**
2. **No time ceilings on agent work.** Do not add a `timeout`, a deadline, or a
   sleep-and-retry loop anywhere. Not to an action, not to a step, not to a script.
3. **Checks in code, with typed results.** Every claim you make must be produced by a
   command that returns a verdict. A paragraph in your report is not a check.
4. **Do not disturb what you were not asked to change.** The nine existing scripts'
   argument contracts and return shapes stay exactly as they are (two of them gain
   behaviour, described below, with no contract change). `aion awl check` must be green
   on both documents when you are done.

Also binding, from the repository's own standards (`CLAUDE.md`): no lazy code, no silent
failures, no swallowed errors, handle every edge case. This code runs a fleet that
commits to real branches.

## 2. The three gaps

### GAP 1 — session transcripts die with the sessions

Every agent action returns a `Reply` record (`fleet_dev.awl:97`):
`{text, final_message, stop_reason, session_id}`. The run keeps `text`; the `session_id`
— the handle of the whole conversation — is thrown away, and the transcript dies with
the session. Six agent conversations happen in a full run: the counsel control, the dev
control, the planner, one builder per leg, any fix rounds, and the judge.

**The transcripts exist on this box, at these paths (verified, not assumed):**

- **norn** (the `dev` seat: dev control, builders, fix rounds) —
  `/home/aion/.norn/session-store/<session_id>.jsonl`, a flat id-named JSONL file whose
  first line is `{"norn_session_format":2}`. Tonight's two builders, its fix round and
  both dev controls are all there. The adapter takes `session_id` straight off the
  wire — "the session norn actually persisted, never a value this adapter derives"
  (`crates/aion-integration-norn/src/translate/outcome.rs:113-121`).
- **ACP/Claude** (the `counsel` seat: counsel control, planner, judge) —
  `/home/aion/.claude/projects/<cwd-with-slashes-turned-into-dashes>/<session_id>.jsonl`.
  Tonight's three counsel calls are in
  `/home/aion/.claude/projects/-home-aion-fleet-runs-fleet-63-first-repo/`, each file
  named for its session uuid, which also appears as `sessionId` on every line. The id is
  "the session the turn ran in — minted by `session/new`"
  (`crates/aion-integration-acp/src/outcome.rs:86-88`).

**Required:** before the run tears its tree down, each named session's transcript is
copied into `.fleet/sessions/<step>-<session_id>.jsonl` **in the tree that ran it** and
committed. If a transcript is genuinely unreachable, the script commits a typed absence
(`.fleet/sessions/ABSENT-<step>.md` naming what was looked for and where) — never a
silent skip.

### GAP 2 — a green gate leaves no proof

`bin/fleet-gates.sh` commits its findings to `.fleet/gates/` **only on red** (`:21-25`:
`red-<n>.md` plus the stable `RED-LATEST.md` the fix-round prompt addresses). On green it
commits nothing, so a landed run carries no record of what its green looked like.

**Required:** on green, commit the last 120 lines of the gate log as
`.fleet/gates/green-<n>.md`, with the same commit discipline as the red path.

### GAP 3 — leg branch refs vanish at teardown

`bin/fleet-fanout.sh:14-16` cuts one worktree and one branch `leg/<id>` per leg.
`bin/fleet-fanin.sh:16-23` merges them `--no-ff` in order. After that, nothing records
which branch was merged at which sha, and `bin/fleet-teardown.sh:15` (`rm -rf "$tree"`)
takes the whole run tree — worktrees, branches and all — with it.

**Required:** `bin/fleet-fanin.sh` records each leg id, its branch name, its branch head
sha **at merge time**, and the merge commit sha, in a committed `.fleet/legs-merged.md`,
so the exact pre-merge state of every leg is recoverable from the result branch alone.

## 3. What to build

### 3.1 New script — `workflows/fleet-dev/bin/fleet-sessions.sh`

Two modes. The mode word is a literal supplied by the action's command template, never a
caller's value:

```
fleet-sessions.sh primary <workspace_root> <run_name> <counsel_control_session> <dev_control_session> <plan_session> <judge_session>
fleet-sessions.sh one     <workspace_root> <run_name> <tree> <step_label> <session_id>
```

`primary` captures the four primary-tree sessions under the fixed labels
`counsel_control`, `dev_control`, `plan_legs`, `judgement` (positional — the workflow
cannot compose strings, so the labels live here). `one` captures a single session under a
caller-named label into a named tree (a leg worktree, or the primary tree for the fix
round).

Exit codes carried as data: **0** every named session captured and verified at HEAD;
**1** at least one transcript was unreachable and a committed absence was written
instead; **2** a script-face refusal (`op_guard_fail`).

The following reference implementation was written and exercised end to end against a
scratch run root before this brief was issued: leg-worktree capture, primary capture with
two present and two missing sessions, an activity-retry re-capture, all three guards, and
a fanin carrying a leg's transcript home. **You own it now** — read every line, satisfy
yourself it is right, and complete or correct it rather than pasting it blind.

```sh
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
```

Why the details are the way they are — do not undo these:

- **`git commit -- <path>` (pathspec-limited).** A builder may leave content staged in
  its worktree's index; a bare `git add`/`git commit` would sweep that into the capture
  commit. Verified: with `staged.txt` staged, the capture commits only the transcript and
  leaves `staged.txt` staged and uncommitted.
- **`git rev-parse --is-inside-work-tree`, not `[ -d .git ]`.** A linked worktree's
  `.git` is a *file*. The `[ -d ]` test the other scripts use is right for them (they
  only ever look at the primary repo) and would refuse **every** leg here.
- **The environment overrides** (`FLEET_NORN_STORE`, `FLEET_CLAUDE_PROJECTS`) exist so
  the self-check can exercise the script without touching real transcripts. Same
  box-binding register as commit `360d1dc1` ("fix(gates): bind box toolchain home"). The
  defaults are the real paths.
- **Idempotency.** Agent actions declare `retry 3` (`fleet_dev.awl:214,219,240`); a
  re-captured transcript must report `already-committed`, not die on git's empty-commit
  refusal.
- **No shared index file under `.fleet/sessions/`.** Every leg commits on its own branch
  and the branches are merged in order; a file every leg appends to would conflict, and a
  fanin conflict routes the whole run to `unmerged`. Session ids are unique, so
  per-session filenames never collide.

### 3.2 Edit — `workflows/fleet-dev/bin/fleet-gates.sh` (GAP 2)

Two changes, plus the header comment.

**(a) The round counter at line 17** must count both colours, so green rounds are
numbered in the same sequence:

```sh
n=$(ls .fleet/gates/red-*.md .fleet/gates/green-*.md 2>/dev/null | wc -l | tr -d ' '); n=$((n + 1))
```

(On every path reachable today this is identical to the old count — a green ends the gate
loop, so no red ever follows one — and it stays correct if a tree is re-gated.)

**(b) Give the `if [ "$rc" -ne 0 ]` block at lines 21-25 an `else` arm.** Leave the red
arm byte-identical:

```sh
else
    mkdir -p .fleet/gates || op_guard_fail "cannot create .fleet/gates"
    tail -n 120 "$log" > ".fleet/gates/green-$n.md" || op_guard_fail "cannot write the green tail into the tree"
    git add .fleet/gates && git commit -q -m "gates: green round $n — tail verbatim" || op_guard_fail "cannot commit the green tail"
    git cat-file -e "HEAD:.fleet/gates/green-$n.md" 2>/dev/null || op_guard_fail "green-$n.md is not at HEAD after commit"
fi
```

Do **not** invent a `GREEN-LATEST.md`, and do not touch `RED-LATEST.md`: that stable
address is what the fix-round prompt names (`fleet_dev.awl:348-352`). The red path keeps
copying the *whole* log (the fix round needs all of it); the green path commits the
120-line tail, which is the same text the probe already carries at `:26-27`.

### 3.3 Edit — `workflows/fleet-dev/bin/fleet-fanin.sh` (GAP 3)

Read each leg's branch head **before** its merge and the merge commit **after** it, then
write and commit the record once every merge has landed. The conflict path at `:17-21`
stays exactly as it is (abort, name the conflicting files, `op_emit 1`) — on that path
every leg branch survives in the kept tree, so nothing is lost by not writing the file.

```sh
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
op_emit 0 "merged in order:$merged; branch refs recorded in .fleet/legs-merged.md" ""
```

The last three blocks are the check-in-code half of law 3: the file is at HEAD, every leg
in the csv has a row, and every recorded sha resolves to a real commit — verified by the
script, not asserted in a report.

### 3.4 Edit — the two AWL documents (GAP 1 wiring)

This exact wiring was prototyped against the committed documents and checked with the
real binary before this brief was issued:
`ok: fleet_dev.awl (17 steps)`, `ok: fleet_ops_worker.awl (worker fleet_ops, 10 actions)`.
Reproduce it as written.

**`workflows/fleet-dev/fleet_ops_worker.awl`** — append two actions after the `teardown`
action (after line 50), before the `type` declarations:

```
  /// Copy the four primary-tree agent sessions' transcripts into
  /// `.fleet/sessions/` and COMMIT them; an unreachable source is a
  /// committed typed absence, never a silent skip.
  action sessions(workspace_root: String, run_name: String, counsel_control_session: String, dev_control_session: String, plan_session: String, judge_session: String) -> OpProbe
    run json "sh /home/aion/fleet/bin/fleet-sessions.sh primary $workspace_root $run_name $counsel_control_session $dev_control_session $plan_session $judge_session"
  /// One named session's transcript, copied and committed into the tree it
  /// ran in — a leg worktree (riding home through fanin) or the primary.
  action session(workspace_root: String, run_name: String, tree: String, step_label: String, session_id: String) -> OpProbe
    run json "sh /home/aion/fleet/bin/fleet-sessions.sh one $workspace_root $run_name $tree $step_label $session_id"
```

Also update the `fanin` action's doc comment (`:29-31`) to say it records the merged leg
branch refs in `.fleet/legs-merged.md`, and the `gates` action's (`:38-40`) to say the
green tail is committed too. Doc comments only — the contracts do not change.

**`workflows/fleet-dev/fleet_dev.awl`** — five edits.

1. Declare the same two actions, bodiless, at the end of the `worker fleet_ops` block
   (after line 255):

```
  action sessions(workspace_root: String, run_name: String, counsel_control_session: String, dev_control_session: String, plan_session: String, judge_session: String) -> OpProbe
  action session(workspace_root: String, run_name: String, tree: String, step_label: String, session_id: String) -> OpProbe
```

2. `step build_one` (`:318-320`) gains one statement — the leg's transcript is committed
   **on the leg branch**, so it rides home through fanin exactly as the leg's report does:

```
step build_one
  delegate(repo: leg.repo, prompt: leg.prompt) -> built
  session(workspace_root: workspace_root, run_name: run_name, tree: leg.repo,
    step_label: leg.id, session_id: built.session_id) -> leg_captured
  built.text -> report
```

3. `step fix_round` (`:366-368`) gains one statement, before its `route gate`:

```
step fix_round
  delegate(repo: repo, prompt: fix_prompt) -> fixed
  session(workspace_root: workspace_root, run_name: run_name, tree: repo,
    step_label: "fix_round", session_id: fixed.session_id) -> fix_captured
  route gate
```

   It must live **inside** the loop. Reading `fixed.session_id` from a later step is
   refused by the checker: *"`fixed` is bound on some path but not guaranteed on every
   path into this step — bindings are readable only where guaranteed."*

4. A new step between `judgement` and `push_result`, and `judgement`'s `clear` arm
   (`:378`) routed to it (`route push_result` → `route capture_sessions`). It falls
   through to `push_result`, so the transcripts are committed **before** the push and
   ride the result branch home:

```
step capture_sessions
  sessions(workspace_root: workspace_root, run_name: run_name,
    counsel_control_session: counsel_control.session_id, dev_control_session: control.session_id,
    plan_session: planned.session_id, judge_session: judged.session_id) -> captured
```

5. `type FleetDevReport` (`:123-136`) gains two fields, populated in `clean_up`'s
   `route done(...)` (`:395-399`) — a capture refusal is data on the report, never a
   failure of a run whose work is already judged and home, exactly as a teardown refusal
   is (`:114-116`):

```
  teardown_ok: Bool,
  teardown_refusal: String,
  sessions_exit: Int,
  sessions_note: String,
}
```
```
    teardown_ok: td.ok, teardown_refusal: td.refusal,
    sessions_exit: captured.exit_code, sessions_note: captured.stdout)
```

**Do not run `aion awl fmt`.** It rewrites in place, has no `--check` mode, and the
committed document is already 56 lines from the canonical rendering — running it would
bury your change in unrelated churn. `aion awl check` is the acceptance.

### 3.5 New — `workflows/fleet-dev/selfcheck/capture-selfcheck.sh`

The behavioural half of the acceptance: `sh -n` and `aion awl check` cannot see a deleted
`cp`, so this is what turns red when a gap regresses. POSIX `sh`, no bashisms. It prints
one `ok: <assertion>` or `FAIL: <assertion>` line per check and exits non-zero if any
failed. It tests the scripts in **this tree** (`workflows/fleet-dev/bin/`), not the
deployed copies.

Structure:

- Assert `/home/aion/fleet/bin/fleet-lib.sh` exists (the scripts under test source it)
  and fail loudly naming it if not.
- Scratch run root `/home/aion/fleet/runs/selfcheck-$$` — **refuse to run if it already
  exists** (never touch another run's tree, the law `bin/fleet-fanout.sh:15` and
  `bin/fleet-clone.sh:15` state), and at the end remove only that path, after checking
  the variable still has the `/home/aion/fleet/runs/selfcheck-` prefix. The pinned root
  in `fleet_root_guard` is a confinement guard: it is **not** to be made overridable.
- Scratch transcript stores under the run root, exported as `FLEET_NORN_STORE` and
  `FLEET_CLAUDE_PROJECTS`, holding fabricated `<uuid>.jsonl` files. Real transcripts are
  never read, written, or moved.
- `git init` a repo at `<root>/repo` with a `user.name`/`user.email` config and one
  commit, then cut two leg worktrees by calling the tree's own `bin/fleet-fanout.sh`, and
  commit a file in each leg.
- Parse every probe with `jq` and assert on `.exit_code` — that is the typed result.

The assertions, all of which must hold:

| # | Gap | Assertion |
|---|---|---|
| 1 | 1 | `fleet-sessions.sh one` into a leg worktree returns `exit_code 0` and `.fleet/sessions/<leg>-<id>.jsonl` is at HEAD **on that leg's branch** |
| 2 | 1 | an unrelated file staged in that leg's index is still staged and uncommitted afterwards |
| 3 | 1 | re-running the same capture returns `exit_code 0` and reports `already-committed` |
| 4 | 1 | `fleet-sessions.sh primary` with two reachable and two unreachable ids returns `exit_code 1`, commits both transcripts, and commits `ABSENT-<label>.md` for both missing labels, each naming both search roots |
| 5 | 1 | a `tree` outside the run root (e.g. `/etc`) and a session id containing `/` or `..` are both refused with `exit_code 2` |
| 6 | 2 | `fleet-gates.sh` against a gate entry that exits 1 returns `exit_code 1` and puts `red-1.md` and `RED-LATEST.md` at HEAD |
| 7 | 2 | `fleet-gates.sh` against a gate entry that exits 0 returns `exit_code 0` and puts a non-empty `green-2.md` at HEAD containing a marker the gate entry printed |
| 8 | 3 | `fleet-fanin.sh` returns `exit_code 0` and `.fleet/legs-merged.md` is at HEAD |
| 9 | 3 | that file has a row per leg, and each row's recorded head sha equals the `git rev-parse leg/<id>` the self-check itself captured before the merge |
| 10 | 3 | every 40-hex sha in the file resolves (`git cat-file -e <sha>^{commit}`) |

Keep it under ~200 lines and single-purpose.

### 3.6 Edit — `workflows/fleet-dev/README.md`

Add a short "What a landed run carries home" section (≤30 lines): `.fleet/sessions/`,
`.fleet/gates/green-<n>.md`, `.fleet/legs-merged.md`, where each is written and by which
script, and update the step walk-through to mention `capture_sessions`. The README
documents the shape; it must not now be lying.

### 3.7 Not yours to touch

`.fleet-hardening-gate.sh` at the repo root is already committed by the planner and is
the run's gate entry. Do not edit it. Do not edit `bin/fleet-lib.sh` (its deployed copy
is what the other scripts source at runtime; drift there breaks live runs). Do not edit
`.fleet/plan.md`. Do not touch anything outside `workflows/fleet-dev/` and your own
report. Never land, merge, push, or deploy: commit to your leg branch and stop.

## 4. How to check your work

Run all of these from the repository root. **All must pass.**

```sh
aion awl check workflows/fleet-dev/fleet_dev.awl          # → ok: … (17 steps)
aion awl check workflows/fleet-dev/fleet_ops_worker.awl   # → ok: … (worker `fleet_ops`, 10 actions)
for s in workflows/fleet-dev/bin/*.sh; do sh -n "$s" || echo "PARSE FAIL $s"; done
sh workflows/fleet-dev/selfcheck/capture-selfcheck.sh     # → every ok:, exit 0
sh .fleet-hardening-gate.sh                               # → fleet-hardening gate: GREEN
```

If `aion` is not on your PATH, it is at `/home/aion/.cargo/bin/aion` (**not**
`$HOME/.local/bin`, which holds only the `claude` symlink).

### The mutation checks — one per gap, mandatory

A check nobody has ever seen fail is not a check. For each gap: break the mechanism, show
the gate goes **red** naming the failing command, revert, show it green again. Paste the
verbatim command and output of each into your report.

| Gap | Mutation | Expected |
|---|---|---|
| 1 | in `fleet-sessions.sh`, make `capture` skip the `cp` (or point `locate` at a name that cannot exist) | self-check assertions 1/3/4 FAIL → `sh .fleet-hardening-gate.sh` prints `GATE FAILED: sh workflows/fleet-dev/selfcheck/capture-selfcheck.sh` and exits 1 |
| 2 | delete the `else` arm you added to `fleet-gates.sh` | assertion 7 FAILs → gate red |
| 3 | delete the `.fleet/legs-merged.md` write in `fleet-fanin.sh` | assertions 8-10 FAIL → gate red |
| AWL | rename `capture_sessions`'s route target in `judgement` to a step that does not exist | `aion awl check` errors → gate red naming that command |

Revert every mutation and re-run the gate green before you commit. `git status` must be
clean of them — your final commit contains the hardening, never a mutation.

## 5. What to commit

Commit to **your leg branch** (`leg/capture`) in **your own worktree**, which is your
current working directory. Your deliverables:

- `workflows/fleet-dev/bin/fleet-sessions.sh` (new, executable — `chmod +x`, like its siblings)
- `workflows/fleet-dev/bin/fleet-gates.sh` (edited)
- `workflows/fleet-dev/bin/fleet-fanin.sh` (edited)
- `workflows/fleet-dev/fleet_ops_worker.awl` (edited)
- `workflows/fleet-dev/fleet_dev.awl` (edited)
- `workflows/fleet-dev/selfcheck/capture-selfcheck.sh` (new)
- `workflows/fleet-dev/README.md` (edited)
- **`.fleet/reports/capture.md`** — required. A run-time script verifies this file exists
  non-empty at HEAD in the merged tree and fails the whole run if it does not
  (`bin/fleet-report-check.sh:13-19`). It must contain: what you changed and why, per
  gap; the verbatim output of every acceptance command in §4; the verbatim
  before/after of all four mutation checks; and anything you found that this brief got
  wrong. If you disagreed with something here and did it differently, say so and say why —
  a judge reads this against the brief.

Everything must be **committed**. The tree is the blackboard and the blackboard is
committed: an uncommitted change does not exist as far as the run is concerned, and a
loose `.fleet` working tree is itself a red (`bin/fleet-plan-check.sh:14-15`).
