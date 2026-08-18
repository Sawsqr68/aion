# fleet_dev capture hardening — the plan

**Run:** `fleet-hardening-1`. **Legs:** one, id `capture`, brief at `.fleet/legs/capture.md`.
**Gate entry:** `.fleet-hardening-gate.sh` (committed with this plan).
**Result:** the fleet_dev workflow family carries its own evidence home — session
transcripts, green gate tails, and leg branch refs — instead of losing it at teardown.

The fleet_dev family flew live tonight and worked. Three things it produced were
destroyed rather than recorded. This run closes exactly those three, and nothing else.

---

## 1. The system as it stands

| Piece | Path | What it is |
|---|---|---|
| The workflow | `workflows/fleet-dev/fleet_dev.awl` (399 lines, 16 steps) | plan → build legs in parallel → merge → gate → fix-round loop → judge → push → teardown |
| The ops worker document | `workflows/fleet-dev/fleet_ops_worker.awl` (58 lines, 8 actions) | bodied `run json` actions; every one is `sh /home/aion/fleet/bin/fleet-<op>.sh $params` |
| The nine operation scripts | `workflows/fleet-dev/bin/*.sh` | clone, plan-check, fanout, fanin, report-check, gates, push, teardown, plus the sourced `fleet-lib.sh` |

The scripts in `workflows/fleet-dev/bin/` are the **source of truth**; provisioning
copies them to `/home/aion/fleet/bin/`, which is what the seated ops worker actually
executes (`fleet_ops_worker.awl:3-5`, and every action's command line, e.g. `:18`).
Verified on this box at plan time: all nine deployed copies are byte-identical to the
committed ones (`cmp` over `/home/aion/fleet/bin/fleet-*.sh`). **Consequence, stated
plainly: the edits this plan commissions harden the NEXT provisioning, not the flight
that carries them.** Nothing in this run's own execution changes behaviour mid-flight,
and no acceptance check may depend on a live re-dispatch.

## 2. The laws that bind every edit

1. **Exit zero always; the inner exit code is typed data.** Under `run json` a non-zero
   exit is an activity failure and its stdout is **discarded** — stated at
   `workflows/gates/repo_gates_worker.awl:38-42` and again at
   `fleet_ops_worker.awl:7-12` and `fleet_dev.awl:12-17`. The mechanism is
   `fleet-lib.sh:5` (`op_guard_fail`, exit_code 2 for a script-face refusal) and
   `fleet-lib.sh:6` (`op_emit`, the inner code), both of which `exit 0`.
2. **No time ceilings on agent work.** No timeout is added anywhere by this plan — not
   on an action, not on a step, not as a `sleep` retry loop inside a script.
3. **Checks in code, with typed results.** The verification of each gap is a script that
   returns a typed verdict, not a paragraph. Prose in a report is never the check.
4. **The nine scripts' contracts and the 16-step shape stay intact unless a gap
   requires the change.** Where a gap requires it, this plan names the change and its
   reason. `aion awl check` must stay green on both documents.
5. **The tree is the blackboard and the blackboard is committed** (`fleet_dev.awl:19-29`).
   Every artifact this plan adds is written into the tree **and committed**, on the
   branch that carries it home.

## 3. What was found on the box (the grounding for GAP 1)

The `Reply` record already carries the handle: `{text, final_message, stop_reason,
session_id}` (`fleet_dev.awl:97`). Both adapters treat `session_id` as a fact read off
the wire, never derived:

- **norn (the `dev` seat).** `session_id` is the envelope's own value — "the session
  norn actually persisted, never a value this adapter derives"
  (`crates/aion-integration-norn/src/translate/outcome.rs:113-121`; the key itself at
  `crates/aion-integration-norn/src/protocol.rs:74-76`).
- **ACP (the `counsel` seat).** `session_id` is "the session the turn ran in — minted by
  `session/new` or restored by `session/load`"
  (`crates/aion-integration-acp/src/outcome.rs:86-88`).

Where those transcripts live on this host, established by reading the box, not by
assumption:

- **norn:** `/home/aion/.norn/session-store/<session_id>.jsonl` — a flat, id-named JSONL
  file (first line `{"norn_session_format":2}`), with a sibling `<session_id>/` directory
  for artifacts. Confirmed against tonight's flight: the two leg builders, the fix round,
  and both dev controls are all present there as `<uuid>.jsonl`, their first user message
  being the verbatim fleet prompt. `norn session export <ID> --format jsonl` exists as a
  documented second path (`norn session --help`), but the store file is the primary
  source: it needs no subprocess and no cwd context.
- **ACP/Claude:** `/home/aion/.claude/projects/<cwd-with-slashes-as-dashes>/<session_id>.jsonl`.
  Confirmed: `/home/aion/.claude/projects/-home-aion-fleet-runs-fleet-63-first-repo/`
  holds exactly three transcripts for tonight's three counsel calls, each filename equal
  to the session uuid and equal to the `sessionId` field on every line, and their first
  user messages are the counsel control probe, the planner brief, and the judge charter.

Neither location is inside the run tree, so **teardown (`bin/fleet-teardown.sh:15`,
`rm -rf "$tree"`) is not what destroys them** — the sessions themselves end and the box
keeps the files until the account is cleaned. What is missing is the *record*: nothing in
the pushed branch says which session produced which step, and nothing copies the
transcript anywhere durable. That is GAP 1, and it is closed by copying at a point where
every id is in scope and the tree is still the run's own.

## 4. Design decisions, each proven before being prescribed

Every AWL construct below was prototyped in `/tmp` against the committed documents and
checked with the real binary (`/home/aion/.cargo/bin/aion`). Result:
`ok: fleet_dev.awl (17 steps)` and `ok: fleet_ops_worker.awl (worker fleet_ops, 10 actions)`.

- **D1 — A parameter is one argv element; a list cannot be passed.**
  `crates/aion-worker/src/shell/template.rs:1-16` (a value is substituted "as one whole
  argv element", never re-parsed by a shell) and `:63-71` +`:144`
  (`SubstitutionError::UnrepresentableValue` — "supply a string, number, or boolean").
  So the collected `[String]` of leg sessions can **never** be handed to a script, and
  AWL composes no strings at runtime (`fleet_dev.awl:19-21`). Each leg's session id must
  therefore be captured **inside its own track**, one String at a time.
- **D2 — A `distribute` track may be a multi-step graph.** The language guide's own
  example runs `inspect_host` then `verify_host` inside one region
  (`crates/aion-awl/src/guide/examples/flow_graph.rs:184-207`). A step body is a list of
  statements (`crates/aion-awl/src/ast/steps.rs:26-28,61-89`), so a second call can also
  simply be added to an existing step — no new step is needed for the per-leg or the
  fix-round capture. Both forms were checked; the statement form is prescribed because it
  keeps the step count at 17.
- **D3 — The fix-round session can only be captured inside the loop.** Prototyped the
  alternative (reading `fixed.session_id` from a later step) and the checker refused it:
  `error: 'fixed' is bound on some path but not guaranteed on every path into this step —
  bindings are readable only where guaranteed`. This is a language ruling, not a
  preference: the capture call belongs in `fix_round` itself.
- **D4 — A string literal is a legal call argument.** `step_label: "fix_round"` checks
  green, so one action serves both the per-leg (`step_label: leg.id`) and the fix-round
  capture without any runtime composition.
- **D5 — The capture must be committed BEFORE the push, not before teardown only.** The
  commissioned wording is "before teardown"; teardown is `clean_up` (`fleet_dev.awl:393-394`),
  which runs *after* `push_result` (`:385-389`). A capture between them would be committed
  into a tree that is about to be deleted and never pushed. So the primary capture step
  sits between `judgement` and `push_result`, and the transcripts ride the result branch
  home — which is also what GAP 3 demands of the leg refs.
- **D6 — A capture failure must not fail a judged, green run.** The document already has
  this precedent: a teardown refusal "is data on the report, never a run failure"
  (`fleet_dev.awl:114-116,391-393`). So `capture_sessions` routes nowhere new; its typed
  probe is carried onto `FleetDevReport` as `sessions_exit` / `sessions_note`, and the
  durable activity result is in history regardless.
- **D7 — No shared index file under `.fleet/sessions/`.** Each leg commits on its own
  branch and the branches are merged in order (`bin/fleet-fanin.sh:16-23`); a file every
  leg appends to would conflict, and a fanin conflict routes the whole run to `unmerged`
  (`fleet_dev.awl:330-335`). Per-session filenames are disjoint by construction because a
  session id is unique.
- **D8 — Commits must be pathspec-limited.** A builder may leave content staged in its
  worktree's index; a bare `git commit` after `git add` would sweep it into the capture
  commit. Verified in a scratch repo that `git add -- <path>` followed by
  `git commit -m … -- <path>` commits only that path and leaves an unrelated staged file
  staged and uncommitted.
- **D9 — `aion awl fmt` must not be run.** It rewrites in place and has no `--check` mode
  (`aion awl fmt --help`), and the committed `fleet_dev.awl` is already 56 lines away
  from the canonical rendering. Running it would bury the real change in unrelated churn.
  The acceptance is `aion awl check`.
- **D10 — The gate needs behavioural teeth.** The acceptance requires that disabling a
  gap's mechanism turns the acceptance check red. `sh -n` and `aion awl check` cannot see
  a deleted `cp`, so `.fleet-hardening-gate.sh` also runs a committed self-check
  (`workflows/fleet-dev/selfcheck/capture-selfcheck.sh`) that exercises all three gaps
  against a scratch tree. This is one command beyond the commissioned wording, named here
  as an addition and justified by the acceptance clause it serves.

## 5. Requirements

### GAP 1 — session transcripts

- **R1.** A new operation script `workflows/fleet-dev/bin/fleet-sessions.sh` copies a
  named session's transcript into `<tree>/.fleet/sessions/<step_label>-<session_id>.jsonl`
  and commits it in `<tree>`. It carries the same face as its siblings: sources
  `/home/aion/fleet/bin/fleet-lib.sh`, emits exactly one `OpProbe` JSON on stdout, and
  **exits zero on every anticipated face** (`fleet-lib.sh:5-6`).
- **R2.** It has two modes, distinguished by a literal first argument that the action's
  command template supplies (never a caller's value):
  `primary <workspace_root> <run_name> <counsel_control_session> <dev_control_session> <plan_session> <judge_session>`
  and `one <workspace_root> <run_name> <tree> <step_label> <session_id>`.
  Argument shapes are guarded before use: `fleet_root_guard` for root and run name
  (`fleet-lib.sh:7-11`), session ids restricted to `[A-Za-z0-9-]`, `step_label` to
  `[A-Za-z0-9_-]`, and `tree` accepted only as exactly `<root>/repo` or
  `<root>/legs/<name>` containing a `.git` — the same confinement register as
  `fleet-clone.sh:10-15` and `fleet-teardown.sh:10-14`.
- **R3.** Lookup order per session id: `/home/aion/.norn/session-store/<id>.jsonl`, then
  `/home/aion/.claude/projects/*/<id>.jsonl`. Both roots are pinned constants with an
  environment override (`FLEET_NORN_STORE`, `FLEET_CLAUDE_PROJECTS`), exactly the
  box-binding register ruled in for the gate in commit 360d1dc1 — the override exists so
  the self-check can exercise the script without touching real transcripts.
- **R4.** **A typed absence, never a silent skip.** If neither source holds the id, the
  script writes and commits `<tree>/.fleet/sessions/ABSENT-<step_label>.md` naming the
  session id, both roots searched, the exact patterns tried, and what was found in their
  place; the probe's `exit_code` is 1 and its `stdout` names the absent labels.
- **R5.** Every claimed file is verified **at HEAD** after the commit
  (`git cat-file -e HEAD:.fleet/sessions/<name>`), the same law `fleet-plan-check.sh:16-25`
  and `fleet-report-check.sh:13-19` apply to agents. A copy that did not land is a
  script-face refusal (`op_guard_fail`, exit_code 2), not a success.
- **R6.** The script is idempotent: an action retry (`fleet_dev.awl:213-219,239-240` all
  declare `retry 3`) that re-captures an already-committed transcript reports
  `already-committed` and does not fail on git's empty-commit refusal.
- **R7.** Two bodiless actions are added to the `fleet_ops` worker block in
  `fleet_dev.awl` (after `:255`) and their bodied twins to `fleet_ops_worker.awl` (after
  `:50`), both returning `OpProbe` (`fleet_ops_worker.awl:54`) — exact text in the leg brief.
- **R8.** Wiring, exactly as prototyped and checked:
  - `build_one` (`fleet_dev.awl:318-320`) gains one `session(...)` statement with
    `tree: leg.repo`, `step_label: leg.id`, `session_id: built.session_id` — the leg's
    transcript is committed **on the leg branch** and rides home through fanin, exactly
    as its report does (D1, D7).
  - `fix_round` (`:366-368`) gains one `session(...)` statement with `step_label:
    "fix_round"` before its `route gate` (D3).
  - A new step `capture_sessions` between `judgement` and `push_result` calls
    `sessions(...)` with `counsel_control.session_id`, `control.session_id`,
    `planned.session_id`, `judged.session_id`; `judgement`'s `clear` arm (`:378`) routes
    to it, and it falls through to `push_result` (D5).
  - `FleetDevReport` (`:123-136`) gains `sessions_exit: Int` and `sessions_note: String`,
    populated in `clean_up`'s `route done(...)` (`:395-399`) (D6).
- **R9.** Stated residual, by design: the failure routes
  (`unmeasured`/`unplanned`/`unforked`/`unmerged`/`gates_red`/`judge_flagged`/`unpushed`)
  reach no capture step, and none of them tear the tree down — "failure keeps the tree"
  (`fleet_dev.awl:44-50`). On those routes the leg transcripts are still committed on the
  leg branches in the kept tree, and the counsel transcripts remain at their box paths,
  which the plan names above. Extending capture to every failure route is a larger
  wiring change than the commission covers and is left named, not done.

### GAP 2 — green gate tails

- **R10.** `bin/fleet-gates.sh` commits the tail of the gate log on **green** as
  `.fleet/gates/green-<n>.md`, with the same discipline the red path already uses at
  `:21-25`: `mkdir -p .fleet/gates`, copy, `git add`, `git commit -q`, and
  `op_guard_fail` on any of those failing. The tail is the same last 120 lines the probe
  already carries (`:26`), so the committed proof and the routed payload are one text.
- **R11.** The round counter at `:17` counts `red-*.md` **and** `green-*.md`. On every
  reachable path this is identical to today's count (a green ends the gate loop, so no
  red can follow one), and it stays correct if a tree is ever re-gated. `RED-LATEST.md`
  is untouched: the fix-round prompt's stable address (`fleet_dev.awl:348-352`) is not
  this gap's to move, and no `GREEN-LATEST.md` is invented.
- **R12.** The commit lands before `judgement` and therefore before `push_result`, so a
  landed run's branch carries the proof of what its green looked like.

### GAP 3 — leg branch refs

- **R13.** `bin/fleet-fanin.sh` records, for every leg it merges, the leg id, its branch
  name (`leg/<id>`, cut at `bin/fleet-fanout.sh:14-16`), the branch head sha **read
  before the merge**, and the merge commit sha read after it, into
  `.fleet/legs-merged.md`, committed in the primary tree after the last merge — so the
  exact pre-merge state of every leg is recoverable from the result branch alone even
  after teardown discards the branches. Mechanics verified in a scratch repo:
  `git rev-parse "leg/$id^{commit}"` before each `git merge --no-ff`, `git rev-parse HEAD`
  after it.
- **R14.** The script verifies its own record in code before reporting success: the file
  is present at HEAD, every leg id in the csv has a row, and every recorded sha resolves
  (`git cat-file -e <sha>^{commit}`). Failure of any of those is `op_guard_fail`.
  The conflict path at `:17-21` is untouched — it still aborts, names the conflicting
  files, and emits exit_code 1 with the legs merged so far; the record is written only
  when every merge landed, and on the conflict path every leg branch survives in the kept
  tree, so nothing is lost by not writing it.

### The gate and the self-check

- **R15.** `.fleet-hardening-gate.sh` (repo root, committed with this plan) runs
  `aion awl check` on both documents, `sh -n` on every `workflows/fleet-dev/bin/*.sh`,
  and the self-check; it names every failing command and exits non-zero on any failure.
  It binds `HOME`/`CARGO_HOME`/`PATH` explicitly — `aion` is at `/home/aion/.cargo/bin/aion`
  on this box, **not** `$HOME/.local/bin` (which holds only the `claude` symlink) — after
  the pattern of commit 360d1dc1, which fixed exactly this red tonight.
- **R16.** `workflows/fleet-dev/selfcheck/capture-selfcheck.sh` exercises all three gaps
  behaviourally against a scratch run root and scratch transcript stores, prints one
  `ok:`/`FAIL:` line per assertion, and exits non-zero if any assertion fails. It creates
  only `/home/aion/fleet/runs/selfcheck-$$`, refuses if that path already exists, and
  removes only what it created — the fleet's own law: never delete another run's tree
  (`bin/fleet-fanout.sh:15`, `bin/fleet-clone.sh:15`).

## 6. The leg

One leg, `capture`, brief at `.fleet/legs/capture.md`. Its file zone is the whole of
`workflows/fleet-dev/` plus `.fleet/reports/capture.md`; `.fleet-hardening-gate.sh` is
already committed and is not the builder's to edit. There is no second leg, so no
disjointness question arises.

## 7. Acceptance

1. `aion awl check workflows/fleet-dev/fleet_dev.awl` → `ok: … (17 steps)`.
2. `aion awl check workflows/fleet-dev/fleet_ops_worker.awl` → `ok: … (10 actions)`.
3. `sh -n` clean on every `workflows/fleet-dev/bin/*.sh` (now ten files).
4. `sh .fleet-hardening-gate.sh` → `fleet-hardening gate: GREEN`, exit 0.
5. One mutation check per gap, each run and its verbatim output pasted into
   `.fleet/reports/capture.md`: disable the mechanism, show the gate goes red naming the
   failing command, revert, show it green again. Specified in the leg brief.

## 8. What this does not prove

The self-check proves the three mechanisms behave on a scratch tree with fabricated
transcripts; it does not prove a live flight captures a real counsel session, because the
deployed scripts only change at the next provisioning (§1). The first live proof is the
next flight's own `.fleet/sessions/`, `.fleet/gates/green-*.md`, and `.fleet/legs-merged.md`.
That is stated, not hidden: this run lands the mechanism and its own check of it.
